use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

struct HelloWorld;

#[async_trait::async_trait]
impl Tool for HelloWorld {
    fn name(&self) -> &str {
        "hello_world"
    }

    fn description(&self) -> &str {
        "A simple hello world tool for testing"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![]
    }

    async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
        Ok(serde_json::json!({
            "message": "Hello, World!"
        }))
    }
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(HelloWorld));
}
