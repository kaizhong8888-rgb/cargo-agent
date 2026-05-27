//! Code Review Tool: performs comprehensive static analysis of Rust source code.
//!
//! # Checks Performed
//!
//! - **unsafe**: Safe Rust violations, unsafe blocks, raw pointer usage
//! - **error_handling**: unwrap/expect/panic usage, ignored Result types
//! - **performance**: Clone(), large heap allocations, unnecessary conversions
//! - **style**: Naming conventions, function length, nesting depth, formatting
//! - **safety**: Integer overflow, transmute, null pointer dereference risks
//! - **correctness**: todo!, unimplemented!, unreachable!, non-exhaustive patterns
//! - **concurrency**: Shared mutable state without sync, Send/Sync violations
//! - **documentation**: Missing docs on public items, incomplete doc comments
//! - **naming**: Naming convention violations (CamelCase types, SCREAMING_CASE constants, snake_case vars)
//! - **async**: Async-specific pitfalls (blocking calls in async, holding locks across .await)
//! - **security**: SQL injection, command injection, hardcoded secrets, path traversal, missing SAFETY comments
//! - **complexity**: Cyclomatic complexity, excessive parameters, large files
//! - **testing**: Test functions without assertions, overly long tests
//! - **debug**: Residual dbg!(), println!() in library code
//!
//! All regex patterns are compiled once using `once_cell::sync::Lazy` for optimal performance.
//! Thresholds (function length, nesting depth, line length) are user-configurable.

use crate::tools::builtin::config_store::ConfigStore;
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Namespace prefix for saved code_review config profiles in the config store.
const CONFIG_NAMESPACE: &str = "code_review_config:";

// ============================================================================
// Global Compiled Regex Patterns (compiled once at startup)
// ============================================================================

// Unsafe code patterns
static RE_UNSAFE_BLOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"unsafe\s*\{").unwrap());
static RE_UNSAFE_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*unsafe\s+(?:extern\s+)?fn\s+").unwrap());
static RE_UNSAFE_TRAIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*unsafe\s+trait\s+").unwrap());
static RE_PTR_DEREF: Lazy<Regex> = Lazy::new(|| Regex::new(r"\*\s*(?:const|mut)\s+").unwrap());
static RE_TRANSMUTE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?:std::)?mem::transmute\b").unwrap());

// Error handling patterns
static RE_UNWRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\w+)\.unwrap\s*\(\s*\)").unwrap());
static RE_EXPECT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\w+)\.expect\s*\(").unwrap());
static RE_PANIC: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*panic!\s*\(").unwrap());
static RE_IGNORE_RESULT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*let\s+_\s*=\s*.+;$").unwrap());
static RE_WRITELN_RESULT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(?:write!|writeln!)\s*\([^)]*\)\s*;").unwrap());

// Performance patterns
static RE_CLONE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\w+)\.clone\s*\(\s*\)").unwrap());
static RE_BOX_NEW: Lazy<Regex> = Lazy::new(|| Regex::new(r"Box::new\s*\(").unwrap());
static RE_VEC_CAPACITY: Lazy<Regex> = Lazy::new(|| Regex::new(r"Vec::with_capacity\s*\(\s*(\d+)\s*\)").unwrap());
static RE_COLLECT_VEC: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.collect::<Vec<_>>\s*\(\)").unwrap());

// Style patterns
static RE_FN_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap());
static RE_NON_SNAKE_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Z][a-zA-Z0-9_]*)").unwrap());
static RE_TODO_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(?m)^\s*//\s*(TODO|FIXME|HACK|XXX|BUG|WORKAROUND)").unwrap());

// Safety patterns
static RE_MAYBE_UNINIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"MaybeUninit").unwrap());
static RE_PTR_OFFSET: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.offset\s*\(").unwrap());

// Correctness patterns
static RE_TODO_MACRO: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*todo!\s*\(").unwrap());
static RE_UNIMPLEMENTED: Lazy<Regex> = Lazy::new(|| Regex::new(r"unimplemented!\s*\(").unwrap());
static RE_UNREACHABLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"unreachable!\s*\(").unwrap());
static RE_MATCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*match\s+").unwrap());
static RE_DEREF_IMPL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*impl\s+(?:<[^>]*>\s+)?Deref\s+for\s+").unwrap());

// Concurrency patterns
static RE_REFCELL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?:RefCell|Cell)\s*<").unwrap());
static RE_STD_MUTEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"std::sync::Mutex").unwrap());
static RE_UNSAFE_SEND_SYNC: Lazy<Regex> = Lazy::new(|| Regex::new(r"unsafe\s+impl\s+(Send|Sync)").unwrap());
static RE_STATIC_MUT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*static\s+mut\s+").unwrap());
static RE_ARC: Lazy<Regex> = Lazy::new(|| Regex::new(r"Arc<").unwrap());

// Documentation patterns
static RE_PUB_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]").unwrap());

// Naming patterns - using inline regex in check_naming for specific patterns
// RE_LOWERCASE_STRUCT and RE_LOWERCASE_ENUM defined below

// Async patterns
static RE_ASYNC_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?async\s+fn\s+").unwrap());
static RE_AWAIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.await\b").unwrap());
static RE_STD_MUTEX_LOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"std::sync::Mutex[\s\S]*?\.lock\s*\(").unwrap());
static RE_BLOCKING_IO: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(std::)?(fs|net|process|thread)::").unwrap());

// Non-snake_case type names (should be CamelCase)
static RE_LOWERCASE_STRUCT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+([a-z][a-zA-Z0-9_]*)").unwrap());
static RE_LOWERCASE_ENUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?enum\s+([a-z][a-zA-Z0-9_]*)").unwrap());

// ============================================================================
// Security patterns
// ============================================================================

/// SQL injection: format!(...) with SQL keyword in string + interpolated variable
static RE_SQL_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"(?x)
        (?:format!|concat!|write!|writeln!)\s*\(
        [^)]*["']
        (?:SELECT|INSERT|UPDATE|DELETE|DROP|CREATE|ALTER|TRUNCATE|EXEC)\b
    "##).unwrap()
});

/// Hardcoded secrets: api_key/secret/password/token assigned a long string value
static RE_HARDCODED_SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"(?i)(?x)
        (?:api[_-]?key|apikey|secret|password|token|auth|credential|private_key)
        \s*[=:]\s*["'][A-Za-z0-9_\-]{16,}["']
    "##).unwrap()
});

/// Private key / certificate content embedded in string
static RE_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"["']-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----"##).unwrap()
});

/// OpenAI API key pattern
static RE_OPENAI_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"["']sk-[A-Za-z0-9]{32,}["']"##).unwrap()
});

// Non-SCREAMING_CASE constants/statics
static RE_NON_SCREAMING_CONST: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?const\s+([a-z][a-zA-Z0-9_]*)").unwrap());
static RE_NON_SCREAMING_STATIC: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?static\s+(?:mut\s+)?([a-z][a-zA-Z0-9_]*)").unwrap());

// ============================================================================
// Ignore system: // code-review: ignore[check1,check2,all]
// ============================================================================

/// Matches inline ignore directives: `// code-review: ignore[unsafe,style]` anywhere on a line
static RE_IGNORE_DIRECTIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"//\s*code-review:\s*ignore\s*\[([^\]]*)\]").unwrap()
});

// ============================================================================
// CodeReviewTool
// ============================================================================

/// Perform comprehensive code review on Rust source files.
pub struct CodeReviewTool;

#[async_trait::async_trait]
impl Tool for CodeReviewTool {
    fn name(&self) -> &str {
        "code_review"
    }

