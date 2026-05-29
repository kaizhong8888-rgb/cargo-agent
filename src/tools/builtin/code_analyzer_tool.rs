//! Code analysis tool: analyzes Rust source code structure, dependencies,
//! complexity, and patterns using regex-based parsing.
//!
//! # Analysis Modes
//!
//! - **structure**: Extract code structure (functions, structs, enums, traits, impls, modules)
//! - **dependencies**: Find use statements and external crate references
//! - **complexity**: Measure function length, nesting depth, cognitive complexity
//! - **patterns**: Find specific code patterns (unsafe, unwrap, todo, etc.)
//! - **summary**: High-level overview of a Rust file or project

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Pre-compiled regex patterns (one-time compile, eliminates all unwrap() calls)
// ============================================================================

// Structure analysis patterns
static RE_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]"#)
        .expect("valid regex")
});
static RE_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});
static RE_ENUM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?enum\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});
static RE_TRAIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?trait\s+([a-zA-Z_][a-zA-Z0-9_]*)"#)
        .expect("valid regex")
});
static RE_IMPL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:unsafe\s+)?impl\s+([a-zA-Z_][a-zA-Z0-9_<>]*(?:\s+for\s+[a-zA-Z_][a-zA-Z0-9_<>]*)?)\s*\{?"#).expect("valid regex")
});
static RE_MOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\{|;)"#)
        .expect("valid regex")
});
static RE_TYPE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?type\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});
static RE_CONST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?const\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});
static RE_STATIC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?static\s+(?:mut\s+)?([a-zA-Z_][a-zA-Z0-9_]*)"#)
        .expect("valid regex")
});
static RE_MACRO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?macro_rules!\s*\(?\s*([a-zA-Z_][a-zA-Z0-9_]*)"#)
        .expect("valid regex")
});

// Dependency analysis patterns
static RE_EXTERN_CRATE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});
static RE_USE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*use\s+([a-zA-Z_][a-zA-Z0-9_:*{}\s]*(?:;|$))"#).expect("valid regex")
});

// Complexity analysis patterns
static RE_FN_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)"#)
        .expect("valid regex")
});
static RE_UNSAFE_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"unsafe\s*\{|#\[allow\(unsafe"#).expect("valid regex"));

// Summary analysis patterns
static RE_FN_SUMMARY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+[a-zA-Z_][a-zA-Z0-9_]*"#)
        .expect("valid regex")
});
static RE_STRUCT_SUMMARY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?struct\s+[a-zA-Z_][a-zA-Z0-9_]*"#).expect("valid regex")
});
static RE_ENUM_SUMMARY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?enum\s+[a-zA-Z_][a-zA-Z0-9_]*"#).expect("valid regex")
});
static RE_TRAIT_SUMMARY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?trait\s+[a-zA-Z_][a-zA-Z0-9_]*"#)
        .expect("valid regex")
});
static RE_IMPL_SUMMARY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*(?:unsafe\s+)?impl\s+"#).expect("valid regex"));
static RE_MOD_SUMMARY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?mod\s+[a-zA-Z_][a-zA-Z0-9_]*"#).expect("valid regex")
});
static RE_TEST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*#\[test\]"#).expect("valid regex"));
static RE_UNSAFE_SUMMARY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"unsafe\s*\{"#).expect("valid regex"));
static RE_DOC_COMMENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*///"#).expect("valid regex"));
static RE_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?m)^\s*//"#).expect("valid regex"));
static RE_BLANK_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?m)^\s*$"#).expect("valid regex"));

// ============================================================================
// CodeAnalyzerTool
// ============================================================================

/// Analyze Rust source code for structure, dependencies, complexity, and patterns.
pub struct CodeAnalyzerTool;

#[async_trait::async_trait]
impl Tool for CodeAnalyzerTool {
    fn name(&self) -> &str {
        "code_analyze"
    }

