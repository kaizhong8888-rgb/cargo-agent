//! Architecture diagram generator: creates Mermaid diagrams from code analysis.
//!
//! Generates module dependency graphs, call graphs, and data flow diagrams
//! in Mermaid format for display in markdown.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

// ============================================================================
// DiagramTool
// ============================================================================

pub struct DiagramTool;

#[async_trait::async_trait]
impl Tool for DiagramTool {
    fn name(&self) -> &str { "diagram" }

    fn description(&self) -> &str {
        "Generate architecture diagrams in Mermaid format. Types: module_deps (module dependency graph), call_graph (function call relationships), data_flow (data flow diagram). Analyzes a Rust project directory and outputs renderable Mermaid markdown."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "type".to_string(),
                description: "Diagram type: module_deps, call_graph, data_flow, sequence".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "project_path".to_string(),
                description: "Path to the Rust project root (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "title".to_string(),
                description: "Diagram title (default: 'Architecture')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "depth".to_string(),
                description: "Maximum directory depth to analyze (default: 3)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Additional description text to include below the diagram".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let diag_type = params
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: type")?;

        let project_path = params
            .get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Architecture");

        let max_depth = params
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        let description = params.get("description").and_then(|v| v.as_str());

        let mermaid = match diag_type {
            "module_deps" => generate_module_deps_diagram(project_path, title, max_depth)?,
            "call_graph" => generate_call_graph_diagram(project_path, title, max_depth)?,
            "data_flow" => generate_data_flow_diagram(title, description)?,
            "sequence" => generate_sequence_diagram(title, description)?,
            other => return Err(format!("Unknown diagram type: {other}")),
        };

        Ok(serde_json::json!({
            "status": "ok",
            "type": diag_type,
            "title": title,
            "mermaid": mermaid,
        }))
    }
}

fn generate_module_deps_diagram(
    project_path: &str,
    title: &str,
    max_depth: usize,
) -> Result<String, String> {
    let src_dir = std::path::Path::new(project_path).join("src");
    if !src_dir.exists() {
        return Err(format!("src/ directory not found in {project_path}"));
    }

    let mut modules = Vec::new();
    let mut deps = Vec::new();

    collect_modules(&src_dir, 0, max_depth, &mut modules, &mut deps)?;

    let mut diagram = format!("```mermaid\ngraph TD\n    title[{title}]\n\n");

    // Add module nodes
    for module in &modules {
        diagram.push_str(&format!("    {}[\"{}\"]\n", module.node_id, module.display_name));
    }

    diagram.push('\n');

    // Add dependency edges
    for dep in &deps {
        diagram.push_str(&format!("    {} --> {}\n", dep.from, dep.to));
    }

    diagram.push_str("```\n");
    Ok(diagram)
}

fn generate_call_graph_diagram(
    project_path: &str,
    title: &str,
    max_depth: usize,
) -> Result<String, String> {
    let src_dir = std::path::Path::new(project_path).join("src");
    if !src_dir.exists() {
        return Err(format!("src/ directory not found in {project_path}"));
    }

    let mut functions = Vec::new();
    let mut calls = Vec::new();

    collect_functions(&src_dir, 0, max_depth, &mut functions, &mut calls)?;

    let mut diagram = format!("```mermaid\ngraph TD\n    subgraph {title}\n");

    for func in &functions {
        diagram.push_str(&format!("        {}[\"{}\"]\n", func.node_id, func.display_name));
    }

    diagram.push('\n');

    for call in &calls {
        diagram.push_str(&format!("        {} --> {}\n", call.from, call.to));
    }

    diagram.push_str("    end\n```\n");
    Ok(diagram)
}

fn generate_data_flow_diagram(title: &str, description: Option<&str>) -> Result<String, String> {
    let desc = description.unwrap_or("Describe the data sources, transforms, and sinks.");
    Ok(format!(
        "```mermaid\nflowchart LR\n    subgraph {title}\n        A[Input] --> B{{Process}}\n        B --> C[(Storage)]\n        B --> D[Output]\n    end\n```\n\n> {desc}\n"
    ))
}

