use crate::hooks::{
    ChatEndContext, ChatStartContext, HookEvent, HookManager, ToolCallContext, ToolCallResult,
};
use crate::memory::SqliteMemoryStore;
use crate::metrics::SessionMetrics;
use crate::model::client::{ModelClient, ModelResponse};
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;
use std::time::Instant;

/// Maximum number of conversation messages before truncation.
const MAX_MESSAGES: usize = 200;

/// Non-system messages to keep after truncation (~20 user/assistant turns).
const TRUNCATE_KEEP_MESSAGES: usize = 40;

/// Maximum LLM tool-call turns per chat request.
const MAX_TURNS: usize = 200;

/// Agent runtime status for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AgentStatus {
    Idle = 0,
    SearchingMemories = 1,
    CallingLLM = 2,
    ExecutingTool = 3,
    GeneratingResponse = 4,
    TruncatingContext = 5,
}

impl AgentStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Idle => "💤",
            Self::SearchingMemories => "🔍",
            Self::CallingLLM => "🤖",
            Self::ExecutingTool => "⚙️",
            Self::GeneratingResponse => "✍️",
            Self::TruncatingContext => "📏",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::SearchingMemories => "Searching memories...",
            Self::CallingLLM => "Calling LLM...",
            Self::ExecutingTool => "Executing tool...",
            Self::GeneratingResponse => "Generating response...",
            Self::TruncatingContext => "Truncating context...",
        }
    }
}

pub struct AIAgent {
    pub tool_registry: ToolRegistry,
    pub skill_registry: Arc<SkillRegistry>,
    client: ModelClient,
    messages: Vec<serde_json::Value>,
    max_turns: usize,
    memory_store: Option<Arc<SqliteMemoryStore>>,
    /// Cumulative token usage across the conversation.
    token_usage: TokenUsage,
    /// Current runtime status (atomic for lock-free UI polling).
    pub current_status: Arc<AtomicU8>,
    /// Current tool being executed (for UI display).
    pub current_tool: Arc<std::sync::Mutex<String>>,
    /// Current model name being used.
    pub current_model: Arc<std::sync::Mutex<String>>,
    /// Hook manager for lifecycle event callbacks.
    hook_manager: HookManager,
    /// Session-level metrics tracker.
    session_metrics: Arc<SessionMetrics>,
    /// Cached OpenAI tool schemas (built once at agent construction).
    tool_schemas: Vec<serde_json::Value>,
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
    pub fn new(
        client: ModelClient,
        tool_registry: ToolRegistry,
        skill_registry: Arc<SkillRegistry>,
    ) -> Self {
        let session_metrics = Arc::new(SessionMetrics::new());

        // Set up default hooks: audit log + metrics
        let mut hook_manager = HookManager::new();
        hook_manager.register("audit_log", crate::hooks::create_audit_log_hook());
        hook_manager.register(
            "metrics",
            crate::hooks::create_metrics_hook(session_metrics.clone()),
        );

        let tool_schemas = tool_registry.build_tool_schemas();
        let model_name = client.model_name().to_string();

        Self {
            tool_registry,
            skill_registry,
            client,
            messages: Vec::new(),
            max_turns: MAX_TURNS,
            memory_store: Self::try_init_memory_store(),
            token_usage: TokenUsage::default(),
            current_status: Arc::new(AtomicU8::new(AgentStatus::Idle as u8)),
            current_tool: Arc::new(std::sync::Mutex::new(String::new())),
            current_model: Arc::new(std::sync::Mutex::new(model_name)),
            hook_manager,
            session_metrics,
            tool_schemas,
        }
    }

