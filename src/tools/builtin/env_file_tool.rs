//! EnvFile Tool: parse, validate, generate, and manipulate .env files.
//!
//! Actions: parse (load and display env vars from file), validate (check syntax),
//! generate (create .env from template), merge (combine multiple env files),
//! diff (compare two env files).

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(EnvFileTool));
}

struct EnvFileTool;

#[async_trait::async_trait]
impl Tool for EnvFileTool {
    fn name(&self) -> &str {
        "env_file"
    }

    fn description(&self) -> &str {
        "Parse, validate, generate, and manipulate .env files. \
         Actions: parse (load env vars from file), validate (check syntax), \
         generate (create .env from variables), merge (combine env files), \
         diff (compare two env files)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: parse, validate, generate, merge, diff".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to .env file (for parse/validate)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "paths".to_string(),
                parameter_type: "array".to_string(),
                description: "JSON array of .env file paths (for merge/diff)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "variables".to_string(),
                parameter_type: "object".to_string(),
                description: "JSON object of key-value pairs (for generate)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path (for generate/merge)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "parse" => parse_env_file(params),
            "validate" => validate_env_file(params),
            "generate" => generate_env_file(params),
            "merge" => merge_env_files(params),
            "diff" => diff_env_files(params),
            _ => Err(format!("Unknown action: {action}. Valid: parse, validate, generate, merge, diff")),
        }
    }
}

/// Parse a .env file and return key-value pairs.
fn parse_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("'path' is required for parse action")?;

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file '{path}': {e}"))?;

    let (vars, errors) = parse_env_content(&content);

    Ok(serde_json::json!({
        "path": path,
        "variables": vars,
        "count": vars.len(),
        "errors": if errors.is_empty() { Value::Null } else { Value::Array(errors) },
    }))
}

/// Validate a .env file's syntax.
fn validate_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("'path' is required for validate action")?;

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file '{path}': {e}"))?;

    let (_, errors) = parse_env_content(&content);
    let is_valid = errors.is_empty();
    let error_count = errors.len();

    Ok(serde_json::json!({
        "path": path,
        "valid": is_valid,
        "errors": if is_valid { Value::Null } else { Value::Array(errors) },
        "error_count": error_count,
    }))
}

/// Generate a .env file from key-value pairs.
fn generate_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let variables = params
        .get("variables")
        .and_then(|v| v.as_object())
        .ok_or("'variables' (JSON object) is required for generate action")?;

    let output_path = params
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or(".env");

    // Sort keys for deterministic output
    let mut sorted: Vec<(&String, &Value)> = variables.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut content = String::with_capacity(sorted.len() * 40);
    content.push_str("# Auto-generated .env file\n\n");

    for (key, value) in &sorted {
        let val_str = value.as_str().unwrap_or("");
        if val_str.is_empty() || val_str.contains(char::is_whitespace) || val_str.contains('=') || val_str.contains('#') {
            content.push_str(&format!("{key}=\"{val_str}\"\n"));
        } else {
            content.push_str(&format!("{key}={val_str}\n"));
        }
    }

    std::fs::write(output_path, &content)
        .map_err(|e| format!("Failed to write to '{output_path}': {e}"))?;

    Ok(serde_json::json!({
        "success": true,
        "output": output_path,
        "variables_count": sorted.len(),
    }))
}

/// Merge multiple .env files (later files override earlier).
fn merge_env_files(params: &HashMap<String, Value>) -> Result<Value, String> {
    let paths: Vec<String> = params
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .ok_or("'paths' (JSON array) is required for merge action")?;

    let output_path = params
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or(".env.merged");

    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();

    for path in &paths {
        if !Path::new(path).exists() {
            errors.push(serde_json::json!({
                "path": path,
                "error": "File not found",
            }));
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("Read error: {e}"),
                }));
                continue;
            }
        };

        let (vars, parse_errors) = parse_env_content(&content);
        for err in parse_errors {
            errors.push(serde_json::json!({
                "path": path,
                "error": err,
            }));
        }

        for (key, value) in vars {
            if let Some(existing) = merged.get(&key) {
                sources.insert(key.clone(), format!("overridden (was '{existing}') by '{path}'"));
            } else {
                sources.insert(key.clone(), path.clone());
            }
            merged.insert(key, value);
        }
    }

    // Write merged file
    let mut content = String::with_capacity(merged.len() * 40);
    content.push_str("# Merged .env file\n\n");

    for (key, value) in &merged {
        if value.is_empty() || value.contains(char::is_whitespace) || value.contains('=') {
            content.push_str(&format!("{key}=\"{value}\"\n"));
        } else {
            content.push_str(&format!("{key}={value}\n"));
        }
    }

    std::fs::write(output_path, &content)
        .map_err(|e| format!("Failed to write to '{output_path}': {e}"))?;

    Ok(serde_json::json!({
        "success": true,
        "output": output_path,
        "total_variables": merged.len(),
        "sources": sources,
        "errors": if errors.is_empty() { Value::Null } else { Value::Array(errors) },
    }))
}

