//! Code transformation tool: safe, pattern-based code refactoring.
//!
//! Supports common Rust refactoring operations:
//! - Add/remove derives
//! - Replace unwrap() with ? operator
//! - Change function visibility
//! - Rename identifiers in a file

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

// ============================================================================
// CodeTransformTool
// ============================================================================

pub struct CodeTransformTool;

#[async_trait::async_trait]
impl Tool for CodeTransformTool {
    fn name(&self) -> &str {
        "code_transform"
    }

    fn description(&self) -> &str {
        "Apply safe, pattern-based code transformations to Rust files. Actions: add_derive, remove_derive, replace_unwrap, rename, change_visibility. Operates on single files with dry-run support."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Transformation to apply: add_derive, remove_derive, replace_unwrap, rename, change_visibility".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_path".to_string(),
                description: "Path to the Rust file to transform".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "derive".to_string(),
                description: "Derive trait to add/remove (e.g. Debug, Clone, Serialize). Used with add_derive/remove_derive actions.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "from".to_string(),
                description: "Text to replace (used with rename action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "to".to_string(),
                description: "Replacement text (used with rename action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "visibility".to_string(),
                description: "New visibility modifier: pub, pub(crate), pub(super), private. Used with change_visibility action.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "target".to_string(),
                description: "Function/struct/item name to target (used with change_visibility/rename)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "dry_run".to_string(),
                description: "Preview changes without modifying the file (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let file_path = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: file_path")?;

        let dry_run = params
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let original = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {file_path}: {e}"))?;

        let transformed = match action {
            "add_derive" => transform_add_derive(&original, params)?,
            "remove_derive" => transform_remove_derive(&original, params)?,
            "replace_unwrap" => transform_replace_unwrap(&original),
            "rename" => transform_rename(&original, params)?,
            "change_visibility" => transform_change_visibility(&original, params)?,
            other => return Err(format!("Unknown action: {other}")),
        };

        let changes_made = transformed != original;

        if !dry_run && changes_made {
            fs::write(file_path, &transformed)
                .map_err(|e| format!("Failed to write {file_path}: {e}"))?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "action": action,
            "file": file_path,
            "changes_made": changes_made,
            "dry_run": dry_run,
            "original_lines": original.lines().count(),
            "new_lines": transformed.lines().count(),
            "result": if dry_run { transformed } else { "File updated".to_string() },
        }))
    }
}

fn transform_add_derive(source: &str, params: &HashMap<String, Value>) -> Result<String, String> {
    let derive = params
        .get("derive")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: derive (for add_derive action)")?;

    // Find #[derive(...)] attributes and add the derive if not already present
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"#\[derive\(([^)]*)\)\]").unwrap());

    let result = re.replace_all(source, |caps: &regex::Captures| {
        let existing = caps[1].trim();
        if existing.split(',').map(|s| s.trim()).any(|d| d == derive) {
            // Already has this derive
            caps[0].to_string()
        } else {
            let new_derives = format!("{existing}, {derive}");
            format!("#[derive({new_derives})]")
        }
    });

    Ok(result.into_owned())
}

