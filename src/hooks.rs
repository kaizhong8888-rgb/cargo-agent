//! Hook system for cargo-agent.
//!
//! Provides an event-driven hook framework that allows registering callbacks
//! to react to lifecycle events: before/after tool calls, before/after chat.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     dispatch      ┌─────────────┐
//! │  AIAgent /   │ ────────────────► │  HookManager │
//! │  Gateway     │ ◄──────────────── │              │
//! └──────────────┘   hook results    │ ┌─────────┐  │
//!                                    │ │Hook A   │  │
//!                                    │ │Hook B   │  │
//!                                    │ │Hook C   │  │
//!                                    │ └─────────┘  │
//!                                    └─────────────┘
//! ```
//!
//! # Built-in Hooks
//!
//! - `audit_log_hook`: Records all tool calls for auditing
//! - `metrics_hook`: Feeds tool timing into SessionMetrics

use std::collections::HashMap;
use std::sync::Arc;

// ── Context types ──────────────────────────────────────────

/// Data available to hooks before a tool call.
#[derive(Debug, Clone)]
pub struct ToolCallContext<'a> {
    pub tool_name: &'a str,
    pub arguments: &'a HashMap<String, serde_json::Value>,
}

/// Data available to hooks after a tool call.
#[derive(Debug, Clone)]
pub struct ToolCallResult<'a> {
    pub tool_name: &'a str,
    pub arguments: &'a HashMap<String, serde_json::Value>,
    pub result: &'a str,
    pub duration_ms: u64,
    pub success: bool,
}

/// Data available to hooks before a chat request.
#[derive(Debug, Clone)]
pub struct ChatStartContext<'a> {
    pub user_message: &'a str,
    pub model: &'a str,
    pub message_count: usize,
}

/// Data available to hooks after a chat response.
#[derive(Debug, Clone)]
pub struct ChatEndContext {
    pub user_message: String,
    pub response: String,
    pub duration_ms: u64,
    pub tool_calls: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub error: Option<String>,
}

// ── Hook event and function type ───────────────────────────

/// Events that can trigger hooks.
pub enum HookEvent<'a> {
    BeforeChat(ChatStartContext<'a>),
    AfterChat(ChatEndContext),
    BeforeTool(ToolCallContext<'a>),
    AfterTool(ToolCallResult<'a>),
}

/// A hook function that reacts to an event.
/// Returns a string message for logging/diagnostics. Empty = no action.
pub type HookFn = Arc<dyn for<'a> Fn(&HookEvent<'a>) -> String + Send + Sync + 'static>;

// ── HookManager ────────────────────────────────────────────

/// Manages a collection of named hooks.
///
/// Hooks are dispatched sequentially when an event fires.
/// Clone-safe for sharing across async contexts.
#[derive(Clone)]
pub struct HookManager {
    hooks: Arc<Vec<(String, HookFn)>>,
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(Vec::new()),
        }
    }

    /// Register a named hook callback.
    pub fn register(&mut self, name: &str, hook_fn: HookFn) {
        let hooks = Arc::make_mut(&mut self.hooks);
        hooks.push((name.to_string(), hook_fn));
    }

    /// Dispatch an event to all hooks, collecting (name, response) pairs.
    pub fn dispatch<'a>(&self, event: &HookEvent<'a>) -> Vec<(String, String)> {
        self.hooks
            .iter()
            .map(|(name, fn_)| {
                let response = fn_(event);
                (name.clone(), response)
            })
            .collect()
    }

    /// Dispatch and log non-empty hook responses via tracing::debug.
    pub fn dispatch_and_log<'a>(&self, event: &HookEvent<'a>) {
        let results = self.dispatch(event);
        for (name, response) in &results {
            if !response.is_empty() {
                tracing::debug!("[hook:{name}] {response}");
            }
        }
    }

    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }

    pub fn hook_names(&self) -> Vec<String> {
        self.hooks.iter().map(|(name, _)| name.clone()).collect()
    }
}

// ── Built-in hooks ─────────────────────────────────────────

/// Audit log hook: records tool call activity.
pub fn create_audit_log_hook() -> HookFn {
    Arc::new(|event| match event {
        HookEvent::BeforeTool(ctx) => {
            let args_preview = if ctx.arguments.is_empty() {
                "{}".to_string()
            } else {
                let keys: Vec<&str> = ctx.arguments.keys().map(|s| s.as_str()).collect();
                format!("{{{}}}", keys.join(", "))
            };
            format!("tool_call: {}({})", ctx.tool_name, args_preview)
        }
        HookEvent::AfterTool(res) => {
            let status = if res.success { "ok" } else { "error" };
            format!(
                "tool_result: {} [{status}] {}ms",
                res.tool_name, res.duration_ms
            )
        }
        _ => String::new(),
    })
}

/// Metrics hook: feeds tool call timing into shared metrics sink.
pub fn create_metrics_hook(
    sink: Arc<parking_lot::Mutex<crate::metrics::ToolCallMetrics>>,
) -> HookFn {
    Arc::new(move |event| {
        if let HookEvent::AfterTool(res) = event {
            let mut m = sink.lock();
            m.total_calls += 1;
            m.total_duration_ms += res.duration_ms;
            if res.success {
                m.success_calls += 1;
            } else {
                m.error_calls += 1;
            }
            m.per_tool
                .entry(res.tool_name.to_string())
                .or_default()
                .record(res.duration_ms, res.success);
            format!(
                "metrics: {} calls, avg {:.1}ms",
                m.total_calls,
                m.average_duration_ms()
            )
        } else {
            String::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_register_and_dispatch() {
        let mut mgr = HookManager::new();
        mgr.register("ping", |_e| "pong".to_string());
        assert_eq!(mgr.hook_count(), 1);

        let ctx = ToolCallContext {
            tool_name: "test",
            arguments: &HashMap::new(),
        };
        let results = mgr.dispatch(&HookEvent::BeforeTool(ctx));
        assert_eq!(results.len(), 1);
        assert_eq!(&results[0].1, "pong");
    }

    #[test]
    fn audit_log_before_tool() {
        let hook = create_audit_log_hook();
        let mut args = HashMap::new();
        args.insert("path".to_string(), serde_json::json!("src/lib.rs"));
        let ctx = ToolCallContext {
            tool_name: "read_file",
            arguments: &args,
        };
        let out = hook(&HookEvent::BeforeTool(ctx));
        assert!(out.contains("read_file"));
        assert!(out.contains("path"));
    }

    #[test]
    fn audit_log_after_tool() {
        let hook = create_audit_log_hook();
        let res = ToolCallResult {
            tool_name: "code_analyze",
            arguments: &HashMap::new(),
            result: "ok",
            duration_ms: 42,
            success: true,
        };
        let out = hook(&HookEvent::AfterTool(res));
        assert!(out.contains("code_analyze"));
        assert!(out.contains("[ok]"));
        assert!(out.contains("42ms"));
    }
}
