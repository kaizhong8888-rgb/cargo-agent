//! Regex tool: advanced regex testing, validation, replacement, and Rust code generation.
//!
//! # Actions
//!
//! - **test**: Test if a pattern matches text
//! - **find_all**: Find all matches with positions and captured groups
//! - **replace**: Replace matches with a pattern
//! - **generate_rust**: Generate ready-to-use Rust regex code
//! - **validate**: Validate regex syntax without executing
//! - **explain**: Explain regex pattern components

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;

// ============================================================================
// RegexTool
// ============================================================================

pub struct RegexTool;

#[async_trait::async_trait]
impl Tool for RegexTool {
    fn name(&self) -> &str {
        "regex_tool"
    }

    fn description(&self) -> &str {
        "Advanced regex testing and Rust code generation: test patterns, find all matches with capture groups, replace text, generate Rust regex code, validate syntax, and explain regex components."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: test (check if pattern matches), find_all (find all matches with captures), replace (replace matches), generate_rust (generate Rust code), validate (validate syntax), explain (explain pattern components)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "pattern".to_string(),
                description: "Regular expression pattern".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "text".to_string(),
                description: "Text to match against (required for test/find_all/replace)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "replacement".to_string(),
                description: "Replacement string (for replace action, supports $1, $2, etc.)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "case_insensitive".to_string(),
                description: "Enable case-insensitive matching (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "multiline".to_string(),
                description: "Enable multiline mode: ^ and $ match line boundaries (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "dot_matches_newline".to_string(),
                description: "Make . match newline (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "function_name".to_string(),
                description: "Function name for generated Rust code (default: 'match_pattern')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_matches".to_string(),
                description: "Maximum number of matches to return (default: 100)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: pattern")?;

        match action {
            "test" => self.action_test(pattern, params),
            "find_all" => self.action_find_all(pattern, params),
            "replace" => self.action_replace(pattern, params),
            "generate_rust" => self.action_generate_rust(pattern, params),
            "validate" => self.action_validate(pattern, params),
            "explain" => self.action_explain(pattern),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: test, find_all, replace, generate_rust, validate, explain"),
            })),
        }
    }
}

impl RegexTool {
    fn action_test(&self, pattern: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: text")?;

        let re = build_regex(pattern, params)?;

        if let Some(captures) = re.captures(text) {
            let mut groups = Vec::new();
            for (i, m) in captures.iter().enumerate() {
                if let Some(m) = m {
                    groups.push(json!({
                        "group": i,
                        "name": captures.name(&i.to_string()).map(|n| n.as_str()),
                        "match": m.as_str(),
                        "start": m.start(),
                        "end": m.end(),
                    }));
                }
            }
            Ok(json!({
                "status": "ok",
                "action": "test",
                "matched": true,
                "pattern": pattern,
                "groups": groups,
            }))
        } else {
            Ok(json!({
                "status": "ok",
                "action": "test",
                "matched": false,
                "pattern": pattern,
            }))
        }
    }

    fn action_find_all(&self, pattern: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: text")?;

        let max_matches = params.get("max_matches").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let re = build_regex(pattern, params)?;

        let mut matches = Vec::new();
        for cap in re.captures_iter(text).take(max_matches) {
            let mut groups = Vec::new();
            for (i, m) in cap.iter().enumerate() {
                if let Some(m) = m {
                    groups.push(json!({
                        "group": i,
                        "match": m.as_str(),
                        "start": m.start(),
                        "end": m.end(),
                    }));
                }
            }
            matches.push(json!({
                "full_match": cap.get(0).map(|m| m.as_str()),
                "groups": groups,
            }));
        }

        Ok(json!({
            "status": "ok",
            "action": "find_all",
            "pattern": pattern,
            "total_matches": matches.len(),
            "matches": matches,
        }))
    }

    fn action_replace(&self, pattern: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: text")?;

        let replacement = params
            .get("replacement")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: replacement")?;

        let re = build_regex(pattern, params)?;

        let result = re.replace_all(text, replacement);

        // Count replacements
        let match_count = re.find_iter(text).count();

        Ok(json!({
            "status": "ok",
            "action": "replace",
            "pattern": pattern,
            "replacement": replacement,
            "replacements_made": match_count,
            "original": text,
            "result": result,
        }))
    }

