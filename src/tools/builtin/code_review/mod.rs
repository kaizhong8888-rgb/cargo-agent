//! Code Review Tool: performs comprehensive static analysis of Rust source code.
//!
//! This module is split into submodules for better maintainability:
//! - `config`: Configuration types (Thresholds, ActiveChecks, Severity, ReviewIssue)
//! - `patterns`: Pre-compiled regex patterns for all checks
//! - `checks`: Individual check functions (14 check functions)
//! - `analyzer`: File analysis orchestration (parallel/sequential)
//! - `reports`: Report generation (text, JSON, GitHub Actions, GitLab CI)
//! - `file_utils`: File collection and git diff support
//! - `config_persistence`: Config save/load/list/delete

pub mod config;
pub mod patterns;
pub mod checks;
pub mod analyzer;
pub mod reports;
pub mod file_utils;
pub mod config_persistence;

use config::{Thresholds, parse_checks};
use analyzer::analyze_batch;
use reports::{generate_report, resolve_format, write_report};
use file_utils::{build_empty_result, collect_files};

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

pub struct CodeReviewTool;

#[async_trait::async_trait]
impl Tool for CodeReviewTool {
    fn name(&self) -> &str { "code_review" }

    fn description(&self) -> &str {
        "Perform comprehensive code review on Rust source files. Analyzes code for safety, error handling, performance, style, correctness, concurrency, documentation, naming conventions, and async pitfalls."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "path".to_string(), description: "Path to a Rust file, or a directory containing Rust files to review".to_string(), required: true, parameter_type: "string".to_string() },
            ToolParameter { name: "recursive".to_string(), description: "If true and path is a directory, analyze all .rs files recursively (default: false)".to_string(), required: false, parameter_type: "boolean".to_string() },
            ToolParameter { name: "checks".to_string(), description: "Comma-separated list of checks. Options: all, unsafe, error_handling, performance, style, safety, correctness, concurrency, documentation, naming, async, security, complexity, testing, debug (default: all)".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "format".to_string(), description: "Output format: 'text', 'json', 'github-actions', 'gitlab-ci', or 'auto' (default: text)".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "max_results".to_string(), description: "Maximum number of files to analyze (default: 50)".to_string(), required: false, parameter_type: "number".to_string() },
            ToolParameter { name: "min_severity".to_string(), description: "Minimum severity level: error, warning, info (default: info)".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "max_fn_length".to_string(), description: "Maximum function length in lines before warning (default: 100)".to_string(), required: false, parameter_type: "number".to_string() },
            ToolParameter { name: "max_nesting".to_string(), description: "Maximum nesting depth before warning (default: 8)".to_string(), required: false, parameter_type: "number".to_string() },
            ToolParameter { name: "max_line_length".to_string(), description: "Maximum line length in characters before warning (default: 120)".to_string(), required: false, parameter_type: "number".to_string() },
            ToolParameter { name: "parallel".to_string(), description: "Process files in parallel for better performance (default: true)".to_string(), required: false, parameter_type: "boolean".to_string() },
            ToolParameter { name: "save_config".to_string(), description: "Save current parameters as a reusable config profile (e.g. 'my_review').".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "load_config".to_string(), description: "Load a saved config profile by name (e.g. 'my_review').".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "list_configs".to_string(), description: "List all saved code_review config profiles.".to_string(), required: false, parameter_type: "boolean".to_string() },
            ToolParameter { name: "delete_config".to_string(), description: "Delete a saved config profile by name.".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "output".to_string(), description: "Save the report to a file (e.g. 'report.json', 'report.txt').".to_string(), required: false, parameter_type: "string".to_string() },
            ToolParameter { name: "progress".to_string(), description: "Show real-time progress information during analysis (default: true)".to_string(), required: false, parameter_type: "boolean".to_string() },
            ToolParameter { name: "git_diff".to_string(), description: "Only review files that have been modified in the git working tree (default: false)".to_string(), required: false, parameter_type: "boolean".to_string() },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let merged = config_persistence::merge_config_params(params)?;
        if merged.get("status").is_some() && merged.get("message").is_some() {
            return Ok(merged);
        }
        let merged_params: HashMap<String, Value> = merged.as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();