    fn description(&self) -> &str {
        "Perform comprehensive code review on Rust source files. Analyzes code for safety, error handling, performance, style, safety, correctness, concurrency, documentation, naming conventions, and async pitfalls. Generates structured reports with severity levels and actionable recommendations."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to a Rust file, or a directory containing Rust files to review".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "If true and path is a directory, analyze all .rs files recursively (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "checks".to_string(),
                description: "Comma-separated list of checks. Options: all, unsafe, error_handling, performance, style, safety, correctness, concurrency, documentation, naming, async, security, complexity, testing, debug (default: all)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: 'text' (readable report), 'json' (structured data), 'github-actions' (GitHub Annotations), 'gitlab-ci' (GitLab Code Quality), or 'auto' (auto-detect CI env, default: text)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_results".to_string(),
                description: "Maximum number of files to analyze (default: 50)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "min_severity".to_string(),
                description: "Minimum severity level: error, warning, info (default: info)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_fn_length".to_string(),
                description: "Maximum function length in lines before warning (default: 100)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "max_nesting".to_string(),
                description: "Maximum nesting depth before warning (default: 8)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "max_line_length".to_string(),
                description: "Maximum line length in characters before warning (default: 120)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "parallel".to_string(),
                description: "Process files in parallel for better performance (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "save_config".to_string(),
                description: "Save current parameters as a reusable config profile (e.g. 'my_review'). Saved profiles can be loaded later with --load_config.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "load_config".to_string(),
                description: "Load a saved config profile by name (e.g. 'my_review'). Parameters passed in the current call override saved values.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "list_configs".to_string(),
                description: "List all saved code_review config profiles. No analysis is performed when this is set.".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "delete_config".to_string(),
                description: "Delete a saved config profile by name (e.g. 'old_profile').".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        // ── Config persistence ──────────────────────────────────────────────
        let save_config = params.get("save_config").and_then(|v| v.as_str());
        let load_config = params.get("load_config").and_then(|v| v.as_str());
        let list_configs = params.get("list_configs").and_then(|v| v.as_bool()).unwrap_or(false);
        let delete_config = params.get("delete_config").and_then(|v| v.as_str());

        // Handle list_configs - no analysis needed
        if list_configs {
            return list_saved_configs();
        }

        // Handle delete_config - remove a saved profile
        if let Some(name) = delete_config {
            return delete_saved_config(name);
        }

        // Handle load_config: load saved params as defaults, then override with explicit params
        let merged_params = if let Some(config_name) = load_config {
            let store = ConfigStore::load();
            let key = format!("{CONFIG_NAMESPACE}{config_name}");
            match store.get(&key) {
                Some(config_obj) => {
                    if let Some(obj) = config_obj.as_object() {
                        let mut merged = HashMap::new();
                        for (k, v) in obj {
                            merged.insert(k.clone(), v.clone());
                        }
                        // Explicit params override saved values
                        for (k, v) in params {
                            merged.insert(k.clone(), v.clone());
                        }
                        merged
                    } else {
                        return Ok(serde_json::json!({
                            "status": "error",
                            "message": format!("Config profile '{config_name}' is corrupted (not an object)."),
                        }));
                    }
                }
                None => {
                    return Ok(serde_json::json!({
                        "status": "error",
                        "message": format!("Config profile '{config_name}' not found. Use --save_config '{config_name}' to create it, or --list_configs to see available profiles."),
                    }));
                }
            }
        } else {
            let mut p = HashMap::new();
            for (k, v) in params {
                p.insert(k.clone(), v.clone());
            }
            p
        };

        // Handle save_config: store merged parameters for later reuse
        if let Some(config_name) = save_config {
            let store = ConfigStore::load();
            let key = format!("{CONFIG_NAMESPACE}{config_name}");
            let mut config = serde_json::Map::new();
            for param_name in configurable_param_names() {
                if let Some(val) = merged_params.get(*param_name) {
                    config.insert(param_name.to_string(), val.clone());
                }
            }
            store.set(&key, Value::Object(config));
        }

        // ── Parse parameters (merged) ───────────────────────────────────────
        let path = merged_params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let recursive = merged_params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let checks_str = merged_params
            .get("checks")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let format = merged_params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let effective_format = resolve_format(format);

        let max_results = merged_params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        let min_severity = merged_params
            .get("min_severity")
            .and_then(|v| v.as_str())
            .unwrap_or("info");

        let max_fn_length = merged_params
            .get("max_fn_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let max_nesting = merged_params
            .get("max_nesting")
            .and_then(|v| v.as_u64())
            .unwrap_or(8) as usize;

        let max_line_length = merged_params
            .get("max_line_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(120) as usize;

        let parallel = merged_params
            .get("parallel")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let thresholds = Thresholds { max_fn_length, max_nesting, max_line_length };
        let file_path = Path::new(path);

        if !file_path.exists() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        let active_checks = parse_checks(checks_str)?;

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

        if files.len() > max_results {
            files.truncate(max_results);
        }

        // Run all checks — in parallel for speed
        let mut all_issues: Vec<ReviewIssue> = Vec::new();
        let mut file_summaries: Vec<Value> = Vec::new();
        let mut total_errors = 0u32;
        let mut total_warnings = 0u32;
        let mut total_info = 0u32;

        if parallel && files.len() > 1 {
            // ── Parallel processing ─────────────────────────────────────────
            let num_files = files.len();
            let mut handles = Vec::with_capacity(num_files);

            for file in &files {
                let file = file.clone();
                let checks_clone = active_checks.clone();
                let thresh_clone = Thresholds {
                    max_fn_length: thresholds.max_fn_length,
                    max_nesting: thresholds.max_nesting,
                    max_line_length: thresholds.max_line_length,
                };
                let min_sev = min_severity.to_string();

                handles.push(tokio::task::spawn_blocking(move || {
                    analyze_file(&file, &checks_clone, &thresh_clone, &min_sev)
                }));
            }

            for handle in handles {
                match handle.await {
                    Ok(Some(result)) => {
                        total_errors += result.errors;
                        total_warnings += result.warnings;
                        total_info += result.info;
                        file_summaries.push(result.summary);
                        all_issues.extend(result.issues);
                    }
                    Ok(None) => {} // I/O error already included as issue
                    Err(e) => {
                        all_issues.push(ReviewIssue {
                            severity: Severity::Error,
                            check: "io".to_string(),
                            file: "<parallel>".to_string(),
                            line: 0,
                            column: 0,
                            message: format!("Parallel task failed: {e}"),
                            recommendation: Some("Check file permissions and encoding.".to_string()),
                        });
                        total_errors += 1;
                    }
                }
            }
        } else {
            // ── Sequential processing ────────────────────────────────────────
            for file in &files {
                if let Some(result) = analyze_file(file, &active_checks, &thresholds, min_severity) {
                    total_errors += result.errors;
                    total_warnings += result.warnings;
                    total_info += result.info;
                    file_summaries.push(result.summary);
                    all_issues.extend(result.issues);
                }
            }
        }

        // Sort: errors first, then by file name
        all_issues.sort_by(|a, b| {
            let sev_cmp = (b.severity as u8).cmp(&(a.severity as u8));
            if sev_cmp != std::cmp::Ordering::Equal {
                sev_cmp
            } else {
                a.file.cmp(&b.file)
            }
        });

        match effective_format.as_str() {
            "json" => generate_json_report(&all_issues, &file_summaries, &files, total_errors, total_warnings, total_info, &active_checks),
            "github-actions" | "github_actions" => {
                generate_github_actions_report(&all_issues, total_errors, total_warnings, total_info)
            }
            "gitlab-ci" | "gitlab_ci" => {
                generate_gitlab_ci_report(&all_issues, total_errors, total_warnings, total_info)
            }
            _ => {
                generate_text_report(&all_issues, &file_summaries, &files, total_errors, total_warnings, total_info)
            }
        }
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// User-configurable thresholds for style checks.
struct Thresholds {
    max_fn_length: usize,
    max_nesting: usize,
    max_line_length: usize,
}

/// Which checks to run.
#[derive(Clone, Debug)]
struct ActiveChecks {
    unsafe_check: bool,
    error_handling: bool,
    performance: bool,
    style: bool,
    safety: bool,
    correctness: bool,
    concurrency: bool,
    documentation: bool,
    naming: bool,
    async_check: bool,
    security: bool,
    complexity: bool,
    testing: bool,
    debug: bool,
}

impl ActiveChecks {
    fn all() -> Self {
        Self {
            unsafe_check: true,
            error_handling: true,
            performance: true,
            style: true,
            safety: true,
            correctness: true,
            concurrency: true,
            documentation: true,
            naming: true,
            async_check: true,
            security: true,
            complexity: true,
            testing: true,
            debug: true,
        }
    }
}

fn parse_checks(checks_str: &str) -> Result<ActiveChecks, String> {
    let trimmed = checks_str.trim().to_lowercase();
    if trimmed == "all" {
        return Ok(ActiveChecks::all());
    }

    let mut checks = ActiveChecks {
        unsafe_check: false,
        error_handling: false,
        performance: false,
        style: false,
        safety: false,
        correctness: false,
        concurrency: false,
        documentation: false,
        naming: false,
        async_check: false,
        security: false,
        complexity: false,
        testing: false,
        debug: false,
    };

    for check in trimmed.split(',') {
        let c = check.trim();
        match c {
            "unsafe" => checks.unsafe_check = true,
            "error_handling" | "errorhandling" => checks.error_handling = true,
            "performance" | "perf" => checks.performance = true,
            "style" => checks.style = true,
            "safety" => checks.safety = true,
            "correctness" => checks.correctness = true,
            "concurrency" | "conc" => checks.concurrency = true,
            "documentation" | "docs" => checks.documentation = true,
            "naming" | "name" => checks.naming = true,
            "async" => checks.async_check = true,
            "security" | "sec" => checks.security = true,
            "complexity" | "compl" => checks.complexity = true,
            "testing" | "test" => checks.testing = true,
            "debug" => checks.debug = true,
            "" => {},
            _ => return Err(format!("Unknown check: '{c}'. Available: all, unsafe, error_handling, performance, style, safety, correctness, concurrency, documentation, naming, async, security, complexity, testing, debug")),
        }
    }

    Ok(checks)
}

// ============================================================================
// Severity Levels
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error = 3,
    Warning = 2,
    Info = 1,
}

impl Severity {
    fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        }
    }
}

fn severity_threshold(level: &str) -> Severity {
    match level {
        "error" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        _ => Severity::Info,
    }
}

// ============================================================================
// Issue Representation
// ============================================================================

#[derive(Debug, Clone)]
struct ReviewIssue {
    severity: Severity,
    check: String,
    file: String,
    line: usize,
    column: usize,
    message: String,
    recommendation: Option<String>,
}

// ============================================================================
// File Collection
// ============================================================================

fn collect_rust_files(dir: &Path, files: &mut Vec<String>, recursive: bool, depth: usize) -> Result<(), String> {
    if depth > 20 {
        return Ok(());
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if recursive {
                collect_rust_files(&path, files, true, depth + 1)?;
            }
        } else if path.is_file()
            && path.extension().map(|e| e == "rs").unwrap_or(false)
        {
            files.push(path.to_string_lossy().to_string());
        }
    }

    Ok(())
}

// ============================================================================
// Check Functions
// ============================================================================

/// Calculate the 1-based line number for a byte position in content.
fn line_at(content: &str, pos: usize) -> usize {
    content[..pos].matches('\n').count() + 1
}

fn check_unsafe_code(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_UNSAFE_BLOCK.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe block detected. Review for memory safety invariants.".to_string(),
            recommendation: Some(
                "Minimize unsafe code. Document safety invariants with // SAFETY: comments. \
                 Consider safe abstractions like std::cell::UnsafeCell or pin::Pin.".to_string(),
            ),
        });
    }

    for caps in RE_UNSAFE_FN.captures_iter(content) {
        let m = caps.get(0).unwrap();
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe function declaration. All callers must uphold safety preconditions.".to_string(),
            recommendation: Some(
                "Document safety preconditions in doc comments (# Safety section). \
                 Keep unsafe functions small and focused.".to_string(),
            ),
        });
    }

    for caps in RE_UNSAFE_TRAIT.captures_iter(content) {
        let m = caps.get(0).unwrap();
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe trait declaration. All implementors must uphold safety contracts.".to_string(),
            recommendation: Some(
                "Prefer safe traits with internal unsafe impls. If necessary, document all safety invariants.".to_string(),
            ),
        });
    }

    for m in RE_PTR_DEREF.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Raw pointer dereference. Only valid inside unsafe blocks.".to_string(),
            recommendation: Some(
                "Ensure pointer is aligned, non-null, and points to valid memory. \
                 Use .as_ref() / .as_mut() instead.".to_string(),
            ),
        });
    }

    for m in RE_TRANSMUTE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "mem::transmute used. Type layout assumptions are fragile.".to_string(),
            recommendation: Some(
                "Use safe alternatives: bytemuck, From/Into, or TryFrom. \
                 If transmute is necessary, add a SAFETY comment.".to_string(),
            ),
        });
    }
}

