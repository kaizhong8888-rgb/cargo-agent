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
    fn name(&self) -> &str { "env_file" }

    fn description(&self) -> &str {
        "Parse, validate, generate, and manipulate .env files. \
         Actions: parse (load env vars from file), validate (check syntax), \
         generate (create .env from variables), merge (combine env files), \
         diff (compare two env files)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "action".to_string(), parameter_type: "string".to_string(), description: "Action: parse, validate, generate, merge, diff".to_string(), required: true },
            ToolParameter { name: "path".to_string(), parameter_type: "string".to_string(), description: "Path to .env file (for parse/validate)".to_string(), required: false },
            ToolParameter { name: "paths".to_string(), parameter_type: "array".to_string(), description: "JSON array of .env file paths (for merge/diff)".to_string(), required: false },
            ToolParameter { name: "variables".to_string(), parameter_type: "object".to_string(), description: "JSON object of key-value pairs (for generate)".to_string(), required: false },
            ToolParameter { name: "output".to_string(), parameter_type: "string".to_string(), description: "Output file path (for generate/merge)".to_string(), required: false },
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

fn parse_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let path = params.get("path").and_then(|v| v.as_str()).ok_or("'path' is required for parse action")?;
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    let (vars, errors) = parse_env_content(&content);
    Ok(serde_json::json!({
        "path": path, "variables": vars, "count": vars.len(),
        "errors": if errors.is_empty() { Value::Null } else { Value::Array(errors) },
    }))
}

fn validate_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let path = params.get("path").and_then(|v| v.as_str()).ok_or("'path' is required for validate action")?;
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;
    let (_, errors) = parse_env_content(&content);
    let is_valid = errors.is_empty();
    let error_count = errors.len();
    Ok(serde_json::json!({
        "path": path, "valid": is_valid,
        "errors": if is_valid { Value::Null } else { Value::Array(errors) },
        "error_count": error_count,
    }))
}

fn generate_env_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let variables = params.get("variables").and_then(|v| v.as_object()).ok_or("'variables' (JSON object) is required for generate action")?;
    let output_path = params.get("output").and_then(|v| v.as_str()).unwrap_or(".env");
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
    std::fs::write(output_path, &content).map_err(|e| format!("Failed to write to '{output_path}': {e}"))?;
    Ok(serde_json::json!({ "success": true, "output": output_path, "variables_count": sorted.len() }))
}

fn merge_env_files(params: &HashMap<String, Value>) -> Result<Value, String> {
    let paths: Vec<String> = params.get("paths").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .ok_or("'paths' (JSON array) is required for merge action")?;
    let output_path = params.get("output").and_then(|v| v.as_str()).unwrap_or(".env.merged");
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    for path in &paths {
        if !Path::new(path).exists() {
            errors.push(serde_json::json!({ "path": path, "error": "File not found" }));
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => { errors.push(serde_json::json!({ "path": path, "error": format!("Read error: {e}") })); continue; }
        };
        let (vars, parse_errors) = parse_env_content(&content);
        for err in parse_errors { errors.push(serde_json::json!({ "path": path, "error": err })); }
        for (key, value) in vars {
            if let Some(existing) = merged.get(&key) {
                sources.insert(key.clone(), format!("overridden (was '{existing}') by '{path}'"));
            } else { sources.insert(key.clone(), path.clone()); }
            merged.insert(key, value);
        }
    }
    let mut content = String::with_capacity(merged.len() * 40);
    content.push_str("# Merged .env file\n\n");
    for (key, value) in &merged {
        if value.is_empty() || value.contains(char::is_whitespace) || value.contains('=') {
            content.push_str(&format!("{key}=\"{value}\"\n"));
        } else { content.push_str(&format!("{key}={value}\n")); }
    }
    std::fs::write(output_path, &content).map_err(|e| format!("Failed to write to '{output_path}': {e}"))?;
    Ok(serde_json::json!({ "success": true, "output": output_path, "total_variables": merged.len(), "sources": sources, "errors": if errors.is_empty() { Value::Null } else { Value::Array(errors) } }))
}

