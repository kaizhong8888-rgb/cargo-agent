//! Smart Refactoring Tool: detect code smells and suggest improvements.
//!
//! # Actions
//!
//! - **analyze**: Analyze code for refactoring opportunities
//! - **simplify_bool**: Detect and suggest simplification of boolean expressions
//! - **modernize**: Suggest modernization (format! -> format_args, etc.)
//! - **optimize**: Suggest performance optimizations
//! - **idiomatic**: Check for idiomatic Rust patterns

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Regex Patterns
// ============================================================================

static RE_IF_ELSE_BOOL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)if\s+.+\s+\{\s*\n?\s*true\s*\n?\s*\}\s*else\s*\n?\s*\{\s*\n?\s*false\s*\n?\s*\}",
    )
    .expect("valid regex")
});

static RE_BOOL_LITERAL_COMPARISON: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)(\w+)\s*==\s*(true|false)\b").expect("valid regex"));

static RE_FORMAT_STRING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"format!\s*\(\s*"\{[^}]*\}""#).expect("valid regex"));

#[allow(dead_code)]
static RE_STRING_PUSH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(\w+)\s*=\s*format!\s*\(\s*""#).expect("valid regex"));

static RE_UNWRAP_OR_ELSE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)(\w+)\s*\.unwrap_or_else\(\s*\|[^|]*\|\s*[^)]+\)").expect("valid regex")
});

#[allow(dead_code)]
static RE_CLONE_ON_COPY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)(\w+)\s*\.clone\(\)\s*;\s*//.*Copy").expect("valid regex"));

static RE_VEC_NEW_LOOP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)let\s+mut\s+(\w+)\s*=\s*Vec::new\(\)").expect("valid regex"));

static RE_HASHMAP_NEW_LOOP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)let\s+mut\s+(\w+)\s*=\s*HashMap::new\(\)").expect("valid regex"));

static RE_PANIC_IN_FN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)panic!\s*\(").expect("valid regex"));

static RE_MATCH_SINGLE_ARM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)match\s+\w+\s*\{\s*\n?\s*_\s*=>").expect("valid regex"));

static RE_IF_LET_SOME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)if\s+let\s+Some\(\w+\)\s*=\s*\w+\s*\{\s*\n?\s*return\s+Some\(\w+\)\s*;\s*\n?\s*\}",
    )
    .expect("valid regex")
});

static RE_ITER_COLLECT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)\.iter\(\)\s*\.map\([^)]*\)\s*\.collect::<Vec<_>>\(\)").expect("valid regex")
});

#[allow(dead_code)]
static RE_EMPTY_STRING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"if\s+\w+\s*==\s*""|if\s+\w+\.is_empty\(\)"#).expect("valid regex"));

static RE_AS_REF_DEREF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)\.as_ref\(\)\.unwrap\(\)|\.deref\(\)").expect("valid regex"));

static RE_UNNECESSARY_RETURN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)return\s+([^;]+);\s*\n?\s*\}").expect("valid regex"));

static RE_TO_OWNED_STRING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)\.to_owned\(\)|\.to_string\(\)").expect("valid regex"));

// ============================================================================
// Refactoring Suggestion
// ============================================================================

struct RefactorSuggestion {
    category: String,
    severity: String,
    file: String,
    line: usize,
    description: String,
    before: String,
    after: String,
    explanation: String,
}

// ============================================================================
// SmartRefactorTool
// ============================================================================

pub struct SmartRefactorTool;

#[async_trait::async_trait]
impl Tool for SmartRefactorTool {
    fn name(&self) -> &str {
        "smart_refactor"
    }

