//! MCP transport layer abstraction.
//!
//! Provides a `Transport` trait with two implementations:
//! - `StdioTransport`: communicates with a subprocess over stdio
//! - `HttpSseTransport`: communicates over HTTP POST

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

/// Maximum time to wait for a response (milliseconds).
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Trait for MCP transport implementations.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Establish the connection.
    async fn connect(&mut self) -> anyhow::Result<()>;

    /// Close the connection.
    async fn disconnect(&mut self) -> anyhow::Result<()>;

    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(&mut self, request: &Value) -> anyhow::Result<Value>;

    /// Check if the transport is connected.
    fn is_connected(&self) -> bool;

    /// Human-readable name for this transport.
    fn name(&self) -> &str;
}

// ─── StdioTransport ──────────────────────────────────────────────

/// Transport that communicates with a subprocess over stdin/stdout.
pub struct StdioTransport {
    name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    timeout_ms: u64,

    // Runtime state
    child: Option<tokio::process::Child>,
    connected: Arc<AtomicBool>,
    request_tx: Option<mpsc::Sender<(Value, oneshot::Sender<anyhow::Result<Value>>)>>,
    read_loop_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new(
        name: &str,
        command: &str,
        args: Vec<String>,
        env: HashMap<String, String>,
        timeout_ms: Option<u64>,
    ) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args,
            env,
            timeout_ms: timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
            child: None,
            connected: Arc::new(AtomicBool::new(false)),
            request_tx: None,
            read_loop_handle: None,
        }
    }

    fn build_command(&self) -> Command {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn connect(&mut self) -> anyhow::Result<()> {
        let mut child = self.build_command().spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            anyhow::anyhow!("failed to capture stdin from subprocess '{}'", self.command)
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            anyhow::anyhow!(
                "failed to capture stdout from subprocess '{}'",
                self.command
            )
        })?;

        self.child = Some(child);

        // Request/response channel
        let (req_tx, mut req_rx) =
            mpsc::channel::<(Value, oneshot::Sender<anyhow::Result<Value>>)>(32);

        let connected = self.connected.clone();

        // Spawn the I/O handler task
        let read_handle = tokio::spawn(async move {
            let mut reader = TokioBufReader::new(stdout);
            let mut writer = tokio::io::BufWriter::new(stdin);
            let mut pending: HashMap<serde_json::Value, oneshot::Sender<anyhow::Result<Value>>> =
                HashMap::new();

            connected.store(true, Ordering::SeqCst);

            let mut line = String::new();
            loop {
                tokio::select! {
                    // Read a line from stdout
                    n = reader.read_line(&mut line) => {
                        match n {
                            Ok(0) => {
                                // EOF — server exited
                                connected.store(false, Ordering::SeqCst);
                                for (_, reply) in pending.drain() {
                                    let _ = reply.send(Err(anyhow::anyhow!(
                                        "MCP server exited unexpectedly"
                                    )));
                                }
                                break;
                            }
                            Ok(_) => {
                                if let Ok(resp) = serde_json::from_str::<Value>(line.trim()) {
                                    if let Some(id) = resp.get("id").cloned() {
                                        if let Some(reply) = pending.remove(&id) {
                                            let _ = reply.send(Ok(resp));
                                        }
                                    }
                                }
                                line.clear();
                            }
                            Err(e) => {
                                tracing::error!("MCP read error: {e}");
                                connected.store(false, Ordering::SeqCst);
                                break;
                            }
                        }
                    }

                    // Send a request
                    Some((request, reply)) = req_rx.recv() => {
                        if let Some(id) = request.get("id").cloned() {
                            pending.insert(id, reply);
                        }
                        let req_line = serde_json::to_string(&request).unwrap_or_default();
                        if let Err(e) = writer.write_all(format!("{req_line}\n").as_bytes()).await {
                            connected.store(false, Ordering::SeqCst);
                            let req_id = request.get("id").cloned();
                            if let Some(id) = req_id {
                                if let Some(reply) = pending.remove(&id) {
                                    let _ = reply.send(Err(anyhow::anyhow!("write error: {e}")));
                                }
                            }
                            break;
                        }
                        if let Err(_e) = writer.flush().await {
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                    }

                    // Channel closed — no more requests coming
                    else => {
                        break;
                    }
                }
            }
        });

        self.request_tx = Some(req_tx);
        self.read_loop_handle = Some(read_handle);

        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        // Drop the request channel to signal the read loop to exit
        self.request_tx = None;
        if let Some(handle) = self.read_loop_handle.take() {
            handle.abort();
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        Ok(())
    }

    async fn send_request(&mut self, request: &Value) -> anyhow::Result<Value> {
        let tx = self
            .request_tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;

        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send((request.clone(), reply_tx)).await.map_err(|_| {
            anyhow::anyhow!("MCP server '{}' dropped the request channel", self.name)
        })?;

        let timeout_ms = self.timeout_ms;
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), reply_rx)
            .await
            .map_err(|_| anyhow::anyhow!("request timed out after {timeout_ms}ms"))?
            .map_err(|e| anyhow::anyhow!("response channel closed: {e}"))?
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ─── HttpSseTransport ────────────────────────────────────────────