fn check_error_handling(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_UNWRAP.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        let context = &content[m.start()..std::cmp::min(m.end() + 40, content.len())];
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!(".unwrap() call. Will panic if `{}` is None/Err.", extract_var_name(context)),
            recommendation: Some(
                "Use `?` operator, .ok_or() with context, .unwrap_or_default(), or match.".to_string(),
            ),
        });
    }

    for m in RE_EXPECT.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".expect() call. Will panic on error.".to_string(),
            recommendation: Some(
                "Use `?` operator or anyhow::Context for error propagation. \
                 Reserve .expect() for unrecoverable states only.".to_string(),
            ),
        });
    }

    for m in RE_PANIC.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "panic!() in production code. Return an error instead.".to_string(),
            recommendation: Some(
                "Replace with return Err(...), anyhow::bail!(), or anyhow::ensure!().".to_string(),
            ),
        });
    }

    for m in RE_IGNORE_RESULT.find_iter(content) {
        let line = &content[m.start()..m.end()];
        if line.contains("write") || line.contains("read") || line.contains("send")
            || line.contains("save") || line.contains("remove") || line.contains("delete")
            || line.contains("create") || line.contains("insert") || line.contains("update")
        {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "error_handling".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Result ignored via `let _ = ...`. Errors silently discarded.".to_string(),
                recommendation: Some(
                    "Handle errors: if let Err(e) = ... { log::error!(...); } \
                     or .inspect_err(|e| ...).".to_string(),
                ),
            });
        }
    }

    for m in RE_WRITELN_RESULT.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Result from write!/writeln! is ignored.".to_string(),
            recommendation: Some(
                "Use .ok() to explicitly ignore, or add `?` to propagate.".to_string(),
            ),
        });
    }
}

fn check_performance(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_CLONE.find_iter(content) {
        if m.as_str().contains("Arc::clone") { continue; } // Arc::clone is cheap
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".clone() call. May cause unnecessary allocations.".to_string(),
            recommendation: Some(
                "Consider borrowing instead. Use Cow or Arc for shared ownership.".to_string(),
            ),
        });
    }

    for m in RE_BOX_NEW.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Box::new() heap allocation.".to_string(),
            recommendation: Some(
                "Only box when necessary: trait objects, recursive types, or large data across .await.".to_string(),
            ),
        });
    }

    for m in RE_VEC_CAPACITY.find_iter(content) {
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(cap) = digits.parse::<usize>() {
            if cap > 10_000 {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "performance".to_string(),
                    file: file.to_string(),
                    line: line_at(content, m.start()),
                    column: 1,
                    message: format!("Large Vec allocation ({cap} elements). May cause memory pressure."),
                    recommendation: Some(
                        "Consider streaming approach or Box<[T]> for fixed-size buffers.".to_string(),
                    ),
                });
            }
        }
    }

    for m in RE_COLLECT_VEC.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "collect::<Vec<_>>() intermediate allocation.".to_string(),
            recommendation: Some(
                "Chain iterator adaptors directly: .map(), .filter(), .take().".to_string(),
            ),
        });
    }
}

fn check_style(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>, thresholds: &Thresholds) {
    // Long functions
    let fn_positions: Vec<(usize, String)> = RE_FN_START
        .captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            Some((line_at(content, m0.start()), name))
        })
        .collect();

    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let end_line = if i + 1 < fn_positions.len() {
            fn_positions[i + 1].0
        } else {
            lines.len()
        };
        let length = end_line - start_line;

        if length > thresholds.max_fn_length {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "style".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` is {length} lines (max: {}). Refactor.", thresholds.max_fn_length),
                recommendation: Some(
                    "Split into smaller functions. Apply single-responsibility principle.".to_string(),
                ),
            });
        } else if length > thresholds.max_fn_length / 2 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "style".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` is {length} lines."),
                recommendation: Some(
                    "Consider extracting helper functions for readability.".to_string(),
                ),
            });
        }
    }

    // Deep nesting
    let mut max_nesting_val = 0usize;
    let mut current_nesting = 0usize;
    let mut first_deep_line: Option<usize> = None;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("* ") {
            continue;
        }
        for ch in trimmed.chars() {
            match ch {
                '{' | '[' => {
                    current_nesting += 1;
                    if current_nesting > thresholds.max_nesting && first_deep_line.is_none() {
                        first_deep_line = Some(line_idx + 1);
                    }
                    max_nesting_val = max_nesting_val.max(current_nesting);
                }
                '}' | ']' => {
                    current_nesting = current_nesting.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    if max_nesting_val > thresholds.max_nesting {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "style".to_string(),
            file: file.to_string(),
            line: first_deep_line.unwrap_or(1),
            column: 1,
            message: format!("Deep nesting (max depth: {max_nesting_val}, limit: {}). Refactor.", thresholds.max_nesting),
            recommendation: Some(
                "Extract nested logic, use early returns, guard clauses, or iterator combinators.".to_string(),
            ),
        });
    }

    // Non-snake_case functions
    for caps in RE_NON_SNAKE_FN.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "style".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Function `{}` should be snake_case.", name.as_str()),
                recommendation: Some(
                    "Rename to snake_case. Use CamelCase for types, snake_case for functions.".to_string(),
                ),
            });
        }
    }

    // TODO/FIXME comments
    for caps in RE_TODO_COMMENT.captures_iter(content) {
        let tag = caps.get(1).map(|t| t.as_str()).unwrap_or("TODO");
        let m0 = caps.get(0).unwrap();
        let line_num = line_at(content, m0.start()) - 1;
        let s: &str = lines.get(line_num).map(|s| s.trim()).unwrap_or("");
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "style".to_string(),
            file: file.to_string(),
            line: line_num + 1,
            column: 1,
            message: format!("{tag} comment: \"{s}\""),
            recommendation: Some("Resolve before committing. Create tasks for each TODO.".to_string()),
        });
    }

    // Long lines
    for (idx, line) in lines.iter().enumerate() {
        if line.len() > thresholds.max_line_length {
            let trimmed = line.trim();
            if trimmed.starts_with("//") { continue; }
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "style".to_string(),
                file: file.to_string(),
                line: idx + 1,
                column: thresholds.max_line_length + 1,
                message: format!("Line is {} chars (max: {}).", line.len(), thresholds.max_line_length),
                recommendation: Some(
                    "Break long lines at logical points. Use rustfmt.".to_string(),
                ),
            });
        }
    }
}

fn check_safety(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_TRANSMUTE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "mem::transmute: type sizes must match. Extremely unsafe.".to_string(),
            recommendation: Some(
                "Use bytemuck::Pod for plain-data casts, or From/Into/TryFrom.".to_string(),
            ),
        });
    }

    for m in RE_MAYBE_UNINIT.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "MaybeUninit: incorrect use causes UB.".to_string(),
            recommendation: Some(
                "Initialize all bytes before calling .assume_init(). Prefer safe init patterns.".to_string(),
            ),
        });
    }

    for m in RE_PTR_OFFSET.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".offset() pointer arithmetic: out-of-bounds risk.".to_string(),
            recommendation: Some(
                "Use .add()/.sub() (still unsafe but clearer). Ensure bounds checking.".to_string(),
            ),
        });
    }
}

fn check_correctness(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_TODO_MACRO.find_iter(content) {
        let context = &content[m.start()..std::cmp::min(m.start() + 60, content.len())];
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("todo!(): \"{}\"", context.trim()),
            recommendation: Some(
                "Implement the missing functionality. Use anyhow::bail!() for runtime errors.".to_string(),
            ),
        });
    }

    for m in RE_UNIMPLEMENTED.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "unimplemented!(): will panic at runtime.".to_string(),
            recommendation: Some("Implement the method body before committing.".to_string()),
        });
    }

    for m in RE_UNREACHABLE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "unreachable!(): confirm this path is truly impossible.".to_string(),
            recommendation: Some(
                "Only for logically impossible states. Consider if the type system can prove it.".to_string(),
            ),
        });
    }

    for m in RE_MATCH.find_iter(content) {
        let remaining = &content[m.end()..std::cmp::min(m.end() + 2000, content.len())];
        if !remaining.contains("_ =>") && !remaining.contains("other =>") && remaining.contains("::") {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "correctness".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Match without wildcard `_ =>` arm. Will fail on new enum variants.".to_string(),
                recommendation: Some(
                    "Add `_ => { ... }` arm. Or use #[non_exhaustive] on the enum.".to_string(),
                ),
            });
        }
    }

    for m in RE_DEREF_IMPL.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Deref impl: coercions cause subtle bugs.".to_string(),
            recommendation: Some(
                "Implement Deref only for smart pointers. Avoid Deref for method delegation (anti-pattern).".to_string(),
            ),
        });
    }
}

fn check_concurrency(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_REFCELL.find_iter(content) {
        let surrounding = &content[m.start().saturating_sub(30)..std::cmp::min(m.end() + 30, content.len())];
        if surrounding.contains("static") || surrounding.contains("lazy_static") || surrounding.contains("OnceCell") {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "concurrency".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "RefCell/Cell with shared/static state: not thread-safe.".to_string(),
                recommendation: Some(
                    "Use Mutex<T> or RwLock<T> for thread-safe interior mutability.".to_string(),
                ),
            });
        }
    }

    for m in RE_STD_MUTEX.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "std::sync::Mutex: may block thread in async context.".to_string(),
            recommendation: Some(
                "Use tokio::sync::Mutex in async code if holding across .await points.".to_string(),
            ),
        });
    }

    for m in RE_UNSAFE_SEND_SYNC.find_iter(content) {
        let trait_name = if m.as_str().contains("Send") { "Send" } else { "Sync" };
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("Unsafe impl {trait_name}: incorrect guarantees => UB."),
            recommendation: Some(
                "Only impl Send/Sync manually if certain of thread-safety. Use internal sync primitives.".to_string(),
            ),
        });
    }

    for m in RE_STATIC_MUT.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "static mut: UB without sync. Use Mutex/RwLock/OnceLock.".to_string(),
            recommendation: Some(
                "Use `static` with Mutex<T> for mutable global state, or OnceLock for lazy init.".to_string(),
            ),
        });
    }

    for m in RE_ARC.find_iter(content) {
        let context = &content[m.start()..std::cmp::min(m.start() + 50, content.len())];
        let non_send = ["Rc<", "RefCell<", "Cell<", "raw pointer", "*const", "*mut"];
        if non_send.iter().any(|t| context.contains(t)) {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "concurrency".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Arc wraps non-Send type: not thread-safe.".to_string(),
                recommendation: Some(
                    "Use Arc<Mutex<T>> for thread-safe shared ownership.".to_string(),
                ),
            });
        }
    }
}

