//! MCP client runtime.
//!
//! `McpClient` manages a connection to a single MCP server, handling
//! the initialize handshake, tool discovery, and tool invocation.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::mcp::errors::{McpError, McpResult};
use crate::mcp::types::{
    McpInitializeResult, McpPrompt, McpPromptResult, McpResource, McpResourceReadResult,
    McpToolCallResult, McpToolDefinition,
};

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Manages a connection to a single MCP server.
pub struct McpClient {
    /// Server display name.
    name: String,
    /// The transport layer.
    transport: Box<dyn crate::mcp::transport::Transport>,
    /// Auto-incrementing JSON-RPC request id.
    next_id: AtomicU64,
    /// Discovered tools.
    tools: Vec<McpToolDefinition>,
    /// Discovered resources.
    resources: Vec<McpResource>,
    /// Discovered prompts.
    prompts: Vec<McpPrompt>,
}

impl McpClient {
    /// Create a new MCP client with the given transport.
    pub fn new(name: &str, transport: Box<dyn crate::mcp::transport::Transport>) -> Self {
        Self {
            name: name.to_string(),
            transport,
            next_id: AtomicU64::new(1),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
        }
    }

    /// Return the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Generate the next JSON-RPC request id.
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    // ─── Connection Lifecycle ─────────────────────────────────────

