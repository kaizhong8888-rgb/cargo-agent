//! Clippy Lint tool: run cargo clippy, categorize lint warnings,
//! suggest fixes, score code quality, and auto-fix common patterns.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;

// ============================================================================
// ClippyLintTool
// ============================================================================

pub struct ClippyLintTool;

#[async_trait::async_trait]
impl Tool for ClippyLintTool {
    fn name(&self) -> &str {
        "clippy_lint"
    }

    fn description(&self) -> &str {
        "Run cargo clippy with structured output: categorize lints by severity and type, suggest fixes, score code quality, and auto-fix common patterns. Supports: run, categorize, suggest_fixes, score, auto_fix."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: run (run clippy), categorize (categorize results), suggest_fixes (suggest fixes), score (quality score), auto_fix (auto-fix common patterns)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the Rust project (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "allow".to_string(),
                description: "Comma-separated clippy allows (e.g. 'clippy::unwrap_used,clippy::expect_used')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "deny".to_string(),
                description: "Comma-separated clippy denies (e.g. 'clippy::unwrap_used')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "tests".to_string(),
                description: "Include test code in linting (true/false, default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "fix".to_string(),
                description: "Auto-apply safe fixes from clippy (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "result_json".to_string(),
                description: "JSON output from clippy --message-format=json (for categorize/suggest_fixes/score actions)".to_string(),
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

        match action {
            "run" => self.action_run(params),
            "categorize" => self.action_categorize(params),
            "suggest_fixes" => self.action_suggest_fixes(params),
            "score" => self.action_score(params),
            "auto_fix" => self.action_auto_fix(params),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: run, categorize, suggest_fixes, score, auto_fix"),
            })),
        }
    }
}