fn check_documentation(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    let pub_item_patterns: Vec<(&str, &str)> = vec![
        (r"(?m)^\s*pub\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)", "function"),
        (r"(?m)^\s*pub\s+struct\s+([a-zA-Z_][a-zA-Z0-9_]*)", "struct"),
        (r"(?m)^\s*pub\s+(?:enum|union)\s+([a-zA-Z_][a-zA-Z0-9_]*)", "enum"),
        (r"(?m)^\s*pub\s+trait\s+([a-zA-Z_][a-zA-Z0-9_]*)", "trait"),
        (r"(?m)^\s*pub\s+type\s+([a-zA-Z_][a-zA-Z0-9_]*)", "type alias"),
        (r"(?m)^\s*pub\s+const\s+([a-zA-Z_][a-zA-Z0-9_]*)", "constant"),
    ];

    for (pattern, item_type) in &pub_item_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for caps in re.captures_iter(content) {
                let m0 = match caps.get(0) { Some(m) => m, None => continue };
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");

                if *item_type == "function" && (name.starts_with("test_") || name == "new") { continue; }

                // Check if doc comment exists before this item
                let before = &content[..m0.start()];
                let has_doc = before.lines().rev().take(5).any(|l| {
                    let t = l.trim();
                    t.starts_with("///") || t.starts_with("/**") || t.starts_with("* ")
                });

                if !has_doc {
                    issues.push(ReviewIssue {
                        severity: Severity::Warning,
                        check: "documentation".to_string(),
                        file: file.to_string(),
                        line: line_at(content, m0.start()),
                        column: 1,
                        message: format!("Public {item_type} `{name}` missing doc comments."),
                        recommendation: Some(
                            "Add /// comments: purpose, params, returns, panics, errors.".to_string(),
                        ),
                    });
                }
            }
        }
    }

    // Functions with params but no parameter docs
    for caps in RE_PUB_FN.captures_iter(content) {
        let m0 = match caps.get(0) { Some(m) => m, None => continue };
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");

        let sig_end = content[m0.start()..].find('{').map(|i| m0.start() + i).unwrap_or(content.len());
        let sig = &content[m0.start()..sig_end];

        if sig.contains('(') && !sig.contains("()") {
            let before = &content[..m0.start()];
            let has_param_docs = before.lines().rev().take(15).any(|l| {
                let t = l.trim();
                (t.starts_with("///") && (t.contains("- ") || t.contains('`')))
                    || t.contains("# Parameters") || t.contains("# Arguments")
            });
            if !has_param_docs {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "documentation".to_string(),
                    file: file.to_string(),
                    line: line_at(content, m0.start()),
                    column: 1,
                    message: format!("Public fn `{name}` has undocmented parameters."),
                    recommendation: Some(
                        "Add `# Parameters` section: `name` - description.".to_string(),
                    ),
                });
            }
        }
    }
}

// ============================================================================
// Check: Naming Conventions
// ============================================================================

fn check_naming(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // Struct names should be CamelCase (start with uppercase)
    for caps in RE_LOWERCASE_STRUCT.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Struct `{}` should use CamelCase (e.g. `{}`).", name.as_str(), to_camel_case(name.as_str())),
                recommendation: Some(
                    "Rename to CamelCase: type names use PascalCase convention.".to_string(),
                ),
            });
        }
    }

    // Enum names should be CamelCase
    for caps in RE_LOWERCASE_ENUM.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Enum `{}` should use CamelCase (e.g. `{}`).", name.as_str(), to_camel_case(name.as_str())),
                recommendation: Some(
                    "Rename to CamelCase: type names use PascalCase convention.".to_string(),
                ),
            });
        }
    }

    // Constants should be SCREAMING_SNAKE_CASE
    for caps in RE_NON_SCREAMING_CONST.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            // Skip if already screaming or has pub(crate) visibility weirdness
            let n = name.as_str();
            if n.chars().any(|c| c.is_uppercase()) && n.contains('_') { continue; }
            if n.len() <= 2 { continue; } // short names are ok
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Constant `{n}` should use SCREAMING_SNAKE_CASE."),
                recommendation: Some(
                    "Rust convention: const values use UPPER_SNAKE_CASE naming.".to_string(),
                ),
            });
        }
    }

    // Statics should be SCREAMING_SNAKE_CASE
    for caps in RE_NON_SCREAMING_STATIC.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let n = name.as_str();
            if n.chars().any(|c| c.is_uppercase()) && n.contains('_') { continue; }
            if n.len() <= 2 { continue; }
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Static `{n}` should use SCREAMING_SNAKE_CASE."),
                recommendation: Some(
                    "Rust convention: static values use UPPER_SNAKE_CASE.".to_string(),
                ),
            });
        }
    }
}

/// Convert a snake_case name to CamelCase for suggestion purposes.
fn to_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for ch in name.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}

// ============================================================================
// Check: Async Pitfalls
// ============================================================================

fn check_async(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    let is_async_file = RE_ASYNC_FN.find(content).is_some() || RE_AWAIT.find(content).is_some();
    if !is_async_file {
        return;
    }

    // std::sync::Mutex in async code (likely blocking)
    for m in RE_STD_MUTEX_LOCK.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "async".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "std::sync::Mutex::lock() in async code: will block the thread.".to_string(),
            recommendation: Some(
                "Use tokio::sync::Mutex or futures::lock::Mutex for async code.".to_string(),
            ),
        });
    }

    // Blocking I/O in async functions (heuristic)
    if RE_ASYNC_FN.find(content).is_some() {
        for m in RE_BLOCKING_IO.find_iter(content) {
            // Only flag if we're inside an async fn block
            // Simple heuristic: check if there's an async fn in the file
            let line_num = line_at(content, m.start());
            // Check if this line is within an async fn
            // We use a simple approach: look for async fn before this position
            let before_pos = &content[..m.start()];
            if has_recent_async_fn(before_pos) {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "async".to_string(),
                    file: file.to_string(),
                    line: line_num,
                    column: 1,
                    message: "Potential blocking I/O in async context. Use async alternatives.".to_string(),
                    recommendation: Some(
                        "Use tokio::fs, tokio::net, or tokio::process instead of std. \
                         Or use spawn_blocking().".to_string(),
                    ),
                });
            }
        }
    }
}

/// Check if there's an async fn declaration before a position in content.
fn has_recent_async_fn(before: &str) -> bool {
    // Check the last 100 lines for an async fn
    let recent: Vec<&str> = before.lines().rev().take(100).collect();
    for line in &recent {
        let t = line.trim();
        if t.starts_with("async fn") || t.starts_with("pub async fn") || t.starts_with("pub(crate) async fn") {
            return true;
        }
        if t == "}" {
            return false; // We've exited an fn block
        }
    }
    false
}

// ============================================================================
// Check: Security
// ============================================================================

fn check_security(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // SQL injection: format!/concat! with SQL keyword + interpolated variable
    for m in RE_SQL_INJECTION.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Possible SQL injection: format!/concat! with SQL query containing interpolated variables.".to_string(),
            recommendation: Some(
                "Use parameterized queries (sqlx::query! or diesel) instead of string formatting. \\\n                 Never concatenate user input into SQL strings.".to_string(),
            ),
        });
    }

    // Hardcoded secrets: api_key, secret, password, token assignments
    for m in RE_HARDCODED_SECRET.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Possible hardcoded secret (API key, password, or token).".to_string(),
            recommendation: Some(
                "Use environment variables (std::env::var), .env files, or a secrets manager. \\\n                 Never commit secrets to version control.".to_string(),
            ),
        });
    }

    // Private keys embedded in source
    for m in RE_PRIVATE_KEY.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Private key content detected in source code!".to_string(),
            recommendation: Some(
                "Remove private key from source. Use environment variables or a secrets manager. \\\n                 Rotate the compromised key immediately.".to_string(),
            ),
        });
    }

    // OpenAI API keys
    for m in RE_OPENAI_KEY.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "OpenAI API key detected in source code!".to_string(),
            recommendation: Some(
                "Use environment variables (std::env::var(\"OPENAI_API_KEY\")) instead. \\\n                 Rotate the compromised key immediately.".to_string(),
            ),
        });
    }
}

// ============================================================================
// Check: Complexity
// ============================================================================

/// Regex to match function parameters section
static RE_FN_PARAMS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\(([^)]*)\)").unwrap()
});

fn check_complexity(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // 1. Large file check (>1000 lines)
    if lines.len() > 1000 {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "complexity".to_string(),
            file: file.to_string(),
            line: 1,
            column: 1,
            message: format!("File is {} lines (>1000). Consider splitting.", lines.len()),
            recommendation: Some(
                "Split into smaller modules. Aim for <500 lines per file for maintainability.".to_string(),
            ),
        });
    }

    // 2. Excessive function parameters (>5)
    for caps in RE_FN_PARAMS.captures_iter(content) {
        let params_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        // Count comma-separated parameters (heuristic: skip empty and self)
        let param_count = params_str.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "self" && *s != "&self" && *s != "&mut self" && !s.starts_with("self:"))
            .count();

        if param_count > 5 {
            let m0 = caps.get(0).unwrap();
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Function has {param_count} parameters (max: 5). Consider refactoring."),
                recommendation: Some(
                    "Use a struct to group related parameters, or split the function.".to_string(),
                ),
            });
        }
    }

    // 3. Cyclomatic complexity: count if/else/match/loop/while/for branches
    // Simple heuristic: count decision points per function
    let fn_positions: Vec<(usize, String)> = RE_FN_START
        .captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            Some((line_at(content, m0.start()), name))
        })
        .collect();

    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let start_byte = find_line_start(content, *start_line);
        let end_byte = if i + 1 < fn_positions.len() {
            find_line_start(content, fn_positions[i + 1].0)
        } else {
            content.len()
        };

        if start_byte >= end_byte { continue; }
        let fn_body = &content[start_byte..end_byte];

        // Count decision points
        let mut complexity = 1; // Base complexity
        // Count if (but not "if let" which counts separately)
        for m in fn_body.match_indices("if ") {
            let before = &fn_body[..m.0];
            let prev_ch = before.chars().last().unwrap_or(' ');
            // Only count if it's not inside a comment or string
            if prev_ch == ' ' || prev_ch == '\t' || prev_ch == '\n' || prev_ch == '{' || prev_ch == ';' {
                complexity += 1;
            }
        }
        // Count else if
        for _m in fn_body.match_indices("else if ") {
            complexity += 1; // Already counted by "if" above, but "else if" adds another branch
        }
        // Count match arms
        complexity += fn_body.matches("=>").count();
        // Count loops
        complexity += fn_body.matches("for ").count();
        complexity += fn_body.matches("while ").count();
        complexity += fn_body.matches("loop ").count();
        // Count && and || (boolean conditions add complexity)
        complexity += fn_body.matches("&&").count();
        complexity += fn_body.matches("||").count();

        if complexity > 15 {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` has high cyclomatic complexity (~{complexity}, max: 15). Refactor."),
                recommendation: Some(
                    "Extract conditions into helper functions, use early returns, or simplify match arms.".to_string(),
                ),
            });
        } else if complexity > 10 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` has moderate cyclomatic complexity (~{complexity})."),
                recommendation: Some(
                    "Consider extracting helper functions to improve readability.".to_string(),
                ),
            });
        }
    }
}

/// Find the byte offset of a given 1-based line number in content.
fn find_line_start(content: &str, line_num: usize) -> usize {
    if line_num <= 1 { return 0; }
    let mut pos = 0;
    for _ in 1..line_num {
        if let Some(nl) = content[pos..].find('\n') {
            pos += nl + 1;
        } else {
            break;
        }
    }
    pos
}