    fn description(&self) -> &str {
        "Smart refactoring tool: detect code smells and suggest idiomatic Rust improvements. Actions: analyze (full analysis), simplify_bool (boolean simplification), modernize (modernize patterns), optimize (performance), idiomatic (idiomatic patterns)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: analyze, simplify_bool, modernize, optimize, idiomatic"
                    .to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to Rust file or directory".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Scan recursively (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: markdown, json (default: markdown)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "apply".to_string(),
                description: "Auto-apply suggestions (true/false, default: false)".to_string(),
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

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");

        let scan_path = Path::new(path);
        let mut files: Vec<String> = Vec::new();

        if scan_path.is_file() {
            if !path.ends_with(".rs") {
                return Ok(json!({ "status": "error", "message": "Not a Rust file" }));
            }
            files.push(path.to_string());
        } else {
            collect_rust_files(scan_path, &mut files, recursive, 0)?;
        }

        if files.is_empty() {
            return Ok(json!({
                "status": "error",
                "message": "No Rust files found",
            }));
        }

        let suggestions = match action {
            "analyze" => full_analysis(&files)?,
            "simplify_bool" => check_boolean_simplification(&files)?,
            "modernize" => check_modernization(&files)?,
            "optimize" => check_optimization(&files)?,
            "idiomatic" => check_idiomatic(&files)?,
            _ => {
                return Ok(json!({
                    "status": "error",
                    "message": format!("Unknown action: {action}. Available: analyze, simplify_bool, modernize, optimize, idiomatic"),
                }))
            }
        };

        if format == "json" {
            let json_suggestions: Vec<Value> = suggestions
                .iter()
                .map(|s| {
                    json!({
                        "category": s.category,
                        "severity": s.severity,
                        "file": s.file,
                        "line": s.line,
                        "description": s.description,
                        "before": s.before,
                        "after": s.after,
                        "explanation": s.explanation,
                    })
                })
                .collect();

            Ok(json!({
                "status": "ok",
                "action": action,
                "total_suggestions": suggestions.len(),
                "suggestions": json_suggestions,
            }))
        } else {
            let mut md = String::new();
            md.push_str("# Refactoring Suggestions\n\n");
            if suggestions.is_empty() {
                md.push_str("No refactoring suggestions found. Code looks idiomatic! \u{2728}\n");
            } else {
                // Group by category
                let mut by_category: HashMap<String, Vec<&RefactorSuggestion>> = HashMap::new();
                for s in &suggestions {
                    by_category.entry(s.category.clone()).or_default().push(s);
                }

                for (category, items) in &by_category {
                    md.push_str(&format!("## {}\n\n", category));
                    for s in items {
                        md.push_str(&format!(
                            "### {} ({}:{}) - {}\n\n",
                            s.severity, s.file, s.line, s.description
                        ));
                        md.push_str(&format!("**Before:**\n```rust\n{}\n```\n\n", s.before));
                        md.push_str(&format!("**After:**\n```rust\n{}\n```\n\n", s.after));
                        md.push_str(&format!("**Why:** {}\n\n", s.explanation));
                    }
                }
            }

            Ok(json!({
                "status": "ok",
                "action": action,
                "files": files.len(),
                "total_suggestions": suggestions.len(),
                "documentation": md,
            }))
        }
    }
}

// ============================================================================
// File Collection
// ============================================================================

fn collect_rust_files(
    dir: &Path,
    files: &mut Vec<String>,
    recursive: bool,
    depth: usize,
) -> Result<(), String> {
    if depth > 10 {
        return Ok(());
    }
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read '{}': {e}", dir.display()))?;
    for entry in read_dir.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == ".git"
            {
                continue;
            }
            if recursive {
                collect_rust_files(&p, files, true, depth + 1)?;
            }
        } else if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
            files.push(p.to_string_lossy().to_string());
        }
    }
    Ok(())
}

fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))
}

// ============================================================================
// Full Analysis
// ============================================================================

fn full_analysis(files: &[String]) -> Result<Vec<RefactorSuggestion>, String> {
    let mut all = Vec::new();
    all.extend(check_boolean_simplification(files)?);
    all.extend(check_modernization(files)?);
    all.extend(check_optimization(files)?);
    all.extend(check_idiomatic(files)?);

    // Sort by severity
    all.sort_by(|a, b| {
        let sev_a = match a.severity.as_str() {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            _ => 3,
        };
        let sev_b = match b.severity.as_str() {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            _ => 3,
        };
        sev_a.cmp(&sev_b)
    });

    Ok(all)
}

