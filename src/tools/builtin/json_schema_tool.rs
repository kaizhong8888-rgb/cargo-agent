//! JSON Schema Tool: generate and validate JSON Schema from JSON data.
//!
//! Actions: generate (infer schema from JSON), validate (check JSON against schema),
//! info (display schema info), merge (combine two schemas).

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(JsonSchemaTool));
}

struct JsonSchemaTool;

#[async_trait::async_trait]
impl Tool for JsonSchemaTool {
    fn name(&self) -> &str { "json_schema" }

    fn description(&self) -> &str {
        "Generate and validate JSON Schema from JSON data. \
         Actions: generate (infer schema from JSON), validate (check JSON against schema), \
         info (display schema info), merge (combine two schemas)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "action".to_string(), parameter_type: "string".to_string(), description: "Action: generate, validate, info, merge".to_string(), required: true },
            ToolParameter { name: "data".to_string(), parameter_type: "string".to_string(), description: "JSON data (for generate)".to_string(), required: false },
            ToolParameter { name: "schema".to_string(), parameter_type: "string".to_string(), description: "JSON Schema (for validate/info)".to_string(), required: false },
            ToolParameter { name: "schema2".to_string(), parameter_type: "string".to_string(), description: "Second JSON Schema (for merge)".to_string(), required: false },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        match action {
            "generate" => generate_schema(params),
            "validate" => validate_json(params),
            "info" => schema_info(params),
            "merge" => merge_schemas(params),
            _ => Err(format!("Unknown action: {action}. Valid: generate, validate, info, merge")),
        }
    }
}

fn generate_schema(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data_str = params.get("data").and_then(|v| v.as_str()).ok_or("'data' (JSON string) is required for generate action")?;
    let data: Value = serde_json::from_str(data_str).map_err(|e| format!("Invalid JSON: {e}"))?;
    let schema = infer_schema(&data);
    Ok(serde_json::json!({ "schema": schema, "input_type": get_json_type(&data) }))
}

fn validate_json(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data_str = params.get("data").and_then(|v| v.as_str()).ok_or("'data' (JSON string) is required for validate action")?;
    let schema_str = params.get("schema").and_then(|v| v.as_str()).ok_or("'schema' (JSON Schema string) is required for validate action")?;
    let data: Value = serde_json::from_str(data_str).map_err(|e| format!("Invalid JSON data: {e}"))?;
    let schema: Value = serde_json::from_str(schema_str).map_err(|e| format!("Invalid JSON Schema: {e}"))?;
    let (valid, errors) = simple_validate(&data, &schema, "$");
    let error_count = errors.len();
    Ok(serde_json::json!({ "valid": valid, "errors": if valid { Value::Null } else { Value::Array(errors) }, "error_count": error_count }))
}

fn schema_info(params: &HashMap<String, Value>) -> Result<Value, String> {
    let schema_str = params.get("schema").and_then(|v| v.as_str()).ok_or("'schema' (JSON Schema string) is required for info action")?;
    let schema: Value = serde_json::from_str(schema_str).map_err(|e| format!("Invalid JSON Schema: {e}"))?;
    let info = analyze_schema(&schema, 0);
    Ok(serde_json::json!({ "schema_type": schema.get("type").and_then(|v| v.as_str()).unwrap_or("unknown"), "info": info }))
}

fn merge_schemas(params: &HashMap<String, Value>) -> Result<Value, String> {
    let schema1_str = params.get("schema").and_then(|v| v.as_str()).ok_or("'schema' (first JSON Schema) is required for merge action")?;
    let schema2_str = params.get("schema2").and_then(|v| v.as_str()).ok_or("'schema2' (second JSON Schema) is required for merge action")?;
    let schema1: Value = serde_json::from_str(schema1_str).map_err(|e| format!("Invalid first schema: {e}"))?;
    let schema2: Value = serde_json::from_str(schema2_str).map_err(|e| format!("Invalid second schema: {e}"))?;
    let merged = merge_schema_values(&schema1, &schema2);
    Ok(serde_json::json!({ "merged": merged, "source_schemas": 2 }))
}