// ============================================================================
// Check: Testing Quality
// ============================================================================

static RE_TEST_FN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*#\[test\]\s*$").unwrap());
static RE_ASSERT_MACRO: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bassert(|_eq|_ne|_approx_eq)!").unwrap());

fn check_testing(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // 1. Find all #[test] functions and check for assertions
    let test_positions: Vec<usize> = RE_TEST_FN.find_iter(content)
        .map(|m| line_at(content, m.start()))
        .collect();

    for test_line in &test_positions {
        // Find the fn body start after #[test]
        let fn_body_start = find_line_start(content, *test_line + 1);
        // Find the next #[test] AFTER this one (skip past this #[test])
        let search_start = content[fn_body_start..].find('\n')
            .map(|pos| fn_body_start + pos + 1)
            .unwrap_or(fn_body_start);
        let next_test = content[search_start..].find("#[test]")
            .map(|pos| search_start + pos)
            .unwrap_or(content.len());
        let test_body = &content[fn_body_start..next_test];

        // Count assertions in this test function
        let assert_count = RE_ASSERT_MACRO.find_iter(test_body).count();

        if assert_count == 0 {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "testing".to_string(),
                file: file.to_string(),
                line: *test_line,
                column: 1,
                message: "Test function has no assertions. May not provide value.".to_string(),
                recommendation: Some(
                    "Add assertions (assert_eq!, assert_ne!, assert!) to verify expected behavior.".to_string(),
                ),
            });
        }
    }

    // 2. Check for overly long test functions (>100 lines)
    let fn_positions: Vec<(usize, String)> = RE_FN_START
        .captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            // Only consider test functions
            let line_num = line_at(content, m0.start());
            let before = &content[..m0.start()];
            if before.lines().rev().take(3).any(|l| l.trim() == "#[test]") {
                Some((line_num, name))
            } else {
                None
            }
        })
        .collect();

    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let end_line = if i + 1 < fn_positions.len() {
            fn_positions[i + 1].0
        } else {
            lines.len()
        };
        let length = end_line - start_line;

        if length > 100 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "testing".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Test function `{name}` is {length} lines. Consider splitting."),
                recommendation: Some(
                    "Split into multiple focused test cases, or use test parameterization.".to_string(),
                ),
            });
        }
    }
}

// ============================================================================
// Check: Debug Residuals
// ============================================================================

static RE_DBG_MACRO: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(dbg!|eprintln!|println!)\s*\(").unwrap());

fn check_debug(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_DBG_MACRO.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        let macro_name = if m.as_str().contains("dbg!") { "dbg!" }
            else if m.as_str().contains("eprintln!") { "eprintln!" }
            else { "println!" };

        // Only flag println! in non-main, non-binary contexts (heuristic: look for lib.rs or src/lib)
        if macro_name == "println!" {
            // Check if this looks like library code (not main.rs)
            if file.ends_with("main.rs") || file.ends_with("bin/") {
                continue; // println! in binary entry point is fine
            }
            // Check if we're inside a struct's Display/Debug impl
            let before = &content[..m.start()];
            let recent_lines: Vec<&str> = before.lines().rev().take(20).collect();
            let in_display = recent_lines.iter().any(|l| {
                let t = l.trim();
                t.contains("impl") && (t.contains("Display") || t.contains("Debug") || t.contains("fmt::Formatter"))
            });
            if in_display { continue; }
        }

        issues.push(ReviewIssue {
            severity: if macro_name == "dbg!" { Severity::Warning } else { Severity::Info },
            check: "debug".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("{macro_name}() call in production code. Debug residual."),
            recommendation: Some(
                "Remove debug statements before committing. Use logging (log::info!) for persistent output.".to_string(),
            ),
        });
    }
}

/// Parse inline ignore directives from content.
/// Returns a set of (line_number, check_category) pairs that should be suppressed.
fn parse_ignore_directives(content: &str) -> Vec<(usize, String)> {
    let mut ignores: Vec<(usize, String)> = Vec::new();
    for caps in RE_IGNORE_DIRECTIVE.captures_iter(content) {
        let line_num = line_at(content, caps.get(0).unwrap().start());
        let checks_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        for check in checks_str.split(',') {
            let c = check.trim().to_lowercase();
            if c == "all" {
                ignores.push((line_num, "all".to_string()));
            } else if !c.is_empty() {
                ignores.push((line_num, c));
            }
        }
    }
    ignores
}

/// Check if an issue at a given line should be ignored.
fn is_ignored(ignores: &[(usize, String)], line: usize, check: &str) -> bool {
    ignores.iter().any(|(l, c)| *l == line && (c == "all" || c == check))
}

/// Check if a position in the file is within test code.
fn is_in_test_code(content: &str, pos: usize) -> bool {
    let before = &content[..pos];
    let lines_before: Vec<&str> = before.lines().collect();
    let mut test_block_depth: i32 = 0; // positive = inside a test block

    for line in lines_before.iter().rev().take(100) {
        let trimmed = line.trim();

        // Direct test markers: #[test] right before a fn
        if trimmed == "#[test]" || trimmed == "#[cfg(test)]" || trimmed.starts_with("#[cfg(test") {
            return true;
        }

        // Track mod tests { ... } blocks going backward
        let opens = trimmed.chars().filter(|c| *c == '{').count() as i32;
        let closes = trimmed.chars().filter(|c| *c == '}').count() as i32;

        // Going backward: { means exiting a block, } means entering a block
        let was_negative = test_block_depth < 0;
        test_block_depth += opens;  // { going backward: removing block nesting
        test_block_depth -= closes; // } going backward: adding block nesting

        // If we just exited a block (going backward = entered one going forward),
        // check if this block is a test module
        if !was_negative && test_block_depth < 0 {
            // The current line has a `}` that closes a block in forward direction.
            // We need to check if that block is mod tests or #[cfg(test)]
            // Look at the line below (forward direction) which contains the block content
            return false; // Conservative: not test code
        }

        // Check if we just entered a `mod tests` block (going forward)
        // Going backward: if we see `mod tests` followed by `{`, we're inside it
        if trimmed.contains("mod tests") && trimmed.ends_with('{') {
            return true;
        }
    }
    false
}

/// Extract the variable name from context around an .unwrap() call.
fn extract_var_name(context: &str) -> String {
    if let Some(c) = RE_UNWRAP.captures(context) {
        c.get(1).map(|m| m.as_str().to_string()).unwrap_or_else(|| "value".to_string())
    } else {
        "value".to_string()
    }
}

// ============================================================================
// Report Generation
// ============================================================================

