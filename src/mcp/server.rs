//! MCP (Model Context Protocol) server: exposes cargo-agent tools via JSON-RPC 2.0.
//!
//! Implements the MCP transport layer over stdio, allowing external MCP-compatible
//! clients to discover and invoke the agent's built-in tools.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

/// MCP tool definition.
#[derive(Debug, serde::Serialize)]
struct McpTool {
    name: String,
    description: String,
    input_schema: McpInputSchema,
}

#[derive(Debug, serde::Serialize)]
struct McpInputSchema {
    #[serde(rename = "type")]
    schema_type: String,
    properties: HashMap<String, McpProperty>,
    required: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct McpProperty {
    #[serde(rename = "type")]
    prop_type: String,
    description: String,
}

/// Start the MCP server loop on stdio.
///
/// Reads JSON-RPC 2.0 requests from stdin, dispatches to the tool registry,
/// and writes responses to stdout.
pub async fn run_stdio_server(tool_registry: &crate::tools::ToolRegistry) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let tools = tool_registry.list_tools();

    for line in stdin.lock().lines() {
        let line = line?;
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = error_response(None, -32700, format!("Parse error: {e}"));
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let method = request["method"].as_str().unwrap_or("");
        let id = request.get("id").cloned();

        let response = match method {
            "initialize" => Ok(initialize_response(id.clone(), &tools)),
            "tools/list" => Ok(tools_list_response(id.clone(), &tools)),
            "tools/call" => {
                let tool_name = request["params"]["name"].as_str().unwrap_or("");
                let arguments = &request["params"]["arguments"];
                call_tool(id.clone(), tool_registry, tool_name, arguments).await
            }
            _ => Ok(error_response(
                id.clone(),
                -32601,
                format!("Method not found: {method}"),
            )),
        };

        if let Ok(resp) = response {
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn initialize_response(id: Option<Value>, _tools: &[&dyn crate::tools::registry::Tool]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "cargo-agent",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn tools_list_response(id: Option<Value>, tools: &[&dyn crate::tools::registry::Tool]) -> Value {
    let mcp_tools: Vec<McpTool> = tools
        .iter()
        .map(|t| {
            let mut properties = HashMap::new();
            let mut required = Vec::new();
            for param in t.parameters() {
                properties.insert(
                    param.name.clone(),
                    McpProperty {
                        prop_type: param.parameter_type.clone(),
                        description: param.description.clone(),
                    },
                );
                if param.required {
                    required.push(param.name.clone());
                }
            }
            McpTool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: McpInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required,
                },
            }
        })
        .collect();

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": mcp_tools
        }
    })
}

async fn call_tool(
    id: Option<Value>,
    registry: &crate::tools::ToolRegistry,
    name: &str,
    arguments: &Value,
) -> anyhow::Result<Value> {
    let tool = match registry.get(name) {
        Some(t) => t,
        None => {
            return Ok(error_response(id, -32602, format!("Unknown tool: {name}")));
        }
    };

    let params: HashMap<String, Value> = arguments
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    match tool.execute(&params).await {
        Ok(result) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": result.to_string()
                }]
            }
        })),
        Err(e) => Ok(error_response(id, -32000, format!("Tool error: {e}"))),
    }
}

fn error_response(id: Option<Value>, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}