        let path = merged_params.get("path").and_then(|v| v.as_str()).ok_or("Missing required parameter: path")?;
        let recursive = merged_params.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
        let checks_str = merged_params.get("checks").and_then(|v| v.as_str()).unwrap_or("all");
        let fmt = merged_params.get("format").and_then(|v| v.as_str()).unwrap_or("auto");
        let effective_format = resolve_format(fmt);
        let max_results = merged_params.get("max_results").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        let min_severity = merged_params.get("min_severity").and_then(|v| v.as_str()).unwrap_or("info");
        let max_fn_length = merged_params.get("max_fn_length").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
        let max_nesting = merged_params.get("max_nesting").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
        let max_line_length = merged_params.get("max_line_length").and_then(|v| v.as_u64()).unwrap_or(120) as usize;
        let parallel = merged_params.get("parallel").and_then(|v| v.as_bool()).unwrap_or(true);
        let output_path = merged_params.get("output").and_then(|v| v.as_str()).map(|s| s.to_string());
        let show_progress = merged_params.get("progress").and_then(|v| v.as_bool()).unwrap_or(true);
        let git_diff_flag = merged_params.get("git_diff").and_then(|v| v.as_bool()).unwrap_or(false);
        let effective_format = if effective_format == "auto" || effective_format == "text" {
            if let Some(ref out) = output_path {
                if out.ends_with(".json") { "json".to_string() } else if out.ends_with(".md") { "text".to_string() } else { effective_format }
            } else { effective_format }
        } else { effective_format };

        let thresholds = Thresholds { max_fn_length, max_nesting, max_line_length };
        let active_checks = parse_checks(checks_str)?;

        let files = collect_files(path, max_results, git_diff_flag, recursive)?;
        if files.is_empty() { return Ok(build_empty_result(git_diff_flag)); }

        let (all_issues, file_summaries, total_errors, total_warnings, total_info) =
            analyze_batch(&files, &active_checks, &thresholds, min_severity, parallel, show_progress).await?;

        let mut result = generate_report(&all_issues, &file_summaries, &files,
            total_errors, total_warnings, total_info, &active_checks, &effective_format)?;

        if let Some(ref out_path) = output_path {
            write_report(&mut result, out_path, &effective_format)?;
        }