fn generate_text_report(
    issues: &[ReviewIssue],
    file_summaries: &[Value],
    files: &[String],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
) -> Result<Value, String> {
    let total_issues = issues.len();
    let mut report = String::new();

    report.push_str(&format!(
        "╔══════════════════════════════════════════════════════════════════╗\n\
         ║                       CODE REVIEW REPORT                       ║\n\
         ╚══════════════════════════════════════════════════════════════════╝\n\n"
    ));
    report.push_str(&format!("Files reviewed: {}\n", files.len()));
    report.push_str(&format!("Total issues found: {total_issues}\n"));
    report.push_str(&format!(
        "  {e} Errors | {w} Warnings | {i} Info\n\n",
        e = total_errors, w = total_warnings, i = total_info,
    ));

    // File summaries
    report.push_str("─── File Summary ──────────────────────────────────────────────\n");
    for fs in file_summaries {
        let file = fs["file"].as_str().unwrap_or("unknown");
        let errs = fs["errors"].as_u64().unwrap_or(0);
        let warns = fs["warnings"].as_u64().unwrap_or(0);
        let infos = fs["info"].as_u64().unwrap_or(0);
        let lines = fs["code_lines"].as_u64().unwrap_or(0);
        report.push_str(&format!(
            "  {:<40} {:>4} issues ({}, {}, {}) - {} lines\n",
            file, errs + warns + infos,
            plural(errs, "error"), plural(warns, "warning"), plural(infos, "info"), lines,
        ));
    }
    report.push('\n');

    if !issues.is_empty() {
        report.push_str("─── Issues ────────────────────────────────────────────────────\n\n");

        let errors: Vec<&ReviewIssue> = issues.iter().filter(|i| matches!(i.severity, Severity::Error)).collect();
        let warnings: Vec<&ReviewIssue> = issues.iter().filter(|i| matches!(i.severity, Severity::Warning)).collect();
        let infos: Vec<&ReviewIssue> = issues.iter().filter(|i| matches!(i.severity, Severity::Info)).collect();

        for (label, icon, group) in [("ERRORS", "🔴", &errors), ("WARNINGS", "🟡", &warnings), ("INFO", "🔵", &infos)] {
            if group.is_empty() { continue; }
            report.push_str(&format!("{icon} {label} ({})\n", group.len()));
            report.push_str("   ──────────────────────────────────────────────────────\n");
            for issue in group.iter() {
                report.push_str(&format!(
                    "   [{:<6}] {}:{}\n   {:>10} {}\n",
                    issue.check, shorten_path(&issue.file), issue.line, "", issue.message,
                ));
                if let Some(rec) = &issue.recommendation {
                    report.push_str(&format!("   {:>10} 💡 {}\n\n", "", rec));
                } else {
                    report.push('\n');
                }
            }
        }

        // Check summary table
        report.push_str("\n─── Check Summary ───────────────────────────────────────────\n");
        report.push_str(&format!("  {:<20} {:>5} {:>5} {:>5}\n", "Check", "Errors", "Warn.", "Info"));
        report.push_str(&format!("  {}\n", "-".repeat(37)));
        let mut check_counts: HashMap<String, (u32, u32, u32)> = HashMap::new();
        for issue in issues {
            let entry = check_counts.entry(issue.check.clone()).or_insert((0, 0, 0));
            match issue.severity {
                Severity::Error => entry.0 += 1,
                Severity::Warning => entry.1 += 1,
                Severity::Info => entry.2 += 1,
            }
        }
        let mut sorted: Vec<(&String, &(u32, u32, u32))> = check_counts.iter().collect();
        sorted.sort_by(|a, b| (b.1.0, b.1.1, b.1.2).cmp(&(a.1.0, a.1.1, a.1.2)));
        for (check, (e, w, i)) in &sorted {
            report.push_str(&format!("  {:<20} {:>5} {:>5} {:>5}\n", check, e, w, i));
        }
    } else {
        report.push_str("✅ No issues found! Code looks clean.\n");
    }

    Ok(serde_json::json!({
        "status": "ok", "format": "text", "report": report,
        "summary": {
            "files": files.len(), "total_issues": total_issues,
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
    }))
}

fn generate_json_report(
    issues: &[ReviewIssue],
    file_summaries: &[Value],
    files: &[String],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
    _checks: &ActiveChecks,
) -> Result<Value, String> {
    let json_issues: Vec<Value> = issues.iter().map(|i| {
        serde_json::json!({
            "severity": i.severity.as_str(),
            "check": i.check,
            "file": i.file,
            "line": i.line,
            "column": i.column,
            "message": i.message,
            "recommendation": i.recommendation,
        })
    }).collect();

    Ok(serde_json::json!({
        "status": "ok", "format": "json",
        "summary": {
            "files_reviewed": files.len(),
            "total_issues": issues.len(),
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
        "files": file_summaries,
        "issues": json_issues,
    }))
}

// ============================================================================
// CI Integration: GitHub Actions Annotations
// ============================================================================

/// Resolve the output format, auto-detecting CI environment if set to "auto".
fn resolve_format(requested: &str) -> String {
    match requested.trim().to_lowercase().as_str() {
        "auto" => {
            // Auto-detect CI environment
            if std::env::var("GITHUB_ACTIONS").is_ok() {
                "github-actions".to_string()
            } else if std::env::var("GITLAB_CI").is_ok() {
                "gitlab-ci".to_string()
            } else {
                "text".to_string()
            }
        }
        f => f.to_string(),
    }
}

/// Generate GitHub Actions annotation report.
/// See: https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions
///
/// Format: ::{severity} file={path},line={line},col={col},title={title}::{message}
fn generate_github_actions_report(
    issues: &[ReviewIssue],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
) -> Result<Value, String> {
    let mut annotations = String::new();

    let total_issues = issues.len();
    annotations.push_str(&format!(
        "::notice title=Code Review Summary::Files reviewed: {total_issues} issues \\\n\
         ({total_errors} errors, {total_warnings} warnings, {total_info} info)\\n"
    ));

    for issue in issues {
        let severity_label = match issue.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "notice",
        };

        // Escape message for GitHub Actions: %, \\n, \\r
        let message = issue.message
            .replace('%', "%25")
            .replace('\n', "%0A")
            .replace('\r', "%0D");
        let title = format!("{}/{}", issue.check, issue.severity.as_str().to_lowercase());

        annotations.push_str(&format!(
            "::{severity_label} file={file},line={line},col={col},title={title}::{message}\n",
            severity_label = severity_label,
            file = issue.file,
            line = issue.line,
            col = issue.column,
            title = title,
            message = message,
        ));
    }

    Ok(serde_json::json!({
        "status": "ok", "format": "github-actions",
        "report": annotations,
        "summary": {
            "files": "N/A",
            "total_issues": total_issues,
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
    }))
}

// ============================================================================
// CI Integration: GitLab CI Code Quality
// ============================================================================

/// Generate GitLab CI Code Quality report.
/// See: https://docs.gitlab.com/ee/ci/testing/code_quality.html#implement-a-custom-tool
///
/// Outputs a JSON array of code quality violations in GitLab's expected format.
fn generate_gitlab_ci_report(
    issues: &[ReviewIssue],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
) -> Result<Value, String> {
    let total_issues = issues.len();
    let mut gitlab_issues: Vec<Value> = Vec::with_capacity(issues.len());

    for issue in issues {
        let severity = match issue.severity {
            Severity::Error => "blocker",
            Severity::Warning => "major",
            Severity::Info => "minor",
        };

        gitlab_issues.push(serde_json::json!({
            "type": "issue",
            "check_name": issue.check,
            "description": issue.message,
            "content": {
                "body": issue.recommendation.as_deref().unwrap_or(""),
            },
            "categories": ["Code Style"],
            "location": {
                "path": issue.file,
                "lines": {
                    "begin": issue.line,
                    "end": issue.line,
                }
            },
            "severity": severity,
            "fingerprint": format!("{}:{}:{}", issue.check, issue.file, issue.line),
        }));
    }

    // Also return a text summary for convenience
    let summary = format!(
        "Code Review Summary: {total_issues} issues found ({total_errors} errors, {total_warnings} warnings, {total_info} info)\n"
    );

    Ok(serde_json::json!({
        "status": "ok", "format": "gitlab-ci",
        "report": gitlab_issues,
        "summary_text": summary,
        "summary": {
            "files": "N/A",
            "total_issues": total_issues,
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
    }))
}

// ============================================================================
// Parallel Processing Support
// ============================================================================

/// Per-file analysis result returned by `analyze_file`.
struct FileAnalysisResult {
    issues: Vec<ReviewIssue>,
    summary: Value,
    errors: u32,
    warnings: u32,
    info: u32,
}

/// Analyze a single file: read content, run all enabled checks, apply ignores and
/// severity filtering. Returns `None` if the file could not be read (an I/O error
/// issue is included in the result's issue list in that case).
fn analyze_file(
    file: &str,
    active_checks: &ActiveChecks,
    thresholds: &Thresholds,
    min_severity: &str,
) -> Option<FileAnalysisResult> {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            // Return a single I/O error issue
            let io_issue = ReviewIssue {
                severity: Severity::Error,
                check: "io".to_string(),
                file: file.to_string(),
                line: 0,
                column: 0,
                message: format!("Failed to read file: {e}"),
                recommendation: Some("Check file permissions and encoding.".to_string()),
            };
            return Some(FileAnalysisResult {
                issues: vec![io_issue],
                summary: serde_json::json!({
                    "file": file,
                    "issues": 1,
                    "errors": 1,
                    "warnings": 0,
                    "info": 0,
                    "code_lines": 0,
                }),
                errors: 1,
                warnings: 0,
                info: 0,
            });
        }
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut file_issues: Vec<ReviewIssue> = Vec::new();

    // Parse ignore directives for this file
    let ignores = parse_ignore_directives(&content);

    if active_checks.unsafe_check {
        check_unsafe_code(&content, &lines, file, &mut file_issues);
    }
    if active_checks.error_handling {
        check_error_handling(&content, &lines, file, &mut file_issues);
    }
    if active_checks.performance {
        check_performance(&content, &lines, file, &mut file_issues);
    }
    if active_checks.style {
        check_style(&content, &lines, file, &mut file_issues, thresholds);
    }
    if active_checks.safety {
        check_safety(&content, &lines, file, &mut file_issues);
    }
    if active_checks.correctness {
        check_correctness(&content, &lines, file, &mut file_issues);
    }
    if active_checks.concurrency {
        check_concurrency(&content, &lines, file, &mut file_issues);
    }
    if active_checks.documentation {
        check_documentation(&content, &lines, file, &mut file_issues);
    }
    if active_checks.naming {
        check_naming(&content, &lines, file, &mut file_issues);
    }
    if active_checks.async_check {
        check_async(&content, &lines, file, &mut file_issues);
    }
    if active_checks.security {
        check_security(&content, &lines, file, &mut file_issues);
    }
    if active_checks.complexity {
        check_complexity(&content, &lines, file, &mut file_issues);
    }
    if active_checks.testing {
        check_testing(&content, &lines, file, &mut file_issues);
    }
    if active_checks.debug {
        check_debug(&content, &lines, file, &mut file_issues);
    }

    // Apply ignore directives: // code-review: ignore[check_name]
    file_issues.retain(|i| !is_ignored(&ignores, i.line, &i.check));

    // Filter by severity threshold
    let threshold = severity_threshold(min_severity);
    file_issues.retain(|i| i.severity as u8 >= threshold as u8);

    // Count
    let mut errors = 0u32;
    let mut warnings = 0u32;
    let mut info = 0u32;
    for issue in &file_issues {
        match issue.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => info += 1,
        }
    }

    let summary = serde_json::json!({
        "file": file,
        "issues": file_issues.len(),
        "errors": errors,
        "warnings": warnings,
        "info": info,
        "code_lines": lines.len(),
    });

    Some(FileAnalysisResult {
        issues: file_issues,
        summary,
        errors,
        warnings,
        info,
    })
}

fn shorten_path(path: &str) -> &str {
    // Show just the filename
    if let Some(pos) = path.rfind('/') {
        &path[pos + 1..]
    } else {
        path
    }
}

fn plural(count: u64, word: &str) -> String {
    if count == 1 { format!("{count} {word}") } else { format!("{count} {word}s") }
}

// ============================================================================
// Config Persistence
// ============================================================================

/// Parameters that can be saved/loaded as config profiles.
/// These are the user-facing parameters (excluding path which is always required).
fn configurable_param_names() -> &'static [&'static str] {
    &[
        "recursive", "checks", "format", "min_severity",
        "max_fn_length", "max_nesting", "max_line_length", "parallel",
    ]
}

