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
    fn name(&self) -> &str {
        "json_schema"
    }

    fn description(&self) -> &str {
        "Generate and validate JSON Schema from JSON data. \
         Actions: generate (infer schema from JSON), validate (check JSON against schema), \
         info (display schema info), merge (combine two schemas)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: generate, validate, info, merge".to_string(),
                required: true,
            },
            ToolParameter {
                name: "data".to_string(),
                parameter_type: "string".to_string(),
                description: "JSON data (for generate)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "schema".to_string(),
                parameter_type: "string".to_string(),
                description: "JSON Schema (for validate/info)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "schema2".to_string(),
                parameter_type: "string".to_string(),
                description: "Second JSON Schema (for merge)".to_string(),
                required: false,
            },
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
    let data_str = params
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or("'data' (JSON string) is required for generate action")?;

    let data: Value = serde_json::from_str(data_str)
        .map_err(|e| format!("Invalid JSON: {e}"))?;

    let schema = infer_schema(&data);

    Ok(serde_json::json!({
        "schema": schema,
        "input_type": get_json_type(&data),
    }))
}

fn validate_json(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data_str = params
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or("'data' (JSON string) is required for validate action")?;

    let schema_str = params
        .get("schema")
        .and_then(|v| v.as_str())
        .ok_or("'schema' (JSON Schema string) is required for validate action")?;

    let data: Value = serde_json::from_str(data_str)
        .map_err(|e| format!("Invalid JSON data: {e}"))?;

    let schema: Value = serde_json::from_str(schema_str)
        .map_err(|e| format!("Invalid JSON Schema: {e}"))?;

    let (valid, errors) = simple_validate(&data, &schema, "$");
    let error_count = errors.len();

    Ok(serde_json::json!({
        "valid": valid,
        "errors": if valid { Value::Null } else { Value::Array(errors) },
        "error_count": error_count,
    }))
}

fn schema_info(params: &HashMap<String, Value>) -> Result<Value, String> {
    let schema_str = params
        .get("schema")
        .and_then(|v| v.as_str())
        .ok_or("'schema' (JSON Schema string) is required for info action")?;

    let schema: Value = serde_json::from_str(schema_str)
        .map_err(|e| format!("Invalid JSON Schema: {e}"))?;

    let info = analyze_schema(&schema, 0);

    Ok(serde_json::json!({
        "schema_type": schema.get("type").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "info": info,
    }))
}

fn merge_schemas(params: &HashMap<String, Value>) -> Result<Value, String> {
    let schema1_str = params
        .get("schema")
        .and_then(|v| v.as_str())
        .ok_or("'schema' (first JSON Schema) is required for merge action")?;

    let schema2_str = params
        .get("schema2")
        .and_then(|v| v.as_str())
        .ok_or("'schema2' (second JSON Schema) is required for merge action")?;

    let schema1: Value = serde_json::from_str(schema1_str)
        .map_err(|e| format!("Invalid first schema: {e}"))?;

    let schema2: Value = serde_json::from_str(schema2_str)
        .map_err(|e| format!("Invalid second schema: {e}"))?;

    let merged = merge_schema_values(&schema1, &schema2);

    Ok(serde_json::json!({
        "merged": merged,
        "source_schemas": 2,
    }))
}

fn infer_schema(value: &Value) -> Value {
    match value {
        Value::Null => {
            serde_json::json!({ "type": "null" })
        }
        Value::Bool(_) => {
            serde_json::json!({ "type": "boolean" })
        }
        Value::Number(n) => {
            let num_type = if n.is_i64() { "integer" } else { "number" };
            serde_json::json!({ "type": num_type })
        }
        Value::String(s) => {
            let mut schema = serde_json::json!({ "type": "string" });
            if is_iso_date(s) {
                schema["format"] = Value::String("date-time".to_string());
            } else if is_email(s) {
                schema["format"] = Value::String("email".to_string());
            } else if is_url(s) {
                schema["format"] = Value::String("uri".to_string());
            }
            schema["minLength"] = Value::Number(0.into());
            schema["maxLength"] = Value::Number(s.len().into());
            schema
        }
        Value::Array(arr) => {
            let mut schema = serde_json::json!({
                "type": "array",
                "minItems": 0,
                "maxItems": arr.len(),
            });

            if !arr.is_empty() {
                let item_schema = infer_schema(&arr[0]);
                schema["items"] = item_schema;

                let first_type = get_json_type(&arr[0]);
                let all_same = arr.iter().all(|v| get_json_type(v) == first_type);
                schema["uniformItems"] = Value::Bool(all_same);
            } else {
                schema["items"] = serde_json::json!({});
            }

            schema
        }
        Value::Object(obj) => {
            let mut schema = serde_json::json!({
                "type": "object",
                "properties": {},
                "required": [],
            });

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
            "integer" => actual_type == "integer" || actual_type == "number",
            "number" => actual_type == "integer" || actual_type == "number",
            _ => actual_type == expected_type,
        };

        if !type_matches {
            errors.push(serde_json::json!({
                "path": path,
                "error": format!("Expected type '{expected_type}', got '{actual_type}'"),
            }));
            return (false, errors);
        }
    }

    if let (Value::Object(obj), Some(Value::Object(props))) = (data, schema.get("properties")) {
        for (key, prop_schema) in props {
            if let Some(value) = obj.get(key) {
                let (valid, sub_errors) = simple_validate(value, prop_schema, &format!("{path}.{key}"));
                if !valid {
                    errors.extend(sub_errors);
                }
            } else if let Some(Value::Array(required)) = schema.get("required") {
                if required.iter().any(|r| r.as_str() == Some(key)) {
                    errors.push(serde_json::json!({
                        "path": path,
                        "error": format!("Missing required property '{key}'"),
                    }));
                }
            }
        }
    }

    if let (Value::Array(arr), Some(item_schema)) = (data, schema.get("items")) {
        for (i, item) in arr.iter().enumerate() {
            let (valid, sub_errors) = simple_validate(item, item_schema, &format!("{path}[{i}]"));
            if !valid {
                errors.extend(sub_errors);
            }
        }
    }

    if let Value::String(s) = data {
        if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if s.len() < min as usize {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("String length {} is less than minimum {}", s.len(), min),
                }));
            }
        }
        if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            if s.len() > max as usize {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("String length {} exceeds maximum {}", s.len(), max),
                }));
            }
        }
    }

    if let Value::Array(arr) = data {
        if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
            if arr.len() < min as usize {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("Array length {} is less than minimum {}", arr.len(), min),
                }));
            }
        }
        if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
            if arr.len() > max as usize {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("Array length {} exceeds maximum {}", arr.len(), max),
                }));
            }
        }
    }

    if let Value::Number(n) = data {
        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            if n.as_f64().unwrap_or(0.0) < min {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("Value {} is less than minimum {}", n, min),
                }));
            }
        }
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            if n.as_f64().unwrap_or(0.0) > max {
                errors.push(serde_json::json!({
                    "path": path,
                    "error": format!("Value {} exceeds maximum {}", n, max),
                }));
            }
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
        for key in props.keys() {
            prop_names.push(Value::String(key.clone()));
        }
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
                if let Some(v1) = merged.get(key) {
                    merged.insert(key.clone(), merge_schema_values(v1, v2));
                } else {
                    merged.insert(key.clone(), v2.clone());
                }
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
        Value::Number(n) => {
            if n.is_i64() { "integer" } else { "number" }
        }
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
