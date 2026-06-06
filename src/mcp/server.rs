//! MCP (Model Context Protocol) server: exposes cargo-agent tools via JSON-RPC 2.0.
//!
//! Implements the MCP protocol over stdio, allowing external MCP-compatible
//! clients to discover and invoke the agent's built-in tools, resources, and prompts.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use crate::mcp::types::{
    METHOD_NOT_FOUND, PARSE_ERROR, PROMPT_NOT_FOUND, RESOURCE_NOT_FOUND, TOOL_NOT_FOUND,
};
use crate::tools::registry::Tool;

/// MCP tool definition for the protocol.
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

/// MCP resource definition.
#[derive(Debug, serde::Serialize)]
struct McpResourceDef {
    uri: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

/// MCP prompt definition.
#[derive(Debug, serde::Serialize)]
struct McpPromptDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    arguments: Vec<McpPromptArg>,
}

#[derive(Debug, serde::Serialize)]
struct McpPromptArg {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
}

/// Start the MCP server loop on stdio.
///
/// Reads JSON-RPC 2.0 requests from stdin, dispatches to the tool registry,
/// and writes responses to stdout.
pub async fn run_stdio_server(tool_registry: &crate::tools::ToolRegistry) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let tools = tool_registry.list_tools();

    eprintln!("🚀 MCP server starting on stdio");
    eprintln!("   Tools: {} available", tools.len());
    eprintln!("   Protocol: 2024-11-05");
    eprintln!("   Capabilities: tools, resources, prompts, roots");
    eprintln!();

    for line in stdin.lock().lines() {
        let line = line?;
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = error_response(None, PARSE_ERROR, format!("Parse error: {e}"));
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let method = request["method"].as_str().unwrap_or("");
        let id = request.get("id").cloned();

        // MCP: notifications (no id) should not receive a response
        if id.is_none() {
            tracing::debug!("MCP notification received: {method}");
            continue;
        }

        let response = match method {
            // ─── Core MCP methods ─────────────────────────────
            "initialize" => Ok(initialize_response(id.clone(), &tools)),
            "tools/list" => Ok(tools_list_response(id.clone(), &tools)),
            "tools/call" => {
                let tool_name = request["params"]["name"].as_str().unwrap_or("");
                let arguments = &request["params"]["arguments"];
                call_tool(id.clone(), tool_registry, tool_name, arguments).await
            }

            // ─── Resources capability ─────────────────────────
            "resources/list" => Ok(resources_list_response(id.clone())),
            "resources/read" => {
                let uri = request["params"]["uri"].as_str().unwrap_or("");
                Ok(resources_read_response(id.clone(), uri))
            }

            // ─── Prompts capability ───────────────────────────
            "prompts/list" => Ok(prompts_list_response(id.clone())),
            "prompts/get" => {
                let name = request["params"]["name"].as_str().unwrap_or("");
                let args = &request["params"]["arguments"];
                Ok(prompts_get_response(id.clone(), name, args))
            }

            // ─── Roots capability ─────────────────────────────
            "roots/list" => Ok(roots_list_response(id.clone())),

            // ─── Unknown method ───────────────────────────────
            _ => Ok(error_response(
                id.clone(),
                METHOD_NOT_FOUND,
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

fn initialize_response(id: Option<Value>, _tools: &[&dyn Tool]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                },
                "prompts": {
                    "listChanged": false
                },
                "logging": {}
            },
            "serverInfo": {
                "name": "cargo-agent",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn tools_list_response(id: Option<Value>, tools: &[&dyn Tool]) -> Value {
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
            return Ok(error_response(
                id,
                TOOL_NOT_FOUND,
                format!("Unknown tool: {name}"),
            ));
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
        Err(e) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": format!("Error: {e}")
                }],
                "isError": true
            }
        })),
    }
}

// ─── Resources ───────────────────────────────────────────────────

fn resources_list_response(id: Option<Value>) -> Value {
    let resources = vec![
        McpResourceDef {
            uri: "config://cargo-agent/config.yaml".to_string(),
            name: "Configuration".to_string(),
            description: Some("cargo-agent configuration file".to_string()),
            mime_type: Some("application/yaml".to_string()),
        },
        McpResourceDef {
            uri: "agent://tools".to_string(),
            name: "Tools Registry".to_string(),
            description: Some("List of all registered tools".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ];

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "resources": resources
        }
    })
}