    fn action_generate_rust(&self, pattern: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
        let function_name = params
            .get("function_name")
            .and_then(|v| v.as_str())
            .unwrap_or("match_pattern");

        let case_insensitive = params.get("case_insensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let multiline = params.get("multiline").and_then(|v| v.as_bool()).unwrap_or(false);
        let dot_matches_newline = params.get("dot_matches_newline").and_then(|v| v.as_bool()).unwrap_or(false);

        let text = params.get("text").and_then(|v| v.as_str());

        // Escape the pattern for inclusion in a Rust string literal
        let escaped_pattern = pattern
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        let mut code = String::new();

        code.push_str("//! Auto-generated regex function\n");
        code.push_str("//!\n");
        code.push_str(&format!("//! Pattern: `{pattern}`\n\n"));

        // Check if pattern has named captures
        let has_named_captures = pattern.contains("(?P<");

        // Determine return type
        let return_type = if has_named_captures {
            "Option<Vec<(String, String)>>"
        } else {
            "Option<Vec<String>>"
        };

        code.push_str("use regex::Regex;\n\n");
        code.push_str(&format!("/// Match text against the pattern: `{pattern}`\n"));
        code.push_str("#[allow(dead_code)]\n");
        code.push_str(&format!("pub fn {function_name}(text: &str) -> {return_type} {{\n"));

        // Build regex
        if case_insensitive || multiline || dot_matches_newline {
            code.push_str("    let re = RegexBuilder::new(r#\"");
            code.push_str(&escaped_pattern);
            code.push_str("\"#)\n");
            if case_insensitive {
                code.push_str("        .case_insensitive(true)\n");
            }
            if multiline {
                code.push_str("        .multi_line(true)\n");
            }
            if dot_matches_newline {
                code.push_str("        .dot_matches_new_line(true)\n");
            }
            code.push_str("        .build()\n");
            code.push_str("        .expect(\"valid regex\");\n");
        } else {
            code.push_str("    let re = Regex::new(r#\"");
            code.push_str(&escaped_pattern);
            code.push_str("\"#).expect(\"valid regex\");\n");
        }

        code.push('\n');

        if has_named_captures {
            code.push_str("    let mut results = Vec::new();\n");
            code.push_str("    for cap in re.captures_iter(text) {\n");
            code.push_str("        let mut groups = Vec::new();\n");
            code.push_str("        for name in re.capture_names().flatten() {\n");
            code.push_str("            if let Some(m) = cap.name(name) {\n");
            code.push_str("                groups.push((name.to_string(), m.as_str().to_string()));\n");
            code.push_str("            }\n");
            code.push_str("        }\n");
            code.push_str("        if !groups.is_empty() {\n");
            code.push_str("            results.push(groups);\n");
            code.push_str("        }\n");
            code.push_str("    }\n");
            code.push_str("    if results.is_empty() { None } else { Some(results) }\n");
        } else {
            code.push_str("    let results: Vec<String> = re\n");
            code.push_str("        .captures_iter(text)\n");
            code.push_str("        .filter_map(|cap| cap.get(1).or_else(|| cap.get(0)))\n");
            code.push_str("        .map(|m| m.as_str().to_string())\n");
            code.push_str("        .collect();\n");
            code.push_str("    if results.is_empty() { None } else { Some(results) }\n");
        }

        code.push_str("}\n\n");

        // Generate test if text provided
        if let Some(sample_text) = text {
            let escaped_text = sample_text
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            code.push_str("#[cfg(test)]\n");
            code.push_str("mod tests {\n");
            code.push_str("    use super::*;\n\n");
            code.push_str("    #[test]\n");
            code.push_str(&format!("    fn test_{function_name}() {{\n"));
            code.push_str(&format!("        let text = r#\"{escaped_text}\"#;\n"));
            code.push_str(&format!("        let result = {function_name}(text);\n"));
            code.push_str("        assert!(result.is_some());\n");
            code.push_str("    }\n");
            code.push_str("}\n");
        }

        // Validate the pattern
        build_regex(pattern, params).map_err(|e| format!("Invalid pattern: {e}"))?;

        Ok(json!({
            "status": "ok",
            "action": "generate_rust",
            "pattern": pattern,
            "function_name": function_name,
            "has_named_captures": has_named_captures,
            "generated_code": code,
        }))
    }

    fn action_validate(&self, pattern: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
        match build_regex(pattern, params) {
            Ok(_) => {
                let info = analyze_pattern(pattern);
                Ok(json!({
                    "status": "ok",
                    "action": "validate",
                    "valid": true,
                    "pattern": pattern,
                    "analysis": info,
                }))
            }
            Err(e) => Ok(json!({
                "status": "ok",
                "action": "validate",
                "valid": false,
                "pattern": pattern,
                "error": e,
            })),
        }
    }

    fn action_explain(&self, pattern: &str) -> Result<Value, String> {
        let components = explain_pattern(pattern);
        Ok(json!({
            "status": "ok",
            "action": "explain",
            "pattern": pattern,
            "components": components,
        }))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn build_regex(pattern: &str, params: &HashMap<String, Value>) -> Result<Regex, String> {
    let case_insensitive = params.get("case_insensitive").and_then(|v| v.as_bool()).unwrap_or(false);
    let multiline = params.get("multiline").and_then(|v| v.as_bool()).unwrap_or(false);
    let dot_matches_newline = params.get("dot_matches_newline").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut builder = RegexBuilder::new(pattern);
    builder.case_insensitive(case_insensitive);
    builder.multi_line(multiline);
    builder.dot_matches_new_line(dot_matches_newline);

    builder.build().map_err(|e| e.to_string())
}

fn analyze_pattern(pattern: &str) -> Value {
    let has_anchors = pattern.contains('^') || pattern.contains('$');
    let has_captures = pattern.contains('(') && pattern.contains(')');
    let has_named_captures = pattern.contains("(?P<");
    let has_lookahead = pattern.contains("(?=") || pattern.contains("(?!");
    let has_lookbehind = pattern.contains("(?<=") || pattern.contains("(?<!");
    let has_character_class = pattern.contains('[') && pattern.contains(']');
    let has_quantifiers = pattern.contains('*') || pattern.contains('+') || pattern.contains('?') || pattern.contains('{');
    let has_alternation = pattern.contains('|');
    let has_backreference = pattern.contains("\\1") || pattern.contains("\\2");
    let is_greedy = (pattern.contains('*') || pattern.contains('+')) && !pattern.contains("*?") && !pattern.contains("+?");

    json!({
        "has_anchors": has_anchors,
        "has_capture_groups": has_captures,
        "has_named_captures": has_named_captures,
        "has_lookahead": has_lookahead,
        "has_lookbehind": has_lookbehind,
        "has_character_classes": has_character_class,
        "has_quantifiers": has_quantifiers,
        "has_alternation": has_alternation,
        "has_backreferences": has_backreference,
        "is_greedy": is_greedy,
        "length": pattern.len(),
        "complexity": if pattern.len() < 20 { "simple" } else if pattern.len() < 50 { "moderate" } else { "complex" },
    })
}

fn explain_pattern(pattern: &str) -> Vec<Value> {
    let mut components = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = pattern.chars().collect();

    while i < chars.len() {
        let component = match chars[i] {
            '^' => {
                i += 1;
                json!({
                    "token": "^",
                    "meaning": "Start of string/line anchor",
                    "type": "anchor",
                })
            }
            '$' => {
                i += 1;
                json!({
                    "token": "$",
                    "meaning": "End of string/line anchor",
                    "type": "anchor",
                })
            }
            '.' => {
                i += 1;
                json!({
                    "token": ".",
                    "meaning": "Matches any single character (except newline)",
                    "type": "wildcard",
                })
            }
            '*' => {
                i += 1;
                json!({
                    "token": "*",
                    "meaning": "Zero or more of the preceding element",
                    "type": "quantifier",
                })
            }
            '+' => {
                i += 1;
                json!({
                    "token": "+",
                    "meaning": "One or more of the preceding element",
                    "type": "quantifier",
                })
            }
            '?' => {
                i += 1;
                json!({
                    "token": "?",
                    "meaning": "Zero or one of the preceding element",
                    "type": "quantifier",
                })
            }
            '|' => {
                i += 1;
                json!({
                    "token": "|",
                    "meaning": "Alternation (OR)",
                    "type": "alternation",
                })
            }
            '\\' => {
                if i + 1 < chars.len() {
                    let next = chars[i + 1];
                    let meaning = match next {
                        'd' => "Any digit (0-9)",
                        'D' => "Any non-digit",
                        'w' => "Any word character (a-z, A-Z, 0-9, _)",
                        'W' => "Any non-word character",
                        's' => "Any whitespace character",
                        'S' => "Any non-whitespace character",
                        'n' => "Newline character",
                        't' => "Tab character",
                        'r' => "Carriage return",
                        _ => &format!("Literal character '{}'", next),
                    };
                    let token = format!("\\{}", next);
                    i += 2;
                    json!({
                        "token": token,
                        "meaning": meaning,
                        "type": "escape",
                    })
                } else {
                    i += 1;
                    json!({
                        "token": "\\",
                        "meaning": "Trailing backslash (invalid)",
                        "type": "error",
                    })
                }
            }
            '[' => {
                // Find the closing bracket
                let start = i;
                let mut end = i + 1;
                while end < chars.len() && chars[end] != ']' {
                    if chars[end] == '\\' && end + 1 < chars.len() {
                        end += 2;
                    } else {
                        end += 1;
                    }
                }
                if end < chars.len() {
                    end += 1;
                }
                let class: String = chars[start..end].iter().collect();
                let meaning = if class.starts_with("[^") {
                    "Negated character class (matches any character NOT in the set)"
                } else {
                    "Character class (matches any character in the set)"
                };
                i = end;
                json!({
                    "token": class,
                    "meaning": meaning,
                    "type": "character_class",
                })
            }
            '(' => {
                // Find the matching closing paren
                let start = i;
                let mut end = i + 1;
                let mut depth = 1;
                while end < chars.len() && depth > 0 {
                    if chars[end] == '(' && (end == 0 || chars[end - 1] != '\\') {
                        depth += 1;
                    } else if chars[end] == ')' && chars[end - 1] != '\\' {
                        depth -= 1;
                    }
                    end += 1;
                }
                let group: String = chars[start..end].iter().collect();
                let meaning = if group.starts_with("(?P<") {
                    "Named capture group"
                } else if group.starts_with("(?:") {
                    "Non-capturing group"
                } else if group.starts_with("(?=") {
                    "Positive lookahead"
                } else if group.starts_with("(?!") {
                    "Negative lookahead"
                } else if group.starts_with("(?<=") {
                    "Positive lookbehind"
                } else if group.starts_with("(?<!") {
                    "Negative lookbehind"
                } else {
                    "Capturing group"
                };
                i = end;
                json!({
                    "token": group.chars().take(30).collect::<String>(),
                    "meaning": meaning,
                    "type": "group",
                })
            }
            '{' => {
                // Find the closing brace
                let start = i;
                let mut end = i + 1;
                while end < chars.len() && chars[end] != '}' {
                    end += 1;
                }
                if end < chars.len() {
                    end += 1;
                }
                let quantifier: String = chars[start..end].iter().collect();
                let meaning = if quantifier.contains(',') {
                    "Range quantifier: between min and max occurrences"
                } else {
                    "Exact quantifier: exactly N occurrences"
                };
                i = end;
                json!({
                    "token": quantifier,
                    "meaning": meaning,
                    "type": "quantifier",
                })
            }
            c => {
                let token = c.to_string();
                i += 1;
                let meaning = if c.is_alphanumeric() || c == '_' {
                    format!("Literal character '{}'", c)
                } else {
                    format!("Escaped literal '{}'", c)
                };
                json!({
                    "token": token,
                    "meaning": meaning,
                    "type": "literal",
                })
            }
        };
        components.push(component);
    }

    components
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(RegexTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool() -> RegexTool {
        RegexTool
    }

    #[tokio::test]
    async fn test_action_test_match() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("test".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));
        params.insert("text".to_string(), Value::String("abc123def".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["matched"], true);
    }

    #[tokio::test]
    async fn test_action_test_no_match() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("test".to_string()));
        params.insert("pattern".to_string(), Value::String(r"^\d+$".to_string()));
        params.insert("text".to_string(), Value::String("abc123".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["matched"], false);
    }

    #[tokio::test]
    async fn test_action_test_case_insensitive() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("test".to_string()));
        params.insert("pattern".to_string(), Value::String("HELLO".to_string()));
        params.insert("text".to_string(), Value::String("hello world".to_string()));
        params.insert("case_insensitive".to_string(), Value::Bool(true));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["matched"], true);
    }

    #[tokio::test]
    async fn test_action_test_missing_text() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("test".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_action_find_all() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("find_all".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));
        params.insert("text".to_string(), Value::String("123 abc 456 def 789".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["total_matches"], 3);
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches[0]["full_match"], "123");
        assert_eq!(matches[1]["full_match"], "456");
        assert_eq!(matches[2]["full_match"], "789");
    }

    #[tokio::test]
    async fn test_action_find_all_with_groups() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("find_all".to_string()));
        params.insert(
            "pattern".to_string(),
            Value::String(r"(\w+)=(\d+)".to_string()),
        );
        params.insert(
            "text".to_string(),
            Value::String("a=1 b=2 c=3".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_matches"], 3);
        let matches = result["matches"].as_array().unwrap();
        let groups = matches[0]["groups"].as_array().unwrap();
        assert_eq!(groups[0]["match"], "a=1");
        assert_eq!(groups[1]["match"], "a");
        assert_eq!(groups[2]["match"], "1");
    }

    #[tokio::test]
    async fn test_action_find_all_limit() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("find_all".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d".to_string()));
        params.insert(
            "text".to_string(),
            Value::String("1 2 3 4 5 6 7 8 9 0".to_string()),
        );
        params.insert("max_matches".to_string(), Value::Number(3.into()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_matches"], 3);
    }

    #[tokio::test]
    async fn test_action_replace() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("replace".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\s+".to_string()));
        params.insert("text".to_string(), Value::String("hello   world  test".to_string()));
        params.insert("replacement".to_string(), Value::String("-".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["result"], "hello-world-test");
        assert_eq!(result["replacements_made"], 2);
    }

    #[tokio::test]
    async fn test_action_replace_with_groups() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("replace".to_string()));
        params.insert(
            "pattern".to_string(),
            Value::String(r"(\w+)\s+(\w+)".to_string()),
        );
        params.insert(
            "text".to_string(),
            Value::String("John Doe".to_string()),
        );
        params.insert("replacement".to_string(), Value::String("$2, $1".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["result"], "Doe, John");
    }

    #[tokio::test]
    async fn test_action_replace_missing_params() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("replace".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_action_validate_valid() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("validate".to_string()));
        params.insert("pattern".to_string(), Value::String(r"^\d{3}-\d{4}$".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["valid"], true);
        assert_eq!(result["analysis"]["has_anchors"], true);
        assert_eq!(result["analysis"]["has_quantifiers"], true);
    }

    #[tokio::test]
    async fn test_action_validate_invalid() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("validate".to_string()));
        params.insert("pattern".to_string(), Value::String(r"[invalid".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["valid"], false);
        assert!(result["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_action_explain() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("explain".to_string()));
        params.insert("pattern".to_string(), Value::String(r"^\d{3}-\d{4}$".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let components = result["components"].as_array().unwrap();
        assert!(!components.is_empty());

        // Check anchor
        let anchor = components.iter().find(|c| c["type"] == "anchor");
        assert!(anchor.is_some());
    }

    #[tokio::test]
    async fn test_action_generate_rust() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert(
            "action".to_string(),
            Value::String("generate_rust".to_string()),
        );
        params.insert(
            "pattern".to_string(),
            Value::String(r"(?P<name>\w+)=(?P<value>\d+)".to_string()),
        );
        params.insert(
            "text".to_string(),
            Value::String("a=1 b=2".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["has_named_captures"], true);
        let code = result["generated_code"].as_str().unwrap();
        assert!(code.contains("pub fn"));
        assert!(code.contains("#[cfg(test)]"));
        assert!(code.contains("fn test_"));
    }

    #[tokio::test]
    async fn test_action_generate_rust_custom_name() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert(
            "action".to_string(),
            Value::String("generate_rust".to_string()),
        );
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));
        params.insert(
            "function_name".to_string(),
            Value::String("find_numbers".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        let code = result["generated_code"].as_str().unwrap();
        assert!(code.contains("pub fn find_numbers"));
    }

    #[tokio::test]
    async fn test_action_generate_rust_with_options() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert(
            "action".to_string(),
            Value::String("generate_rust".to_string()),
        );
        params.insert("pattern".to_string(), Value::String(r"hello".to_string()));
        params.insert("case_insensitive".to_string(), Value::Bool(true));
        params.insert("multiline".to_string(), Value::Bool(true));

        let result = tool.execute(&params).await.unwrap();
        let code = result["generated_code"].as_str().unwrap();
        assert!(code.contains(".case_insensitive(true)"));
        assert!(code.contains(".multi_line(true)"));
    }

    #[tokio::test]
    async fn test_action_unknown() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("invalid".to_string()));
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_missing_pattern() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("test".to_string()));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String(r"\d+".to_string()));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_pattern() {
        let analysis = analyze_pattern(r"^(\w+)@(\w+)\.(\w+)$");
        assert_eq!(analysis["has_anchors"], true);
        assert_eq!(analysis["has_capture_groups"], true);
        assert_eq!(analysis["has_named_captures"], false);
        assert_eq!(analysis["complexity"], "moderate");
    }

    #[test]
    fn test_analyze_pattern_complex() {
        let analysis = analyze_pattern(
            r"(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})[T ](?P<hour>\d{2}):(?P<min>\d{2}):(?P<sec>\d{2})",
        );
        assert_eq!(analysis["has_named_captures"], true);
        assert_eq!(analysis["has_quantifiers"], true);
        assert_eq!(analysis["complexity"], "complex");
    }

    #[test]
    fn test_analyze_pattern_simple() {
        let analysis = analyze_pattern(r"abc");
        assert_eq!(analysis["complexity"], "simple");
        assert_eq!(analysis["has_anchors"], false);
        assert_eq!(analysis["has_capture_groups"], false);
    }

    #[test]
    fn test_build_regex_case_insensitive() {
        let mut params = HashMap::new();
        params.insert("case_insensitive".to_string(), Value::Bool(true));

        let re = build_regex("HELLO", &params).unwrap();
        assert!(re.is_match("hello"));
        assert!(re.is_match("HELLO"));
        assert!(re.is_match("Hello"));
    }

    #[test]
    fn test_build_regex_multiline() {
        let mut params = HashMap::new();
        params.insert("multiline".to_string(), Value::Bool(true));

        let re = build_regex("^test", &params).unwrap();
        assert!(re.is_match("test\nline2"));
    }

    #[test]
    fn test_build_regex_invalid() {
        let params = HashMap::new();
        let result = build_regex("[invalid", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_explain_pattern_anchors() {
        let components = explain_pattern("^hello$");
        assert_eq!(components.len(), 7); // ^, h, e, l, l, o, $
        assert_eq!(components[0]["type"], "anchor");
        assert_eq!(components[6]["type"], "anchor");
    }

    #[test]
    fn test_explain_pattern_escapes() {
        let components = explain_pattern(r"\d\w\s");
        let escapes: Vec<_> = components.iter().filter(|c| c["type"] == "escape").collect();
        assert_eq!(escapes.len(), 3);
    }

    #[test]
    fn test_explain_pattern_character_class() {
        let components = explain_pattern(r"[a-zA-Z0-9]");
        assert_eq!(components.len(), 1);
        assert_eq!(components[0]["type"], "character_class");
    }

    #[test]
    fn test_explain_pattern_negated_class() {
        let components = explain_pattern(r"[^a-z]");
        assert_eq!(components.len(), 1);
        let meaning = components[0]["meaning"].as_str().unwrap();
        assert!(meaning.contains("NOT"));
    }

    #[test]
    fn test_explain_pattern_groups() {
        let components = explain_pattern(r"(hello)(?:world)(?<name>\w+)");
        let groups: Vec<_> = components.iter().filter(|c| c["type"] == "group").collect();
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_explain_pattern_quantifier_brace() {
        let components = explain_pattern(r"a{3}");
        let quants: Vec<_> = components
            .iter()
            .filter(|c| c["type"] == "quantifier")
            .collect();
        assert_eq!(quants.len(), 1);
    }

    #[test]
    fn test_explain_empty() {
        let components = explain_pattern("");
        assert_eq!(components.len(), 0);
    }
}
