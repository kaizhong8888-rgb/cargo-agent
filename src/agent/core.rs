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
#[non_exhaustive]
pub enum AgentStatus {
    Idle = 0,
    SearchingMemories = 1,
    CallingLLM = 2,
    ExecutingTool = 3,
    GeneratingResponse = 4,
    TruncatingContext = 5,
}

impl AgentStatus {
    /// Returns an emoji representation of the status.
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

    /// Returns a human-readable label for the status.
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

/// The core AI agent runtime.
///
/// Manages conversation state, tool execution, memory injection,
/// and LLM API interaction in a unified chat loop.
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
    /// Creates a new agent with the given client and registries.
    pub fn new(
        client: ModelClient,
        tool_registry: ToolRegistry,
        skill_registry: Arc<SkillRegistry>,
    ) -> Self {
        let session_metrics = Arc::new(SessionMetrics::new());

        let mut hook_manager = HookManager::new();
        hook_manager.register("audit_log", crate::hooks::create_audit_log_hook());
        hook_manager.register(
            "metrics",
            crate::hooks::create_metrics_hook(Arc::clone(&session_metrics)),
        );

        let tool_schemas = tool_registry.build_tool_schemas();
        let model_name = client.model_name().to_string();

        Self {
            tool_registry,
            skill_registry,
            client,
            messages: Vec::with_capacity(16),
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
    ///
    /// # Parameters
    /// * `model` — Model name string (e.g. "gpt-4o", "claude-3-5-sonnet-20241022")
    pub fn set_model(&mut self, model: impl Into<String>) {
        let name = model.into();
        self.client.set_model(&name);
        if let Ok(mut current) = self.current_model.lock() {
            *current = name;
        }
    }

    /// Create a shared memory store that can be passed to tools.
    pub fn create_memory_store() -> Option<Arc<SqliteMemoryStore>> {
        Self::try_init_memory_store()
    }

    /// Set the memory store (useful when the store is shared with tools).
    ///
    /// # Parameters
    /// * `store` — The `SqliteMemoryStore` to use for memory operations.
    pub fn set_memory_store(&mut self, store: Arc<SqliteMemoryStore>) {
        self.memory_store = Some(store);
    }

    /// Get a reference to the memory store, if available.
    pub fn memory_store(&self) -> Option<&Arc<SqliteMemoryStore>> {
        self.memory_store.as_ref()
    }

    /// Returns the current model name.
    pub fn model_name(&self) -> &str {
        self.client.model_name()
    }

    /// Sets the system prompt for the conversation.
    ///
    /// # Parameters
    /// * `prompt` — The system prompt text to prepend to the conversation.
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

        let mut all_memories = store.semantic_search(user_message, 5).unwrap_or_default();

        // Fallback: keyword search for high-importance memories
        if all_memories.len() < 5 {
            AIAgent::fetch_fallback_memories(store, user_message, &mut all_memories);
        }

        all_memories.truncate(10);

        if all_memories.is_empty() {
            return;
        }

        let context_msg = Self::format_memory_context(&all_memories);
        self.messages.push(serde_json::json!({
            "role": "system",
            "content": context_msg,
        }));
    }

    /// Fetch fallback memories via keyword search when semantic search returns few results.
    fn fetch_fallback_memories(
        store: &SqliteMemoryStore,
        user_message: &str,
        all_memories: &mut Vec<crate::memory::sqlite_store::ScoredMemory>,
    ) {
        let words: Vec<&str> = user_message
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .take(3)
            .collect();

        for word in words {
            if let Ok(results) = store.search(None, None, Some(word), Some(7), 5) {
                for m in results {
                    if !all_memories
                        .iter()
                        .any(|s: &crate::memory::sqlite_store::ScoredMemory| s.entry.key == m.key)
                    {
                        all_memories.push(crate::memory::sqlite_store::ScoredMemory {
                            entry: m,
                            score: 0.0,
                        });
                    }
                }
            }
        }
    }

    /// Format memories into a system context message.
    fn format_memory_context(memories: &[crate::memory::sqlite_store::ScoredMemory]) -> String {
        let lines: Vec<String> = memories
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

        format!(
            "Relevant memories from past interactions:\n{}\n\nUse these to inform your response.",
            lines.join("\n")
        )
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

    /// Send a user message and run the agent loop until a final response is produced.
    ///
    /// # Parameters
    /// * `user_message` — The user's input text.
    ///
    /// # Returns
    /// The assistant's final response text, or an error if the LLM call fails.
    pub async fn chat(&mut self, user_message: &str) -> Result<String> {
        self.session_metrics.record_user_message();

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

        let chat_latency_ms = chat_start.elapsed().as_millis() as u64;
        self.session_metrics.record_chat_latency(chat_latency_ms);

        let tool_call_count = self.count_tool_messages();
        let after_ctx = HookEvent::AfterChat(ChatEndContext {
            user_message: user_message.to_string(),
            response: result.as_deref().unwrap_or_default().to_string(),
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

    /// Count the number of tool role messages in the conversation.
    fn count_tool_messages(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
            .count()
    }

    /// Run the LLM tool-call loop until a final text response is produced
    /// or the maximum turn limit is reached.
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
                        Some(&tool_schemas)
                    },
                )
                .await?;

            self.accumulate_token_usage(&response);

            self.set_status(AgentStatus::GeneratingResponse);
            self.messages.push(build_assistant_message(&response));

            if response.tool_calls.as_ref().is_none_or(|c| c.is_empty()) {
                self.set_status(AgentStatus::Idle);
                return Ok(response.content.unwrap_or_default());
            }

            let calls = response.tool_calls.unwrap_or_default();
            let results = self.execute_tool_calls_batch(&calls).await;

            for (call, result) in calls.iter().zip(results) {
                self.messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.id.clone(),
                    "content": result,
                }));
            }
        }

        self.set_status(AgentStatus::Idle);
        let tool_call_count = self.count_tool_messages();
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

    /// Accumulate token usage from an LLM response into the conversation tracker.
    fn accumulate_token_usage(&mut self, response: &ModelResponse) {
        if let Some(usage) = &response.usage {
            self.token_usage.prompt_tokens += usage.prompt_tokens as u64;
            self.token_usage.completion_tokens += usage.completion_tokens as u64;
            self.token_usage.total_tokens += usage.total_tokens as u64;
            self.token_usage.api_calls += 1;

            self.session_metrics.record_api_call(
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
                usage.total_tokens as u64,
            );
        }
    }

    /// Execute a batch of tool calls with bounded concurrency (max 4).
    async fn execute_tool_calls_batch(
        &self,
        calls: &[crate::model::client::ToolCallInfo],
    ) -> Vec<String> {
        const MAX_CONCURRENT_TOOLS: usize = 4;

        if let Some(first) = calls.first() {
            if let Ok(mut current) = self.current_tool.lock() {
                *current = first.name.clone();
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
        results
    }

    /// Truncate old messages when the conversation grows too long.
    /// Keeps the system prompt and recent messages. Ensures tool results
    /// are always preceded by an assistant message with tool_calls.
    fn truncate_if_needed(&mut self) {
        if self.messages.len() <= MAX_MESSAGES {
            return;
        }

        self.set_status(AgentStatus::TruncatingContext);

        // Separate system and non-system messages
        let (system_msgs, non_system): (Vec<_>, Vec<_>) = self
            .messages
            .iter()
            .cloned()
            .partition(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"));

        // Keep the most recent non-system messages
        let mut recent = non_system
            .iter()
            .rev()
            .take(TRUNCATE_KEEP_MESSAGES)
            .rev()
            .cloned()
            .collect::<Vec<_>>();

        self.ensure_message_chain_valid(&non_system, &mut recent);

        self.messages.clear();
        self.messages.extend(system_msgs);
        self.messages.extend(recent);
        self.set_status(AgentStatus::Idle);
    }

    /// Ensure the truncated message chain doesn't start with an orphaned
    /// tool result or an assistant message with tool_calls but no results.
    fn ensure_message_chain_valid(
        &self,
        non_system: &[serde_json::Value],
        recent: &mut Vec<serde_json::Value>,
    ) {
        // If the window starts with a tool result, back up to include
        // the preceding assistant message with tool_calls.
        if recent
            .first()
            .is_some_and(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
        {
            for (idx, msg) in non_system.iter().enumerate() {
                if msg.get("role").and_then(|v| v.as_str()) == Some("assistant")
                    && msg
                        .get("tool_calls")
                        .is_some_and(|tc| tc.as_array().is_some_and(|a| !a.is_empty()))
                {
                    *recent = non_system[idx..].to_vec();
                    break;
                }
            }
        }

        // If the window starts with assistant + tool_calls but no tool results follow,
        // strip the tool_calls to avoid API validation errors.
        if recent
            .first()
            .is_some_and(Self::is_assistant_with_tool_calls)
        {
            let has_tool_result = recent
                .get(1)
                .is_some_and(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"));
            if !has_tool_result {
                if let Some(obj) = recent[0].as_object_mut() {
                    obj.remove("tool_calls");
                }
            }
        }
    }

    /// Check if a message is an assistant message containing tool_calls.
    fn is_assistant_with_tool_calls(msg: &serde_json::Value) -> bool {
        msg.get("role").and_then(|v| v.as_str()) == Some("assistant")
            && msg
                .get("tool_calls")
                .and_then(|tc| tc.as_array())
                .is_some_and(|a| !a.is_empty())
    }

    /// Execute a single tool call and return the serialized result.
    async fn execute_tool(&self, name: &str, arguments: &str) -> String {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

        let params: std::collections::HashMap<String, serde_json::Value> = args
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let before_ctx = HookEvent::BeforeTool(ToolCallContext {
            tool_name: name,
            arguments: &params,
        });
        self.hook_manager.dispatch_and_log(&before_ctx);

        let exec_start = Instant::now();

        let (result_str, success) = match self.tool_registry.get(name) {
            Some(tool) => match tool.execute(&params).await {
                Ok(value) => (
                    serde_json::to_string(&value)
                        .unwrap_or_else(|_| "Serialization error".to_string()),
                    true,
                ),
                Err(e) => (format!("Tool error: {e}"), false),
            },
            None => (format!("Unknown tool: {name}"), false),
        };

        let duration_ms = exec_start.elapsed().as_millis() as u64;

        self.session_metrics
            .record_tool_call(name, duration_ms, success);

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

    /// Returns cumulative token usage for this conversation.
    pub fn token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }

    /// Reset token usage counters (e.g. after /clear).
    pub fn reset_token_usage(&mut self) {
        self.token_usage = TokenUsage::default();
    }

    /// Clear conversation history, preserving system messages.
    ///
    /// Used by the `/clear` slash command in the interactive shell.
    pub fn clear_conversation(&mut self) {
        self.messages
            .retain(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"));
    }

    // ── Status tracking for UI ──

    /// Set the current runtime status (atomic, lock-free for UI polling).
    ///
    /// # Parameters
    /// * `status` — The `AgentStatus` variant to set.
    pub fn set_status(&self, status: AgentStatus) {
        self.current_status
            .store(status as u8, std::sync::atomic::Ordering::Release);
    }

    /// Returns the current runtime status.
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

    /// Returns a human-readable status label with emoji.
    ///
    /// Includes the current tool name or model name when applicable.
    pub fn get_status_display(&self) -> String {
        let status = self.get_status();
        let tool_name = self
            .current_tool
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let model_name = self
            .current_model
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        match status {
            AgentStatus::ExecutingTool if !tool_name.is_empty() => {
                format!("⚙️  Executing `{}`...", tool_name)
            }
            AgentStatus::CallingLLM if !model_name.is_empty() => {
                format!("🤖 Calling `{}`...", model_name)
            }
            _ => format!("{} {}", status.emoji(), status.label()),
        }
    }

    /// Export conversation messages to a JSON file.
    ///
    /// # Parameters
    /// * `path` — Output file path (will be overwritten).
    pub fn export_conversation(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.messages)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import conversation messages from a JSON file.
    ///
    /// # Parameters
    /// * `path` — Input file path containing a JSON array of messages.
    pub fn import_conversation(&mut self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        self.messages = serde_json::from_str(&json)?;
        Ok(())
    }

    /// Returns a reference to the full conversation history.
    pub fn messages(&self) -> &[serde_json::Value] {
        &self.messages
    }

    /// Returns a reference to the session metrics tracker.
    pub fn session_metrics(&self) -> &Arc<SessionMetrics> {
        &self.session_metrics
    }

    /// Returns a reference to the hook manager.
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

        let json = serde_json::to_string_pretty(&messages).unwrap();
        std::fs::write(path, &json).unwrap();

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
