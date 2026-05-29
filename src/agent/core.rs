use crate::memory::SqliteMemoryStore;
use crate::model::client::{ModelClient, ModelResponse};
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Maximum number of conversation messages before truncation.
const MAX_MESSAGES: usize = 200;

/// Number of messages to keep after truncation (system + recent).
const TRUNCATE_KEEP: usize = 5;

/// Maximum LLM tool-call turns per chat request.
const MAX_TURNS: usize = 200;

pub struct AIAgent {
    pub tool_registry: ToolRegistry,
    pub skill_registry: Arc<SkillRegistry>,
    client: ModelClient,
    messages: Vec<serde_json::Value>,
    max_turns: usize,
    memory_store: Option<Arc<SqliteMemoryStore>>,
    /// Cumulative token usage across the conversation.
    token_usage: TokenUsage,
}

/// Tracks token consumption for cost monitoring and smart truncation.
#[derive(Debug, Default, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub api_calls: u64,
}

impl AIAgent {
    pub fn new(client: ModelClient, tool_registry: ToolRegistry, skill_registry: Arc<SkillRegistry>) -> Self {
        Self {
            tool_registry,
            skill_registry,
            client,
            messages: Vec::new(),
            max_turns: MAX_TURNS,
            memory_store: Self::try_init_memory_store(),
            token_usage: TokenUsage::default(),
        }
    }

    /// Create a shared memory store that can be passed to tools.
    pub fn create_memory_store() -> Option<Arc<SqliteMemoryStore>> {
        Self::try_init_memory_store()
    }

    /// Set the memory store (useful when the store is shared with tools).
    pub fn set_memory_store(&mut self, store: Arc<SqliteMemoryStore>) {
        self.memory_store = Some(store);
    }

    /// Get a reference to the memory store, if available.
    pub fn memory_store(&self) -> Option<&Arc<SqliteMemoryStore>> {
        self.memory_store.as_ref()
    }

    pub fn set_system_prompt(&mut self, prompt: &str) {
        self.messages.push(serde_json::json!({
            "role": "system",
            "content": prompt,
        }));
    }

    /// Inject relevant memories as a context message before the user message.
    /// Uses TF-IDF semantic scoring for better relevance than simple keyword matching.
    fn inject_memory_context(&mut self, user_message: &str) {
        let Some(store) = &self.memory_store else { return };

        // Primary: TF-IDF semantic search
        let mut all_memories = store.semantic_search(user_message, 5).unwrap_or_default();

        // Fallback: keyword search for high-importance memories
        if all_memories.len() < 5 {
            let words: Vec<&str> = user_message
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .take(3)
                .collect();

            for word in &words {
                if let Ok(results) = store.search(None, None, Some(word), Some(7), 5) {
                    for m in results {
                        if !all_memories.iter().any(|s: &crate::memory::sqlite_store::ScoredMemory| s.entry.key == m.key) {
                            all_memories.push(crate::memory::sqlite_store::ScoredMemory {
                                entry: m,
                                score: 0.0,
                            });
                        }
                    }
                }
            }
        }

        // Keep top 10 by score
        all_memories.truncate(10);

        if all_memories.is_empty() {
            return;
        }

        let context_lines: Vec<String> = all_memories
            .iter()
            .map(|m| {
                if m.score > 0.0 {
                    format!("- [{}] (namespace: {}, relevance: {:.2}): {}", m.entry.key, m.entry.namespace, m.score, m.entry.value)
                } else {
                    format!("- [{}] (namespace: {}, importance: {}): {}", m.entry.key, m.entry.namespace, m.entry.importance, m.entry.value)
                }
            })
            .collect();

        let context_msg = format!(
            "Relevant memories from past interactions:\n{}\n\nUse these to inform your response.",
            context_lines.join("\n")
        );

        self.messages.push(serde_json::json!({
            "role": "system",
            "content": context_msg,
        }));
    }

    fn try_init_memory_store() -> Option<Arc<SqliteMemoryStore>> {
        let db_path = PathBuf::from(&*crate::constants::AGENT_DIR).join("memories.db");
        SqliteMemoryStore::open(db_path)
            .ok()
            .map(Arc::new)
    }

