//! Dependency manager: cargo add, remove, update, tree, audit.
//!
//! Wraps cargo CLI for dependency management operations.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

// ============================================================================
// DepManagerTool
// ============================================================================

pub struct DepManagerTool;

#[async_trait::async_trait]
impl Tool for DepManagerTool {
    fn name(&self) -> &str {
        "dep_manager"
    }

    fn description(&self) -> &str {
        "Manage Rust project dependencies. Actions: add (cargo add), remove (cargo rm), update (cargo update), tree (show dependency graph), audit (security audit), outdated (check for outdated crates)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action to perform: add, remove, update, tree, audit, outdated"
                    .to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "package".to_string(),
                description: "Package name to add/remove/update (e.g. 'serde', 'tokio')"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "features".to_string(),
                description:
                    "Comma-separated features to enable (used with add action, e.g. 'derive,serde')"
                        .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "version".to_string(),
                description:
                    "Specific version constraint (used with add action, e.g. '1.0', '^1.4', '~0.3')"
                        .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "dev_dependency".to_string(),
                description: "Add as dev-dependency (used with add action, default: false)"
                    .to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "project_path".to_string(),
                description: "Path to the project (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let project_path = params
            .get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match action {
            "add" => cmd_add(params, project_path),
            "remove" => cmd_remove(params, project_path),
            "update" => cmd_update(project_path),
            "tree" => cmd_tree(project_path),
            "audit" => cmd_audit(project_path),
            "outdated" => cmd_outdated(project_path),
            other => Err(format!(
                "Unknown action: {other}. Supported: add, remove, update, tree, audit, outdated"
            )),
        }
    }
}

fn cmd_add(params: &HashMap<String, Value>, project_path: &str) -> Result<Value, String> {
    let package = params
        .get("package")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: package (for add action)")?;

    let mut args = vec!["add".to_string(), package.to_string()];

    if let Some(version) = params.get("version").and_then(|v| v.as_str()) {
        args.push("--version".to_string());
        args.push(version.to_string());
    }

    if let Some(features) = params.get("features").and_then(|v| v.as_str()) {
        args.push("--features".to_string());
        args.push(features.to_string());
    }

    if params
        .get("dev_dependency")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        args.push("--dev".to_string());
    }

    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_cargo_with_output(project_path, &str_args).map(|output| {
        serde_json::json!({
            "status": "ok",
            "action": "add",
            "package": package,
            "output": output,
        })
    })
}

fn cmd_remove(params: &HashMap<String, Value>, project_path: &str) -> Result<Value, String> {
    let package = params
        .get("package")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: package (for remove action)")?;

    run_cargo_with_output(project_path, &["rm", package]).map(|output| {
        serde_json::json!({
            "status": "ok",
            "action": "remove",
            "package": package,
            "output": output,
        })
    })
}

fn cmd_update(project_path: &str) -> Result<Value, String> {
    run_cargo_with_output(project_path, &["update"]).map(|output| {
        serde_json::json!({
            "status": "ok",
            "action": "update",
            "output": output,
        })
    })
}

fn cmd_tree(project_path: &str) -> Result<Value, String> {
    let output = run_cargo_with_output(project_path, &["tree"])?;
    let dep_count = output.lines().count();
    Ok(serde_json::json!({
        "status": "ok",
        "action": "tree",
        "dependency_count": dep_count,
        "tree": output,
    }))
}

fn cmd_audit(project_path: &str) -> Result<Value, String> {
    // cargo-audit is a separate crate; try it, fall back to cargo tree for manual inspection
    let output = Command::new("cargo")
        .args(["audit"])
        .current_dir(project_path)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let success = out.status.success();

            if success {
                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "audit",
                    "no_advisories": true,
                    "output": format!("{stdout}{stderr}"),
                }))
            } else {
                Ok(serde_json::json!({
                    "status": "warning",
                    "action": "audit",
                    "advisories_found": true,
                    "output": format!("{stdout}{stderr}"),
                }))
            }
        }
        Err(_) => Ok(serde_json::json!({
            "status": "info",
            "action": "audit",
            "message": "cargo-audit is not installed. Install with: cargo install cargo-audit",
            "hint": "Use 'tree' action to inspect dependency tree manually.",
        })),
    }
}

fn cmd_outdated(project_path: &str) -> Result<Value, String> {
    let output = Command::new("cargo")
        .args(["outdated", "--root-deps-only"])
        .current_dir(project_path)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(serde_json::json!({
                "status": "ok",
                "action": "outdated",
                "output": stdout.to_string(),
            }))
        }
        Err(_) => Ok(serde_json::json!({
            "status": "info",
            "action": "outdated",
            "message": "cargo-outdated is not installed. Install with: cargo install cargo-outdated",
        })),
    }
}

fn run_cargo_with_output(work_dir: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(work_dir)
        .output()
        .map_err(|e| format!("Failed to execute cargo: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(format!("{stdout}{stderr}").trim().to_string())
    } else {
        Err(format!("cargo error: {}", stderr.trim()))
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DepManagerTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_manager_tool_metadata() {
        let tool = DepManagerTool;
        assert_eq!(tool.name(), "dep_manager");
        assert!(tool.description().contains("dependencies"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
        assert!(params.iter().any(|p| p.name == "package"));
    }
}