    fn description(&self) -> &str {
        "Analyze Rust source code structure, dependencies, complexity, and patterns. Supports modes: structure (functions/structs/enums/traits), dependencies (use statements, external crates), complexity (function length, nesting), patterns (unsafe, unwrap, todo), summary (overview)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to a Rust file or directory to analyze".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "mode".to_string(),
                description: "Analysis mode: structure, dependencies, complexity, patterns, or summary (default: structure)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "If true and path is a directory, analyze all .rs files recursively (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_results".to_string(),
                description: "Maximum number of results to return (default: 50)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "detail".to_string(),
                description: "Detail level: brief, normal, full (default: normal)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let mode = params
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("structure");

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        let detail = params
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or("normal");

        let file_path = Path::new(path);

        if !file_path.exists() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        // Collect Rust files
        let mut files: Vec<String> = Vec::new();

        if file_path.is_file() {
            if !path.ends_with(".rs") {
                return Ok(serde_json::json!({
                    "status": "error",
                    "message": format!("Not a Rust file: {path}"),
                }));
            }
            files.push(path.to_string());
        } else if file_path.is_dir() {
            collect_rust_files(file_path, &mut files, recursive, 0)?;
            if files.is_empty() {
                return Ok(serde_json::json!({
                    "status": "error",
                    "message": format!("No Rust files found in: {path}"),
                }));
            }
        }

        // Truncate to max_results
        if files.len() > max_results {
            files.truncate(max_results);
        }

        match mode {
            "structure" => analyze_structure(&files, detail),
            "dependencies" => analyze_dependencies(&files),
            "complexity" => analyze_complexity(&files, detail),
            "patterns" => analyze_patterns(&files),
            "summary" => analyze_summary(&files),
            _ => Ok(serde_json::json!({
                "status": "error",
                "message": format!("Unknown mode: {mode}. Available modes: structure, dependencies, complexity, patterns, summary"),
            })),
        }
    }
}

// ============================================================================
// File Collection
// ============================================================================

/// Recursively collect .rs files from a directory.
fn collect_rust_files(
    dir: &Path,
    files: &mut Vec<String>,
    recursive: bool,
    depth: usize,
) -> Result<(), String> {
    if depth > 20 {
        return Ok(());
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip hidden directories and common non-source dirs
        if path.is_dir() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if recursive {
                collect_rust_files(&path, files, true, depth + 1)?;
            }
        } else if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
            files.push(path.to_string_lossy().to_string());
        }
    }

    Ok(())
}

/// Read a file's content.
fn read_file_content(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
}

// ============================================================================
// Structure Analysis
// ============================================================================

