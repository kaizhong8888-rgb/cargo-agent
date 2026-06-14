//! Shared utilities for tool implementations.
//!
//! Provides helper functions that eliminate repetitive boilerplate across 70+ tool files:
//! - Parameter extraction from `HashMap<String, Value>`
//! - Common result builders
//! - Helper macros for tool definition

use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// Parameter Extraction Helpers
// ============================================================================

/// Extract a required string parameter, returning an error if missing or not a string.
///
/// # Example
///
/// ```ignore
/// let action = require_str(params, "action")?;
/// ```
#[inline]
pub fn require_str<'a>(
    params: &'a HashMap<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

/// Extract an optional string parameter with a default value.
///
/// # Example
///
/// ```ignore
/// let path = opt_str(params, "path", ".");
/// ```
#[inline]
pub fn opt_str<'a>(
    params: &'a HashMap<String, Value>,
    key: &str,
    default: &'a str,
) -> &'a str {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

/// Extract an optional string parameter that may be absent.
#[inline]
pub fn maybe_str<'a>(
    params: &'a HashMap<String, Value>,
    key: &str,
) -> Option<&'a str> {
    params.get(key).and_then(|v| v.as_str())
}

/// Extract a required f64 parameter.
#[inline]
pub fn require_f64(params: &HashMap<String, Value>, key: &str) -> Result<f64, String> {
    params
        .get(key)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

/// Extract an optional f64 parameter with a default value.
#[inline]
pub fn opt_f64(params: &HashMap<String, Value>, key: &str, default: f64) -> f64 {
    params
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

/// Extract an optional i64 parameter with a default value.
#[inline]
pub fn opt_i64(params: &HashMap<String, Value>, key: &str, default: i64) -> i64 {
    params
        .get(key)
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

/// Extract an optional u64 parameter with a default value.
#[inline]
pub fn opt_u64(params: &HashMap<String, Value>, key: &str, default: u64) -> u64 {
    params
        .get(key)
        .and_then(|v| v.as_u64())
        .unwrap_or(default)
}

/// Extract an optional bool parameter with a default value.
#[inline]
pub fn opt_bool(params: &HashMap<String, Value>, key: &str, default: bool) -> bool {
    params
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Extract an optional usize parameter with a default value.
#[inline]
pub fn opt_usize(params: &HashMap<String, Value>, key: &str, default: usize) -> usize {
    params
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|v| v as usize)
        .unwrap_or(default)
}

// ============================================================================
// Result Builders
// ============================================================================

/// Build a standard success result with a message.
#[inline]
pub fn ok_result(message: impl Into<String>) -> Value {
    serde_json::json!({ "status": "ok", "message": message.into() })
}

/// Build a standard success result with data.
#[inline]
pub fn ok_result_with(action: &str, data: serde_json::Map<String, Value>) -> Value {
    let mut result = serde_json::Map::new();
    result.insert("status".into(), Value::String("ok".into()));
    result.insert("action".into(), Value::String(action.into()));
    result.extend(data);
    Value::Object(result)
}

/// Build an error result.
#[inline]
pub fn error_result(message: impl Into<String>) -> Value {
    serde_json::json!({ "status": "error", "message": message.into() })
}

// ============================================================================
// Tool Parameter Builders
// ============================================================================

use crate::tools::registry::ToolParameter;

/// Create a required string parameter.
#[inline]
pub fn param_required(name: &str, description: &str) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: description.to_string(),
        required: true,
        parameter_type: "string".to_string(),
    }
}

/// Create an optional string parameter.
#[inline]
pub fn param_optional(name: &str, description: &str) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: description.to_string(),
        required: false,
        parameter_type: "string".to_string(),
    }
}

/// Create a required parameter with custom type.
#[inline]
pub fn param_typed(name: &str, description: &str, param_type: &str) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: description.to_string(),
        required: true,
        parameter_type: param_type.to_string(),
    }
}