fn diff_env_files(params: &HashMap<String, Value>) -> Result<Value, String> {
    let paths = params.get("paths").and_then(|v| v.as_array()).ok_or("'paths' (JSON array) is required for diff action")?;
    if paths.len() != 2 { return Err("Exactly 2 paths are required for diff action".to_string()); }
    let path1 = paths[0].as_str().ok_or("Invalid path in array")?;
    let path2 = paths[1].as_str().ok_or("Invalid path in array")?;
    let content1 = std::fs::read_to_string(path1).map_err(|e| format!("Failed to read '{path1}': {e}"))?;
    let content2 = std::fs::read_to_string(path2).map_err(|e| format!("Failed to read '{path2}': {e}"))?;
    let (vars1, _) = parse_env_content(&content1);
    let (vars2, _) = parse_env_content(&content2);
    let mut only_in_first = BTreeMap::new();
    let mut only_in_second = BTreeMap::new();
    let mut different = BTreeMap::new();
    for (key, value) in &vars1 {
        match vars2.get(key) {
            Some(v2) if v2 != value => { different.insert(key.clone(), (value.clone(), v2.clone())); }
            None => { only_in_first.insert(key.clone(), value.clone()); }
            _ => {}
        }
    }
    for (key, value) in &vars2 {
        if !vars1.contains_key(key) { only_in_second.insert(key.clone(), value.clone()); }
    }
    Ok(serde_json::json!({
        "path1": path1, "path2": path2,
        "only_in_first": if only_in_first.is_empty() { Value::Null } else { serde_json::to_value(&only_in_first).unwrap() },
        "only_in_second": if only_in_second.is_empty() { Value::Null } else { serde_json::to_value(&only_in_second).unwrap() },
        "different_values": if different.is_empty() { Value::Null } else {
            serde_json::to_value(different.iter().map(|(k, (v1, v2))| (k, serde_json::json!({"old": v1, "new": v2}))).collect::<BTreeMap<_, _>>()).unwrap()
        },
        "summary": {
            "only_in_first_count": only_in_first.len(),
            "only_in_second_count": only_in_second.len(),
            "different_count": different.len(),
            "total_changes": only_in_first.len() + only_in_second.len() + different.len(),
        },
    }))
}