fn resources_read_response(id: Option<Value>, uri: &str) -> Value {
    match uri {
        "config://cargo-agent/config.yaml" => {
            let config_path = crate::constants::config_path();
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    return json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "contents": [{
                                "uri": uri,
                                "mimeType": "application/yaml",
                                "text": content
                            }]
                        }
                    });
                }
            }
            error_response(id, RESOURCE_NOT_FOUND, "Config file not found".to_string())
        }
        "agent://tools" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "contents": [{
                    "uri": "agent://tools",
                    "mimeType": "application/json",
                    "text": "Tools are available via tools/list and tools/call"
                }]
            }
        }),
        _ => error_response(id, RESOURCE_NOT_FOUND, format!("Resource not found: {uri}")),
    }
}

// ─── Prompts ─────────────────────────────────────────────────────

fn prompts_list_response(id: Option<Value>) -> Value {
    let prompts = vec![
        McpPromptDef {
            name: "code-review".to_string(),
            description: Some("Review code for quality, security, and best practices".to_string()),
            arguments: vec![McpPromptArg {
                name: "file".to_string(),
                description: Some("File path to review".to_string()),
                required: Some(true),
            }],
        },
        McpPromptDef {
            name: "explain-code".to_string(),
            description: Some("Explain how a piece of code works".to_string()),
            arguments: vec![McpPromptArg {
                name: "code".to_string(),
                description: Some("Code to explain".to_string()),
                required: Some(true),
            }],
        },
    ];

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "prompts": prompts
        }
    })
}

fn prompts_get_response(id: Option<Value>, name: &str, _args: &Value) -> Value {
    match name {
        "code-review" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "description": "Review code for quality, security, and best practices",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": "Please review the code in the specified file.\n1. Security vulnerabilities\n2. Code quality and maintainability\n3. Performance issues\n4. Best practices adherence\n5. Error handling completeness"
                    }
                }]
            }
        }),
        "explain-code" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "description": "Explain how a piece of code works",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": "Please explain how the provided code works:\n1. Overall purpose\n2. Key components\n3. Data flow\n4. Important patterns"
                    }
                }]
            }
        }),
        _ => error_response(id, PROMPT_NOT_FOUND, format!("Prompt not found: {name}")),
    }
}

// ─── Roots ───────────────────────────────────────────────────────

fn roots_list_response(id: Option<Value>) -> Value {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let roots = vec![json!({
        "uri": format!("file://{cwd}"),
        "name": "current-directory"
    })];

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "roots": roots
        }
    })
}

// ─── Error helper ────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_response_format() {
        let resp = initialize_response(Some(Value::Number(1.into())), &[]);
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "cargo-agent");
        assert!(resp["result"]["capabilities"].get("tools").is_some());
        assert!(resp["result"]["capabilities"].get("resources").is_some());
        assert!(resp["result"]["capabilities"].get("prompts").is_some());
    }

    #[test]
    fn test_error_response_format() {
        let resp = error_response(
            Some(Value::Number(1.into())),
            METHOD_NOT_FOUND,
            "Method not found".to_string(),
        );
        assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);
        assert!(resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Method"));
    }

    #[test]
    fn test_resources_list_has_entries() {
        let resp = resources_list_response(Some(Value::Number(1.into())));
        let resources = resp["result"]["resources"].as_array().unwrap();
        assert!(!resources.is_empty());
    }

    #[test]
    fn test_prompts_list_has_entries() {
        let resp = prompts_list_response(Some(Value::Number(1.into())));
        let prompts = resp["result"]["prompts"].as_array().unwrap();
        assert!(!prompts.is_empty());
    }

    #[test]
    fn test_roots_list_has_current_dir() {
        let resp = roots_list_response(Some(Value::Number(1.into())));
        let roots = resp["result"]["roots"].as_array().unwrap();
        assert!(!roots.is_empty());
    }

    #[test]
    fn test_unknown_resource_returns_not_found() {
        let resp = resources_read_response(Some(Value::Number(1.into())), "unknown://resource");
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], RESOURCE_NOT_FOUND);
    }

    #[test]
    fn test_unknown_prompt_returns_not_found() {
        let resp = prompts_get_response(Some(Value::Number(1.into())), "nonexistent", &Value::Null);
        assert!(resp.get("error").is_some());
        assert_eq!(resp["error"]["code"], PROMPT_NOT_FOUND);
    }
}