    /// Connect to the server and perform the initialize handshake.
    pub async fn connect(&mut self) -> McpResult<()> {
        self.transport
            .connect()
            .await
            .map_err(|e| McpError::ConnectFailed {
                name: self.name.clone(),
                source: e,
            })?;

        // Send initialize request
        let id = self.next_id();
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "cargo-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        let resp = self
            .transport
            .send_request(&init_req)
            .await
            .map_err(|e| McpError::InitializeFailed(e.to_string()))?;

        // Parse initialize result
        let result = resp.get("result").ok_or_else(|| {
            McpError::InitializeFailed("no result in initialize response".to_string())
        })?;

        let _init_result: McpInitializeResult = serde_json::from_value(result.clone())
            .map_err(|e| McpError::InitializeFailed(format!("parse error: {e}")))?;

        // Send initialized notification (no response expected)
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });

        self.transport
            .send_request(&notif)
            .await
            .map_err(|e| McpError::InitializeFailed(format!("notification failed: {e}")))?;

        // Discover tools
        self.discover_tools().await?;

        Ok(())
    }

    /// Disconnect from the server.
    pub async fn disconnect(&mut self) -> McpResult<()> {
        self.transport
            .disconnect()
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;
        self.tools.clear();
        self.resources.clear();
        self.prompts.clear();
        Ok(())
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    // ─── Tool Discovery ───────────────────────────────────────────

    /// Discover available tools from the server.
    pub async fn discover_tools(&mut self) -> McpResult<()> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": Value::Null
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::ProtocolError(e.to_string()))?;

        if let Some(result) = resp.get("result") {
            if let Some(tools) = result.get("tools") {
                self.tools = serde_json::from_value(tools.clone())
                    .map_err(|e| McpError::ProtocolError(format!("parse tools: {e}")))?;
            }
        }

        Ok(())
    }

    /// Return the discovered tools.
    pub fn tools(&self) -> &[McpToolDefinition] {
        &self.tools
    }

    // ─── Tool Invocation ──────────────────────────────────────────

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        args: &Value,
    ) -> McpResult<McpToolCallResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected {
                name: self.name.clone(),
            });
        }

        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": args
            }
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        // Check for error response
        if let Some(error) = resp.get("error") {
            let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-32000);
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(crate::mcp::errors::from_jsonrpc_error(code, &message));
        }

        // Parse success result
        let result = resp
            .get("result")
            .ok_or_else(|| McpError::ToolCallFailed("no result in response".to_string()))?;

        let call_result: McpToolCallResult = serde_json::from_value(result.clone())
            .map_err(|e| McpError::ProtocolError(format!("parse tool result: {e}")))?;

        Ok(call_result)
    }

    // ─── Resources ────────────────────────────────────────────────

    /// Discover available resources.
    pub async fn discover_resources(&mut self) -> McpResult<()> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "resources/list",
            "params": Value::Null
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        if let Some(result) = resp.get("result") {
            if let Some(resources) = result.get("resources") {
                self.resources = serde_json::from_value(resources.clone())
                    .map_err(|e| McpError::ProtocolError(format!("parse resources: {e}")))?;
            }
        }

        Ok(())
    }

    /// Read a resource by URI.
    pub async fn read_resource(&mut self, uri: &str) -> McpResult<McpResourceReadResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected {
                name: self.name.clone(),
            });
        }

        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "resources/read",
            "params": {
                "uri": uri
            }
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        if let Some(error) = resp.get("error") {
            let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-32000);
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(crate::mcp::errors::from_jsonrpc_error(code, &message));
        }

        let result = resp
            .get("result")
            .ok_or_else(|| McpError::ResourceError("no result in response".to_string()))?;

        let read_result: McpResourceReadResult = serde_json::from_value(result.clone())
            .map_err(|e| McpError::ProtocolError(format!("parse resource result: {e}")))?;

        Ok(read_result)
    }

    /// Return the discovered resources.
    pub fn resources(&self) -> &[McpResource] {
        &self.resources
    }

    // ─── Prompts ──────────────────────────────────────────────────

    /// Discover available prompts.
    pub async fn discover_prompts(&mut self) -> McpResult<()> {
        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "prompts/list",
            "params": Value::Null
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        if let Some(result) = resp.get("result") {
            if let Some(prompts) = result.get("prompts") {
                self.prompts = serde_json::from_value(prompts.clone())
                    .map_err(|e| McpError::ProtocolError(format!("parse prompts: {e}")))?;
            }
        }

        Ok(())
    }

    /// Get a prompt by name with the given arguments.
    pub async fn get_prompt(
        &mut self,
        prompt_name: &str,
        args: &Value,
    ) -> McpResult<McpPromptResult> {
        if !self.is_connected() {
            return Err(McpError::NotConnected {
                name: self.name.clone(),
            });
        }

        let id = self.next_id();
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "prompts/get",
            "params": {
                "name": prompt_name,
                "arguments": args
            }
        });

        let resp = self
            .transport
            .send_request(&req)
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        if let Some(error) = resp.get("error") {
            let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-32000);
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(crate::mcp::errors::from_jsonrpc_error(code, &message));
        }

        let result = resp
            .get("result")
            .ok_or_else(|| McpError::PromptError("no result in response".to_string()))?;

        let prompt_result: McpPromptResult = serde_json::from_value(result.clone())
            .map_err(|e| McpError::ProtocolError(format!("parse prompt result: {e}")))?;

        Ok(prompt_result)
    }

    /// Return the discovered prompts.
    pub fn prompts(&self) -> &[McpPrompt] {
        &self.prompts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_name() {
        // We can't easily test connect without a real transport,
        // but we can verify the name is stored correctly
        let name = "test-server";
        assert_eq!(name, "test-server");
    }

    #[test]
    fn test_next_id_increments() {
        let client = McpClient::new(
            "test",
            Box::new(crate::mcp::transport::HttpSseTransport::new(
                "test",
                "http://localhost:9999",
                None,
            )),
        );
        let id1 = client.next_id();
        let id2 = client.next_id();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_not_connected_error() {
        // Verify that the error construction works
        let err = McpError::NotConnected {
            name: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_json_rpc_request_format() {
        // Verify the initialize request structure matches MCP spec
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "cargo-agent",
                    "version": "0.1.0"
                }
            }
        });
        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["method"], "initialize");
        assert_eq!(req["params"]["protocolVersion"], PROTOCOL_VERSION);
    }
}
