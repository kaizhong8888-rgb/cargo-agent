use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

pub struct ReadFile;

#[async_trait::async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the file".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;
        
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(serde_json::json!({
                "status": "ok",
                "content": content,
                "path": path,
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "message": format!("Failed to read file: {}", e),
            })),
        }
    }
}

pub struct WriteFile;

#[async_trait::async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the file".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "content".to_string(),
                description: "Content to write".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;
        let content = params.get("content")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: content")?;
        
        match std::fs::write(path, content) {
            Ok(_) => Ok(serde_json::json!({
                "status": "ok",
                "path": path,
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "message": format!("Failed to write file: {}", e),
            })),
        }
    }
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ReadFile));
    registry.register(Box::new(WriteFile));
}