fn infer_schema(value: &Value) -> Value {
    match value {
        Value::Null => serde_json::json!({ "type": "null" }),
        Value::Bool(_) => serde_json::json!({ "type": "boolean" }),
        Value::Number(n) => {
            let t = if n.is_i64() { "integer" } else { "number" };
            serde_json::json!({ "type": t })
        }
        Value::String(s) => {
            let mut schema = serde_json::json!({ "type": "string" });
            if is_iso_date(s) { schema["format"] = Value::String("date-time".to_string()); }
            else if is_email(s) { schema["format"] = Value::String("email".to_string()); }
            else if is_url(s) { schema["format"] = Value::String("uri".to_string()); }
            schema["minLength"] = Value::Number(0.into());
            schema["maxLength"] = Value::Number(s.len().into());
            schema
        }
        Value::Array(arr) => {
            let mut schema = serde_json::json!({ "type": "array", "minItems": 0, "maxItems": arr.len() });
            if !arr.is_empty() {
                schema["items"] = infer_schema(&arr[0]);
                let ft = get_json_type(&arr[0]);
                schema["uniformItems"] = Value::Bool(arr.iter().all(|v| get_json_type(v) == ft));
            } else { schema["items"] = serde_json::json!({}); }
            schema
        }
        Value::Object(obj) => {
            let mut schema = serde_json::json!({ "type": "object", "properties": {}, "required": [] });
            let mut properties: Map<String, Value> = Map::new();
            let mut required = Vec::new();
            for (key, val) in obj {
                properties.insert(key.clone(), infer_schema(val));
                required.push(Value::String(key.clone()));
            }
            schema["properties"] = Value::Object(properties);
            schema["required"] = Value::Array(required);
            schema
        }
    }
}

fn simple_validate(data: &Value, schema: &Value, path: &str) -> (bool, Vec<Value>) {
    let mut errors = Vec::new();
    if let Some(expected_type) = schema.get("type").and_then(|v| v.as_str()) {
        let actual_type = get_json_type(data);
        let type_matches = match expected_type {
            "integer" | "number" => actual_type == "integer" || actual_type == "number",
            _ => actual_type == expected_type,
        };
        if !type_matches {
            errors.push(serde_json::json!({ "path": path, "error": format!("Expected type '{expected_type}', got '{actual_type}'") }));
            return (false, errors);
        }
    }
    if let (Value::Object(obj), Some(Value::Object(props))) = (data, schema.get("properties")) {
        for (key, prop_schema) in props {
            if let Some(value) = obj.get(key) {
                let (valid, sub_errors) = simple_validate(value, prop_schema, &format!("{path}.{key}"));
                if !valid { errors.extend(sub_errors); }
            } else if let Some(Value::Array(required)) = schema.get("required") {
                if required.iter().any(|r| r.as_str() == Some(key)) {
                    errors.push(serde_json::json!({ "path": path, "error": format!("Missing required property '{key}'") }));
                }
            }
        }
    }
    if let (Value::Array(arr), Some(item_schema)) = (data, schema.get("items")) {
        for (i, item) in arr.iter().enumerate() {
            let (valid, sub_errors) = simple_validate(item, item_schema, &format!("{path}[{i}]"));
            if !valid { errors.extend(sub_errors); }
        }
    }
    if let Value::String(s) = data {
        if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if s.len() < min as usize { errors.push(serde_json::json!({ "path": path, "error": format!("String length {} < minimum {}", s.len(), min) })); }
        }
        if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            if s.len() > max as usize { errors.push(serde_json::json!({ "path": path, "error": format!("String length {} > maximum {}", s.len(), max) })); }
        }
    }
    if let Value::Array(arr) = data {
        if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
            if arr.len() < min as usize { errors.push(serde_json::json!({ "path": path, "error": format!("Array length {} < minimum {}", arr.len(), min) })); }
        }
        if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
            if arr.len() > max as usize { errors.push(serde_json::json!({ "path": path, "error": format!("Array length {} > maximum {}", arr.len(), max) })); }
        }
    }
    if let Value::Number(n) = data {
        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            if n.as_f64().unwrap_or(0.0) < min { errors.push(serde_json::json!({ "path": path, "error": format!("Value {} < minimum {}", n, min) })); }
        }
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            if n.as_f64().unwrap_or(0.0) > max { errors.push(serde_json::json!({ "path": path, "error": format!("Value {} > maximum {}", n, max) })); }
        }
    }
    (errors.is_empty(), errors)
}