/// List all saved config profiles.
fn list_saved_configs() -> Result<Value, String> {
    let store = ConfigStore::load();
    let all_prefs = store.list();
    let mut profiles: Vec<Value> = Vec::new();

    for (key, value) in &all_prefs {
        if let Some(name) = key.strip_prefix(CONFIG_NAMESPACE) {
            if let Some(obj) = value.as_object() {
                let settings: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                profiles.push(serde_json::json!({
                    "name": name,
                    "settings": settings,
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "status": "ok",
        "configs": profiles,
        "count": profiles.len(),
    }))
}

/// Delete a saved config profile by name.
fn delete_saved_config(name: &str) -> Result<Value, String> {
    let store = ConfigStore::load();
    let key = format!("{CONFIG_NAMESPACE}{name}");
    let existing = store.get(&key);

    if existing.is_none() {
        return Ok(serde_json::json!({
            "status": "error",
            "message": format!("Config profile '{name}' not found. Use --list_configs to see available profiles."),
        }));
    }

    store.delete(&key);
    Ok(serde_json::json!({
        "status": "ok",
        "action": "deleted",
        "config": name,
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeReviewTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_checks_all() {
        let checks = parse_checks("all").unwrap();
        assert!(checks.unsafe_check);
        assert!(checks.naming);
        assert!(checks.async_check);
    }

    #[test]
    fn test_parse_checks_subset() {
        let checks = parse_checks("unsafe,style").unwrap();
        assert!(checks.unsafe_check);
        assert!(checks.style);
        assert!(!checks.error_handling);
        assert!(!checks.naming);
        assert!(!checks.async_check);
    }

    #[test]
    fn test_parse_checks_invalid() {
        assert!(parse_checks("nonexistent").is_err());
    }

    #[test]
    fn test_severity_threshold() {
        assert_eq!(severity_threshold("error") as u8, Severity::Error as u8);
        assert_eq!(severity_threshold("warning") as u8, Severity::Warning as u8);
        assert_eq!(severity_threshold("info") as u8, Severity::Info as u8);
        assert_eq!(severity_threshold("invalid") as u8, Severity::Info as u8);
    }

    #[test]
    fn test_shorten_path() {
        assert_eq!(shorten_path("/a/b/c.rs"), "c.rs");
        assert_eq!(shorten_path("file.rs"), "file.rs");
        assert_eq!(shorten_path("/single/file.rs"), "file.rs");
    }

    #[test]
    fn test_plural() {
        assert_eq!(plural(1, "error"), "1 error");
        assert_eq!(plural(3, "warning"), "3 warnings");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("my_struct"), "MyStruct");
        assert_eq!(to_camel_case("foo_bar_baz"), "FooBarBaz");
        assert_eq!(to_camel_case("simple"), "Simple");
    }

    #[test]
    fn test_is_in_test_code() {
        let code = r#"
fn main() {
    let x = Some(1);
    let y = x.unwrap();
}

#[test]
fn test_foo() {
    let x = Some(1);
    let y = x.unwrap();
}
"#;
        // Position of x.unwrap() in main should NOT be test code
        let main_unwrap_pos = code.find("x.unwrap()").unwrap();
        assert!(!is_in_test_code(code, main_unwrap_pos));

        // Position of x.unwrap() in test_foo SHOULD be test code
        let test_unwrap_pos = code.rfind("x.unwrap()").unwrap();
        assert!(is_in_test_code(code, test_unwrap_pos));
    }

    #[test]
    fn test_is_in_test_code_mod_tests() {
        let code = r#"
fn main() {
    let x = Some(1);
}

mod tests {
    fn helper() {
        let x = Some(1);
        x.unwrap();
    }
}
"#;
        let unwrap_pos = code.rfind("x.unwrap()").unwrap();
        assert!(is_in_test_code(code, unwrap_pos));
    }

    #[test]
    fn test_check_unsafe_code_basic() {
        let code = r#"
fn main() {
    let x = 5;
    let ptr = &x as *const i32;
    unsafe {
        println!("{}", *ptr);
    }
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_unsafe_code(code, &lines, "test.rs", &mut issues);
        assert!(!issues.is_empty(), "Should detect unsafe block and ptr deref");
        assert!(issues.iter().any(|i| i.check == "unsafe"));
    }

    #[test]
    fn test_check_error_handling_unwrap() {
        let code = r#"
fn main() {
    let x: Option<i32> = Some(5);
    let y = x.unwrap();
    let z = x.expect("should exist");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_error_handling(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("unwrap")));
        assert!(issues.iter().any(|i| i.message.contains("expect")));
    }

    #[test]
    fn test_check_naming_bad_struct() {
        let code = r#"
struct myStruct {
    field: i32,
}

enum myEnum {
    Variant,
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("myStruct")), "Should flag lowercase struct");
        assert!(issues.iter().any(|i| i.message.contains("myEnum")), "Should flag lowercase enum");
    }

    #[test]
    fn test_check_naming_good_struct() {
        let code = r#"
struct MyStruct {
    field: i32,
}

enum MyEnum {
    Variant,
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        let naming_issues: Vec<_> = issues.iter().filter(|i| i.check == "naming").collect();
        assert!(naming_issues.is_empty(), "Should not flag correct CamelCase: {:?}", naming_issues);
    }

    #[test]
    fn test_check_naming_constants() {
        let code = r#"
const max_size: usize = 100;
const MAX_SIZE: usize = 100;
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("max_size")), "Should flag lowercase const");
        assert!(!issues.iter().any(|i| i.message.contains("MAX_SIZE")), "Should not flag SCREAMING const");
    }

    #[test]
    fn test_check_style_long_function() {
        let mut code = String::from("fn long_function() {\n");
        for _ in 0..120 {
            code.push_str("    let _ = 1;\n");
        }
        code.push_str("}\n");

        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        let thresholds = Thresholds { max_fn_length: 50, max_nesting: 8, max_line_length: 120 };
        check_style(&code, &lines, "test.rs", &mut issues, &thresholds);
        assert!(issues.iter().any(|i| i.check == "style" && i.message.contains("long_function")));
    }

    #[test]
    fn test_check_correctness_todo() {
        let code = r#"
fn main() {
    todo!("implement this");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_correctness(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("todo")));
    }

    #[test]
    fn test_check_performance_clone() {
        let code = r#"
fn main() {
    let s = String::from("hello");
    let s2 = s.clone();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_performance(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("clone")));
    }

    #[test]
    fn test_check_safety_maybe_uninit() {
        let code = r#"
use std::mem::MaybeUninit;
fn main() {
    let mut x = MaybeUninit::uninit();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_safety(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("MaybeUninit")));
    }

    #[test]
    fn test_check_concurrency_static_mut() {
        let code = r#"
static mut COUNTER: i32 = 0;
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_concurrency(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("static mut")));
    }

    #[test]
    fn test_check_documentation_missing() {
        let code = r#"
pub fn undocumented() -> i32 { 5 }

/// Documented function
pub fn documented() -> i32 { 5 }
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_documentation(code, &lines, "test.rs", &mut issues);
        let has_undocumented = issues.iter().any(|i| i.message.contains("`undocumented`"));
        let has_documented = issues.iter().any(|i| i.message.contains("`documented`"));
        assert!(has_undocumented, "Should flag undocumented fn, issues: {:?}", issues);
        assert!(!has_documented, "Should not flag documented fn, issues: {:?}", issues);
    }

    #[test]
    fn test_check_async_blocking_mutex() {
        let code = r#"
async fn async_fn() {
    let m = std::sync::Mutex::new(5);
    let _g = m.lock().unwrap();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_async(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.check == "async"), "Should detect blocking mutex in async");
    }

    #[test]
    fn test_line_at() {
        let content = "line1\nline2\nline3\n";
        assert_eq!(line_at(content, 0), 1);   // start of line 1
        assert_eq!(line_at(content, 5), 1);   // inside line 1 (newline at 5)
        assert_eq!(line_at(content, 6), 2);   // after first newline, start of line 2
        assert_eq!(line_at(content, 12), 3);  // after newline at 11, start of line 3
        assert_eq!(line_at(content, 16), 3);  // inside line 3 (newline at 17)
        assert_eq!(line_at(content, 17), 3);  // at the final newline itself (part of line 3)
    }

    #[test]
    fn test_extract_var_name() {
        let ctx = "x.unwrap()";
        assert_eq!(extract_var_name(ctx), "x");

        let ctx2 = "something_else.unwrap()";
        assert_eq!(extract_var_name(ctx2), "something_else");
    }

    // Security check tests
    #[test]
    fn test_check_security_sql_injection() {
        let code = r#"
fn bad_query(user: &str) {
    let sql = format!("SELECT * FROM users WHERE name = '{}'", user);
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("SQL injection")),
            "Should detect SQL injection, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_hardcoded_secret() {
        let code = r#"
fn configure() {
    let api_key = "sk-abcdefghijklmnopqrstuvwxyz123456";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("hardcoded secret")),
            "Should detect hardcoded secret, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_private_key() {
        let code = r#"
fn main() {
    let key = "-----BEGIN RSA PRIVATE KEY-----abc123-----END RSA PRIVATE KEY-----";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("Private key")),
            "Should detect private key, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_openai_key() {
        let code = r#"
fn call_llm() {
    let key = "sk-abcdefghijklmnopqrstuvwxyz123456";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("OpenAI API key")),
            "Should detect OpenAI key, issues: {:?}", issues);
    }

    #[test]
    fn test_parse_checks_security() {
        let checks = parse_checks("security").unwrap();
        assert!(checks.security);
        assert!(!checks.unsafe_check);
        assert!(!checks.naming);
    }

    // Ignore system tests
    #[test]
    fn test_parse_ignore_directives_single() {
        let code = "// code-review: ignore[unsafe]\nunsafe {}\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].0, 1);
        assert_eq!(ignores[0].1, "unsafe");
    }

    #[test]
    fn test_parse_ignore_directives_multiple() {
        let code = "// code-review: ignore[unsafe,style,error_handling]\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 3);
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "unsafe"));
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "style"));
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "error_handling"));
    }

    #[test]
    fn test_parse_ignore_directives_all() {
        let code = "// code-review: ignore[all]\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].1, "all");
    }

    #[test]
    fn test_is_ignored_exact() {
        let ignores = vec![(5, "unsafe".to_string()), (10, "style".to_string())];
        assert!(is_ignored(&ignores, 5, "unsafe"));
        assert!(!is_ignored(&ignores, 5, "style"));
        assert!(is_ignored(&ignores, 10, "style"));
        assert!(!is_ignored(&ignores, 10, "unsafe"));
        assert!(!is_ignored(&ignores, 3, "unsafe"));
    }

    #[test]
    fn test_is_ignored_all() {
        let ignores = vec![(5, "all".to_string())];
        assert!(is_ignored(&ignores, 5, "unsafe"));
        assert!(is_ignored(&ignores, 5, "style"));
        assert!(is_ignored(&ignores, 5, "error_handling"));
        assert!(!is_ignored(&ignores, 6, "unsafe"));
    }

    #[test]
    fn test_ignore_directive_suppresses_issue() {
        // Test that an unsafe block is suppressed by ignore directive on the same line
        let code = "unsafe { let x = 1; } // code-review: ignore[unsafe]\n";
        let lines: Vec<&str> = code.lines().collect();
        let ignores = parse_ignore_directives(code);

        let mut issues = Vec::new();
        check_unsafe_code(code, &lines, "test.rs", &mut issues);

        // Apply ignore filter
        issues.retain(|i| !is_ignored(&ignores, i.line, &i.check));
        assert!(issues.is_empty(), "Unsafe issue should be suppressed by ignore directive, got: {:?}", issues);
    }

    // ========================================================================
    // P1: Complexity tests
    // ========================================================================

    #[test]
    fn test_check_complexity_large_file() {
        let mut code = String::new();
        for i in 0..1200 {
            code.push_str(&format!("let x_{} = {};\n", i, i));
        }
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(&code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains(">1000")),
            "Should flag large file, got: {:?}", issues);
    }

    #[test]
    fn test_check_complexity_excessive_params() {
        let code = r#"
fn bad_function(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("parameters")),
            "Should flag excessive params, got: {:?}", issues);
    }

    #[test]
    fn test_check_complexity_ok_params() {
        let code = r#"
fn good_function(a: i32, b: i32) {}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        let param_issues: Vec<_> = issues.iter().filter(|i| i.message.contains("parameters")).collect();
        assert!(param_issues.is_empty(), "Should not flag 2 params, got: {:?}", param_issues);
    }

    #[test]
    fn test_check_complexity_high_cyclomatic() {
        let code = r#"
fn complex_function(x: i32) -> i32 {
    let mut result = 0;
    if x > 0 && x < 100 {
        if x > 10 || x < 5 {
            if x > 20 {
                if x < 30 {
                    result = 1;
                }
            }
        }
    }
    match x {
        1 => result = 10,
        2 => result = 20,
        3 => result = 30,
        _ => result = 0,
    }
    for i in 0..x {
        result += i;
    }
    while result > 0 {
        result -= 1;
    }
    result
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("complexity")),
            "Should flag high cyclomatic complexity, got: {:?}", issues);
    }

    // ========================================================================
    // P1: Testing quality tests
    // ========================================================================

    #[test]
    fn test_check_testing_no_assertions() {
        let code = r#"
#[test]
fn test_no_assert() {
    let x = 5;
    let y = 10;
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("no assertions")),
            "Should flag test without assertions, got: {:?}", issues);
    }

    #[test]
    fn test_check_testing_with_assertions() {
        let code = r#"
#[test]
fn test_with_assert() {
    let x = 5;
    assert_eq!(x, 5);
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(code, &lines, "test.rs", &mut issues);
        let no_assert_issues: Vec<_> = issues.iter().filter(|i| i.message.contains("no assertions")).collect();
        assert!(no_assert_issues.is_empty(), "Should not flag test with assertions, got: {:?}", no_assert_issues);
    }

    #[test]
    fn test_check_testing_long_test() {
        let mut code = String::from("#[test]\nfn test_long() {\n");
        for _ in 0..120 {
            code.push_str("    let _x = 1;\n");
        }
        code.push_str("}\n");

        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(&code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("long")),
            "Should flag long test, got: {:?}", issues);
    }

    // ========================================================================
    // P1: Debug residual tests
    // ========================================================================

    #[test]
    fn test_check_debug_dbg() {
        let code = r#"
fn calculate() -> i32 {
    let x = 5;
    dbg!(x);
    x + 1
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_debug(code, &lines, "src/lib.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("dbg!")),
            "Should flag dbg!(), got: {:?}", issues);
    }

    #[test]
    fn test_check_debug_eprintln() {
        let code = r#"
fn process() {
    eprintln!("processing...");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_debug(code, &lines, "src/lib.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("eprintln!")),
            "Should flag eprintln!(), got: {:?}", issues);
    }

    #[test]
    fn test_check_debug_parse_checks() {
        let checks = parse_checks("complexity,testing,debug").unwrap();
        assert!(checks.complexity);
        assert!(checks.testing);
        assert!(checks.debug);
        assert!(!checks.unsafe_check);
        assert!(!checks.naming);
    }

    // ========================================================================
    // CI Integration tests
    // ========================================================================

    #[test]
    fn test_resolve_format_text_default() {
        assert_eq!(resolve_format("text"), "text");
        assert_eq!(resolve_format("json"), "json");
        assert_eq!(resolve_format("github-actions"), "github-actions");
        assert_eq!(resolve_format("gitlab-ci"), "gitlab-ci");
    }

    #[test]
    fn test_resolve_format_auto_text() {
        // Without CI env vars set, auto should resolve to "text"
        let old_gh = std::env::var("GITHUB_ACTIONS").ok();
        let old_gl = std::env::var("GITLAB_CI").ok();
        std::env::remove_var("GITHUB_ACTIONS");
        std::env::remove_var("GITLAB_CI");

        let result = resolve_format("auto");
        assert_eq!(result, "text");

        // Restore
        if let Some(v) = old_gh { std::env::set_var("GITHUB_ACTIONS", v); }
        if let Some(v) = old_gl { std::env::set_var("GITLAB_CI", v); }
    }

    #[test]
    fn test_resolve_format_case_insensitive() {
        assert_eq!(resolve_format("GITHUB-ACTIONS"), "github-actions");
        assert_eq!(resolve_format("GitLab-CI"), "gitlab-ci");
        assert_eq!(resolve_format("JSON"), "json");
    }

    #[test]
    fn test_generate_github_actions_report_basic() {
        let issues = vec![
            ReviewIssue {
                severity: Severity::Error,
                check: "unsafe".to_string(),
                file: "src/main.rs".to_string(),
                line: 10,
                column: 5,
                message: "Unsafe block detected.".to_string(),
                recommendation: Some("Add SAFETY comments.".to_string()),
            },
            ReviewIssue {
                severity: Severity::Warning,
                check: "style".to_string(),
                file: "src/lib.rs".to_string(),
                line: 25,
                column: 1,
                message: "Long line.".to_string(),
                recommendation: None,
            },
        ];

        let result = generate_github_actions_report(&issues, 1, 1, 0).unwrap();
        let report = result["report"].as_str().unwrap();

        assert!(report.contains("::error file=src/main.rs,line=10,col=5,title=unsafe/error::Unsafe block detected."),
            "Should contain GitHub error annotation, got: {}", report);
        assert!(report.contains("::warning file=src/lib.rs,line=25,col=1,title=style/warning::Long line."),
            "Should contain GitHub warning annotation, got: {}", report);
        assert!(report.contains("::notice title=Code Review Summary"),
            "Should contain summary notice, got: {}", report);
    }

    #[test]
    fn test_generate_github_actions_report_empty() {
        let result = generate_github_actions_report(&[], 0, 0, 0).unwrap();
        let report = result["report"].as_str().unwrap();
        assert!(report.contains("0 issues"));
        // Only the summary annotation, no issue annotations
        let error_count = report.matches("::error").count();
        let warning_count = report.matches("::warning").count();
        let notice_count = report.matches("::notice").count();
        assert_eq!(error_count, 0, "No error annotations for empty issues");
        assert_eq!(warning_count, 0, "No warning annotations for empty issues");
        assert_eq!(notice_count, 1, "Should have exactly 1 notice (summary)");
    }

    #[test]
    fn test_generate_gitlab_ci_report_basic() {
        let issues = vec![
            ReviewIssue {
                severity: Severity::Error,
                check: "unsafe".to_string(),
                file: "src/main.rs".to_string(),
                line: 10,
                column: 1,
                message: "Unsafe block.".to_string(),
                recommendation: Some("Fix it.".to_string()),
            },
        ];

        let result = generate_gitlab_ci_report(&issues, 1, 0, 0).unwrap();
        let report = result["report"].as_array().unwrap();
        assert_eq!(report.len(), 1);

        let item = &report[0];
        assert_eq!(item["check_name"], "unsafe");
        assert_eq!(item["description"], "Unsafe block.");
        assert_eq!(item["severity"], "blocker");
        assert_eq!(item["location"]["path"], "src/main.rs");
        assert_eq!(item["location"]["lines"]["begin"], 10);
        assert_eq!(item["fingerprint"], "unsafe:src/main.rs:10");
    }

    #[test]
    fn test_generate_gitlab_ci_report_empty() {
        let result = generate_gitlab_ci_report(&[], 0, 0, 0).unwrap();
        let report = result["report"].as_array().unwrap();
        assert!(report.is_empty());
    }

    #[test]
    fn test_gitlab_ci_severity_mapping() {
        let issues = vec![
            ReviewIssue { severity: Severity::Error, check: "err".to_string(), file: "a.rs".to_string(), line: 1, column: 1, message: "E".to_string(), recommendation: None },
            ReviewIssue { severity: Severity::Warning, check: "warn".to_string(), file: "a.rs".to_string(), line: 2, column: 1, message: "W".to_string(), recommendation: None },
            ReviewIssue { severity: Severity::Info, check: "info".to_string(), file: "a.rs".to_string(), line: 3, column: 1, message: "I".to_string(), recommendation: None },
        ];

        let result = generate_gitlab_ci_report(&issues, 1, 1, 1).unwrap();
        let report = result["report"].as_array().unwrap();
        assert_eq!(report[0]["severity"], "blocker");
        assert_eq!(report[1]["severity"], "major");
        assert_eq!(report[2]["severity"], "minor");
    }

    // ========================================================================
    // P2: Parallel Processing tests
    // ========================================================================

    #[test]
    fn test_analyze_file_basic() {
        let checks = ActiveChecks::all();
        let thresholds = Thresholds { max_fn_length: 100, max_nesting: 8, max_line_length: 120 };

        // Write a temporary Rust file to analyze
        let tmp = std::env::temp_dir().join("code_review_test_analyze_basic.rs");
        std::fs::write(&tmp, r#"
fn main() {
    let x = unsafe { 5 };
    unsafe fn bad() {}
    println!("hello");
    let _ = std::fs::read_to_string("/nonexistent");
}
"#).unwrap();

        let result = analyze_file(tmp.to_str().unwrap(), &checks, &thresholds, "info");
        assert!(result.is_some(), "analyze_file should return Some");
        let r = result.unwrap();
        assert!(r.errors > 0 || r.warnings > 0, "Should find issues, got errors={}, warnings={}, info={}", r.errors, r.warnings, r.info);
        assert!(r.issues.iter().any(|i| i.check == "unsafe"), "Should find unsafe issues");
        assert!(r.issues.iter().any(|i| i.check == "debug"), "Should find println!/debug issues");

        // Clean up
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_analyze_file_io_error() {
        let checks = ActiveChecks::all();
        let thresholds = Thresholds { max_fn_length: 100, max_nesting: 8, max_line_length: 120 };

        // Non-existent file
        let result = analyze_file("/tmp/nonexistent_file_12345.rs", &checks, &thresholds, "info");
        assert!(result.is_some(), "Should return Some even for I/O error");
        let r = result.unwrap();
        assert!(r.issues.iter().any(|i| i.check == "io"), "Should produce an I/O error issue");
        assert_eq!(r.errors, 1);
    }

    #[test]
    fn test_analyze_file_severity_filtering() {
        let checks = ActiveChecks::all();
        let thresholds = Thresholds { max_fn_length: 100, max_nesting: 8, max_line_length: 120 };

        let tmp = std::env::temp_dir().join("code_review_test_severity.rs");
        std::fs::write(&tmp, r#"
pub fn ok_fn() {}

/// docs
pub fn documented() {}

// This triggers Error-level issues:
unsafe trait BadTrait {}
panic!("oh no");
"#).unwrap();

        // With "error" severity filter, only errors should pass
        let result = analyze_file(tmp.to_str().unwrap(), &checks, &thresholds, "error");
        assert!(result.is_some());
        let r = result.unwrap();
        // unsafe trait should be an Error severity issue
        assert!(r.issues.iter().any(|i| i.severity == Severity::Error), "Should retain errors, got: {:?}", r.issues);
        // panic! should be an Error severity issue
        assert!(r.issues.iter().any(|i| i.message.contains("panic!")), "Should flag panic! as error");
        // Info/warning items should be filtered out
        assert!(r.issues.iter().all(|i| i.severity == Severity::Error), "Only errors should remain");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_parallel_parameter_parsed() {
        // Test that the 'parallel' parameter is present in the tool definition
        let tool = CodeReviewTool;
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "parallel"), "Should have parallel parameter");
    }
}
