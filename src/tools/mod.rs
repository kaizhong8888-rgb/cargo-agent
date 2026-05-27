pub mod builtin;
pub mod registry;

pub use registry::{ToolRegistry, Tool, ToolParameter};

use serde_json::Value;
use std::collections::HashMap;

/// Context passed to tool calls.
///
/// Contains metadata about the current session and task,
/// allowing tools to access contextual information.
///
/// # Example
///
/// ```
/// use cargo_agent::tools::ToolContext;
///
/// let ctx = ToolContext::default();
/// assert_eq!(ctx.session_id, "default");
/// assert!(ctx.extras.is_empty());
/// ```
pub struct ToolContext {
    pub session_id: String,
    pub task_id: Option<String>,
    pub cargo_home: std::path::PathBuf,
    pub extras: HashMap<String, Value>,
}

impl Default for ToolContext {
    fn default() -> Self {
        ToolContext {
            session_id: "default".to_string(),
            task_id: None,
            cargo_home: std::path::PathBuf::from("."),
            extras: HashMap::new(),
        }
    }
}

impl ToolContext {
    /// Create a new `ToolContext` with the given session ID.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::tools::ToolContext;
    /// use std::collections::HashMap;
    ///
    /// let ctx = ToolContext::new("session-123".into());
    /// assert_eq!(ctx.session_id, "session-123");
    /// assert!(ctx.task_id.is_none());
    /// ```
    pub fn new(session_id: String) -> Self {
        ToolContext {
            session_id,
            task_id: None,
            cargo_home: std::path::PathBuf::from("."),
            extras: HashMap::new(),
        }
    }

    /// Set the task ID for this context.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::tools::ToolContext;
    ///
    /// let mut ctx = ToolContext::default();
    /// ctx.set_task_id("task-456");
    /// assert_eq!(ctx.task_id.as_deref(), Some("task-456"));
    /// ```
    pub fn set_task_id(&mut self, task_id: &str) {
        self.task_id = Some(task_id.to_string());
    }

    /// Insert an extra key-value pair into the context.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::tools::ToolContext;
    /// use serde_json::json;
    ///
    /// let mut ctx = ToolContext::default();
    /// ctx.insert_extra("user_id".into(), json!("u-789"));
    /// assert_eq!(ctx.extras.get("user_id").and_then(|v| v.as_str()), Some("u-789"));
    /// ```
    pub fn insert_extra(&mut self, key: String, value: Value) {
        self.extras.insert(key, value);
    }
}