fn analyze_schema(schema: &Value, depth: usize) -> Map<String, Value> {
    let mut result: Map<String, Value> = Map::new();
    if let Some(schema_type) = schema.get("type").and_then(|v| v.as_str()) {
        result.insert("type".to_string(), Value::String(schema_type.to_string()));
    }
    if let Some(Value::Object(props)) = schema.get("properties") {
        let prop_count = props.len();
        result.insert("property_count".to_string(), Value::Number(prop_count.into()));
        let mut prop_names = Vec::with_capacity(prop_count);
        for key in props.keys() { prop_names.push(Value::String(key.clone())); }
        result.insert("properties".to_string(), Value::Array(prop_names));
        if depth < 5 {
            let mut nested: Map<String, Value> = Map::new();
            for (key, prop_schema) in props {
                nested.insert(key.clone(), Value::Object(analyze_schema(prop_schema, depth + 1)));
            }
            result.insert("nested_schemas".to_string(), Value::Object(nested));
        }
    }
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        result.insert("required_count".to_string(), Value::Number(required.len().into()));
    }
    if let Some(items) = schema.get("items") {
        result.insert("has_items_schema".to_string(), Value::Bool(true));
        result.insert("items_type".to_string(), Value::Object(analyze_schema(items, depth + 1)));
    }
    if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
        result.insert("format".to_string(), Value::String(format.to_string()));
    }
    result
}

fn merge_schema_values(s1: &Value, s2: &Value) -> Value {
    match (s1, s2) {
        (Value::Object(o1), Value::Object(o2)) => {
            let mut merged = o1.clone();
            for (key, v2) in o2 {
                if let Some(v1) = merged.get(key) { merged.insert(key.clone(), merge_schema_values(v1, v2)); }
                else { merged.insert(key.clone(), v2.clone()); }
            }
            Value::Object(merged)
        }
        (Value::Array(a1), Value::Array(a2)) => {
            let mut merged = a1.clone();
            merged.extend(a2.iter().cloned());
            Value::Array(merged)
        }
        _ => s2.clone(),
    }
}

fn get_json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => { if n.is_i64() { "integer" } else { "number" } }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_iso_date(s: &str) -> bool {
    s.len() >= 10 && s.chars().nth(4) == Some('-') && s[0..4].chars().all(|c| c.is_ascii_digit())
}