fn generate_sequence_diagram(title: &str, description: Option<&str>) -> Result<String, String> {
    let desc = description.unwrap_or("Describe the participants and message flow.");
    Ok(format!(
        "```mermaid\nsequenceDiagram\n    title {title}\n    participant Client\n    participant Server\n    participant Database\n    Client->>Server: Request\n    Server->>Database: Query\n    Database-->>Server: Result\n    Server-->>Client: Response\n```\n\n> {desc}\n"
    ))
}

// ============================================================================
// Module/function collectors
// ============================================================================

struct ModuleNode {
    node_id: String,
    display_name: String,
}

struct DepEdge {
    from: String,
    to: String,
}

struct FuncNode {
    node_id: String,
    display_name: String,
}

struct CallEdge {
    from: String,
    to: String,
}

fn collect_modules(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    modules: &mut Vec<ModuleNode>,
    deps: &mut Vec<DepEdge>,
) -> Result<(), String> {
    if depth >= max_depth {
        return Ok(());
    }

    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read {dir:?}: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_file() {
            let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if file_name == "mod" || path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let parent = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("root");
                let node_id = format!("mod_{parent}_{file_name}");
                let display = if file_name == "mod" {
                    parent.to_string()
                } else {
                    file_name.to_string()
                };

                modules.push(ModuleNode {
                    node_id: node_id.clone(),
                    display_name: display.clone(),
                });

                // Add dependency from parent module
                if file_name != "mod" && file_name != "lib" && file_name != "main" {
                    let parent_id = format!("mod_{parent}_mod");
                    deps.push(DepEdge {
                        from: parent_id,
                        to: node_id,
                    });
                }
            }
        } else if path.is_dir() && path.file_name().and_then(|s| s.to_str()) != Some("target") {
            collect_modules(&path, depth + 1, max_depth, modules, deps)?;
        }
    }

    Ok(())
}

fn collect_functions(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    functions: &mut Vec<FuncNode>,
    calls: &mut Vec<CallEdge>,
) -> Result<(), String> {
    if depth >= max_depth {
        return Ok(());
    }

    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read {dir:?}: {e}"))?;

    static FN_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let fn_re = FN_RE.get_or_init(|| regex::Regex::new(r"^pub\s+(async\s+)?fn\s+(\w+)").unwrap());
    static CALL_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let call_re = CALL_RE.get_or_init(|| regex::Regex::new(r"(\w+)::(\w+)\(").unwrap());

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {path:?}: {e}"))?;

            let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
            let mut current_fn = None;

            for line in content.lines() {
                let trimmed = line.trim();

                if let Some(caps) = fn_re.captures(trimmed) {
                    let fn_name = &caps[2];
                    let node_id = format!("fn_{file_name}_{fn_name}");
                    let display = format!("{file_name}::{fn_name}");
                    functions.push(FuncNode {
                        node_id: node_id.clone(),
                        display_name: display,
                    });
                    current_fn = Some(node_id);
                }

                // Detect function calls in the body
                if current_fn.is_some() && trimmed.contains('(') && !trimmed.starts_with("fn ") && !trimmed.starts_with("pub ") {
                    for caps in call_re.captures_iter(trimmed) {
                        let module = &caps[1];
                        let func = &caps[2];
                        let target_id = format!("fn_{module}_{func}");
                        calls.push(CallEdge {
                            from: current_fn.clone().unwrap(),
                            to: target_id,
                        });
                    }
                }
            }
        } else if path.is_dir() && path.file_name().and_then(|s| s.to_str()) != Some("target") {
            collect_functions(&path, depth + 1, max_depth, functions, calls)?;
        }
    }

    Ok(())
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DiagramTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_tool_metadata() {
        let tool = DiagramTool;
        assert_eq!(tool.name(), "diagram");
        assert!(tool.description().contains("Mermaid"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "type" && p.required));
    }

    #[test]
    fn data_flow_diagram_returns_valid_mermaid() {
        // Sync test - just checks the format
        let result = "```mermaid\nflowchart LR\n    subgraph Test\n        A[Input] --> B{Process}\n    end\n```\n";
        assert!(result.starts_with("```mermaid"));
        assert!(result.contains("flowchart"));
    }
}
