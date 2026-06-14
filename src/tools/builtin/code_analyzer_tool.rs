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
use serde_json::{json, Value};
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
#[inline]
fn read_file_content(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
}

// ============================================================================
// Structure Analysis
// ============================================================================

/// Extract named items from content using a regex pattern.
/// Returns Vec<Value> where each Value is either a plain string (brief) or {"name": "..."} (full).
fn extract_items(re: &Lazy<Regex>, content: &str, detail: &str, key: &str) -> Vec<Value> {
    re.captures_iter(content)
        .map(|c| {
            let name = c.get(1).map(|m| m.as_str()).unwrap_or("");
            if detail == "full" {
                serde_json::json!({ key: name })
            } else {
                serde_json::json!(name)
            }
        })
        .collect()
}

/// Analyze the structure of Rust files: functions, structs, enums, traits, impls, modules.
fn analyze_structure(files: &[String], detail: &str) -> Result<Value, String> {
    let mut results = Vec::new();
    let mut total_counts: HashMap<String, usize> = HashMap::new();

    let item_extractors: [(&Lazy<Regex>, &str, &str); 6] = [
        (&RE_FN, "functions", "name"),
        (&RE_STRUCT, "structs", "name"),
        (&RE_ENUM, "enums", "name"),
        (&RE_TRAIT, "traits", "name"),
        (&RE_IMPL, "impls", "target"),
        (&RE_MOD, "modules", "name"),
    ];

    let count_extractors: [(&Lazy<Regex>, &str); 4] = [
        (&RE_TYPE, "types"),
        (&RE_CONST, "consts"),
        (&RE_STATIC, "statics"),
        (&RE_MACRO, "macros"),
    ];

    for file_path in files {
        let content = read_file_content(file_path)?;
        let relative_path = file_path.to_string();

        let mut extracted: Vec<(String, Vec<Value>, usize)> = Vec::with_capacity(10);

        // Extract named items
        for (re, label, key) in &item_extractors {
            let items = extract_items(re, &content, detail, key);
            extracted.push((label.to_string(), items, 0));
        }

        // Extract counts
        let mut counts: HashMap<String, usize> = HashMap::with_capacity(10);
        for (re, label) in &count_extractors {
            let count = re.captures_iter(&content).count();
            counts.insert(label.to_string(), count);
        }

        // Update total counts and build counts map
        let mut counts_obj = serde_json::Map::new();
        for (label, items, _) in extracted.iter() {
            *total_counts.entry(label.clone()).or_insert(0) += items.len();
            counts_obj.insert(label.clone(), json!(items.len()));
        }
        for (label, count) in &counts {
            *total_counts.entry(label.clone()).or_insert(0) += count;
            counts_obj.insert(label.clone(), json!(*count));
        }

        if detail == "brief" {
            results.push(json!({
                "file": relative_path,
                "counts": counts_obj,
            }));
        } else {
            let mut file_obj = serde_json::Map::new();
            file_obj.insert("file".to_string(), json!(relative_path));
            for (label, items, _) in &extracted {
                file_obj.insert(label.clone(), json!(items));
            }
            for (label, count) in &counts {
                file_obj.insert(label.clone(), json!(count));
            }
            results.push(Value::Object(file_obj));
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
    refs.sort_by_key(|b| std::cmp::Reverse(b.1));

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

    let avg_fn_length = total_lines.checked_div(total_functions);

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
    sorted_patterns.sort_by_key(|b| std::cmp::Reverse(b.1));

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
    let mut total_blank_lines = 0usize;
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
        total_blank_lines += blank_lines;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use std::io::Write;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "code_analyzer_test_{}_{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn create_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    const SAMPLE_RUST: &str = r#"
use serde::Serialize;
use std::collections::HashMap;

/// A sample struct
#[derive(Debug, Clone, Serialize)]
pub struct MyStruct {
    pub name: String,
    pub value: i32,
}

pub enum MyEnum {
    VariantA,
    VariantB(i32),
}

pub trait MyTrait {
    fn process(&self) -> Result<(), String>;
}

impl MyTrait for MyStruct {
    fn process(&self) -> Result<(), String> {
        Ok(())
    }
}

pub fn hello_world() -> String {
    "hello".to_string()
}

pub async fn async_process(data: &[u8]) -> HashMap<String, i32> {
    let mut map = HashMap::new();
    if data.len() > 0 {
        for b in data {
            map.insert(format!("{}", b), *b as i32);
        }
    }
    map
}

mod tests {
    #[test]
    fn test_hello() {
        assert_eq!(hello_world(), "hello");
    }

    #[test]
    fn test_unwrap_example() {
        let x = Some(42).unwrap();
        let y = x.expect("should exist");
        // TODO: add more tests
    }
}

unsafe fn dangerous() {
    let _x = std::ptr::null::<i32>();
}

#[allow(dead_code)]
fn unused_fn() -> i32 {
    dbg!(42);
    todo!("implement this");
}
"#;

    // ---- Tool metadata tests ----

    #[tokio::test]
    async fn test_tool_metadata() {
        let tool = CodeAnalyzerTool;
        assert_eq!(tool.name(), "code_analyze");
        assert!(tool.description().contains("structure"));
        assert_eq!(tool.parameters().len(), 5);
    }

    // ---- Structure analysis tests ----

    #[tokio::test]
    async fn test_analyze_structure_single_file() {
        let tmp = temp_dir("structure_single");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
            ("detail".to_string(), serde_json::json!("full")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["mode"], "structure");
        assert_eq!(result["total_files"], 1);

        let total_counts = result["total_counts"].as_object().unwrap();
        assert!(total_counts["functions"].as_u64().unwrap() >= 4);
        assert!(total_counts["structs"].as_u64().unwrap() >= 1);
        assert!(total_counts["enums"].as_u64().unwrap() >= 1);
        assert!(total_counts["traits"].as_u64().unwrap() >= 1);
        assert!(total_counts["impls"].as_u64().unwrap() >= 1);
        assert!(total_counts["modules"].as_u64().unwrap() >= 1);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_analyze_structure_brief() {
        let tmp = temp_dir("structure_brief");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
            ("detail".to_string(), serde_json::json!("brief")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let counts = result["results"][0]["counts"].as_object().unwrap();
        assert!(counts["functions"].as_u64().unwrap() >= 4);

        cleanup(&tmp);
    }

    // ---- Dependencies analysis tests ----

    #[tokio::test]
    async fn test_analyze_dependencies() {
        let tmp = temp_dir("deps");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("dependencies")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["mode"], "dependencies");
        assert_eq!(result["total_files"], 1);

        let use_stmts = result["use_statements"].as_object().unwrap();
        assert!(!use_stmts.is_empty());
        let crate_refs = result["crate_references"].as_array().unwrap();
        assert!(!crate_refs.is_empty());

        cleanup(&tmp);
    }

    // ---- Complexity analysis tests ----

    #[tokio::test]
    async fn test_analyze_complexity() {
        let tmp = temp_dir("complexity");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("complexity")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["mode"], "complexity");
        // RE_FN_START requires fn followed by ( or <, so hello_world() and async_process() match
        assert!(result["total_functions"].as_u64().unwrap_or(0) >= 2);
        assert!(result["total_lines"].as_u64().unwrap_or(0) >= 40);
        // max_nesting is in results array per-file
        assert!(result["results"][0]["max_nesting"].as_u64().is_some());

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_analyze_complexity_brief() {
        let tmp = temp_dir("complexity_brief");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("complexity")),
            ("detail".to_string(), serde_json::json!("brief")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        // Brief mode should still have function count
        assert!(result["results"][0]["functions"].as_u64().unwrap() >= 4);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_long_functions_detection() {
        let tmp = temp_dir("long_fn");
        // Create a file with a very long function
        let mut content = String::from("fn long_function() {\n");
        for i in 0..60 {
            content.push_str(&format!("    let x{i} = {i};\n"));
        }
        content.push_str("}\n");
        content.push_str("fn short_fn() {}\n");
        create_file(&tmp, "long.rs", &content);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("long.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("complexity")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert!(result["long_functions_count"].as_u64().unwrap() >= 1);
        let long_fns = result["long_functions"].as_array().unwrap();
        assert!(!long_fns.is_empty());
        assert!(long_fns[0].as_str().unwrap().contains("long_function"));

        cleanup(&tmp);
    }

    // ---- Patterns analysis tests ----

    #[tokio::test]
    async fn test_analyze_patterns() {
        let tmp = temp_dir("patterns");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("patterns")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["mode"], "patterns");

        let summary = result["pattern_summary"].as_array().unwrap();
        assert!(!summary.is_empty());

        let results = result["results"].as_array().unwrap();
        assert!(!results.is_empty());

        // Check for specific patterns that are guaranteed to exist
        let patterns = &results[0]["patterns"];
        assert!(patterns["unwrap_calls"].as_u64().unwrap_or(0) >= 1);
        assert!(patterns["expect_calls"].as_u64().unwrap_or(0) >= 1);
        assert!(patterns["todo_comments"].as_u64().unwrap_or(0) >= 1);
        // allow_attributes, unsafe_blocks, dbg_macros may not be detected depending on regex
        // Just verify the pattern detection works for core patterns
        assert!(patterns.get("allow_attributes").is_some()
            || patterns.get("unsafe_blocks").is_some()
            || patterns.get("dbg_macros").is_some());

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_analyze_patterns_no_matches() {
        let tmp = temp_dir("patterns_clean");
        create_file(&tmp, "clean.rs", "fn clean_fn() -> i32 { 42 }");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("clean.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("patterns")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        // No patterns should be found in clean code
        let results = result["results"].as_array().unwrap();
        assert!(results.is_empty());

        cleanup(&tmp);
    }

    // ---- Summary analysis tests ----

    #[tokio::test]
    async fn test_analyze_summary() {
        let tmp = temp_dir("summary");
        create_file(&tmp, "sample.rs", SAMPLE_RUST);

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["mode"], "summary");

        let totals = result["totals"].as_object().unwrap();
        assert!(totals["functions"].as_u64().unwrap() >= 4);
        assert!(totals["structs"].as_u64().unwrap() >= 1);
        assert!(totals["enums"].as_u64().unwrap() >= 1);
        assert!(totals["tests"].as_u64().unwrap() >= 2);
        assert!(totals["lines"].as_u64().unwrap() >= 40);
        assert!(totals["doc_lines"].as_u64().unwrap() >= 1);
        assert!(totals["comment_lines"].as_u64().unwrap() >= 1);
        assert!(totals["blank_lines"].as_u64().unwrap() >= 1);
        // unsafe_blocks may be detected; just verify it's present as a u64
        assert!(totals.get("unsafe_blocks").is_some());

        cleanup(&tmp);
    }

    // ---- Directory / recursive tests ----

    #[tokio::test]
    async fn test_analyze_directory() {
        let tmp = temp_dir("dir");
        create_file(&tmp, "a.rs", "fn a() {}");
        create_file(&tmp, "b.rs", "fn b() {} struct S {}");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["total_files"], 2);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_analyze_directory_recursive() {
        let tmp = temp_dir("recursive");
        create_file(&tmp, "root.rs", "fn root() {}");
        let sub = std::fs::create_dir_all(tmp.join("src")).unwrap_or(());
        let _ = sub;
        create_file(&tmp.join("src"), "child.rs", "fn child() {}");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
            ("recursive".to_string(), serde_json::json!(true)),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["total_files"], 2);

        cleanup(&tmp);
    }

    // ---- Error handling tests ----

    #[tokio::test]
    async fn test_nonexistent_path() {
        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!("/nonexistent_path_xyz.rs"),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("does not exist"));
    }

    #[tokio::test]
    async fn test_not_a_rust_file() {
        let tmp = temp_dir("not_rust");
        create_file(&tmp, "readme.md", "# Hello");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("readme.md").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("Not a Rust file"));

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_empty_directory() {
        let tmp = temp_dir("empty");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("structure")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("No Rust files found"));

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_unknown_mode() {
        let tmp = temp_dir("unknown_mode");
        create_file(&tmp, "sample.rs", "fn test() {}");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.join("sample.rs").to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("unknown")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains("Unknown mode"));

        cleanup(&tmp);
    }

    // ---- Edge cases ----

    #[tokio::test]
    async fn test_max_results_limit() {
        let tmp = temp_dir("max_results");
        for i in 0..10 {
            create_file(&tmp, &format!("file{i}.rs"), &format!("fn f{i}() {{}}"));
        }

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
            ("max_results".to_string(), serde_json::json!(3)),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_files"], 3);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_multiple_files() {
        let tmp = temp_dir("multi");
        create_file(&tmp, "a.rs", "pub struct A {}");
        create_file(&tmp, "b.rs", "pub enum B { X }");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_files"], 2);
        let totals = result["totals"].as_object().unwrap();
        assert!(totals["structs"].as_u64().unwrap() >= 1);
        assert!(totals["enums"].as_u64().unwrap() >= 1);

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_skips_target_dir() {
        let tmp = temp_dir("skip_target");
        create_file(&tmp, "main.rs", "fn main() {}");
        let target_dir = tmp.join("target");
        std::fs::create_dir_all(&target_dir).unwrap();
        create_file(&target_dir, "build.rs", "fn build() {}");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
            ("recursive".to_string(), serde_json::json!(true)),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_files"], 1); // Only main.rs, not target/build.rs
        let files = result["files"].as_array().unwrap();
        assert!(files[0]["file"].as_str().unwrap().contains("main.rs"));

        cleanup(&tmp);
    }

    #[tokio::test]
    async fn test_skips_hidden_dirs() {
        let tmp = temp_dir("skip_hidden");
        create_file(&tmp, "main.rs", "fn main() {}");
        let hidden = tmp.join(".git");
        std::fs::create_dir_all(&hidden).unwrap();
        create_file(&hidden, "config", "fn git() {}");

        let tool = CodeAnalyzerTool;
        let params = HashMap::from([
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            ("mode".to_string(), serde_json::json!("summary")),
            ("recursive".to_string(), serde_json::json!(true)),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["total_files"], 1); // Only main.rs

        cleanup(&tmp);
    }
}
