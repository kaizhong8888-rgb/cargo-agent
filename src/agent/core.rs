use crate::memory::SqliteMemoryStore;
use crate::model::client::{ModelClient, ModelResponse};
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Maximum number of conversation messages before truncation.
const MAX_MESSAGES: usize = 50;

/// Number of messages to keep after truncation (system + recent).
const TRUNCATE_KEEP: usize = 5;

/// Maximum LLM tool-call turns per chat request.
const MAX_TURNS: usize = 50;

pub struct AIAgent {
    pub tool_registry: ToolRegistry,
    pub skill_registry: Arc<SkillRegistry>,
    client: ModelClient,
    messages: Vec<serde_json::Value>,
    max_turns: usize,
    memory_store: Option<Arc<SqliteMemoryStore>>,
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

    pub fn set_system_prompt(&mut self, prompt: &str) {
        self.messages.push(serde_json::json!({
            "role": "system",
            "content": prompt,
        }));
    }

    /// Inject relevant memories as a context message before the user message.
    fn inject_memory_context(&mut self, user_message: &str) {
        let Some(store) = &self.memory_store else { return };

        // Extract potential search terms from the user message
        let words: Vec<&str> = user_message
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .take(3)
            .collect();

        let mut all_memories = Vec::new();

        // Search by each word
        for word in &words {
            if let Ok(results) = store.search(None, None, Some(word), Some(3), 5) {
                all_memories.extend(results);
            }
        }

        // Also get high-importance memories (importance >= 7)
        if let Ok(results) = store.search(None, None, None, Some(7), 5) {
            all_memories.extend(results);
        }

        // Deduplicate by key
        all_memories.sort_by_key(|m| (m.key.clone(), m.importance));
        all_memories.dedup_by_key(|m| m.key.clone());

        // Keep top 10 by importance
        all_memories.truncate(10);

        if all_memories.is_empty() {
            return;
        }

        let context_lines: Vec<String> = all_memories
            .iter()
            .map(|m| format!("- [{}] (namespace: {}, importance: {}): {}", m.key, m.namespace, m.importance, m.value))
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

            self.messages.push(build_assistant_message(&response));

            if response.tool_calls.as_ref().is_none_or(|c| c.is_empty()) {
                return Ok(response.content.unwrap_or_default());
            }

            for call in response.tool_calls.unwrap_or_default() {
                let result = self.execute_tool(&call.name, &call.arguments).await;
                self.messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.id,
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
                Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| "Serialization error".to_string()),
                Err(e) => format!("Tool error: {e}"),
            },
            None => format!("Unknown tool: {name}"),
        }
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