/// Analyze the structure of Rust files: functions, structs, enums, traits, impls, modules.
fn analyze_structure(files: &[String], detail: &str) -> Result<Value, String> {
    let mut results = Vec::new();
    let mut total_counts: HashMap<String, usize> = HashMap::new();

    for file_path in files {
        let content = read_file_content(file_path)?;
        let relative_path = file_path.to_string();

        // Extract items using pre-compiled regex statics
        let functions: Vec<Value> = if detail == "full" {
            RE_FN
                .captures_iter(&content)
                .map(|c| serde_json::json!({ "name": c.get(1).map(|m| m.as_str()).unwrap_or("") }))
                .collect()
        } else {
            RE_FN
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let structs: Vec<Value> = if detail == "full" {
            RE_STRUCT
                .captures_iter(&content)
                .map(|c| serde_json::json!({ "name": c.get(1).map(|m| m.as_str()).unwrap_or("") }))
                .collect()
        } else {
            RE_STRUCT
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let enums: Vec<Value> = if detail == "full" {
            RE_ENUM
                .captures_iter(&content)
                .map(|c| serde_json::json!({ "name": c.get(1).map(|m| m.as_str()).unwrap_or("") }))
                .collect()
        } else {
            RE_ENUM
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let traits: Vec<Value> = if detail == "full" {
            RE_TRAIT
                .captures_iter(&content)
                .map(|c| serde_json::json!({ "name": c.get(1).map(|m| m.as_str()).unwrap_or("") }))
                .collect()
        } else {
            RE_TRAIT
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let impls: Vec<Value> = if detail == "full" {
            RE_IMPL
                .captures_iter(&content)
                .map(
                    |c| serde_json::json!({ "target": c.get(1).map(|m| m.as_str()).unwrap_or("") }),
                )
                .collect()
        } else {
            RE_IMPL
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let modules: Vec<Value> = if detail == "full" {
            RE_MOD
                .captures_iter(&content)
                .map(|c| serde_json::json!({ "name": c.get(1).map(|m| m.as_str()).unwrap_or("") }))
                .collect()
        } else {
            RE_MOD
                .captures_iter(&content)
                .map(|c| serde_json::json!(c.get(1).map(|m| m.as_str()).unwrap_or("")))
                .collect()
        };

        let types = RE_TYPE.captures_iter(&content).count();
        let consts = RE_CONST.captures_iter(&content).count();
        let statics = RE_STATIC.captures_iter(&content).count();
        let macros = RE_MACRO.captures_iter(&content).count();

        // Update total counts
        *total_counts.entry("functions".into()).or_insert(0) += functions.len();
        *total_counts.entry("structs".into()).or_insert(0) += structs.len();
        *total_counts.entry("enums".into()).or_insert(0) += enums.len();
        *total_counts.entry("traits".into()).or_insert(0) += traits.len();
        *total_counts.entry("impls".into()).or_insert(0) += impls.len();
        *total_counts.entry("modules".into()).or_insert(0) += modules.len();
        *total_counts.entry("types".into()).or_insert(0) += types;
        *total_counts.entry("consts".into()).or_insert(0) += consts;
        *total_counts.entry("statics".into()).or_insert(0) += statics;
        *total_counts.entry("macros".into()).or_insert(0) += macros;

        if detail == "brief" {
            results.push(serde_json::json!({
                "file": relative_path,
                "counts": {
                    "functions": functions.len(),
                    "structs": structs.len(),
                    "enums": enums.len(),
                    "traits": traits.len(),
                    "impls": impls.len(),
                    "modules": modules.len(),
                    "types": types,
                    "consts": consts,
                    "statics": statics,
                    "macros": macros,
                },
            }));
        } else {
            results.push(serde_json::json!({
                "file": relative_path,
                "functions": functions,
                "structs": structs,
                "enums": enums,
                "traits": traits,
                "impls": impls,
                "modules": modules,
                "types": types,
                "consts": consts,
                "statics": statics,
                "macros": macros,
            }));
        }
    }

    Ok(serde_json::json!({
        "status": "ok",
        "mode": "structure",
        "total_files": files.len(),
        "total_counts": total_counts,
        "results": results,
    }))
}

// ============================================================================
// Dependencies Analysis
// ============================================================================

/// Analyze dependencies: use statements and external crate references.
fn analyze_dependencies(files: &[String]) -> Result<Value, String> {
    let mut all_extern_crates: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_use_statements: HashMap<String, Vec<String>> = HashMap::new();
    let mut crate_references: HashMap<String, usize> = HashMap::new();

    for file_path in files {
        let content = read_file_content(file_path)?;
        let relative_path = file_path.to_string();

        // Find extern crate declarations using pre-compiled regex
        let extern_crates: Vec<String> = RE_EXTERN_CRATE
            .captures_iter(&content)
            .map(|c| c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default())
            .collect();
        if !extern_crates.is_empty() {
            all_extern_crates.insert(relative_path.clone(), extern_crates);
        }

        // Find use statements using pre-compiled regex
        let use_stmts: Vec<String> = RE_USE
            .captures_iter(&content)
            .map(|c| {
                c.get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if !use_stmts.is_empty() {
            all_use_statements.insert(relative_path.clone(), use_stmts.clone());
        }

        // Extract crate-level references (e.g., "serde_json::")
        for use_stmt in &use_stmts {
            if let Some(first) = use_stmt.split("::").next() {
                let crate_name = first.trim();
                if !crate_name.is_empty()
                    && crate_name != "self"
                    && crate_name != "super"
                    && crate_name != "crate"
                {
                    *crate_references.entry(crate_name.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    // Sort crate references by frequency
    let mut refs: Vec<(String, usize)> = crate_references.into_iter().collect();
    refs.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(serde_json::json!({
        "status": "ok",
        "mode": "dependencies",
        "total_files": files.len(),
        "external_crates": all_extern_crates,
        "use_statements": all_use_statements,
        "crate_references": refs.into_iter().map(|(k, v)| serde_json::json!({ "crate": k, "count": v })).collect::<Vec<Value>>(),
    }))
}

// ============================================================================
// Complexity Analysis
// ============================================================================

/// Analyze code complexity: function length, nesting, etc.
fn analyze_complexity(files: &[String], detail: &str) -> Result<Value, String> {
    let mut file_results = Vec::new();
    let mut total_functions = 0usize;
    let mut total_lines = 0usize;
    let mut long_functions = Vec::new();

    for file_path in files {
        let content = read_file_content(file_path)?;
        let relative_path = file_path.to_string();
        let lines: Vec<&str> = content.lines().collect();
        total_lines += lines.len();

        // Find functions and measure their length using pre-compiled regex
        let fn_positions: Vec<(usize, String)> = RE_FN_START
            .captures_iter(&content)
            .filter_map(|c| {
                let name = c.get(1).map(|m| m.as_str().to_string())?;
                let pos = c
                    .get(0)
                    .map(|m| content[..m.start()].lines().count())
                    .unwrap_or(0);
                Some((pos, name))
            })
            .collect();

        let mut fn_metrics = Vec::new();
        for i in 0..fn_positions.len() {
            let (start_line, name) = &fn_positions[i];
            let end_line = if i + 1 < fn_positions.len() {
                fn_positions[i + 1].0
            } else {
                lines.len()
            };
            let length = end_line - start_line;
            total_functions += 1;

            if detail != "brief" {
                fn_metrics.push(serde_json::json!({
                    "name": name,
                    "start_line": start_line + 1,
                    "end_line": end_line,
                    "length": length,
                    "long": length > 50,
                }));
            }

            if length > 50 {
                long_functions.push(format!("{}:{} ({} lines)", relative_path, name, length));
            }
        }

        // Count unsafe blocks using pre-compiled regex
        let unsafe_count = RE_UNSAFE_BLOCK.find_iter(&content).count();

        // Estimate max nesting depth
        let mut max_nesting = 0usize;
        let mut current_nesting = 0usize;
        for ch in content.chars() {
            match ch {
                '{' | '[' => {
                    current_nesting += 1;
                    max_nesting = max_nesting.max(current_nesting);
                }
                '}' | ']' => {
                    current_nesting = current_nesting.saturating_sub(1);
                }
                _ => {}
            }
        }

        if detail == "brief" {
            file_results.push(serde_json::json!({
                "file": relative_path,
                "functions": fn_positions.len(),
                "total_lines": lines.len(),
                "max_nesting": max_nesting,
                "unsafe_blocks": unsafe_count,
                "long_functions": fn_metrics.iter().filter(|m| m["long"].as_bool().unwrap_or(false)).count(),
            }));
        } else {
            file_results.push(serde_json::json!({
                "file": relative_path,
                "functions": fn_metrics,
                "total_lines": lines.len(),
                "max_nesting": max_nesting,
                "unsafe_blocks": unsafe_count,
            }));
        }
    }

    let avg_fn_length = if total_functions > 0 {
        Some(total_lines / total_functions)
    } else {
        None
    };

    Ok(serde_json::json!({
        "status": "ok",
        "mode": "complexity",
        "total_files": files.len(),
        "total_functions": total_functions,
        "total_lines": total_lines,
        "average_function_length": avg_fn_length,
        "long_functions_count": long_functions.len(),
        "long_functions": long_functions,
        "results": file_results,
    }))
}

// ============================================================================
// Patterns Analysis
// ============================================================================

/// Analyze code patterns: unsafe blocks, unwrap, expect, todo, unimplemented, panic, dbg!, etc.
fn analyze_patterns(files: &[String]) -> Result<Value, String> {
    // Pattern definitions: (key, regex_str, description)
    // Regex strings are compiled per-call since they're user-visible patterns,
    // but we use .ok() to avoid unwrap — invalid regex just yields zero matches.
    let patterns: Vec<(&str, &str, &str)> = vec![
        (
            "unsafe_blocks",
            r#"unsafe\s*\{"#,
            "Unsafe blocks (potential memory safety risks)",
        ),
        (
            "unwrap_calls",
            r#"\.unwrap\(\)"#,
            "unwrap() calls (will panic on None/Err)",
        ),
        (
            "expect_calls",
            r#"\.expect\("#,
            "expect() calls (will panic on None/Err)",
        ),
        (
            "todo_macros",
            r#"(?m)^\s*todo!\("#,
            "todo!() macros (unimplemented code)",
        ),
        (
            "unimplemented",
            r#"unimplemented!\("#,
            "unimplemented!() macros",
        ),
        ("panic_calls", r#"(?m)^\s*panic!\("#, "panic!() calls"),
        (
            "dbg_macros",
            r#"dbg!\("#,
            "dbg!() macros (debugging artifacts)",
        ),
        (
            "allow_attributes",
            r#"#\[allow\([^)]*\]"#,
            "#[allow(...)] attributes (suppressed warnings)",
        ),
        (
            "todo_comments",
            r#"(?i)(?m)^\s*//\s*(TODO|FIXME|HACK|XXX|BUG|WORKAROUND|OPTIMIZE)"#,
            "TODO/FIXME/HACK comments",
        ),
        (
            "unwrap_or",
            r#"\.unwrap_or\("#,
            "unwrap_or() calls (safe fallback)",
        ),
        ("as_ref_calls", r#"\.as_ref\(\)"#, "as_ref() calls"),
        (
            "clone_calls",
            r#"\.clone\(\)"#,
            "clone() calls (potential performance issues)",
        ),
        (
            "box_allocations",
            r#"Box::new\("#,
            "Box::new() heap allocations",
        ),
        (
            "rc_allocations",
            r#"(?:Rc|Arc)::new\("#,
            "Rc/Arc reference counting",
        ),
        (
            "lazy_static",
            r#"lazy_static!\s*\{|LazyLock|LazyCell"#,
            "Lazy static initialization",
        ),
    ];

    let mut file_results = Vec::new();
    let mut total_counts: HashMap<String, usize> = HashMap::new();

    for file_path in files {
        let content = read_file_content(file_path)?;
        let relative_path = file_path.to_string();
        let mut file_patterns = HashMap::new();

        for (key, re_str, _desc) in &patterns {
            if let Ok(re) = Regex::new(re_str) {
                let count = re.find_iter(&content).count();
                if count > 0 {
                    file_patterns.insert(key.to_string(), count);
                    *total_counts.entry(key.to_string()).or_insert(0) += count;
                }
            }
        }

        if !file_patterns.is_empty() {
            file_results.push(serde_json::json!({
                "file": relative_path,
                "patterns": file_patterns,
            }));
        }
    }

    // Sort patterns by total count
    let mut sorted_patterns: Vec<(String, usize, &str)> = patterns
        .iter()
        .map(|(key, _, desc)| {
            let count = total_counts.get(*key).copied().unwrap_or(0);
            (key.to_string(), count, *desc)
        })
        .collect();
    sorted_patterns.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(serde_json::json!({
        "status": "ok",
        "mode": "patterns",
        "total_files": files.len(),
        "pattern_summary": sorted_patterns.into_iter().map(|(key, count, desc)| {
            serde_json::json!({
                "pattern": key,
                "count": count,
                "description": desc,
            })
        }).collect::<Vec<Value>>(),
        "results": file_results,
    }))
}

// ============================================================================
// Summary Analysis
// ============================================================================

/// Generate a high-level summary of Rust files/projects.
fn analyze_summary(files: &[String]) -> Result<Value, String> {
    let mut total_fns = 0usize;
    let mut total_structs = 0usize;
    let mut total_enums = 0usize;
    let mut total_traits = 0usize;
    let mut total_impls = 0usize;
    let mut total_mods = 0usize;
    let mut total_tests = 0usize;
    let mut total_unsafe = 0usize;
    let mut total_lines = 0usize;
    let mut total_code_lines = 0usize;
    let mut total_comment_lines = 0usize;
    let mut total_doc_lines = 0usize;
    let total_blank_lines = 0usize;
    let mut file_details = Vec::new();

    for file_path in files {
        let content = read_file_content(file_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let total_file_lines = lines.len();

        // Count different types of lines using pre-compiled regex statics
        let doc_lines = RE_DOC_COMMENT.find_iter(&content).count();
        let comment_lines = RE_COMMENT.find_iter(&content).count();
        let blank_lines = RE_BLANK_LINE.find_iter(&content).count();

        // Estimate code lines (non-comment, non-blank)
        let code_lines = total_file_lines.saturating_sub(comment_lines + blank_lines);

        total_lines += total_file_lines;
        total_code_lines += code_lines;
        total_comment_lines += comment_lines;
        total_doc_lines += doc_lines;

        let fns = RE_FN_SUMMARY.find_iter(&content).count();
        let structs = RE_STRUCT_SUMMARY.find_iter(&content).count();
        let enums = RE_ENUM_SUMMARY.find_iter(&content).count();
        let traits = RE_TRAIT_SUMMARY.find_iter(&content).count();
        let impls = RE_IMPL_SUMMARY.find_iter(&content).count();
        let mods = RE_MOD_SUMMARY.find_iter(&content).count();
        let tests = RE_TEST.find_iter(&content).count();
        let unsafe_count = RE_UNSAFE_SUMMARY.find_iter(&content).count();

        total_fns += fns;
        total_structs += structs;
        total_enums += enums;
        total_traits += traits;
        total_impls += impls;
        total_mods += mods;
        total_tests += tests;
        total_unsafe += unsafe_count;

        file_details.push(serde_json::json!({
            "file": file_path,
            "lines": total_file_lines,
            "code_lines": code_lines,
            "comment_lines": comment_lines,
            "doc_lines": doc_lines,
            "blank_lines": blank_lines,
            "functions": fns,
            "structs": structs,
            "enums": enums,
            "traits": traits,
            "impls": impls,
            "mods": mods,
            "tests": tests,
            "unsafe_blocks": unsafe_count,
        }));
    }

    Ok(serde_json::json!({
        "status": "ok",
        "mode": "summary",
        "total_files": files.len(),
        "totals": {
            "lines": total_lines,
            "code_lines": total_code_lines,
            "comment_lines": total_comment_lines,
            "doc_lines": total_doc_lines,
            "blank_lines": total_blank_lines,
            "functions": total_fns,
            "structs": total_structs,
            "enums": total_enums,
            "traits": total_traits,
            "impls": total_impls,
            "mods": total_mods,
            "tests": total_tests,
            "unsafe_blocks": total_unsafe,
        },
        "files": file_details,
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeAnalyzerTool));
}