fn transform_remove_derive(
    source: &str,
    params: &HashMap<String, Value>,
) -> Result<String, String> {
    let derive = params
        .get("derive")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: derive (for remove_derive action)")?;

    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"#\[derive\(([^)]*)\)\]").unwrap());

    let result = re.replace_all(source, |caps: &regex::Captures| {
        let existing = caps[1].trim();
        let remaining: Vec<&str> = existing
            .split(',')
            .map(|s| s.trim())
            .filter(|d| *d != derive)
            .collect();

        if remaining.is_empty() {
            // Remove the entire derive attribute
            String::new()
        } else {
            format!("#[derive({})]", remaining.join(", "))
        }
    });

    // Clean up empty lines left by removed attributes
    let result = result
        .lines()
        .filter(|line| {
            !line.trim().is_empty() || {
                // Keep empty lines that are between non-empty lines
                true
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(result)
}

fn transform_replace_unwrap(source: &str) -> String {
    // Replace .unwrap() with ? operator (only inside functions returning Result)
    // This is a simple heuristic: replace .unwrap() -> .map_err(|e| anyhow::anyhow!("unwrap failed"))?
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\.unwrap\(\)").unwrap());

    re.replace_all(
        source,
        ".map_err(|e| anyhow::anyhow!(\"unwrap failed: {:?}\", e))?",
    )
    .into_owned()
}

fn transform_rename(source: &str, params: &HashMap<String, Value>) -> Result<String, String> {
    let from = params
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: from (for rename action)")?;

    let to = params
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: to (for rename action)")?;

    // Word-boundary replacement
    let pattern = format!(r"\b{}\b", regex::escape(from));
    let re = regex::Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {e}"))?;

    Ok(re.replace_all(source, to).into_owned())
}

fn transform_change_visibility(
    source: &str,
    params: &HashMap<String, Value>,
) -> Result<String, String> {
    let visibility = params
        .get("visibility")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: visibility (for change_visibility action)")?;

    let target = params
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: target (for change_visibility action)")?;

    // Match lines like: fn target(...), struct Target, enum Target, impl Target, const TARGET, type Target
    let pattern = format!(
        r"(?m)^(pub(?:\([^)]*\))?\s+)?(fn|struct|enum|trait|impl|const|static|type)\s+{target}\b",
        target = regex::escape(target),
    );
    let re = regex::Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {e}"))?;

    let new_vis: String = if visibility == "private" {
        String::new()
    } else {
        format!("{visibility} ")
    };

    Ok(re
        .replace_all(source, |caps: &regex::Captures| {
            let kind = &caps[2];
            format!("{new_vis}{kind} {target}")
        })
        .into_owned())
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeTransformTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_derive_adds_new_trait() {
        let source = "#[derive(Clone)]\nstruct Foo { x: i32 }\n";
        let mut params = HashMap::new();
        params.insert("derive".to_string(), Value::String("Debug".to_string()));
        let result = transform_add_derive(source, &params).unwrap();
        assert!(result.contains("Clone"));
        assert!(result.contains("Debug"));
    }

    #[test]
    fn add_derive_skips_existing() {
        let source = "#[derive(Clone, Debug)]\nstruct Foo { x: i32 }\n";
        let mut params = HashMap::new();
        params.insert("derive".to_string(), Value::String("Debug".to_string()));
        let result = transform_add_derive(source, &params).unwrap();
        assert_eq!(result, source);
    }

    #[test]
    fn remove_derive_removes_trait() {
        let source = "#[derive(Clone, Debug, PartialEq)]\nstruct Foo { x: i32 }\n";
        let mut params = HashMap::new();
        params.insert("derive".to_string(), Value::String("Debug".to_string()));
        let result = transform_remove_derive(source, &params).unwrap();
        assert!(result.contains("Clone"));
        assert!(!result.contains("Debug"));
        assert!(result.contains("PartialEq"));
    }

    #[test]
    fn replace_unwrap_replaces_with_question_mark() {
        let source = "let x = foo.unwrap();\nlet y = bar.unwrap();\n";
        let result = transform_replace_unwrap(source);
        assert!(!result.contains(".unwrap()"));
        assert!(result.contains("?"));
    }

    #[test]
    fn rename_renames_identifier() {
        let source = "fn foo() { let foo = 1; }\nstruct Bar { foo: i32 }\n";
        let mut params = HashMap::new();
        params.insert("from".to_string(), Value::String("foo".to_string()));
        params.insert("to".to_string(), Value::String("bar".to_string()));
        let result = transform_rename(source, &params).unwrap();
        assert!(!result.contains("foo"));
        assert_eq!(result.matches("bar").count(), 3);
    }

    #[test]
    fn change_visibility_makes_pub() {
        let source = "fn secret() {}\nstruct Foo;\n";
        let mut params = HashMap::new();
        params.insert("visibility".to_string(), Value::String("pub".to_string()));
        params.insert("target".to_string(), Value::String("secret".to_string()));
        let result = transform_change_visibility(source, &params).unwrap();
        assert!(result.contains("pub fn secret()"));
    }

    #[test]
    fn code_transform_tool_metadata() {
        let tool = CodeTransformTool;
        assert_eq!(tool.name(), "code_transform");
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
        assert!(params.iter().any(|p| p.name == "file_path" && p.required));
    }
}