fn parse_env_content(content: &str) -> (BTreeMap<String, String>, Vec<Value>) {
    let mut vars = BTreeMap::new();
    let mut errors = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let mut value = trimmed[eq_pos + 1..].trim().to_string();
            if key.is_empty() {
                errors.push(serde_json::json!({ "line": line_num + 1, "error": "Empty key", "content": line }));
                continue;
            }
            if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                errors.push(serde_json::json!({ "line": line_num + 1, "error": format!("Invalid key '{key}'"), "content": line }));
                continue;
            }
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
                value = value.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\");
            } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            if !trimmed[eq_pos + 1..].trim().starts_with('"') && !trimmed[eq_pos + 1..].trim().starts_with('\'') {
                if let Some(hash_pos) = value.find('#') { value = value[..hash_pos].trim().to_string(); }
            }
            vars.insert(key, value);
        } else {
            errors.push(serde_json::json!({ "line": line_num + 1, "error": "Missing '=' separator", "content": line }));
        }
    }
    (vars, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let (vars, errors) = parse_env_content("FOO=bar\nBAZ=123");
        assert!(errors.is_empty());
        assert_eq!(vars["FOO"], "bar");
        assert_eq!(vars["BAZ"], "123");
    }

    #[test]
    fn test_parse_comments() {
        let (vars, errors) = parse_env_content("# comment\n\nFOO=bar");
        assert!(errors.is_empty());
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_parse_quoted() {
        let (vars, _) = parse_env_content("FOO=\"hello world\"\nBAR='single'");
        assert_eq!(vars["FOO"], "hello world");
        assert_eq!(vars["BAR"], "single");
    }

    #[test]
    fn test_parse_inline_comment() {
        let (vars, _) = parse_env_content("FOO=bar # comment");
        assert_eq!(vars["FOO"], "bar");
    }

    #[test]
    fn test_parse_no_comment_in_quotes() {
        let (vars, _) = parse_env_content("FOO=\"bar # not comment\"");
        assert_eq!(vars["FOO"], "bar # not comment");
    }

    #[test]
    fn test_parse_invalid_key() {
        let (_, errors) = parse_env_content("bad-key=value");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_parse_underscore_key() {
        let (vars, errors) = parse_env_content("MY_VAR_123=test");
        assert!(errors.is_empty());
        assert_eq!(vars["MY_VAR_123"], "test");
    }

    #[test]
    fn test_parse_missing_equals() {
        let (_, errors) = parse_env_content("NOEQUALS");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_parse_empty_key() {
        let (_, errors) = parse_env_content("=value");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_parse_file() {
        let tmp = std::env::temp_dir().join("env_test.env");
        std::fs::write(&tmp, "DB_HOST=localhost\nDB_PORT=5432\n").unwrap();
        let mut p = HashMap::new();
        p.insert("path".to_string(), Value::String(tmp.to_str().unwrap().to_string()));
        let r = parse_env_file(&p).unwrap();
        assert_eq!(r["count"].as_u64().unwrap(), 2);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_parse_file_not_found() {
        let mut p = HashMap::new();
        p.insert("path".to_string(), Value::String("/nonexistent/.env".to_string()));
        assert!(parse_env_file(&p).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let tmp = std::env::temp_dir().join("env_valid.env");
        std::fs::write(&tmp, "FOO=bar\n").unwrap();
        let mut p = HashMap::new();
        p.insert("path".to_string(), Value::String(tmp.to_str().unwrap().to_string()));
        let r = validate_env_file(&p).unwrap();
        assert_eq!(r["valid"], true);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_validate_invalid() {
        let tmp = std::env::temp_dir().join("env_invalid.env");
        std::fs::write(&tmp, "bad-key=val\n").unwrap();
        let mut p = HashMap::new();
        p.insert("path".to_string(), Value::String(tmp.to_str().unwrap().to_string()));
        let r = validate_env_file(&p).unwrap();
        assert_eq!(r["valid"], false);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_generate() {
        let tmp = std::env::temp_dir().join("env_gen.env");
        let mut p = HashMap::new();
        p.insert("output".to_string(), Value::String(tmp.to_str().unwrap().to_string()));
        p.insert("variables".to_string(), serde_json::json!({"DB_HOST": "localhost", "DB_PORT": "5432"}));
        let r = generate_env_file(&p).unwrap();
        assert_eq!(r["variables_count"], 2);
        let c = std::fs::read_to_string(&tmp).unwrap();
        assert!(c.contains("DB_HOST"));
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_generate_with_spaces() {
        let tmp = std::env::temp_dir().join("env_gen2.env");
        let mut p = HashMap::new();
        p.insert("output".to_string(), Value::String(tmp.to_str().unwrap().to_string()));
        p.insert("variables".to_string(), serde_json::json!({"MSG": "hello world"}));
        generate_env_file(&p).unwrap();
        let c = std::fs::read_to_string(&tmp).unwrap();
        assert!(c.contains("\"hello world\""));
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_generate_missing_vars() {
        assert!(generate_env_file(&HashMap::new()).is_err());
    }

    #[test]
    fn test_merge() {
        let t1 = std::env::temp_dir().join("merge1.env");
        let t2 = std::env::temp_dir().join("merge2.env");
        let out = std::env::temp_dir().join("merged.env");
        std::fs::write(&t1, "A=1\nB=2\n").unwrap();
        std::fs::write(&t2, "B=3\nC=4\n").unwrap();
        let mut p = HashMap::new();
        p.insert("paths".to_string(), serde_json::json!([t1.to_str().unwrap(), t2.to_str().unwrap()]));
        p.insert("output".to_string(), Value::String(out.to_str().unwrap().to_string()));
        let r = merge_env_files(&p).unwrap();
        assert_eq!(r["total_variables"], 3);
        let c = std::fs::read_to_string(&out).unwrap();
        assert!(c.contains("B=3"));
        std::fs::remove_file(&t1).ok(); std::fs::remove_file(&t2).ok(); std::fs::remove_file(&out).ok();
    }

    #[test]
    fn test_diff() {
        let t1 = std::env::temp_dir().join("diff1.env");
        let t2 = std::env::temp_dir().join("diff2.env");
        std::fs::write(&t1, "A=1\nB=2\n").unwrap();
        std::fs::write(&t2, "A=1\nB=3\nC=4\n").unwrap();
        let mut p = HashMap::new();
        p.insert("paths".to_string(), serde_json::json!([t1.to_str().unwrap(), t2.to_str().unwrap()]));
        let r = diff_env_files(&p).unwrap();
        assert_eq!(r["summary"]["different_count"], 1);
        assert_eq!(r["summary"]["only_in_second_count"], 1);
        std::fs::remove_file(&t1).ok(); std::fs::remove_file(&t2).ok();
    }

    #[test]
    fn test_diff_wrong_count() {
        let mut p = HashMap::new();
        p.insert("paths".to_string(), serde_json::json!(["a", "b", "c"]));
        assert!(diff_env_files(&p).is_err());
    }

    #[test]
    fn test_tool_metadata() {
        let t = EnvFileTool;
        assert_eq!(t.name(), "env_file");
        assert!(t.description().contains(".env"));
        assert_eq!(t.parameters().len(), 5);
    }
}