fn is_email(s: &str) -> bool {
    s.contains('@') && s.contains('.') && !s.starts_with('@') && !s.ends_with('.')
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_string() {
        let s = infer_schema(&Value::String("hello".to_string()));
        assert_eq!(s["type"], "string");
        assert_eq!(s["maxLength"], 5);
    }

    #[test]
    fn test_infer_email_format() {
        let s = infer_schema(&Value::String("user@example.com".to_string()));
        assert_eq!(s["format"], "email");
    }

    #[test]
    fn test_infer_url_format() {
        let s = infer_schema(&Value::String("https://example.com".to_string()));
        assert_eq!(s["format"], "uri");
    }

    #[test]
    fn test_infer_date_format() {
        let s = infer_schema(&Value::String("2025-01-15".to_string()));
        assert_eq!(s["format"], "date-time");
    }

    #[test]
    fn test_infer_integer() { let s = infer_schema(&Value::Number(42.into())); assert_eq!(s["type"], "integer"); }

    #[test]
    fn test_infer_float() { let s = infer_schema(&Value::Number(serde_json::Number::from_f64(3.14).unwrap())); assert_eq!(s["type"], "number"); }

    #[test]
    fn test_infer_boolean() { let s = infer_schema(&Value::Bool(true)); assert_eq!(s["type"], "boolean"); }

    #[test]
    fn test_infer_null() { let s = infer_schema(&Value::Null); assert_eq!(s["type"], "null"); }

    #[test]
    fn test_infer_array_uniform() {
        let arr = Value::Array(vec![Value::Number(1.into()), Value::Number(2.into())]);
        let s = infer_schema(&arr);
        assert_eq!(s["type"], "array");
        assert_eq!(s["items"]["type"], "integer");
        assert_eq!(s["uniformItems"], true);
    }

    #[test]
    fn test_infer_array_mixed() {
        let arr = Value::Array(vec![Value::Number(1.into()), Value::String("x".to_string())]);
        let s = infer_schema(&arr);
        assert_eq!(s["uniformItems"], false);
    }

    #[test]
    fn test_infer_object() {
        let mut obj = Map::new();
        obj.insert("name".to_string(), Value::String("test".to_string()));
        obj.insert("age".to_string(), Value::Number(30.into()));
        let s = infer_schema(&Value::Object(obj));
        assert_eq!(s["type"], "object");
        assert_eq!(s["properties"]["name"]["type"], "string");
        assert_eq!(s["required"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_validate_type_match() {
        let data: Value = serde_json::from_str(r#""hello""#).unwrap();
        let schema: Value = serde_json::from_str(r#"{"type":"string"}"#).unwrap();
        let (valid, _) = simple_validate(&data, &schema, "$");
        assert!(valid);
    }

    #[test]
    fn test_validate_type_mismatch() {
        let data: Value = serde_json::from_str("42").unwrap();
        let schema: Value = serde_json::from_str(r#"{"type":"string"}"#).unwrap();
        let (valid, errors) = simple_validate(&data, &schema, "$");
        assert!(!valid);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_validate_required_missing() {
        let data: Value = serde_json::from_str(r#"{"name":"test"}"#).unwrap();
        let schema: Value = serde_json::from_str(r#"{"type":"object","properties":{"name":{"type":"string"},"age":{"type":"integer"}},"required":["name","age"]}"#).unwrap();
        let (valid, errors) = simple_validate(&data, &schema, "$");
        assert!(!valid);
        assert!(errors.iter().any(|e| e["error"].as_str().unwrap().contains("age")));
    }

    #[test]
    fn test_validate_string_min_length() {
        let data: Value = serde_json::from_str(r#""ab""#).unwrap();
        let schema: Value = serde_json::from_str(r#"{"type":"string","minLength":3}"#).unwrap();
        let (valid, _) = simple_validate(&data, &schema, "$");
        assert!(!valid);
    }

    #[test]
    fn test_validate_array_items() {
        let data: Value = serde_json::from_str(r#"[1,2,"three"]"#).unwrap();
        let schema: Value = serde_json::from_str(r#"{"type":"array","items":{"type":"integer"}}"#).unwrap();
        let (valid, _) = simple_validate(&data, &schema, "$");
        assert!(!valid);
    }

    #[test]
    fn test_generate_schema() {
        let mut p = HashMap::new();
        p.insert("data".to_string(), Value::String(r#"{"name":"test","age":30}"#.to_string()));
        let r = generate_schema(&p).unwrap();
        assert_eq!(r["schema"]["type"], "object");
    }

    #[test]
    fn test_generate_invalid_json() {
        let mut p = HashMap::new();
        p.insert("data".to_string(), Value::String("not json".to_string()));
        assert!(generate_schema(&p).is_err());
    }

    #[test]
    fn test_validate_json() {
        let mut p = HashMap::new();
        p.insert("data".to_string(), Value::String(r#"{"name":"test"}"#.to_string()));
        p.insert("schema".to_string(), Value::String(r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#.to_string()));
        let r = validate_json(&p).unwrap();
        assert_eq!(r["valid"], true);
    }

    #[test]
    fn test_schema_info() {
        let mut p = HashMap::new();
        p.insert("schema".to_string(), Value::String(r#"{"type":"object","properties":{"a":{"type":"string"}}}"#.to_string()));
        let r = schema_info(&p).unwrap();
        assert_eq!(r["schema_type"], "object");
    }

    #[test]
    fn test_merge_schemas() {
        let mut p = HashMap::new();
        p.insert("schema".to_string(), Value::String(r#"{"type":"object"}"#.to_string()));
        p.insert("schema2".to_string(), Value::String(r#"{"description":"test"}"#.to_string()));
        let r = merge_schemas(&p).unwrap();
        assert_eq!(r["merged"]["type"], "object");
        assert_eq!(r["merged"]["description"], "test");
    }

    #[test]
    fn test_get_json_type_all() {
        assert_eq!(get_json_type(&Value::Null), "null");
        assert_eq!(get_json_type(&Value::Bool(true)), "boolean");
        assert_eq!(get_json_type(&Value::Number(1.into())), "integer");
        assert_eq!(get_json_type(&Value::String("x".into())), "string");
        assert_eq!(get_json_type(&Value::Array(vec![])), "array");
        assert_eq!(get_json_type(&Value::Object(Map::new())), "object");
    }

    #[test]
    fn test_helpers() {
        assert!(is_iso_date("2025-01-15"));
        assert!(!is_iso_date("123"));
        assert!(is_email("a@b.c"));
        assert!(!is_email("@bad"));
        assert!(is_url("https://x.com"));
        assert!(!is_url("not-url"));
    }

    #[test]
    fn test_tool_metadata() {
        let t = JsonSchemaTool;
        assert_eq!(t.name(), "json_schema");
        assert_eq!(t.parameters().len(), 4);
    }
}
