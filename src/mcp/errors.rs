//! MCP-specific error types.
//!
//! Provides a comprehensive `McpError` enum for all failure modes
//! in the MCP client/server lifecycle.

use thiserror::Error;

/// All possible errors in MCP operations.
#[derive(Debug, Error)]
pub enum McpError {
    /// Failed to connect to the MCP server.
    #[error("failed to connect to MCP server '{name}': {source}")]
    ConnectFailed { name: String, source: anyhow::Error },

    /// Transport error during communication.
    #[error("transport error: {0}")]
    TransportError(String),

    /// Protocol-level error (invalid JSON, missing fields, etc.).
    #[error("protocol error: {0}")]
    ProtocolError(String),

    /// The requested tool was not found on the server.
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    /// Tool call returned an error.
    #[error("tool call failed: {0}")]
    ToolCallFailed(String),

    /// The server disconnected unexpectedly.
    #[error("server disconnected: {0}")]
    ServerDisconnected(String),

    /// Operation timed out.
    #[error("operation timed out after {timeout_ms}ms: {operation}")]
    Timeout { operation: String, timeout_ms: u64 },

    /// Initialization handshake failed.
    #[error("MCP initialization failed: {0}")]
    InitializeFailed(String),

    /// Resource operation failed.
    #[error("resource error: {0}")]
    ResourceError(String),

    /// Prompt operation failed.
    #[error("prompt error: {0}")]
    PromptError(String),

    /// The server is not connected.
    #[error("MCP server '{name}' is not connected")]
    NotConnected { name: String },

    /// JSON parsing error.
    #[error("JSON parse error: {0}")]
    ParseError(#[from] serde_json::Error),

    /// Generic anyhow-wrapped error.
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience type alias for `Result<T, McpError>`.
pub type McpResult<T> = Result<T, McpError>;

/// Convert a JSON-RPC error response to an `McpError`.
pub fn from_jsonrpc_error(code: i64, message: &str) -> McpError {
    match code {
        -32602 | crate::mcp::types::TOOL_NOT_FOUND => McpError::ToolNotFound(message.to_string()),
        crate::mcp::types::TOOL_CALL_FAILED => McpError::ToolCallFailed(message.to_string()),
        crate::mcp::types::RESOURCE_NOT_FOUND => McpError::ResourceError(message.to_string()),
        crate::mcp::types::PROMPT_NOT_FOUND => McpError::PromptError(message.to_string()),
        crate::mcp::types::SERVER_NOT_CONNECTED => {
            McpError::ServerDisconnected(message.to_string())
        }
        _ => McpError::ProtocolError(format!("JSON-RPC error {code}: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = McpError::ToolNotFound("read_file".to_string());
        assert!(err.to_string().contains("read_file"));

        let err = McpError::Timeout {
            operation: "initialize".to_string(),
            timeout_ms: 5000,
        };
        assert!(err.to_string().contains("5000ms"));
        assert!(err.to_string().contains("initialize"));
    }

    #[test]
    fn test_from_jsonrpc_error() {
        let err = from_jsonrpc_error(crate::mcp::types::TOOL_NOT_FOUND, "Unknown tool: read_file");
        assert!(matches!(err, McpError::ToolNotFound(_)));
        assert!(err.to_string().contains("read_file"));

        let err = from_jsonrpc_error(-99999, "custom error");
        assert!(matches!(err, McpError::ProtocolError(_)));
    }

    #[test]
    fn test_connect_failed() {
        let err = McpError::ConnectFailed {
            name: "my-server".to_string(),
            source: anyhow::anyhow!("command not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("my-server"));
        assert!(msg.contains("command not found"));
    }

    #[test]
    fn test_not_connected() {
        let err = McpError::NotConnected {
            name: "fs-server".to_string(),
        };
        assert!(err.to_string().contains("fs-server"));
    }

    #[test]
    fn test_mcp_result_type() {
        fn example() -> McpResult<String> {
            Ok("hello".to_string())
        }
        assert_eq!(example().unwrap(), "hello");
    }
}