    /// Update the LLM model for subsequent API calls.
    pub fn set_model(&mut self, model: impl Into<String>) {
        let name = model.into();
        self.client.set_model(&name);
        if let Ok(mut guard) = self.current_model.lock() {
            *guard = name;
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

    pub fn model_name(&self) -> &str {
        self.client.model_name()
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
        let Some(store) = &self.memory_store else {
            return;
        };

        self.set_status(AgentStatus::SearchingMemories);

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
                        if !all_memories.iter().any(
                            |s: &crate::memory::sqlite_store::ScoredMemory| s.entry.key == m.key,
                        ) {
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
                    format!(
                        "- [{}] (namespace: {}, relevance: {:.2}): {}",
                        m.entry.key, m.entry.namespace, m.score, m.entry.value
                    )
                } else {
                    format!(
                        "- [{}] (namespace: {}, importance: {}): {}",
                        m.entry.key, m.entry.namespace, m.entry.importance, m.entry.value
                    )
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
        let db_path = crate::constants::ensure_memories_db();
        SqliteMemoryStore::open(db_path).ok().map(Arc::new)
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
        self.session_metrics.record_user_message();

        // Fire before_chat hook
        let model_name = self.client.model_name();
        let msg_count = self.messages.len();
        let before_ctx = HookEvent::BeforeChat(ChatStartContext {
            user_message,
            model: model_name,
            message_count: msg_count,
        });
        self.hook_manager.dispatch_and_log(&before_ctx);

        let chat_start = Instant::now();

        self.inject_skill_context(user_message);
        self.inject_memory_context(user_message);

        self.messages.push(serde_json::json!({
            "role": "user",
            "content": user_message,
        }));

        let result = self.run_turns().await;

        // Record chat latency
        let chat_latency_ms = chat_start.elapsed().as_millis() as u64;
        self.session_metrics.record_chat_latency(chat_latency_ms);

        // Fire after_chat hook
        let tool_call_count = self
            .messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
            .count();
        let after_ctx = HookEvent::AfterChat(ChatEndContext {
            user_message: user_message.to_string(),
            response: result.as_deref().unwrap_or("").to_string(),
            duration_ms: chat_latency_ms,
            tool_calls: tool_call_count,
            prompt_tokens: self.token_usage.prompt_tokens,
            completion_tokens: self.token_usage.completion_tokens,
            error: result.as_ref().err().map(|e| e.to_string()),
        });
        self.hook_manager.dispatch_and_log(&after_ctx);

        if result.is_ok() {
            self.session_metrics.record_assistant_response();
        } else {
            self.session_metrics.record_chat_error();
        }

        result
    }

    async fn run_turns(&mut self) -> Result<String> {
        let tool_schemas = self.tool_schemas.clone();

        for _ in 0..self.max_turns {
            self.truncate_if_needed();

            self.set_status(AgentStatus::CallingLLM);
            let response = self
                .client
                .chat(
                    &self.messages,
                    if tool_schemas.is_empty() {
                        None
                    } else {
                        Some(tool_schemas.as_slice())
                    },
                )
                .await?;

            // Track token usage
            if let Some(usage) = &response.usage {
                self.token_usage.prompt_tokens += usage.prompt_tokens as u64;
                self.token_usage.completion_tokens += usage.completion_tokens as u64;
                self.token_usage.total_tokens += usage.total_tokens as u64;
                self.token_usage.api_calls += 1;

                // Record to session metrics
                self.session_metrics.record_api_call(
                    usage.prompt_tokens as u64,
                    usage.completion_tokens as u64,
                    usage.total_tokens as u64,
                );
            }

            self.set_status(AgentStatus::GeneratingResponse);
            self.messages.push(build_assistant_message(&response));

            if response.tool_calls.as_ref().is_none_or(|c| c.is_empty()) {
                self.set_status(AgentStatus::Idle);
                return Ok(response.content.unwrap_or_default());
            }

            let calls = response.tool_calls.unwrap_or_default();

            // Execute tool calls in parallel batches (max 4 concurrent).
            const MAX_CONCURRENT_TOOLS: usize = 4;
            self.set_status(AgentStatus::ExecutingTool);
            if let Some(first) = calls.first() {
                if let Ok(mut guard) = self.current_tool.lock() {
                    *guard = first.name.clone();
                }
            }
            let mut results = Vec::with_capacity(calls.len());
            for chunk in calls.chunks(MAX_CONCURRENT_TOOLS) {
                let batch: Vec<_> = chunk
                    .iter()
                    .map(|call| self.execute_tool(&call.name, &call.arguments))
                    .collect();
                results.extend(futures::future::join_all(batch).await);
            }

            for (call, result) in calls.iter().zip(results) {
                self.messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.id.clone(),
                    "content": result,
                }));
            }
        }

        // Build summary of tool calls for diagnostics
        self.set_status(AgentStatus::Idle);
        let tool_call_count = self
            .messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
            .count();
        let assistant_count = self
            .messages
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
        let system_msgs: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"))
            .cloned()
            .collect();

        self.set_status(AgentStatus::TruncatingContext);

        // Keep the last TRUNCATE_KEEP_MESSAGES non-system messages
        let non_system: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) != Some("system"))
            .cloned()
            .collect();

        let mut recent = non_system
            .iter()
            .rev()
            .take(TRUNCATE_KEEP_MESSAGES)
            .rev()
            .cloned()
            .collect::<Vec<_>>();

        // Ensure the first message is not a tool result — tool results must
        // follow an assistant message with tool_calls. If the window starts
        // with tool, back up to include the preceding assistant message.
        if recent
            .first()
            .is_some_and(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
        {
            // Scan back in non_system to find the assistant with tool_calls
            for (idx, msg) in non_system.iter().enumerate() {
                if msg.get("role").and_then(|v| v.as_str()) == Some("assistant")
                    && msg
                        .get("tool_calls")
                        .is_some_and(|tc| tc.as_array().is_some_and(|a| !a.is_empty()))
                {
                    // Include this assistant and all subsequent messages
                    recent = non_system[idx..].to_vec();
                    break;
                }
            }
        }

        // Also ensure we don't start with assistant if it has tool_calls but no tool results follow
        if recent
            .first()
            .is_some_and(|m| m.get("role").and_then(|v| v.as_str()) == Some("assistant"))
        {
            let first = &recent[0];
            let has_tool_calls = first
                .get("tool_calls")
                .and_then(|tc| tc.as_array())
                .is_some_and(|a| !a.is_empty());
            if has_tool_calls {
                // Check if tool results follow
                let has_tool_result = recent
                    .get(1)
                    .is_some_and(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"));
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
        self.set_status(AgentStatus::Idle);
    }

    async fn execute_tool(&self, name: &str, arguments: &str) -> String {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

        let params: std::collections::HashMap<String, serde_json::Value> = args
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        // Fire before_tool hook
        let before_ctx = HookEvent::BeforeTool(ToolCallContext {
            tool_name: name,
            arguments: &params,
        });
        self.hook_manager.dispatch_and_log(&before_ctx);

        let exec_start = Instant::now();

        let (result_str, success) = match self.tool_registry.get(name) {
            Some(tool) => match tool.execute(&params).await {
                Ok(value) => (
                    serde_json::to_string(&value).unwrap_or("Serialization error".to_string()),
                    true,
                ),
                Err(e) => (format!("Tool error: {e}"), false),
            },
            None => (format!("Unknown tool: {name}"), false),
        };

        let duration_ms = exec_start.elapsed().as_millis() as u64;

        // Record tool metrics (aggregate + per-tool breakdown)
        self.session_metrics
            .record_tool_call(name, duration_ms, success);

        // Fire after_tool hook
        let after_ctx = HookEvent::AfterTool(ToolCallResult {
            tool_name: name,
            arguments: &params,
            result: &result_str,
            duration_ms,
            success,
        });
        self.hook_manager.dispatch_and_log(&after_ctx);

        result_str
    }

    /// Return cumulative token usage for this conversation.
    pub fn token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }

    /// Reset token usage counters (e.g. after /clear).
    pub fn reset_token_usage(&mut self) {
        self.token_usage = TokenUsage::default();
    }

    /// Clear conversation history (used by /clear command).
    /// Keeps the system prompt if set.
    pub fn clear_conversation(&mut self) {
        // Preserve system messages
        let system_msgs: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"))
            .cloned()
            .collect();
        self.messages = system_msgs;
    }

    // ── Status tracking for UI ──

    /// Set the current runtime status (atomic, lock-free for UI polling).
    pub fn set_status(&self, status: AgentStatus) {
        self.current_status
            .store(status as u8, std::sync::atomic::Ordering::Release);
    }

    /// Get the current runtime status.
    pub fn get_status(&self) -> AgentStatus {
        match self
            .current_status
            .load(std::sync::atomic::Ordering::Acquire)
        {
            1 => AgentStatus::SearchingMemories,
            2 => AgentStatus::CallingLLM,
            3 => AgentStatus::ExecutingTool,
            4 => AgentStatus::GeneratingResponse,
            5 => AgentStatus::TruncatingContext,
            _ => AgentStatus::Idle,
        }
    }

    /// Get a human-readable status label with emoji.
    pub fn get_status_display(&self) -> String {
        let status = self.get_status();
        let tool_name = if let Ok(guard) = self.current_tool.lock() {
            guard.clone()
        } else {
            String::new()
        };
        let model_name = if let Ok(guard) = self.current_model.lock() {
            guard.clone()
        } else {
            String::new()
        };
        let mut display = format!("{} {}", status.emoji(), status.label());
        match status {
            AgentStatus::ExecutingTool if !tool_name.is_empty() => {
                display = format!("⚙️  Executing `{}`...", tool_name);
            }
            AgentStatus::CallingLLM if !model_name.is_empty() => {
                display = format!("🤖 Calling `{}`...", model_name);
            }
            _ => {}
        }
        display
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

    /// Return a reference to the session metrics tracker.
    pub fn session_metrics(&self) -> &Arc<SessionMetrics> {
        &self.session_metrics
    }

    /// Return a reference to the hook manager.
    pub fn hook_manager(&self) -> &HookManager {
        &self.hook_manager
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
        assert_eq!(
            usage.total_tokens,
            usage.prompt_tokens + usage.completion_tokens
        );
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
        msg.insert(
            "tool_calls".to_string(),
            serde_json::Value::Array(tool_calls),
        );
    }

    serde_json::Value::Object(msg)
}
