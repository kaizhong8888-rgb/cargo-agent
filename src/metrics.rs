//! Session metrics for cargo-agent.
//!
//! Tracks cumulative statistics across a session: API calls, token usage,
//! tool calls, errors, latency, and per-tool breakdown — all in one place.
//!
//! Aggregate counters use atomics (hot-path, lock-free), while per-tool
//! details live behind a single `Mutex` (queried only on demand).
//!
//! # Usage
//!
//! ```
//! use cargo_agent::metrics::SessionMetrics;
//!
//! let metrics = SessionMetrics::new();
//! metrics.record_api_call(100, 50, 150);
//! metrics.record_tool_call("read_file", 10, true);
//! println!("{}", metrics.summary());
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

// ── Per-tool statistics ─────────────────────────────────────

/// Statistics for a single tool.
#[derive(Debug, Clone, Default)]
pub struct ToolStats {
    pub calls: u64,
    pub total_duration_ms: u64,
    pub errors: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
}

impl ToolStats {
    /// Average duration in milliseconds.
    pub fn average_duration_ms(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.calls as f64
        }
    }

    fn record(&mut self, duration_ms: u64, success: bool) {
        self.calls += 1;
        self.total_duration_ms += duration_ms;
        if !success {
            self.errors += 1;
        }
        if self.calls == 1 {
            self.min_duration_ms = duration_ms;
            self.max_duration_ms = duration_ms;
        } else {
            self.min_duration_ms = self.min_duration_ms.min(duration_ms);
            self.max_duration_ms = self.max_duration_ms.max(duration_ms);
        }
    }
}

// ── Unified session metrics ─────────────────────────────────

/// Tracks session-level metrics for monitoring and diagnostics.
///
/// Aggregate counters (API calls, tokens, tool calls, latency) use
/// lock-free atomics. Per-tool breakdowns are stored behind a `Mutex`
/// and only accessed when the summary or per-tool query is requested.
pub struct SessionMetrics {
    session_start: Instant,

    // ── LLM metrics (atomic, hot-path) ──────────────────
    pub api_calls: AtomicU64,
    pub prompt_tokens: AtomicU64,
    pub completion_tokens: AtomicU64,
    pub total_tokens: AtomicU64,

    // ── Tool metrics (atomic, hot-path) ─────────────────
    pub tool_calls: AtomicU64,
    pub tool_success: AtomicU64,
    pub tool_errors: AtomicU64,

    // ── Conversation metrics (atomic) ───────────────────
    pub user_messages: AtomicU64,
    pub assistant_responses: AtomicU64,
    pub chat_errors: AtomicU64,

    // ── Latency metrics (atomic) ────────────────────────
    pub total_chat_latency_ms: AtomicU64,
    pub total_tool_latency_ms: AtomicU64,

    // ── Per-tool breakdown (cold-path, Mutex) ───────────
    per_tool: Mutex<HashMap<String, ToolStats>>,
}

impl Default for SessionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMetrics {
    pub fn new() -> Self {
        Self {
            session_start: Instant::now(),
            api_calls: AtomicU64::new(0),
            prompt_tokens: AtomicU64::new(0),
            completion_tokens: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            tool_calls: AtomicU64::new(0),
            tool_success: AtomicU64::new(0),
            tool_errors: AtomicU64::new(0),
            user_messages: AtomicU64::new(0),
            assistant_responses: AtomicU64::new(0),
            chat_errors: AtomicU64::new(0),
            total_chat_latency_ms: AtomicU64::new(0),
            total_tool_latency_ms: AtomicU64::new(0),
            per_tool: Mutex::new(HashMap::new()),
        }
    }

    // ── LLM metrics ───────────────────────────────────────

    /// Record token usage from an API call.
    pub fn record_api_call(&self, prompt: u64, completion: u64, total: u64) {
        self.api_calls.fetch_add(1, Ordering::Relaxed);
        self.prompt_tokens.fetch_add(prompt, Ordering::Relaxed);
        self.completion_tokens
            .fetch_add(completion, Ordering::Relaxed);
        self.total_tokens.fetch_add(total, Ordering::Relaxed);
    }

    // ── Tool metrics ───────────────────────────────────────

