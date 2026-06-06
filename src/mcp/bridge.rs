//! MCP bridge: manages connections to multiple external MCP servers.
//!
//! `McpBridge` discovers tools from configured MCP servers and registers
//! them into the main `ToolRegistry` via `McpToolAdapter`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::McpServerConfig;
use crate::mcp::adapter::McpToolAdapter;
use crate::mcp::client::McpClient;
use crate::mcp::errors::McpResult;
use crate::mcp::transport;
use crate::tools::registry::Tool;
use crate::tools::ToolRegistry;

/// Status of a single MCP server connection.
#[derive(Debug, Clone)]
pub struct ServerStatus {
    /// Server display name.
    pub name: String,
    /// Whether the server is currently connected.
    pub connected: bool,
    /// Number of tools discovered.
    pub tool_count: usize,
    /// Error message if connection failed.
    pub error: Option<String>,
}

/// Manages multiple MCP server connections.
pub struct McpBridge {
    /// Map from server name to its client.
    clients: HashMap<String, Arc<tokio::sync::Mutex<McpClient>>>,
    /// Map from server name to its config.
    configs: HashMap<String, McpServerConfig>,
    /// Status of each server.
    statuses: HashMap<String, ServerStatus>,
}

impl McpBridge {
    /// Create an empty bridge.
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            configs: HashMap::new(),
            statuses: HashMap::new(),
        }
    }

    /// Add a server configuration to the bridge.
    pub fn add_config(&mut self, name: &str, config: McpServerConfig) {
        self.configs.insert(name.to_string(), config);
    }

    /// Start all enabled servers and register their tools.
    ///
    /// Non-blocking: failures are logged and the bridge continues
    /// with whatever servers successfully connected.
    pub async fn start_all(&mut self, registry: &mut ToolRegistry) {
        let names: Vec<String> = self
            .configs
            .keys()
            .filter(|name| self.configs.get(*name).map(|c| c.enabled).unwrap_or(false))
            .cloned()
            .collect();

        for name in names {
            if let Err(e) = self.start_server(&name, registry).await {
                let err_msg = e.to_string();
                tracing::warn!("MCP server '{name}' failed to start: {err_msg}");
                self.statuses.insert(
                    name.clone(),
                    ServerStatus {
                        name,
                        connected: false,
                        tool_count: 0,
                        error: Some(err_msg),
                    },
                );
            }
        }
    }

    /// Start a single server and register its tools.
    pub async fn start_server(&mut self, name: &str, registry: &mut ToolRegistry) -> McpResult<()> {
        let config = self
            .configs
            .get(name)
            .ok_or_else(|| {
                crate::mcp::errors::McpError::ProtocolError(format!("unknown MCP server: {name}"))
            })?
            .clone();

        if !config.enabled {
            return Ok(());
        }

        tracing::info!("Connecting to MCP server: {name}");

        // Create transport
        let transport = transport::create_transport(
            name,
            config.command.as_deref(),
            config.args.clone(),
            config.env.clone(),
            config.url.as_deref(),
            config.transport.as_deref(),
            config.timeout,
        )
        .map_err(|e| crate::mcp::errors::McpError::ConnectFailed {
            name: name.to_string(),
            source: e,
        })?;

        // Create and connect client
        let mut client = McpClient::new(name, transport);
        client.connect().await?;

        let tool_count = client.tools().len();
        tracing::info!("MCP server '{name}' connected: {tool_count} tool(s) discovered");

        // Wrap tools as internal Tool implementations
        let client_arc = Arc::new(tokio::sync::Mutex::new(client));

        // Register tools
        {
            let client_guard = client_arc.lock().await;
            for tool_def in client_guard.tools() {
                let adapter = McpToolAdapter::from_definition(name, tool_def, client_arc.clone());
                tracing::info!("  Registering MCP tool: {} (from {name})", adapter.name());
                registry.register(Box::new(adapter));
            }
        }

        // Store client reference
        self.clients.insert(name.to_string(), client_arc);

        // Update status
        self.statuses.insert(
            name.to_string(),
            ServerStatus {
                name: name.to_string(),
                connected: true,
                tool_count,
                error: None,
            },
        );

        Ok(())
    }

    /// Stop a single server.
    pub async fn stop_server(&mut self, name: &str) -> McpResult<()> {
        if let Some(client_arc) = self.clients.remove(name) {
            let mut client = client_arc.lock().await;
            client.disconnect().await?;
            tracing::info!("MCP server '{name}' disconnected");
        }

        self.statuses.insert(
            name.to_string(),
            ServerStatus {
                name: name.to_string(),
                connected: false,
                tool_count: 0,
                error: None,
            },
        );

        Ok(())
    }

    /// Restart a server (stop, then start again).
    pub async fn restart_server(
        &mut self,
        name: &str,
        registry: &mut ToolRegistry,
    ) -> McpResult<()> {
        // First, remove old tools from registry
        let old_tools: Vec<String> = if let Some(client_arc) = self.clients.get(name) {
            let client = client_arc.lock().await;
            client
                .tools()
                .iter()
                .map(|t| crate::mcp::adapter::prefixed_name(name, &t.name))
                .collect()
        } else {
            vec![]
        };

        // Remove old tools
        for tool_name in &old_tools {
            registry.remove(tool_name);
        }

        // Stop the client
        let _ = self.stop_server(name).await;

        // Start again
        self.start_server(name, registry).await
    }

    /// Get status of all servers.
    pub fn status(&self) -> Vec<ServerStatus> {
        let mut result: Vec<ServerStatus> = self.statuses.values().cloned().collect();

        // Also include configured but not-yet-started servers
        for (name, config) in &self.configs {
            if !self.statuses.contains_key(name) {
                result.push(ServerStatus {
                    name: name.clone(),
                    connected: false,
                    tool_count: 0,
                    error: if !config.enabled {
                        Some("disabled".to_string())
                    } else {
                        Some("not started".to_string())
                    },
                });
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    /// Check if a specific server is connected.
    pub fn is_connected(&self, name: &str) -> bool {
        self.statuses
            .get(name)
            .map(|s| s.connected)
            .unwrap_or(false)
    }

    /// Return the number of connected servers.
    pub fn connected_count(&self) -> usize {
        self.statuses.values().filter(|s| s.connected).count()
    }

    /// Return the total number of MCP tools registered.
    pub fn total_mcp_tools(&self) -> usize {
        self.statuses.values().map(|s| s.tool_count).sum()
    }
}

impl Default for McpBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bridge_is_empty() {
        let bridge = McpBridge::new();
        assert!(bridge.status().is_empty());
        assert_eq!(bridge.connected_count(), 0);
        assert_eq!(bridge.total_mcp_tools(), 0);
    }

    #[test]
    fn test_add_config() {
        let mut bridge = McpBridge::new();
        bridge.add_config(
            "test-server",
            McpServerConfig {
                enabled: true,
                ..Default::default()
            },
        );
        let status = bridge.status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].name, "test-server");
        // Should show as not started since we haven't called start_all
        assert!(!status[0].connected);
    }

    #[test]
    fn test_disabled_server_skipped() {
        let mut bridge = McpBridge::new();
        bridge.add_config(
            "disabled-server",
            McpServerConfig {
                enabled: false,
                ..Default::default()
            },
        );
        // start_all would skip this server
        let status = bridge.status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].error, Some("disabled".to_string()));
    }

    #[test]
    fn test_status_sorting() {
        let mut bridge = McpBridge::new();
        bridge.add_config("z-server", McpServerConfig::default());
        bridge.add_config("a-server", McpServerConfig::default());
        bridge.add_config("m-server", McpServerConfig::default());

        let status = bridge.status();
        assert_eq!(status.len(), 3);
        assert_eq!(status[0].name, "a-server");
        assert_eq!(status[1].name, "m-server");
        assert_eq!(status[2].name, "z-server");
    }

    #[test]
    fn test_is_connected_unknown() {
        let bridge = McpBridge::new();
        assert!(!bridge.is_connected("unknown"));
    }
}