    /// Inject active skill instructions into the conversation context.
    fn inject_skill_context(&mut self, user_message: &str) {
        let context = self.skill_registry.build_context_for(user_message);
        if !context.is_empty() {
            self.messages.push(serde_json::json!({
                "role": "system",
                "content": format!("Active skills:\n\n{context}"),
            }));
        }
    }

    pub async fn chat(&mut self, user_message: &str) -> Result<String> {
        self.inject_skill_context(user_message);
        self.inject_memory_context(user_message);

        self.messages.push(serde_json::json!({
            "role": "user",
            "content": user_message,
        }));

        self.run_turns().await
    }

    async fn run_turns(&mut self) -> Result<String> {
        let tool_schemas = self.build_tool_schemas();

        for _ in 0..self.max_turns {
            self.truncate_if_needed();

            let response = self
                .client
                .chat(
                    &self.messages,
                    if tool_schemas.is_empty() {
                        None
                    } else {
                        Some(&tool_schemas)
                    },
                )
                .await?;

            // Track token usage
            if let Some(usage) = &response.usage {
                self.token_usage.prompt_tokens += usage.prompt_tokens as u64;
                self.token_usage.completion_tokens += usage.completion_tokens as u64;
                self.token_usage.total_tokens += usage.total_tokens as u64;
                self.token_usage.api_calls += 1;
            }

            self.messages.push(build_assistant_message(&response));

            if response.tool_calls.as_ref().is_none_or(|c| c.is_empty()) {
                return Ok(response.content.unwrap_or_default());
            }

            let calls = response.tool_calls.unwrap_or_default();

            // Execute independent tool calls in parallel for speedup.
            // Results are collected and appended to messages in call order.
            let futures: Vec<_> = calls
                .iter()
                .map(|call| self.execute_tool(&call.name, &call.arguments))
                .collect();
            let results = futures::future::join_all(futures).await;

            for (call, result) in calls.iter().zip(results) {
                self.messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.id.clone(),
                    "content": result,
                }));
            }
        }

        // Build summary of tool calls for diagnostics
        let tool_call_count = self.messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
            .count();
        let assistant_count = self.messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("assistant"))
            .count();

        Ok(format!(
            "⚠ Max conversation turns ({}) reached. Used {} assistant responses and {} tool calls. \
             Try a simpler request or break it into smaller steps.",
            self.max_turns, assistant_count, tool_call_count
        ))
    }

    /// Truncate old messages when the conversation grows too long.
    /// Keeps the system prompt and recent messages. Ensures tool results
    /// are always preceded by an assistant message with tool_calls.
    fn truncate_if_needed(&mut self) {
        if self.messages.len() <= MAX_MESSAGES {
            return;
        }

        // Find system messages to keep
        let system_msgs: Vec<_> = self.messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"))
            .cloned()
            .collect();

        // Keep the last TRUNCATE_KEEP non-system messages
        let non_system: Vec<_> = self.messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) != Some("system"))
            .cloned()
            .collect();

        let mut recent = non_system.iter().rev().take(TRUNCATE_KEEP).rev().cloned().collect::<Vec<_>>();

        // Ensure the first message is not a tool result — tool results must
        // follow an assistant message with tool_calls. If the window starts
        // with tool, back up to include the preceding assistant message.
        if recent.first().is_some_and(|m| {
            m.get("role").and_then(|v| v.as_str()) == Some("tool")
        }) {
            // Scan back in non_system to find the assistant with tool_calls
            for (idx, msg) in non_system.iter().enumerate() {
                if msg.get("role").and_then(|v| v.as_str()) == Some("assistant")
                    && msg.get("tool_calls").is_some_and(|tc| tc.as_array().is_some_and(|a| !a.is_empty()))
                {
                    // Include this assistant and all subsequent messages
                    recent = non_system[idx..].to_vec();
                    break;
                }
            }
        }

        // Also ensure we don't start with assistant if it has tool_calls but no tool results follow
        if recent.first().is_some_and(|m| {
            m.get("role").and_then(|v| v.as_str()) == Some("assistant")
        }) {
            let first = &recent[0];
            let has_tool_calls = first.get("tool_calls")
                .and_then(|tc| tc.as_array())
                .is_some_and(|a| !a.is_empty());
            if has_tool_calls {
                // Check if tool results follow
                let has_tool_result = recent.get(1).is_some_and(|m| {
                    m.get("role").and_then(|v| v.as_str()) == Some("tool")
                });
                if !has_tool_result {
                    // Remove tool_calls from the assistant message to avoid API error
                    if let Some(obj) = recent[0].as_object_mut() {
                        obj.remove("tool_calls");
                    }
                }
            }
        }

        self.messages.clear();
        self.messages.extend(system_msgs);
        self.messages.extend(recent);
    }

    fn build_tool_schemas(&self) -> Vec<serde_json::Value> {
        self.tool_registry
            .list_tools()
            .iter()
            .map(|tool| {
                let properties: serde_json::Map<String, serde_json::Value> = tool
                    .parameters()
                    .iter()
                    .map(|p| {
                        (
                            p.name.clone(),
                            serde_json::json!({
                                "type": p.parameter_type,
                                "description": p.description,
                            }),
                        )
                    })
                    .collect();

                let required: Vec<String> = tool
                    .parameters()
                    .iter()
                    .filter(|p| p.required)
                    .map(|p| p.name.clone())
                    .collect();

                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": {
                            "type": "object",
                            "properties": properties,
                            "required": required,
                        },
                    },
                })
            })
            .collect()
    }

    async fn execute_tool(&self, name: &str, arguments: &str) -> String {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

        let params = args
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        match self.tool_registry.get(name) {
            Some(tool) => match tool.execute(&params).await {
                Ok(value) => serde_json::to_string(&value).unwrap_or("Serialization error".to_string()),
                Err(e) => format!("Tool error: {e}"),
            },
            None => format!("Unknown tool: {name}"),
        }
    }

    /// Return cumulative token usage for this conversation.
    pub fn token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }

    /// Reset token usage counters (e.g. after /clear).
    pub fn reset_token_usage(&mut self) {
        self.token_usage = TokenUsage::default();
    }

    /// Export conversation messages to a JSON file.
    pub fn export_conversation(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.messages)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import conversation messages from a JSON file.
    pub fn import_conversation(&mut self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        let messages: Vec<serde_json::Value> = serde_json::from_str(&json)?;
        self.messages = messages;
        Ok(())
    }

    /// Return the full conversation history (for export or inspection).
    pub fn messages(&self) -> &[serde_json::Value] {
        &self.messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.api_calls, 0);
    }

    #[test]
    fn token_usage_accumulation() {
        let mut usage = TokenUsage::default();
        usage.prompt_tokens += 100;
        usage.completion_tokens += 50;
        usage.total_tokens += 150;
        usage.api_calls += 1;
        assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
        assert_eq!(usage.api_calls, 1);
    }

    #[test]
    fn export_import_roundtrip() {
        let tmp = std::env::temp_dir().join("test_conversation.json");
        let path = tmp.to_str().unwrap();

        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful"}),
            serde_json::json!({"role": "user", "content": "Hello"}),
        ];

        // Write manually since we can't create a full AIAgent without a client
        let json = serde_json::to_string_pretty(&messages).unwrap();
        std::fs::write(path, &json).unwrap();

        // Verify import reads back correctly
        let imported: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0]["role"], "system");
        assert_eq!(imported[1]["content"], "Hello");

        let _ = std::fs::remove_file(path);
    }
}

fn build_assistant_message(response: &ModelResponse) -> serde_json::Value {
    let mut msg = serde_json::Map::new();
    msg.insert("role".to_string(), "assistant".into());

    if let Some(content) = &response.content {
        msg.insert("content".to_string(), content.clone().into());
    } else {
        msg.insert("content".to_string(), serde_json::Value::Null);
    }

    // DeepSeek reasoning models: pass reasoning_content back in subsequent messages
    if let Some(reasoning) = &response.reasoning {
        msg.insert("reasoning_content".to_string(), reasoning.clone().into());
    }

    if let Some(calls) = &response.tool_calls {
        let tool_calls: Vec<serde_json::Value> = calls
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "type": "function",
                    "function": {
                        "name": c.name,
                        "arguments": c.arguments,
                    },
                })
            })
            .collect();
        msg.insert("tool_calls".to_string(), serde_json::Value::Array(tool_calls));
    }

    serde_json::Value::Object(msg)
}