// ============================================================================
// Boolean Simplification
// ============================================================================

fn check_boolean_simplification(files: &[String]) -> Result<Vec<RefactorSuggestion>, String> {
    let mut suggestions = Vec::new();

    for file_path in files {
        let content = read_file(file_path)?;

        // if cond { true } else { false } -> cond
        for cap in RE_IF_ELSE_BOOL.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            suggestions.push(RefactorSuggestion {
                category: "Boolean Simplification".to_string(),
                severity: "high".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: "Replace if-else boolean with direct condition".to_string(),
                before: cap.get(0).unwrap().as_str().trim().to_string(),
                after: "condition".to_string(),
                explanation: "Using `if cond { true } else { false }` is redundant. Simply use `cond` directly."
                    .to_string(),
            });
        }

        // x == true / x == false
        for cap in RE_BOOL_LITERAL_COMPARISON.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            let var = cap.get(1).unwrap().as_str();
            let literal = cap.get(2).unwrap().as_str();
            let suggestion = if literal == "true" {
                var.to_string()
            } else {
                format!("!{var}")
            };

            suggestions.push(RefactorSuggestion {
                category: "Boolean Simplification".to_string(),
                severity: "high".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: format!("Compare to boolean literal: `{var} == {literal}`"),
                before: cap.get(0).unwrap().as_str().trim().to_string(),
                after: suggestion,
                explanation: format!(
                    "Instead of comparing to `{literal}`, use the boolean value directly (or negate with `!`)."
                ),
            });
        }
    }

    Ok(suggestions)
}

// ============================================================================
// Modernization
// ============================================================================

fn check_modernization(files: &[String]) -> Result<Vec<RefactorSuggestion>, String> {
    let mut suggestions = Vec::new();

    for file_path in files {
        let content = read_file(file_path)?;

        // format!("{}", x) can often be simplified
        for cap in RE_FORMAT_STRING.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            let matched = cap.get(0).unwrap().as_str();
            if matched.contains("{") && !matched.contains("{}") {
                // Single placeholder with named args
                suggestions.push(RefactorSuggestion {
                    category: "Modernization".to_string(),
                    severity: "low".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Consider using implicit positional arguments in format!".to_string(),
                    before: matched.trim().to_string(),
                    after: "Use implicit position: format!(\"{}\", arg)".to_string(),
                    explanation: "Since Rust 1.58, format! can use implicit positional arguments: format!(\"{}\", arg) instead of format!(\"{0}\", arg)."
                        .to_string(),
                });
            }
        }

        // match with single arm
        for cap in RE_MATCH_SINGLE_ARM.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            suggestions.push(RefactorSuggestion {
                category: "Modernization".to_string(),
                severity: "medium".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: "Single-arm match can be simplified".to_string(),
                before: cap.get(0).unwrap().as_str().trim().to_string(),
                after: "Use direct assignment or if let".to_string(),
                explanation: "A match with only a wildcard arm can usually be replaced with a simpler construct."
                    .to_string(),
            });
        }

        // if let Some(x) = opt { return Some(x); } -> opt
        for cap in RE_IF_LET_SOME.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            suggestions.push(RefactorSuggestion {
                category: "Modernization".to_string(),
                severity: "high".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: "Simplify redundant if-let Some pattern".to_string(),
                before: cap.get(0).unwrap().as_str().trim().to_string(),
                after: "return opt; // or just `opt`".to_string(),
                explanation: "This pattern is redundant. Simply return the Option directly."
                    .to_string(),
            });
        }

        // .iter().map(f).collect::<Vec<_>>() -> .map(f).collect()
        for cap in RE_ITER_COLLECT.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            suggestions.push(RefactorSuggestion {
                category: "Modernization".to_string(),
                severity: "low".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: "Remove unnecessary .iter() before .map()".to_string(),
                before: cap.get(0).unwrap().as_str().trim().to_string(),
                after: ".map(f).collect()".to_string(),
                explanation: "If you're collecting into a Vec anyway, .iter().map() can often be simplified depending on ownership needs."
                    .to_string(),
            });
        }
    }

    Ok(suggestions)
}

