//! Model router: selects the appropriate LLM model based on task complexity.
//!
//! Routes simple tasks to cheaper models (Haiku-class), complex reasoning
//! to powerful models (Opus-class), and everything else to the default.

/// Task complexity levels used for model routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskComplexity {
    /// Simple tasks: greetings, formatting, short Q&A, lookups
    Low,
    /// Medium tasks: code explanation, debugging, moderate reasoning
    Medium,
    /// Complex tasks: architecture design, multi-step reasoning, security review
    High,
}

impl TaskComplexity {
    /// Heuristic: estimate complexity from message characteristics.
    ///
    /// Longer messages with keywords like "design", "architect", "review",
    /// "security", or multiple questions are classified as higher complexity.
    pub fn estimate(message: &str) -> Self {
        let word_count = message.split_whitespace().count();
        let lower = message.to_lowercase();

        let complex_keywords = [
            "architect",
            "design",
            "review",
            "security",
            "refactor",
            "optimize",
            "analyze",
            "compare",
            "plan",
            "strategy",
            "implementation",
            "trade-off",
            "tradeoff",
        ];
        let has_complex_keyword = complex_keywords.iter().any(|kw| lower.contains(kw));

        let has_multiple_questions = message.matches('?').count() > 1;

        if word_count > 200 || has_complex_keyword || has_multiple_questions {
            Self::High
        } else if word_count > 50 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// Model routing configuration.
#[derive(Debug, Clone)]
pub struct ModelRouter {
    pub default_model: String,
    pub low_complexity_model: Option<String>,
    pub high_complexity_model: Option<String>,
}

impl ModelRouter {
    pub fn new(default_model: String) -> Self {
        Self {
            default_model,
            low_complexity_model: None,
            high_complexity_model: None,
        }
    }

    /// Set a cheaper model for low-complexity tasks.
    pub fn with_low_model(mut self, model: impl Into<String>) -> Self {
        self.low_complexity_model = Some(model.into());
        self
    }

    /// Set a more powerful model for high-complexity tasks.
    pub fn with_high_model(mut self, model: impl Into<String>) -> Self {
        self.high_complexity_model = Some(model.into());
        self
    }

    /// Select the appropriate model for a given message.
    pub fn select(&self, message: &str) -> &str {
        match TaskComplexity::estimate(message) {
            TaskComplexity::Low => self
                .low_complexity_model
                .as_deref()
                .unwrap_or(&self.default_model),
            TaskComplexity::Medium => &self.default_model,
            TaskComplexity::High => self
                .high_complexity_model
                .as_deref()
                .unwrap_or(&self.default_model),
        }
    }

    /// Return the routing decision as a human-readable string.
    pub fn describe(&self, message: &str) -> String {
        let complexity = TaskComplexity::estimate(message);
        let selected = self.select(message);
        format!(
            "Task complexity: {:?} → model: {} (default: {})",
            complexity, selected, self.default_model
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complexity_low_short_message() {
        assert_eq!(TaskComplexity::estimate("hi"), TaskComplexity::Low);
        assert_eq!(
            TaskComplexity::estimate("what time is it"),
            TaskComplexity::Low
        );
    }

    #[test]
    fn complexity_medium_mid_message() {
        // Long enough to exceed 50 words, no complex keywords
        let msg = "The quick brown fox jumps over the lazy dog many times. This is a simple demonstration of how the system processes text without any special requirements. The fox continues to jump around the yard while the dog watches from its kennel. Meanwhile the cat sleeps on the porch and the bird sings in the tree above the garden where flowers bloom in every season of the year regardless of weather conditions.";
        assert_eq!(TaskComplexity::estimate(msg), TaskComplexity::Medium);
    }

    #[test]
    fn complexity_high_long_message() {
        let msg = "We need to design a new authentication system that supports OAuth2, JWT tokens, session management, \
                   rate limiting, CSRF protection, and integrates with our existing PostgreSQL database. \
                   Please provide a detailed architecture comparison between stateless JWT and server-side sessions, \
                   including trade-offs for scalability, security, and user experience.";
        assert_eq!(TaskComplexity::estimate(msg), TaskComplexity::High);
    }

    #[test]
    fn complexity_high_keywords() {
        assert_eq!(
            TaskComplexity::estimate("review this architecture"),
            TaskComplexity::High
        );
        assert_eq!(
            TaskComplexity::estimate("security analysis needed"),
            TaskComplexity::High
        );
    }

    #[test]
    fn complexity_multiple_questions() {
        assert_eq!(
            TaskComplexity::estimate(
                "Is Rust hard? What about the learning curve? How long to master?"
            ),
            TaskComplexity::High
        );
    }

    #[test]
    fn router_default_selects_default() {
        let router = ModelRouter::new("gpt-4".into());
        assert_eq!(router.select("hello"), "gpt-4");
    }

    #[test]
    fn router_with_low_model() {
        let router = ModelRouter::new("gpt-4".into()).with_low_model("gpt-4o-mini");
        assert_eq!(router.select("hi"), "gpt-4o-mini");
    }

    #[test]
    fn router_with_high_model() {
        let router = ModelRouter::new("gpt-4".into()).with_high_model("claude-opus");
        let complex = "design a new microservice architecture for our platform with security review and trade-off analysis";
        assert_eq!(router.select(complex), "claude-opus");
    }

    #[test]
    fn router_describe_returns_readable() {
        let router = ModelRouter::new("gpt-4".into());
        let info = router.describe("hello world");
        assert!(info.contains("Low"));
        assert!(info.contains("gpt-4"));
    }
}