/// Compare two .env files and show differences.
fn diff_env_files(params: &HashMap<String, Value>) -> Result<Value, String> {
    let paths = params
        .get("paths")
        .and_then(|v| v.as_array())
        .ok_or("'paths' (JSON array) is required for diff action")?;

    if paths.len() != 2 {
        return Err("Exactly 2 paths are required for diff action".to_string());
    }

    let path1 = paths[0].as_str().ok_or("Invalid path in array")?;
    let path2 = paths[1].as_str().ok_or("Invalid path in array")?;

    let content1 = std::fs::read_to_string(path1)
        .map_err(|e| format!("Failed to read '{path1}': {e}"))?;
    let content2 = std::fs::read_to_string(path2)
        .map_err(|e| format!("Failed to read '{path2}': {e}"))?;

    let (vars1, _) = parse_env_content(&content1);
    let (vars2, _) = parse_env_content(&content2);

    let mut only_in_first = BTreeMap::new();
    let mut only_in_second = BTreeMap::new();
    let mut different = BTreeMap::new();

    for (key, value) in &vars1 {
        match vars2.get(key) {
            Some(v2) if v2 != value => {
                different.insert(key.clone(), (value.clone(), v2.clone()));
            }
            None => {
                only_in_first.insert(key.clone(), value.clone());
            }
            _ => {} // Same value, no diff
        }
    }

    for (key, value) in &vars2 {
        if !vars1.contains_key(key) {
            only_in_second.insert(key.clone(), value.clone());
        }
    }

    Ok(serde_json::json!({
        "path1": path1,
        "path2": path2,
        "only_in_first": if only_in_first.is_empty() { Value::Null } else { serde_json::to_value(&only_in_first).unwrap() },
        "only_in_second": if only_in_second.is_empty() { Value::Null } else { serde_json::to_value(&only_in_second).unwrap() },
        "different_values": if different.is_empty() { Value::Null } else {
            serde_json::to_value(&different.iter().map(|(k, (v1, v2))| (k, serde_json::json!({"old": v1, "new": v2}))).collect::<BTreeMap<_, _>>()).unwrap()
        },
        "summary": {
            "only_in_first_count": only_in_first.len(),
            "only_in_second_count": only_in_second.len(),
            "different_count": different.len(),
            "total_changes": only_in_first.len() + only_in_second.len() + different.len(),
        },
    }))
}

/// Parse .env file content, returning key-value pairs and errors.
fn parse_env_content(content: &str) -> (BTreeMap<String, String>, Vec<Value>) {
    let mut vars = BTreeMap::new();
    let mut errors = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Find the first '=' that separates key from value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let mut value = trimmed[eq_pos + 1..].trim().to_string();

            // Validate key
            if key.is_empty() {
                errors.push(serde_json::json!({
                    "line": line_num + 1,
                    "error": "Empty key",
                    "content": line,
                }));
                continue;
            }

            if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                errors.push(serde_json::json!({
                    "line": line_num + 1,
                    "error": format!("Invalid key '{key}' (only alphanumeric and underscore allowed)"),
                    "content": line,
                }));
                continue;
            }

            // Remove surrounding quotes
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
                // Process escape sequences in double-quoted values
                value = value.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\");
            } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
                // Single quotes: no escape processing
            }

            // Handle inline comments (only for unquoted values)
            if !trimmed[eq_pos + 1..].trim().starts_with('"')
                && !trimmed[eq_pos + 1..].trim().starts_with('\'')
            {
                if let Some(hash_pos) = value.find('#') {
                    value = value[..hash_pos].trim().to_string();
                }
            }

            vars.insert(key, value);
        } else {
            errors.push(serde_json::json!({
                "line": line_num + 1,
                "error": "Missing '=' separator",
                "content": line,
            }));
        }
    }

    (vars, errors)
}
