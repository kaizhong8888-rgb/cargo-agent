//! Session metrics for cargo-agent.
//!
//! Tracks cumulative statistics across a session: API calls, token usage,
//! tool calls, errors, and latency. All counters are atomic for safe
//! concurrent updates from async contexts.
//!
//! # Usage
//!
//! ```
//! use cargo_agent::metrics::SessionMetrics;
//!
//! let metrics = SessionMetrics::new();
//! metrics.record_api_call(100, 50, 150);
//! metrics.record_tool_success(10);
//! println!("{}", metrics.summary());
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Aggregated metrics for a single tool.
#[derive(Debug, Default)]
pub struct ToolMetric {
    pub calls: u64,
    pub total_duration_ms: u64,
    pub errors: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
}

impl ToolMetric {
    pub fn record(&mut self, duration_ms: u64, success: bool) {
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

    pub fn average_duration_ms(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.calls as f64
        }
    }
}

/// Aggregate tool call metrics.
#[derive(Debug, Default)]
pub struct ToolCallMetrics {
    pub total_calls: u64,
    pub success_calls: u64,
    pub error_calls: u64,
    pub total_duration_ms: u64,
    pub per_tool: std::collections::HashMap<String, ToolMetric>,
}

impl ToolCallMetrics {
    pub fn average_duration_ms(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_calls as f64
        }
    }

    pub fn summary(&self) -> String {
        if self.total_calls == 0 {
            return "No tool calls recorded.".to_string();
        }
        let mut lines = vec![format!(
            "Tool Call Metrics ({} total, {:.1}% success, avg {:.1}ms)",
            self.total_calls,
            if self.total_calls > 0 {
                self.success_calls as f64 / self.total_calls as f64 * 100.0
            } else {
                0.0
            },
            self.average_duration_ms()
        )];
        let mut sorted: Vec<_> = self.per_tool.iter().collect();
        sorted.sort_by_key(|(_, m)| std::cmp::Reverse(m.calls));
        for (name, metric) in sorted.iter().take(10) {
            lines.push(format!(
                "  {}  {} calls, avg {:.1}ms, {} errors",
                name, metric.calls, metric.average_duration_ms(), metric.errors
            ));
        }
        lines.join("\n")
    }
}

/// Tracks session-level metrics for monitoring and diagnostics.
pub struct SessionMetrics {
    session_start: Instant,

    // LLM metrics
    pub api_calls: AtomicU64,
    pub prompt_tokens: AtomicU64,
    pub completion_tokens: AtomicU64,
    pub total_tokens: AtomicU64,

    // Tool metrics
    pub tool_calls: AtomicU64,
    pub tool_success: AtomicU64,
    pub tool_errors: AtomicU64,

    // Conversation metrics
    pub user_messages: AtomicU64,
    pub assistant_responses: AtomicU64,
    pub chat_errors: AtomicU64,

    // Latency metrics
    pub total_chat_latency_ms: AtomicU64,
    pub total_tool_latency_ms: AtomicU64,
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
        }
    }

    /// Record token usage from an API call.
    pub fn record_api_call(&self, prompt: u64, completion: u64, total: u64) {
        self.api_calls.fetch_add(1, Ordering::Relaxed);
        self.prompt_tokens.fetch_add(prompt, Ordering::Relaxed);
        self.completion_tokens.fetch_add(completion, Ordering::Relaxed);
        self.total_tokens.fetch_add(total, Ordering::Relaxed);
    }

    /// Record a successful tool call.
    pub fn record_tool_success(&self, duration_ms: u64) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
        self.tool_success.fetch_add(1, Ordering::Relaxed);
        self.total_tool_latency_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Record a failed tool call.
    pub fn record_tool_error(&self, duration_ms: u64) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
        self.tool_errors.fetch_add(1, Ordering::Relaxed);
        self.total_tool_latency_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

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

    /// Human-readable summary.
    pub fn summary(&self) -> String {
        let api = self.api_calls.load(Ordering::Relaxed);
        let tools = self.tool_calls.load(Ordering::Relaxed);
        let msgs = self.user_messages.load(Ordering::Relaxed);

        if api == 0 && tools == 0 && msgs == 0 {
            return "Session metrics: no activity yet.".to_string();
        }

        let mut lines = vec!["Session Metrics".to_string()];
        lines.push(format!("  Duration:       {:.1}s", self.session_duration_secs()));
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
    }
}

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
        m.record_tool_success(10);
        m.record_tool_success(20);
        m.record_tool_error(5);
        assert_eq!(m.tool_calls.load(Ordering::Relaxed), 3);
        assert_eq!(m.tool_success.load(Ordering::Relaxed), 2);
        assert_eq!(m.tool_errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn success_rate() {
        let m = SessionMetrics::new();
        m.record_tool_success(10);
        m.record_tool_error(10);
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
        m.record_tool_success(10);
        m.record_user_message();
        let s = m.summary();
        assert!(s.contains("Session Metrics"));
        assert!(s.contains("API calls:      1"));
        assert!(s.contains("Tool calls:     1"));
    }

    #[test]
    fn tool_metric_record() {
        let mut tm = ToolMetric::default();
        tm.record(10, true);
        tm.record(20, true);
        tm.record(5, false);
        assert_eq!(tm.calls, 3);
        assert_eq!(tm.errors, 1);
        assert_eq!(tm.min_duration_ms, 5);
        assert_eq!(tm.max_duration_ms, 20);
    }

    #[test]
    fn tool_call_metrics_summary() {
        let mut m = ToolCallMetrics::default();
        m.per_tool
            .entry("read_file".into())
            .or_default()
            .record(10, true);
        assert!(m.summary().contains("read_file"));
    }
}
