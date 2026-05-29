use serde_json::Value;
use std::collections::HashMap;

/// Describes a single parameter of a tool.
///
/// # Example
///
/// ```
/// use cargo_agent::tools::ToolParameter;
///
/// let param = ToolParameter {
///     name: "file_path".to_string(),
///     description: "Path to the file to read".to_string(),
///     required: true,
///     parameter_type: "string".to_string(),
/// };
///
/// assert_eq!(param.name, "file_path");
/// assert!(param.required);
/// ```
#[derive(Clone, Debug)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub parameter_type: String,
}

/// A trait for defining executable tools that can be registered with the system.
///
/// # Example
///
/// ```
/// use cargo_agent::tools::{Tool, ToolParameter, ToolRegistry};
/// use serde_json::Value;
/// use std::collections::HashMap;
///
/// struct GreetTool;
///
/// #[async_trait::async_trait]
/// impl Tool for GreetTool {
///     fn name(&self) -> &str { "greet" }
///     fn description(&self) -> &str { "Greets a person by name" }
///     fn parameters(&self) -> Vec<ToolParameter> {
///         vec![ToolParameter {
///             name: "name".into(),
///             description: "The person's name".into(),
///             required: true,
///             parameter_type: "string".into(),
///         }]
///     }
///     async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
///         let name = params.get("name")
///             .and_then(|v| v.as_str())
///             .unwrap_or("World");
///         Ok(serde_json::json!({ "message": format!("Hello, {}!", name) }))
///     }
/// }
///
/// let mut registry = ToolRegistry::new();
/// registry.register(Box::new(GreetTool));
///
/// let tool = registry.get("greet").unwrap();
/// assert_eq!(tool.name(), "greet");
/// assert!(tool.description().contains("Greets"));
/// assert_eq!(tool.parameters().len(), 1);
/// ```
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String>;
}

/// A registry that maps tool names to their implementations.
///
/// # Example
///
/// ```
/// use cargo_agent::tools::{ToolRegistry, Tool, ToolParameter};
/// use serde_json::Value;
/// use std::collections::HashMap;
///
/// // Create an empty registry
/// let mut registry = ToolRegistry::new();
/// assert!(registry.list_tools().is_empty());
///
/// // Registering and listing tools is done through the Tool trait
/// // (see the Tool trait example for a full implementation)
/// ```
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new, empty tool registry.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::tools::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert!(registry.list_tools().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool with the registry.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::tools::{ToolRegistry, Tool, ToolParameter};
    /// use serde_json::Value;
    /// use std::collections::HashMap;
    ///
    /// struct EchoTool;
    ///
    /// #[async_trait::async_trait]
    /// impl Tool for EchoTool {
    ///     fn name(&self) -> &str { "echo" }
    ///     fn description(&self) -> &str { "Echoes back input" }
    ///     fn parameters(&self) -> Vec<ToolParameter> { vec![] }
    ///     async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
    ///         Ok(serde_json::json!({ "echo": true }))
    ///     }
    /// }
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Box::new(EchoTool));
    /// assert!(registry.get("echo").is_some());
    /// ```
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get a registered tool by name.
    ///
    /// Returns `None` if no tool with the given name is registered.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// List all registered tools.
    pub fn list_tools(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTool;

    #[async_trait::async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test"
        }
        fn description(&self) -> &str {
            "A test tool"
        }
        fn parameters(&self) -> Vec<ToolParameter> {
            vec![]
        }
        async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }

    #[test]
    fn test_registry_new_and_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.list_tools().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(TestTool));
        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(TestTool));
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "test");
    }

    #[test]
    fn test_tool_parameter_struct() {
        let param = ToolParameter {
            name: "input".into(),
            description: "Input value".into(),
            required: true,
            parameter_type: "string".into(),
        };
        assert_eq!(param.name, "input");
        assert!(param.required);
    }

    #[test]
    fn test_registry_overwrite() {
        struct ToolA;
        struct ToolB;

        #[async_trait::async_trait]
        impl Tool for ToolA {
            fn name(&self) -> &str {
                "dup"
            }
            fn description(&self) -> &str {
                "version A"
            }
            fn parameters(&self) -> Vec<ToolParameter> {
                vec![]
            }
            async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
                Ok(serde_json::json!({ "version": "A" }))
            }
        }

        #[async_trait::async_trait]
        impl Tool for ToolB {
            fn name(&self) -> &str {
                "dup"
            }
            fn description(&self) -> &str {
                "version B"
            }
            fn parameters(&self) -> Vec<ToolParameter> {
                vec![]
            }
            async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
                Ok(serde_json::json!({ "version": "B" }))
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ToolA));
        registry.register(Box::new(ToolB)); // overwrites A

        let tool = registry.get("dup").unwrap();
        // ToolB replaced ToolA since they share the same name
        assert_eq!(registry.list_tools().len(), 1);
    }
}