impl ClippyLintTool {
    /// Run cargo clippy with options.
    fn action_run(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let allow = params.get("allow").and_then(|v| v.as_str());
        let deny = params.get("deny").and_then(|v| v.as_str());
        let include_tests = params
            .get("tests")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let do_fix = params.get("fix").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut cmd = Command::new("cargo");
        cmd.current_dir(path);

        if do_fix {
            cmd.arg("clippy").arg("--fix").arg("--allow-dirty");
        } else {
            cmd.arg("clippy");
        }

        cmd.arg("--message-format=json");

        if !include_tests {
            cmd.arg("--lib");
        }

        // Add deny/allow flags
        if let Some(deny_list) = deny {
            for d in deny_list.split(',') {
                let d = d.trim();
                if !d.is_empty() {
                    cmd.arg(format!("-D{d}"));
                }
            }
        }

        if let Some(allow_list) = allow {
            for a in allow_list.split(',') {
                let a = a.trim();
                if !a.is_empty() {
                    cmd.arg(format!("-A{a}"));
                }
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run cargo clippy: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse JSON messages
        let mut lints = Vec::new();
        let mut compiler_messages = Vec::new();

        for line in stdout.lines() {
            if let Ok(msg) = serde_json::from_str::<Value>(line) {
                if let Some(reason) = msg.get("reason") {
                    if reason == "compiler-message" {
                        compiler_messages.push(msg.clone());
                        if let Some(message) = msg.get("message") {
                            lints.push(message.clone());
                        }
                    }
                }
            }
        }

        let (errors, warnings, notes) = classify_lints(&lints);

        Ok(json!({
            "status": if output.status.success() { "ok" } else { "error" },
            "action": "run",
            "path": path,
            "exit_code": output.status.code().unwrap_or(-1),
            "summary": {
                "errors": errors.len(),
                "warnings": warnings.len(),
                "notes": notes.len(),
                "total": lints.len(),
            },
            "errors": errors,
            "warnings": warnings,
            "notes": notes,
            "raw_stderr": stderr.lines().take(20).collect::<Vec<_>>(),
        }))
    }

    /// Categorize lint results.
    fn action_categorize(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let result_json = params
            .get("result_json")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: result_json (JSON array from clippy --message-format=json)")?;

        let lints: Vec<Value> =
            serde_json::from_str(result_json).map_err(|e| format!("Invalid JSON: {e}"))?;

        let categories = categorize_by_type(&lints);
        let by_severity = categorize_by_severity(&lints);
        let by_file = categorize_by_file(&lints);

        Ok(json!({
            "status": "ok",
            "action": "categorize",
            "total_lints": lints.len(),
            "by_type": categories,
            "by_severity": by_severity,
            "by_file": by_file,
        }))
    }

    /// Suggest fixes for lint issues.
    fn action_suggest_fixes(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let result_json = params
            .get("result_json")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: result_json")?;

        let lints: Vec<Value> =
            serde_json::from_str(result_json).map_err(|e| format!("Invalid JSON: {e}"))?;

        let suggestions: Vec<Value> = lints.iter().filter_map(suggest_fix_for_lint).collect();

        let auto_fixable = suggestions
            .iter()
            .filter(|s| s["auto_fixable"].as_bool().unwrap_or(false))
            .count();

        Ok(json!({
            "status": "ok",
            "action": "suggest_fixes",
            "total_issues": lints.len(),
            "auto_fixable": auto_fixable,
            "suggestions": suggestions,
        }))
    }

    /// Calculate a quality score from lint results.
    fn action_score(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let result_json = params.get("result_json");

        let (errors, warnings, notes) = if let Some(json_val) = result_json {
            let lints: Vec<Value> = match json_val {
                Value::Array(arr) => arr.clone(),
                Value::String(s) => serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {e}"))?,
                _ => Vec::new(),
            };
            classify_lints(&lints)
        } else {
            // Run clippy to get fresh results
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let output = Command::new("cargo")
                .args(["clippy", "--message-format=json"])
                .current_dir(path)
                .output()
                .map_err(|e| format!("Failed to run cargo clippy: {e}"))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lints = Vec::new();
            for line in stdout.lines() {
                if let Ok(msg) = serde_json::from_str::<Value>(line) {
                    if msg.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                        if let Some(message) = msg.get("message") {
                            lints.push(message.clone());
                        }
                    }
                }
            }
            classify_lints(&lints)
        };

        // Calculate score (100 = perfect, 0 = terrible)
        let error_penalty = errors.len() as i32 * 15;
        let warning_penalty = warnings.len() as i32 * 3;
        let note_penalty = notes.len() as i32;
        let score = (100 - error_penalty - warning_penalty - note_penalty).max(0);

        let grade = if score >= 95 {
            "A+"
        } else if score >= 90 {
            "A"
        } else if score >= 80 {
            "B"
        } else if score >= 70 {
            "C"
        } else if score >= 60 {
            "D"
        } else {
            "F"
        };

        let recommendations = generate_recommendations(&errors, &warnings);

        Ok(json!({
            "status": "ok",
            "action": "score",
            "quality_score": score,
            "grade": grade,
            "summary": {
                "errors": errors.len(),
                "warnings": warnings.len(),
                "notes": notes.len(),
            },
            "recommendations": recommendations,
        }))
    }

    /// Auto-fix common clippy patterns.
    fn action_auto_fix(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let _result_json = params.get("result_json").and_then(|v| v.as_str());

        // First run clippy --fix for safe auto-fixes
        let output = Command::new("cargo")
            .args(["clippy", "--fix", "--allow-dirty", "--allow-staged"])
            .current_dir(path)
            .output()
            .map_err(|e| format!("Failed to run cargo clippy --fix: {e}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let fixes_applied = stderr
            .lines()
            .filter(|l| l.contains("fixed") || l.contains("Clippy"))
            .count();

        // Analyze remaining issues
        let remaining_output = Command::new("cargo")
            .args(["clippy", "--message-format=json"])
            .current_dir(path)
            .output()
            .map_err(|e| format!("Failed to run cargo clippy: {e}"))?;

        let remaining_stdout = String::from_utf8_lossy(&remaining_output.stdout);
        let mut remaining_lints = Vec::new();
        for line in remaining_stdout.lines() {
            if let Ok(msg) = serde_json::from_str::<Value>(line) {
                if msg.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                    if let Some(message) = msg.get("message") {
                        remaining_lints.push(message.clone());
                    }
                }
            }
        }

        let (errors, warnings, notes) = classify_lints(&remaining_lints);

        // Generate suggestions for remaining non-auto-fixable issues
        let suggestions: Vec<Value> = remaining_lints
            .iter()
            .filter_map(suggest_fix_for_lint)
            .collect();

        Ok(json!({
            "status": if remaining_output.status.success() { "ok" } else { "warnings_remaining" },
            "action": "auto_fix",
            "path": path,
            "fixes_applied": fixes_applied,
            "remaining": {
                "errors": errors.len(),
                "warnings": warnings.len(),
                "notes": notes.len(),
            },
            "remaining_suggestions": suggestions,
        }))
    }
}

// ============================================================================
// Lint Analysis Helpers
// ============================================================================

/// Classify lints into errors, warnings, and notes.
fn classify_lints(lints: &[Value]) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut notes = Vec::new();

    for lint in lints {
        let level = lint.get("level").and_then(|l| l.as_str()).unwrap_or("note");
        match level {
            "error" => errors.push(lint.clone()),
            "warning" => warnings.push(lint.clone()),
            _ => notes.push(lint.clone()),
        }
    }

    (errors, warnings, notes)
}

/// Categorize lints by clippy lint type.
fn categorize_by_type(lints: &[Value]) -> Value {
    let mut categories: HashMap<String, Vec<Value>> = HashMap::new();

    for lint in lints {
        let code = lint
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("unknown");

        // Extract category from lint code (e.g. "clippy::unwrap_used" -> "clippy")
        let category = code.split("::").next().unwrap_or("other").to_string();

        categories.entry(category).or_default().push(json!({
            "code": code,
            "message": lint.get("message").and_then(|m| m.get("0")).and_then(|m| m.as_str()).unwrap_or(""),
            "level": lint.get("level").and_then(|l| l.as_str()).unwrap_or(""),
        }));
    }

    json!(categories)
}

/// Categorize lints by severity level.
fn categorize_by_severity(lints: &[Value]) -> Value {
    let mut by_level: HashMap<String, usize> = HashMap::new();

    for lint in lints {
        let level = lint
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("note")
            .to_string();
        *by_level.entry(level).or_insert(0) += 1;
    }

    json!(by_level)
}

/// Categorize lints by file.
fn categorize_by_file(lints: &[Value]) -> Value {
    let mut by_file: HashMap<String, usize> = HashMap::new();

    for lint in lints {
        if let Some(spans) = lint.get("spans").and_then(|s| s.as_array()) {
            if let Some(first_span) = spans.first() {
                if let Some(file_name) = first_span.get("file_name").and_then(|f| f.as_str()) {
                    *by_file.entry(file_name.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    // Sort by count
    let mut sorted: Vec<(String, usize)> = by_file.into_iter().collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.1));

    json!(sorted)
}

/// Generate a fix suggestion for a lint.
fn suggest_fix_for_lint(lint: &Value) -> Option<Value> {
    let code = lint
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let message = lint
        .get("message")
        .and_then(|m| m.get("0"))
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let (auto_fixable, suggestion) = match code {
        "clippy::unwrap_used" => (
            false,
            "Use .expect(\"descriptive message\") or proper error handling with ? operator",
        ),
        "clippy::expect_used" => (
            false,
            "Use .ok_or_else(|| ...) ? or match for proper error propagation",
        ),
        "clippy::todo" => (
            false,
            "Implement the actual logic or return a proper error",
        ),
        "clippy::unimplemented" => (
            false,
            "Implement the actual logic or use todo!() as a placeholder",
        ),
        "clippy::useless_conversion" => (
            true,
            "Remove the unnecessary .into() or .to_string() call",
        ),
        "clippy::redundant_closure" => (
            true,
            "Replace closure with the function directly: .map(f) instead of .map(|x| f(x))",
        ),
        "clippy::needless_collect" => (
            true,
            "Use iterator directly instead of collecting to Vec and then iterating",
        ),
        "clippy::clone_on_copy" => (
            true,
            "Use .copied() or just dereference instead of .clone() on Copy types",
        ),
        "clippy::let_and_return" => (
            true,
            "Return the expression directly instead of assigning to a variable first",
        ),
        "clippy::match_like_matches_macro" => (
            true,
            "Use matches!() macro instead of match expression",
        ),
        "clippy::if_same_then_else" => (
            false,
            "Both branches of the if expression are identical - simplify the condition",
        ),
        "clippy::manual_map" => (
            true,
            "Use .map() instead of match with Some/None",
        ),
        "clippy::or_fun_call" => (
            true,
            "Use .unwrap_or_else(|| ...) to avoid computing the default value eagerly",
        ),
        "clippy::unnecessary_to_owned" => (
            true,
            "Use &str instead of &String or avoid .to_string() when borrowing is sufficient",
        ),
        "clippy::explicit_iter_loop" => (
            true,
            "Use .iter() directly: for item in &collection instead of for item in collection.iter()",
        ),
        "clippy::needless_borrow" => (
            true,
            "Remove the unnecessary & or ref",
        ),
        _ => {
            // Try to extract suggestion from message
            if message.contains("help:") {
                let help = message
                    .split("help:")
                    .nth(1)
                    .map(|h| h.trim())
                    .unwrap_or("See clippy documentation for this lint");
                (false, help)
            } else {
                return None;
            }
        }
    };

    // Extract file and line info
    let file = lint
        .get("spans")
        .and_then(|s| s.as_array())
        .and_then(|spans| spans.first())
        .and_then(|span| span.get("file_name"))
        .and_then(|f| f.as_str())
        .unwrap_or("unknown");

    let line = lint
        .get("spans")
        .and_then(|s| s.as_array())
        .and_then(|spans| spans.first())
        .and_then(|span| span.get("line_start"))
        .and_then(|l| l.as_u64())
        .unwrap_or(0);

    Some(json!({
        "code": code,
        "auto_fixable": auto_fixable,
        "suggestion": suggestion,
        "file": file,
        "line": line,
        "message": message,
    }))
}

/// Generate recommendations based on lint analysis.
fn generate_recommendations(errors: &[Value], warnings: &[Value]) -> Vec<Value> {
    let mut recommendations = Vec::new();

    let mut lint_counts: HashMap<String, usize> = HashMap::new();
    for lint in warnings.iter().chain(errors.iter()) {
        if let Some(code) = lint
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
        {
            *lint_counts.entry(code.to_string()).or_insert(0) += 1;
        }
    }

    // Generate specific recommendations
    if let Some(count) = lint_counts.get("clippy::unwrap_used") {
        if *count > 3 {
            recommendations.push(json!({
                "priority": "high",
                "category": "error_handling",
                "message": format!("Found {count} unwrap() calls. Consider using proper error handling (? operator) or .expect() with descriptive messages."),
                "lint": "clippy::unwrap_used",
            }));
        }
    }

    if let Some(count) = lint_counts.get("clippy::clone_on_copy") {
        recommendations.push(json!({
            "priority": "low",
            "category": "performance",
            "message": format!("Found {count} unnecessary clone() calls on Copy types. Remove them or use .copied()."),
            "lint": "clippy::clone_on_copy",
        }));
    }

    if let Some(count) = lint_counts.get("clippy::needless_collect") {
        recommendations.push(json!({
            "priority": "medium",
            "category": "performance",
            "message": format!("Found {count} unnecessary .collect() calls. Use iterators directly to avoid allocation."),
            "lint": "clippy::needless_collect",
        }));
    }

    if let Some(count) = lint_counts.get("clippy::redundant_closure") {
        recommendations.push(json!({
            "priority": "low",
            "category": "style",
            "message": format!("Found {count} redundant closures. Pass the function directly instead."),
            "lint": "clippy::redundant_closure",
        }));
    }

    if lint_counts.len() > 10 {
        recommendations.push(json!({
            "priority": "medium",
            "category": "maintenance",
            "message": format!("Found {} different types of clippy warnings. Consider addressing the most common ones first.", lint_counts.len()),
            "lint": "multiple",
        }));
    }

    if !errors.is_empty() {
        recommendations.push(json!({
            "priority": "critical",
            "category": "correctness",
            "message": format!("Found {} compilation errors. These must be fixed before the code can run.", errors.len()),
            "lint": "compiler_error",
        }));
    }

    recommendations
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ClippyLintTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_missing_action() {
        let tool = ClippyLintTool;
        let params = HashMap::new();
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Missing required parameter: action"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("nonexistent"));
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("Unknown action"));
    }

    #[test]
    fn test_classify_lints() {
        let lints = vec![
            json!({"level": "error", "message": {"0": "syntax error"}}),
            json!({"level": "warning", "message": {"0": "unused variable"}}),
            json!({"level": "warning", "message": {"0": "dead code"}}),
            json!({"level": "note", "message": {"0": "consider using"}}),
        ];

        let (errors, warnings, notes) = classify_lints(&lints);
        assert_eq!(errors.len(), 1);
        assert_eq!(warnings.len(), 2);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn test_classify_lints_empty() {
        let (errors, warnings, notes) = classify_lints(&[]);
        assert!(errors.is_empty());
        assert!(warnings.is_empty());
        assert!(notes.is_empty());
    }

    #[test]
    fn test_categorize_by_severity() {
        let lints = vec![
            json!({"level": "error"}),
            json!({"level": "warning"}),
            json!({"level": "warning"}),
        ];
        let result = categorize_by_severity(&lints);
        assert_eq!(result["error"], 1);
        assert_eq!(result["warning"], 2);
    }

    #[test]
    fn test_categorize_by_type() {
        let lints = vec![
            json!({
                "level": "warning",
                "code": {"code": "clippy::unwrap_used"},
                "message": {"0": "used unwrap"}
            }),
            json!({
                "level": "warning",
                "code": {"code": "clippy::clone_on_copy"},
                "message": {"0": "clone on copy"}
            }),
            json!({
                "level": "warning",
                "code": {"code": "unknown_lint"},
                "message": {"0": "unknown"}
            }),
        ];

        let result = categorize_by_type(&lints);
        // All should be categorized under "clippy" or "unknown_lint"
        assert!(result.is_object());
    }

    #[test]
    fn test_categorize_by_file() {
        let lints = vec![
            json!({
                "spans": [{"file_name": "src/main.rs"}]
            }),
            json!({
                "spans": [{"file_name": "src/main.rs"}]
            }),
            json!({
                "spans": [{"file_name": "src/lib.rs"}]
            }),
        ];

        let result = categorize_by_file(&lints);
        let arr = result.as_array().unwrap();
        // Should be sorted by count: main.rs(2) > lib.rs(1)
        assert_eq!(arr[0][0], "src/main.rs");
        assert_eq!(arr[0][1], 2);
    }

    #[test]
    fn test_suggest_fix_unwrap_used() {
        let lint = json!({
            "code": {"code": "clippy::unwrap_used"},
            "message": {"0": "called unwrap on a Result"},
            "spans": [{"file_name": "src/main.rs", "line_start": 10}]
        });

        let suggestion = suggest_fix_for_lint(&lint).unwrap();
        assert_eq!(suggestion["code"], "clippy::unwrap_used");
        assert_eq!(suggestion["auto_fixable"], false);
        assert!(suggestion["suggestion"]
            .as_str()
            .unwrap()
            .contains("expect"));
        assert_eq!(suggestion["file"], "src/main.rs");
        assert_eq!(suggestion["line"], 10);
    }

    #[test]
    fn test_suggest_fix_auto_fixable() {
        let lint = json!({
            "code": {"code": "clippy::clone_on_copy"},
            "message": {"0": "clone on copy type"},
            "spans": [{"file_name": "src/lib.rs", "line_start": 5}]
        });

        let suggestion = suggest_fix_for_lint(&lint).unwrap();
        assert_eq!(suggestion["auto_fixable"], true);
        assert!(suggestion["suggestion"]
            .as_str()
            .unwrap()
            .contains("copied"));
    }

    #[test]
    fn test_suggest_fix_from_message_help() {
        let lint = json!({
            "code": {"code": "unknown_lint"},
            "message": {"0": "some issue\nhelp: try doing this instead"},
            "spans": [{"file_name": "src/test.rs", "line_start": 1}]
        });

        let suggestion = suggest_fix_for_lint(&lint).unwrap();
        assert_eq!(suggestion["auto_fixable"], false);
        assert!(suggestion["suggestion"]
            .as_str()
            .unwrap()
            .contains("try doing this"));
    }

    #[test]
    fn test_suggest_fix_no_info_returns_none() {
        let lint = json!({
            "code": {"code": "unknown"},
            "message": {"0": "some message without help"},
            "spans": []
        });

        // Should return None since no known lint code and no help in message
        let result = suggest_fix_for_lint(&lint);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_recommendations_unwrap_heavy() {
        let errors = vec![];
        let warnings = (0..5)
            .map(|_| {
                json!({
                    "code": {"code": "clippy::unwrap_used"},
                    "message": {"0": "unwrap used"}
                })
            })
            .collect::<Vec<_>>();

        let recs = generate_recommendations(&errors, &warnings);
        assert!(!recs.is_empty());
        assert!(recs.iter().any(|r| r["category"] == "error_handling"));
    }

    #[test]
    fn test_generate_recommendations_errors_critical() {
        let errors = vec![json!({"message": {"0": "compile error"}})];
        let warnings = vec![];

        let recs = generate_recommendations(&errors, &warnings);
        assert!(recs.iter().any(|r| r["priority"] == "critical"));
    }

    #[test]
    fn test_generate_recommendations_multiple_lint_types() {
        let errors = vec![];
        let warnings: Vec<Value> = (0..11)
            .map(|i| {
                json!({
                    "code": {"code": format!("clippy::lint_{}", i)},
                    "message": {"0": "warning"}
                })
            })
            .collect();

        let recs = generate_recommendations(&errors, &warnings);
        // Should recommend addressing multiple lint types
        assert!(recs.iter().any(|r| r["category"] == "maintenance"));
    }

    #[test]
    fn test_generate_recommendations_clone_on_copy() {
        let errors = vec![];
        let warnings = vec![json!({
            "code": {"code": "clippy::clone_on_copy"},
            "message": {"0": "clone on copy"}
        })];

        let recs = generate_recommendations(&errors, &warnings);
        assert!(recs.iter().any(|r| r["category"] == "performance"));
    }

    #[test]
    fn test_generate_recommendations_needless_collect() {
        let errors = vec![];
        let warnings = vec![json!({
            "code": {"code": "clippy::needless_collect"},
            "message": {"0": "needless collect"}
        })];

        let recs = generate_recommendations(&errors, &warnings);
        assert!(recs.iter().any(|r| r["category"] == "performance"));
    }

    #[tokio::test]
    async fn test_categorize_missing_result_json() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("categorize"));
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("result_json"));
    }

    #[tokio::test]
    async fn test_categorize_invalid_json() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("categorize"));
        params.insert("result_json".to_string(), json!("not valid json"));
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_suggest_fixes_missing_result_json() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("suggest_fixes"));
        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_score_with_result_json() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("score"));
        params.insert("result_json".to_string(), json!([]));
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["quality_score"], 100);
        assert_eq!(result["grade"], "A+");
    }

    #[tokio::test]
    async fn test_score_with_warnings() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("score"));
        let lints = vec![
            json!({"level": "warning", "message": {"0": "warn1"}, "code": {"code": "clippy::unwrap_used"}}),
            json!({"level": "warning", "message": {"0": "warn2"}, "code": {"code": "clippy::unwrap_used"}}),
        ];
        params.insert("result_json".to_string(), json!(lints));
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        // Each warning is -3, so 100 - 6 = 94
        assert_eq!(result["quality_score"], 94);
    }

    #[tokio::test]
    async fn test_score_with_errors() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("score"));
        let lints = vec![json!({"level": "error", "message": {"0": "err1"}})];
        params.insert("result_json".to_string(), json!(lints));
        let result = tool.execute(&params).await.unwrap();
        // Each error is -15, so 100 - 15 = 85
        assert_eq!(result["quality_score"], 85);
        assert_eq!(result["grade"], "B");
    }

    #[tokio::test]
    async fn test_score_low_grade() {
        let tool = ClippyLintTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("score"));
        // Many errors to get a low score
        let lints: Vec<Value> = (0..10)
            .map(|_| json!({"level": "error", "message": {"0": "error"}}))
            .collect();
        params.insert("result_json".to_string(), json!(lints));
        let result = tool.execute(&params).await.unwrap();
        // 100 - 10*15 = -50, clamped to 0
        assert_eq!(result["quality_score"], 0);
        assert_eq!(result["grade"], "F");
    }

    #[test]
    fn test_suggest_fix_all_known_lints() {
        let known_lints = [
            "clippy::unwrap_used",
            "clippy::expect_used",
            "clippy::todo",
            "clippy::unimplemented",
            "clippy::useless_conversion",
            "clippy::redundant_closure",
            "clippy::needless_collect",
            "clippy::clone_on_copy",
            "clippy::let_and_return",
            "clippy::match_like_matches_macro",
            "clippy::if_same_then_else",
            "clippy::manual_map",
            "clippy::or_fun_call",
            "clippy::unnecessary_to_owned",
            "clippy::explicit_iter_loop",
            "clippy::needless_borrow",
        ];

        for lint_code in &known_lints {
            let lint = json!({
                "code": {"code": lint_code},
                "message": {"0": "test message"},
                "spans": [{"file_name": "src/test.rs", "line_start": 1}]
            });
            let suggestion = suggest_fix_for_lint(&lint);
            assert!(suggestion.is_some(), "No suggestion for {}", lint_code);
            assert!(!suggestion.as_ref().unwrap()["suggestion"]
                .as_str()
                .unwrap()
                .is_empty());
        }
    }
}
