//! MCP tool adapter: wraps external MCP tools as internal `Tool` implementations.
//!
//! `McpToolAdapter` implements the `Tool` trait, delegating execute() calls
//! to an `McpClient`. Tool names are prefixed with `{server_name}__` to
//! avoid collisions with builtin tools.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::mcp::client::McpClient;
use crate::tools::registry::{Tool, ToolParameter};

/// Build the prefixed tool name: `{server_name}__{tool_name}`.
pub fn prefixed_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}__{tool_name}")
}

/// Convert an MCP input schema to internal `ToolParameter` list.
pub fn parameters_from_schema(
    schema: &crate::mcp::types::McpToolInputSchema,
) -> Vec<ToolParameter> {
    let required_set: std::collections::HashSet<&str> =
        schema.required.iter().map(|s| s.as_str()).collect();

    schema
        .properties
        .iter()
        .map(|(name, value)| {
            let prop_type = value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string")
                .to_string();
            let description = value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ToolParameter {
                name: name.clone(),
                description,
                required: required_set.contains(name.as_str()),
                parameter_type: prop_type,
            }
        })
        .collect()
}

/// Wraps an external MCP tool as an internal `Tool` implementation.
///
/// The name is `{server_name}__{tool_name}` to avoid collisions with
/// builtin tools.
pub struct McpToolAdapter {
    /// The prefixed name: `{server_name}__{tool_name}`.
    name: String,
    /// The original tool name from the MCP server.
    tool_name: String,
    /// Description from the MCP server.
    description: String,
    /// Parameter schema from the MCP server.
    parameters: Vec<ToolParameter>,
    /// Shared reference to the MCP client.
    client: Arc<tokio::sync::Mutex<McpClient>>,
}

impl McpToolAdapter {
    /// Create a new adapter for an external MCP tool.
    pub fn new(
        server_name: &str,
        tool_name: &str,
        description: &str,
        parameters: Vec<ToolParameter>,
        client: Arc<tokio::sync::Mutex<McpClient>>,
    ) -> Self {
        Self {
            name: prefixed_name(server_name, tool_name),
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            parameters,
            client,
        }
    }

    /// Create an adapter from an MCP tool definition.
    pub fn from_definition(
        server_name: &str,
        definition: &crate::mcp::types::McpToolDefinition,
        client: Arc<tokio::sync::Mutex<McpClient>>,
    ) -> Self {
        let parameters = parameters_from_schema(&definition.input_schema);
        Self::new(
            server_name,
            &definition.name,
            &definition.description,
            parameters,
            client,
        )
    }

    /// Return the original (unprefixed) tool name.
    pub fn original_name(&self) -> &str {
        &self.tool_name
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        self.parameters.clone()
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let args: serde_json::Map<String, Value> =
            params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        let mut client = self.client.lock().await;
        match client
            .call_tool(&self.tool_name, &Value::Object(args))
            .await
        {
            Ok(result) => {
                let text = result
                    .content
                    .iter()
                    .filter(|c| c.content_type == "text")
                    .filter_map(|c| c.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n");

                if result.is_error.unwrap_or(false) {
                    Err(text)
                } else {
                    Ok(Value::String(text))
                }
            }
            Err(e) => Err(format!("MCP tool '{name}' failed: {e}", name = self.name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prefixed_name() {
        assert_eq!(
            prefixed_name("filesystem", "read_file"),
            "filesystem__read_file"
        );
    }

    #[test]
    fn test_parameters_from_schema() {
        use crate::mcp::types::McpToolInputSchema;
        use serde_json::Map;

        let mut properties = Map::new();
        properties.insert(
            "path".to_string(),
            json!({
                "type": "string",
                "description": "File path to read"
            }),
        );
        properties.insert(
            "limit".to_string(),
            json!({
                "type": "number",
                "description": "Max bytes to read"
            }),
        );

        let schema = McpToolInputSchema {
            schema_type: "object".to_string(),
            properties,
            required: vec!["path".to_string()],
        };

        let params = parameters_from_schema(&schema);
        assert_eq!(params.len(), 2);

        let path_param = params.iter().find(|p| p.name == "path").unwrap();
        assert!(path_param.required);
        assert_eq!(path_param.parameter_type, "string");
        assert!(path_param.description.contains("File path"));

        let limit_param = params.iter().find(|p| p.name == "limit").unwrap();
        assert!(!limit_param.required);
        assert_eq!(limit_param.parameter_type, "number");
    }

    #[test]
    fn test_parameters_from_empty_schema() {
        use crate::mcp::types::McpToolInputSchema;
        use serde_json::Map;

        let schema = McpToolInputSchema {
            schema_type: "object".to_string(),
            properties: Map::new(),
            required: vec![],
        };

        let params = parameters_from_schema(&schema);
        assert!(params.is_empty());
    }
}