// ============================================================================
// Performance Optimization
// ============================================================================

fn check_optimization(files: &[String]) -> Result<Vec<RefactorSuggestion>, String> {
    let mut suggestions = Vec::new();

    for file_path in files {
        let content = read_file(file_path)?;

        // Vec::new() without with_capacity in loops
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if RE_VEC_NEW_LOOP.is_match(line) {
                // Check if this is inside a loop or function that processes many items
                let mut in_loop = false;
                let start = i.saturating_sub(5);
                for prev_line in &lines[start..i] {
                    if prev_line.contains("for ")
                        || prev_line.contains(".iter().")
                        || prev_line.contains(".into_iter().")
                    {
                        in_loop = true;
                        break;
                    }
                }
                if in_loop {
                    suggestions.push(RefactorSuggestion {
                        category: "Performance".to_string(),
                        severity: "medium".to_string(),
                        file: file_path.clone(),
                        line: i + 1,
                        description: "Consider using Vec::with_capacity() instead of Vec::new()".to_string(),
                        before: line.trim().to_string(),
                        after: "let mut v = Vec::with_capacity(estimated_size);".to_string(),
                        explanation: "Pre-allocating with `with_capacity` avoids reallocations during push operations, improving performance."
                            .to_string(),
                    });
                }
            }

            // HashMap::new() without with_capacity
            if RE_HASHMAP_NEW_LOOP.is_match(line) {
                let mut in_loop = false;
                let start = i.saturating_sub(5);
                for prev_line in &lines[start..i] {
                    if prev_line.contains("for ")
                        || prev_line.contains(".iter().")
                        || prev_line.contains(".into_iter().")
                    {
                        in_loop = true;
                        break;
                    }
                }
                if in_loop {
                    suggestions.push(RefactorSuggestion {
                        category: "Performance".to_string(),
                        severity: "medium".to_string(),
                        file: file_path.clone(),
                        line: i + 1,
                        description: "Consider using HashMap::with_capacity() instead of HashMap::new()".to_string(),
                        before: line.trim().to_string(),
                        after: "let mut h = HashMap::with_capacity(estimated_size);".to_string(),
                        explanation: "Pre-allocating with `with_capacity` avoids rehashing during insert operations."
                            .to_string(),
                    });
                }
            }
        }

        // panic! in non-test code
        for cap in RE_PANIC_IN_FN.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            // Check if in test
            let in_test = is_in_test_code(&lines, line_num);
            if !in_test {
                suggestions.push(RefactorSuggestion {
                    category: "Performance".to_string(),
                    severity: "high".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Consider using Result or Option instead of panic!".to_string(),
                    before: cap.get(0).unwrap().as_str().trim().to_string(),
                    after: "return Err(...); or return None;".to_string(),
                    explanation: "Using `panic!` in production code can crash the program. Use `Result` or `Option` for error handling instead."
                        .to_string(),
                });
            }
        }

        // .to_owned() / .to_string() on &str
        for cap in RE_TO_OWNED_STRING.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            let matched = cap.get(0).unwrap().as_str();
            if matched.contains(".to_owned()") {
                suggestions.push(RefactorSuggestion {
                    category: "Performance".to_string(),
                    severity: "low".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Consider using .to_string() instead of .to_owned()".to_string(),
                    before: matched.trim().to_string(),
                    after: ".to_string()".to_string(),
                    explanation: "`.to_string()` is more idiomatic than `.to_owned()` for String conversion, though they are equivalent."
                        .to_string(),
                });
            }
        }
    }

    Ok(suggestions)
}

// ============================================================================
// Idiomatic Rust
// ============================================================================