    /// Record a tool call result — updates both aggregate atomics and per-tool breakdown.
    pub fn record_tool_call(&self, name: &str, duration_ms: u64, success: bool) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
        self.total_tool_latency_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        if success {
            self.tool_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.tool_errors.fetch_add(1, Ordering::Relaxed);
        }
        // Per-tool — best-effort; lock failure won't crash the session
        if let Ok(mut map) = self.per_tool.lock() {
            map.entry(name.to_string())
                .or_default()
                .record(duration_ms, success);
        }
    }

    /// Record a successful tool call (convenience).
    pub fn record_tool_success(&self, name: &str, duration_ms: u64) {
        self.record_tool_call(name, duration_ms, true);
    }

    /// Record a failed tool call (convenience).
    pub fn record_tool_error(&self, name: &str, duration_ms: u64) {
        self.record_tool_call(name, duration_ms, false);
    }

    // ── Conversation metrics ───────────────────────────────

    /// Record a user message.
    pub fn record_user_message(&self) {
        self.user_messages.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an assistant response.
    pub fn record_assistant_response(&self) {
        self.assistant_responses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a chat error.
    pub fn record_chat_error(&self) {
        self.chat_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record chat turn latency.
    pub fn record_chat_latency(&self, duration_ms: u64) {
        self.total_chat_latency_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    // ── Accessors ─────────────────────────────────────────

    /// Session duration in seconds.
    pub fn session_duration_secs(&self) -> f64 {
        self.session_start.elapsed().as_secs_f64()
    }

    /// Average chat latency in ms.
    pub fn avg_chat_latency_ms(&self) -> f64 {
        let calls = self.api_calls.load(Ordering::Relaxed);
        if calls == 0 {
            0.0
        } else {
            self.total_chat_latency_ms.load(Ordering::Relaxed) as f64 / calls as f64
        }
    }

    /// Average tool latency in ms.
    pub fn avg_tool_latency_ms(&self) -> f64 {
        let calls = self.tool_calls.load(Ordering::Relaxed);
        if calls == 0 {
            0.0
        } else {
            self.total_tool_latency_ms.load(Ordering::Relaxed) as f64 / calls as f64
        }
    }

    /// Tool success rate as percentage.
    pub fn tool_success_rate(&self) -> f64 {
        let total = self.tool_calls.load(Ordering::Relaxed);
        if total == 0 {
            100.0
        } else {
            self.tool_success.load(Ordering::Relaxed) as f64 / total as f64 * 100.0
        }
    }

    /// Snapshot of per-tool breakdown.
    pub fn per_tool_stats(&self) -> HashMap<String, ToolStats> {
        self.per_tool
            .lock()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default()
    }

    /// Per-tool stats for a specific tool.
    pub fn tool_stats(&self, name: &str) -> Option<ToolStats> {
        self.per_tool.lock().ok().and_then(|m| m.get(name).cloned())
    }

    // ── Summary ────────────────────────────────────────────

    /// Human-readable summary of all session metrics.
    pub fn summary(&self) -> String {
        let api = self.api_calls.load(Ordering::Relaxed);
        let tools = self.tool_calls.load(Ordering::Relaxed);
        let msgs = self.user_messages.load(Ordering::Relaxed);

        if api == 0 && tools == 0 && msgs == 0 {
            return "Session metrics: no activity yet.".to_string();
        }

        let mut lines = vec!["Session Metrics".to_string()];
        lines.push(format!(
            "  Duration:       {:.1}s",
            self.session_duration_secs()
        ));
        lines.push(format!(
            "  User messages:  {}",
            self.user_messages.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  API calls:      {}",
            self.api_calls.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  Prompt tokens:  {}",
            self.prompt_tokens.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  Completion:     {}",
            self.completion_tokens.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  Total tokens:   {}",
            self.total_tokens.load(Ordering::Relaxed)
        ));
        lines.push(format!("  Tool calls:     {}", tools));
        lines.push(format!(
            "  Tool success:   {:.1}%",
            self.tool_success_rate()
        ));
        lines.push(format!(
            "  Tool errors:    {}",
            self.tool_errors.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  Chat errors:    {}",
            self.chat_errors.load(Ordering::Relaxed)
        ));
        lines.push(format!(
            "  Avg chat lat:   {:.0}ms",
            self.avg_chat_latency_ms()
        ));
        lines.push(format!(
            "  Avg tool lat:   {:.0}ms",
            self.avg_tool_latency_ms()
        ));

        // Per-tool breakdown (top 10 by call count)
        if let Ok(map) = self.per_tool.lock() {
            if !map.is_empty() {
                lines.push(String::new());
                lines.push("  Per-Tool Breakdown".to_string());
                let mut sorted: Vec<_> = map.iter().collect();
                sorted.sort_by_key(|(_, s)| std::cmp::Reverse(s.calls));
                for (name, stats) in sorted.iter().take(10) {
                    lines.push(format!(
                        "    {}  {} calls, avg {:.1}ms, {} errors (min {}ms, max {}ms)",
                        name,
                        stats.calls,
                        stats.average_duration_ms(),
                        stats.errors,
                        stats.min_duration_ms,
                        stats.max_duration_ms,
                    ));
                }
                if sorted.len() > 10 {
                    lines.push(format!("    ... and {} more", sorted.len() - 10));
                }
            }
        }

        lines.join("\n")
    }

    /// Reset all counters (keeps session_start).
    pub fn reset(&self) {
        self.api_calls.store(0, Ordering::Relaxed);
        self.prompt_tokens.store(0, Ordering::Relaxed);
        self.completion_tokens.store(0, Ordering::Relaxed);
        self.total_tokens.store(0, Ordering::Relaxed);
        self.tool_calls.store(0, Ordering::Relaxed);
        self.tool_success.store(0, Ordering::Relaxed);
        self.tool_errors.store(0, Ordering::Relaxed);
        self.user_messages.store(0, Ordering::Relaxed);
        self.assistant_responses.store(0, Ordering::Relaxed);
        self.chat_errors.store(0, Ordering::Relaxed);
        self.total_chat_latency_ms.store(0, Ordering::Relaxed);
        self.total_tool_latency_ms.store(0, Ordering::Relaxed);
        if let Ok(mut map) = self.per_tool.lock() {
            map.clear();
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zeroes() {
        let m = SessionMetrics::new();
        assert_eq!(m.api_calls.load(Ordering::Relaxed), 0);
        assert_eq!(m.tool_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn record_api_calls() {
        let m = SessionMetrics::new();
        m.record_api_call(100, 50, 150);
        m.record_api_call(200, 80, 280);
        assert_eq!(m.api_calls.load(Ordering::Relaxed), 2);
        assert_eq!(m.prompt_tokens.load(Ordering::Relaxed), 300);
        assert_eq!(m.total_tokens.load(Ordering::Relaxed), 430);
    }

    #[test]
    fn record_tool_results() {
        let m = SessionMetrics::new();
        m.record_tool_success("read", 10);
        m.record_tool_success("write", 20);
        m.record_tool_error("read", 5);
        assert_eq!(m.tool_calls.load(Ordering::Relaxed), 3);
        assert_eq!(m.tool_success.load(Ordering::Relaxed), 2);
        assert_eq!(m.tool_errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn success_rate() {
        let m = SessionMetrics::new();
        m.record_tool_success("a", 10);
        m.record_tool_error("b", 10);
        assert!((m.tool_success_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn reset() {
        let m = SessionMetrics::new();
        m.record_api_call(100, 50, 150);
        m.reset();
        assert_eq!(m.api_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn summary_empty() {
        let m = SessionMetrics::new();
        assert!(m.summary().contains("no activity"));
    }

    #[test]
    fn summary_with_data() {
        let m = SessionMetrics::new();
        m.record_api_call(100, 50, 150);
        m.record_tool_success("read_file", 10);
        m.record_user_message();
        let s = m.summary();
        assert!(s.contains("Session Metrics"));
        assert!(s.contains("API calls:      1"));
        assert!(s.contains("Tool calls:     1"));
    }

    #[test]
    fn per_tool_breakdown() {
        let m = SessionMetrics::new();
        m.record_tool_call("read_file", 10, true);
        m.record_tool_call("read_file", 20, true);
        m.record_tool_call("read_file", 5, false);
        assert_eq!(m.tool_calls.load(Ordering::Relaxed), 3);
        assert_eq!(m.tool_errors.load(Ordering::Relaxed), 1);

        let stats = m.tool_stats("read_file").unwrap();
        assert_eq!(stats.calls, 3);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.min_duration_ms, 5);
        assert_eq!(stats.max_duration_ms, 20);
        assert!((stats.average_duration_ms() - 11.666).abs() < 0.01);

        let per_tool = m.per_tool_stats();
        assert_eq!(per_tool.len(), 1);
    }

    #[test]
    fn summary_includes_per_tool() {
        let m = SessionMetrics::new();
        m.record_tool_success("code_analyze", 42);
        let s = m.summary();
        assert!(s.contains("code_analyze"));
        assert!(s.contains("Per-Tool Breakdown"));
    }
}
