//! MCP (Model Context Protocol) protocol types.
//!
//! Defines JSON-RPC 2.0 core types and MCP-specific protocol types for
//! tools, resources, and prompts capabilities.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── JSON-RPC 2.0 Error Codes ───────────────────────────────────

/// Standard JSON-RPC 2.0 error codes.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// MCP-specific error codes (in the -32000 range for server errors).
pub const TOOL_NOT_FOUND: i64 = -32001;
pub const TOOL_CALL_FAILED: i64 = -32002;
pub const RESOURCE_NOT_FOUND: i64 = -32003;
pub const PROMPT_NOT_FOUND: i64 = -32004;
pub const SERVER_NOT_CONNECTED: i64 = -32005;

// ─── JSON-RPC 2.0 Core Types ────────────────────────────────────

/// A JSON-RPC 2.0 request (has a method and optional id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// Create a new request with an auto-generated numeric id.
    pub fn with_id(id: u64, method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(id.into())),
            method: method.to_string(),
            params: Some(params),
        }
    }

    /// Create a notification (no id, so no response expected).
    pub fn notification(method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params: Some(params),
        }
    }
}

/// A JSON-RPC 2.0 success response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 notification (no id, one-way).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ─── MCP Protocol Types ─────────────────────────────────────────

/// MCP initialize request (client → server).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeRequest {
    pub protocol_version: String,
    pub capabilities: McpClientCapabilities,
    pub client_info: McpClientInfo,
}

/// Capabilities advertised by the client during initialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<McpRootsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRootsCapability {
    pub list_changed: bool,
}

/// Client information sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

/// MCP initialize result (server → client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeResult {
    pub protocol_version: String,
    pub capabilities: McpServerCapabilities,
    pub server_info: McpServerInfo,
}

/// Capabilities advertised by the server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<McpToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<McpResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<McpPromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpToolsCapability {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpResourcesCapability {
    pub subscribe: bool,
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpPromptsCapability {
    pub list_changed: bool,
}

/// Server information sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

// ─── MCP Tool Types ─────────────────────────────────────────────

/// A tool definition as returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: McpToolInputSchema,
}

/// JSON Schema for tool input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
    #[serde(default)]
    pub required: Vec<String>,
}

/// Result from calling a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// A content block in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

// ─── MCP Resource Types ─────────────────────────────────────────

/// A resource definition as returned by `resources/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Result from reading a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceReadResult {
    pub contents: Vec<McpResourceContents>,
}

/// Contents of a resource after reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContents {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

// ─── MCP Prompt Types ───────────────────────────────────────────

/// A prompt definition as returned by `prompts/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<McpPromptArgument>,
}

/// An argument for a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Result from getting a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptResult {
    pub description: Option<String>,
    pub messages: Vec<McpPromptMessage>,
}

/// A message in a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptMessage {
    pub role: String,
    pub content: McpContentBlock,
}

// ─── MCP Root Types ─────────────────────────────────────────────

/// A root directory or workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRoot {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Result from listing roots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRootsListResult {
    pub roots: Vec<McpRoot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest::with_id(1, "tools/list", Value::Null);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
    }

    #[test]
    fn test_json_rpc_notification_has_no_id() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let json = serde_json::to_string(&notif).unwrap();
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_json_rpc_error_serialization() {
        let err = JsonRpcError {
            code: PARSE_ERROR,
            message: "Parse error".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains(&PARSE_ERROR.to_string()));
    }

    #[test]
    fn test_mcp_tool_definition() {
        let tool = McpToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: McpToolInputSchema {
                schema_type: "object".to_string(),
                properties: serde_json::Map::new(),
                required: vec!["path".to_string()],
            },
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("read_file"));
        assert!(json.contains("input_schema"));
    }

    #[test]
    fn test_mcp_initialize_result() {
        let result = McpInitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpServerCapabilities {
                tools: Some(McpToolsCapability::default()),
                resources: Some(McpResourcesCapability {
                    subscribe: false,
                    list_changed: false,
                }),
                prompts: None,
                logging: None,
            },
            server_info: McpServerInfo {
                name: "cargo-agent".to_string(),
                version: "0.1.0".to_string(),
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("2024-11-05"));
        assert!(json.contains("cargo-agent"));
        assert!(json.contains("\"tools\":"));
    }

    #[test]
    fn test_mcp_content_block() {
        let block = McpContentBlock {
            content_type: "text".to_string(),
            text: Some("hello".to_string()),
            data: None,
            mime_type: None,
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
        assert_eq!(TOOL_NOT_FOUND, -32001);
        assert_eq!(TOOL_CALL_FAILED, -32002);
    }
}