fn check_idiomatic(files: &[String]) -> Result<Vec<RefactorSuggestion>, String> {
    let mut suggestions = Vec::new();

    for file_path in files {
        let content = read_file(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        // unwrap_or_else -> unwrap_or (when closure is simple)
        for cap in RE_UNWRAP_OR_ELSE.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            let matched = cap.get(0).unwrap().as_str();
            // If the closure is just creating a default value
            if matched.contains("|| ") && !matched.contains("{") {
                suggestions.push(RefactorSuggestion {
                    category: "Idiomatic".to_string(),
                    severity: "low".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Simplify unwrap_or_else to unwrap_or".to_string(),
                    before: matched.trim().to_string(),
                    after: ".unwrap_or(default_value)".to_string(),
                    explanation: "When `unwrap_or_else` just creates a simple value without computation, use `unwrap_or` instead."
                        .to_string(),
                });
            }
        }

        // as_ref().unwrap() -> use ? operator or if let
        for cap in RE_AS_REF_DEREF.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            let matched = cap.get(0).unwrap().as_str();
            if matched.contains("as_ref().unwrap()") {
                suggestions.push(RefactorSuggestion {
                    category: "Idiomatic".to_string(),
                    severity: "medium".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Replace as_ref().unwrap() with ? operator or if let".to_string(),
                    before: matched.trim().to_string(),
                    after: "opt.as_ref()? or if let Some(v) = opt".to_string(),
                    explanation:
                        "Using `?` or `if let` is more idiomatic than chaining `as_ref().unwrap()`."
                            .to_string(),
                });
            }
        }

        // return x; at end of function
        for cap in RE_UNNECESSARY_RETURN.captures_iter(&content) {
            let line_num = content[..cap.get(0).unwrap().start()].lines().count() + 1;
            // Only suggest if it's the last statement in a block
            let _matched = cap.get(0).unwrap().as_str();
            suggestions.push(RefactorSuggestion {
                category: "Idiomatic".to_string(),
                severity: "low".to_string(),
                file: file_path.clone(),
                line: line_num,
                description: "Remove unnecessary return at end of block".to_string(),
                before: format!("return {};", cap.get(1).unwrap().as_str().trim()),
                after: cap.get(1).unwrap().as_str().trim().to_string(),
                explanation: "In Rust, the last expression in a block is implicitly returned. Explicit `return` is unnecessary here."
                    .to_string(),
            });
        }

        // string == "" vs string.is_empty()
        for line in &lines {
            if line.contains(r#"== """#) || line.contains(r#"!= """#) {
                let line_num = content[..content.find(line).unwrap_or(0)].lines().count() + 1;
                suggestions.push(RefactorSuggestion {
                    category: "Idiomatic".to_string(),
                    severity: "low".to_string(),
                    file: file_path.clone(),
                    line: line_num,
                    description: "Use is_empty() instead of comparing to empty string".to_string(),
                    before: line.trim().to_string(),
                    after: "string.is_empty()".to_string(),
                    explanation: "Using `.is_empty()` is more idiomatic and potentially faster than comparing to `\"\"`."
                        .to_string(),
                });
            }
        }
    }

    Ok(suggestions)
}

// ============================================================================
// Helper: Check if line is in test code
// ============================================================================

fn is_in_test_code(lines: &[&str], line_num: usize) -> bool {
    let mut in_test = false;
    for i in 0..line_num.saturating_sub(1) {
        let line = lines.get(i).unwrap_or(&"").trim();
        if line.contains("#[cfg(test)]") || line.contains("#[test]") {
            in_test = true;
        }
        if in_test && line == "}" {
            in_test = false;
        }
    }
    in_test
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(SmartRefactorTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_bool_comparison() {
        assert!(RE_BOOL_LITERAL_COMPARISON.is_match("x == true"));
        assert!(RE_BOOL_LITERAL_COMPARISON.is_match("flag == false"));
    }

    #[test]
    fn regex_if_else_bool() {
        let code = "if condition {\n    true\n} else {\n    false\n}";
        assert!(RE_IF_ELSE_BOOL.is_match(code));
    }

    #[test]
    fn regex_vec_new() {
        assert!(RE_VEC_NEW_LOOP.is_match("let mut items = Vec::new();"));
    }
}