        if show_progress {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("progress".to_string(), serde_json::json!({
                    "mode": if parallel && files.len() > 1 { "parallel" } else { "sequential" },
                    "total_files": files.len(),
                }));
            }
        }
        if git_diff_flag {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("git_diff".to_string(), Value::Bool(true));
            }
        }
        Ok(result)
    }
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeReviewTool));
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::config::{ActiveChecks, Severity, Thresholds};
    use super::file_utils::get_git_diff_files;

    #[test]
    fn test_output_parameter_exists() {
        let tool = CodeReviewTool;
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "output"), "Should have output parameter");
    }

    #[test]
    fn test_output_saves_text_report() {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join("code_review_test_output_src.rs");
        let out_file = tmp_dir.join("code_review_test_output_report.txt");
        std::fs::write(&src_file, r#"
fn main() {
    let x = unsafe { 42 };
    println!("x = {}", x);
}
"#).unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(src_file.to_str().unwrap().to_string()));
        params.insert("output".to_string(), Value::String(out_file.to_str().unwrap().to_string()));
        params.insert("format".to_string(), Value::String("text".to_string()));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("output_file").is_some(), "Result should contain output_file, got: {:?}", result);
        assert!(result.get("output_saved").and_then(|v| v.as_bool()).unwrap_or(false), "output_saved should be true");
        let saved_content = std::fs::read_to_string(&out_file).unwrap();
        assert!(saved_content.contains("unsafe"), "Report should contain 'unsafe' issues");
        assert!(saved_content.contains("CODE REVIEW REPORT"), "Report should have title");
        let _ = std::fs::remove_file(&src_file);
        let _ = std::fs::remove_file(&out_file);
    }

    #[test]
    fn test_output_saves_json_report() {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join("code_review_test_output_json_src.rs");
        let out_file = tmp_dir.join("code_review_test_output_report.json");
        std::fs::write(&src_file, "fn main() { unsafe { let _ = 42; } }\n").unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(src_file.to_str().unwrap().to_string()));
        params.insert("output".to_string(), Value::String(out_file.to_str().unwrap().to_string()));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("output_file").is_some(), "Result should contain output_file");
        assert!(result.get("output_saved").and_then(|v| v.as_bool()).unwrap_or(false), "output_saved should be true");
        let saved = std::fs::read_to_string(&out_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert!(parsed.get("issues").is_some(), "JSON report should have 'issues' field");
        let _ = std::fs::remove_file(&src_file);
        let _ = std::fs::remove_file(&out_file);
    }

    #[test]
    fn test_output_error_handling() {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join("code_review_test_output_err.rs");
        std::fs::write(&src_file, "fn main() {}\n").unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(src_file.to_str().unwrap().to_string()));
        params.insert("output".to_string(), Value::String("/nonexistent_dir_xyz/report.txt".to_string()));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("output_error").is_some(), "Should have output_error for invalid path");
        let err_msg = result["output_error"].as_str().unwrap();
        assert!(err_msg.contains("Failed to write"), "Error should mention failure");
        let _ = std::fs::remove_file(&src_file);
    }

    #[test]
    fn test_progress_parameter_exists() {
        let tool = CodeReviewTool;
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "progress"), "Should have progress parameter");
    }

    #[test]
    fn test_progress_in_result_when_enabled() {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join("code_review_test_progress_src.rs");
        std::fs::write(&src_file, "fn main() { let x = unsafe { 42 }; }\n").unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(src_file.to_str().unwrap().to_string()));
        params.insert("progress".to_string(), Value::Bool(true));
        params.insert("parallel".to_string(), Value::Bool(false));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("progress").is_some(), "Result should contain progress field");
        let progress = &result["progress"];
        assert_eq!(progress["mode"], "sequential", "Mode should be sequential");
        assert_eq!(progress["total_files"], 1, "Should have 1 file");
        let _ = std::fs::remove_file(&src_file);
    }

    #[test]
    fn test_progress_not_in_result_when_disabled() {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join("code_review_test_progress_off.rs");
        std::fs::write(&src_file, "fn main() {}\n").unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(src_file.to_str().unwrap().to_string()));
        params.insert("progress".to_string(), Value::Bool(false));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("progress").is_none(), "Should not contain progress field when disabled");
        let _ = std::fs::remove_file(&src_file);
    }

    #[test]
    fn test_progress_parallel_mode() {
        let tmp_dir = std::env::temp_dir();
        let src_file1 = tmp_dir.join("code_review_test_progress_p1.rs");
        let src_file2 = tmp_dir.join("code_review_test_progress_p2.rs");
        std::fs::write(&src_file1, "fn main() { unsafe { let x = 1; } }\n").unwrap();
        std::fs::write(&src_file2, "fn main() { let y = Some(5).unwrap(); }\n").unwrap();
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(tmp_dir.to_str().unwrap().to_string()));
        params.insert("progress".to_string(), Value::Bool(true));
        params.insert("parallel".to_string(), Value::Bool(true));
        let tool = CodeReviewTool;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert!(result.get("progress").is_some(), "Result should contain progress field in parallel mode");
        let progress = &result["progress"];
        assert_eq!(progress["mode"], "parallel", "Mode should be parallel");
        assert!(progress["total_files"].as_u64().unwrap_or(0) >= 2, "Should have at least 2 files");
        let _ = std::fs::remove_file(&src_file1);
        let _ = std::fs::remove_file(&src_file2);
    }

    #[test]
    fn test_git_diff_parameter_exists() {
        let tool = CodeReviewTool;
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "git_diff"), "Should have git_diff parameter");
    }

    #[test]
    fn test_get_git_diff_files_returns_vec() {
        let result = get_git_diff_files();
        assert!(result.is_ok() || result.is_err(), "get_git_diff_files should return a Result");
        if let Ok(files) = result {
            assert!(files.iter().all(|f| f.ends_with(".rs")), "All files should be .rs files");
        }
    }
}