/// Transport that communicates over HTTP POST.
pub struct HttpSseTransport {
    name: String,
    url: String,
    client: reqwest::Client,
    timeout_ms: u64,
    connected: bool,
}

impl HttpSseTransport {
    /// Create a new HTTP/SSE transport.
    pub fn new(name: &str, url: &str, timeout_ms: Option<u64>) -> Self {
        Self {
            name: name.to_string(),
            url: url.to_string(),
            client: reqwest::Client::new(),
            timeout_ms: timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
            connected: false,
        }
    }
}

#[async_trait]
impl Transport for HttpSseTransport {
    async fn connect(&mut self) -> anyhow::Result<()> {
        let resp = self
            .client
            .get(&self.url)
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("failed to connect to {url}: {e}", url = self.url))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "MCP server at {} returned HTTP {}",
                self.url,
                resp.status()
            ));
        }

        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn send_request(&mut self, request: &Value) -> anyhow::Result<Value> {
        let resp = self
            .client
            .post(&self.url)
            .json(request)
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "MCP server returned HTTP {}",
                resp.status()
            ));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("failed to parse MCP response: {e}"))?;

        Ok(body)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ─── Transport Factory ───────────────────────────────────────────

/// Create a transport from configuration.
///
/// Returns a boxed `dyn Transport` based on the config's transport type:
/// - `"stdio"` or `None` → `StdioTransport`
/// - `"http"` or `"sse"` → `HttpSseTransport`
pub fn create_transport(
    name: &str,
    command: Option<&str>,
    args: Vec<String>,
    env: HashMap<String, String>,
    url: Option<&str>,
    transport: Option<&str>,
    timeout: Option<u64>,
) -> anyhow::Result<Box<dyn Transport>> {
    match transport.unwrap_or("stdio") {
        "stdio" | "std" | "" => {
            let cmd = command.ok_or_else(|| {
                anyhow::anyhow!("stdio transport requires 'command' for MCP server '{name}'")
            })?;
            Ok(Box::new(StdioTransport::new(name, cmd, args, env, timeout)))
        }
        "http" | "sse" | "https" => {
            let endpoint = url.ok_or_else(|| {
                anyhow::anyhow!("HTTP/SSE transport requires 'url' for MCP server '{name}'")
            })?;
            Ok(Box::new(HttpSseTransport::new(name, endpoint, timeout)))
        }
        other => Err(anyhow::anyhow!(
            "unknown transport type: '{other}' (supported: stdio, http, sse)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_stdio_requires_command() {
        let result = create_transport(
            "test",
            None,
            vec![],
            HashMap::new(),
            None,
            Some("stdio"),
            None,
        );
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("command"));
    }

    #[test]
    fn test_factory_http_requires_url() {
        let result = create_transport(
            "test",
            None,
            vec![],
            HashMap::new(),
            None,
            Some("http"),
            None,
        );
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("url"));
    }

    #[test]
    fn test_factory_unknown_transport() {
        let result = create_transport(
            "test",
            Some("echo"),
            vec![],
            HashMap::new(),
            None,
            Some("websocket"),
            None,
        );
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("unknown transport"));
    }

    #[test]
    fn test_factory_stdio_default() {
        let result = create_transport(
            "test",
            Some("echo"),
            vec![],
            HashMap::new(),
            None,
            None,
            None,
        );
        assert!(result.is_ok());
        let transport = result.unwrap();
        assert_eq!(transport.name(), "test");
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_http_transport_creation() {
        let transport = HttpSseTransport::new("test-http", "http://localhost:3000", None);
        assert_eq!(transport.name(), "test-http");
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_stdio_transport_creation() {
        let transport = StdioTransport::new(
            "test-stdio",
            "node",
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                ".".to_string(),
            ],
            HashMap::new(),
            Some(10000),
        );
        assert_eq!(transport.name(), "test-stdio");
        assert!(!transport.is_connected());
    }
}