/// Create an optional parameter with custom type.
#[inline]
pub fn param_typed_opt(name: &str, description: &str, param_type: &str) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: description.to_string(),
        required: false,
        parameter_type: param_type.to_string(),
    }
}

// ============================================================================
// JSON Parsing Helpers
// ============================================================================

/// Parse JSON from a string parameter, returning a descriptive error.
#[inline]
pub fn parse_json<T>(params: &HashMap<String, Value>, key: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let s = maybe_str(params, key)
        .ok_or_else(|| format!("Missing required parameter: {key}"))?;
    serde_json::from_str(s).map_err(|e| format!("Invalid JSON in {key}: {e}"))
}

// ============================================================================
// Macros for Tool Definition
// ============================================================================

/// Define a tool struct with common trait implementations.
///
/// This macro reduces boilerplate for simple tools that only need
/// name, description, parameters, and a dispatch-based execute method.
///
/// # Example
///
/// ```ignore
/// define_tool! {
///     name: "my_tool",
///     description: "Does something useful",
///     params: [
///         required("action", "What to do"),
///         optional("path", "File path", "."),
///     ],
///     dispatch: match_action(params)
/// }
/// ```
#[macro_export]
macro_rules! define_tool {
    (
        name: $name:expr,
        description: $desc:expr,
        params: [$( $param:expr ),* $(,)?],
    ) => {
        paste::paste! {
            pub struct [<$name:camel Tool>];

            #[async_trait::async_trait]
            impl $crate::tools::registry::Tool for [<$name:camel Tool>] {
                fn name(&self) -> &str { $name }
                fn description(&self) -> &str { $desc }
                fn parameters(&self) -> Vec<$crate::tools::registry::ToolParameter> {
                    vec![$( $param ),*]
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_require_str_success() {
        let mut params = HashMap::new();
        params.insert("name".into(), json!("test"));
        assert_eq!(require_str(&params, "name").unwrap(), "test");
    }

    #[test]
    fn test_require_str_missing() {
        let params: HashMap<String, Value> = HashMap::new();
        assert!(require_str(&params, "name").is_err());
    }

    #[test]
    fn test_require_str_wrong_type() {
        let mut params = HashMap::new();
        params.insert("name".into(), json!(42));
        assert!(require_str(&params, "name").is_err());
    }

    #[test]
    fn test_opt_str_with_default() {
        let params: HashMap<String, Value> = HashMap::new();
        assert_eq!(opt_str(&params, "path", "."), ".");
    }

    #[test]
    fn test_opt_str_with_value() {
        let mut params = HashMap::new();
        params.insert("path".into(), json!("/tmp"));
        assert_eq!(opt_str(&params, "path", "."), "/tmp");
    }

    #[test]
    fn test_opt_f64() {
        let params: HashMap<String, Value> = HashMap::new();
        assert!((opt_f64(&params, "threshold", 0.05) - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_opt_bool() {
        let mut params = HashMap::new();
        params.insert("flag".into(), json!(true));
        assert!(opt_bool(&params, "flag", false));
        assert!(!opt_bool(&params, "missing", false));
    }

    #[test]
    fn test_ok_result() {
        let result = ok_result("done");
        assert_eq!(result["status"], "ok");
        assert_eq!(result["message"], "done");
    }

    #[test]
    fn test_param_builders() {
        let p = param_required("action", "The action");
        assert_eq!(p.name, "action");
        assert!(p.required);

        let p = param_optional("path", "The path");
        assert!(!p.required);

        let p = param_typed("count", "Number", "integer");
        assert_eq!(p.parameter_type, "integer");
    }

    #[test]
    fn test_parse_json() {
        let mut params = HashMap::new();
        params.insert("data".into(), json!("[1,2,3]"));
        let result: Vec<i32> = parse_json(&params, "data").unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_json_invalid() {
        let mut params = HashMap::new();
        params.insert("data".into(), json!("not json"));
        let result: Result<Vec<i32>, String> = parse_json(&params, "data");
        assert!(result.is_err());
    }
}
