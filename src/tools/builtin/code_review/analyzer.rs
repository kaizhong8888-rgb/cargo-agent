//! File analysis orchestration — single-file and batch (parallel/sequential).

use super::checks::{parse_ignore_directives, run_all_checks, is_ignored};
use super::config::{ActiveChecks, ReviewIssue, Severity, Thresholds, severity_threshold};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Per-file analysis result returned by `analyze_file`.
pub(super) struct FileAnalysisResult {
    pub(super) issues: Vec<ReviewIssue>,
    pub(super) summary: Value,
    pub(super) errors: u32,
    pub(super) warnings: u32,
    pub(super) info: u32,
}

/// Analyze a single file: read content, run all enabled checks, apply ignores and
/// severity filtering. Returns `None` if the file could not be read (an I/O error
/// issue is included in the result's issue list in that case).
pub(super) fn analyze_file(
    file: &str,
    active_checks: &ActiveChecks,
    thresholds: &Thresholds,
    min_severity: &str,
) -> Option<FileAnalysisResult> {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
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
    let mut file_issues = run_all_checks(&content, &lines, file, active_checks, thresholds);

    // Apply ignore directives
    let ignores = parse_ignore_directives(&content);
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

/// Run analysis across all files, either sequentially or in parallel.
pub(super) async fn analyze_batch(
    files: &[String],
    active_checks: &ActiveChecks,
    thresholds: &Thresholds,
    min_severity: &str,
    parallel: bool,
    show_progress: bool,
) -> Result<(Vec<ReviewIssue>, Vec<Value>, u32, u32, u32), String> {
    let mut all_issues: Vec<ReviewIssue> = Vec::new();
    let mut file_summaries: Vec<Value> = Vec::new();
    let mut total_errors = 0u32;
    let mut total_warnings = 0u32;
    let mut total_info = 0u32;

    if parallel && files.len() > 1 {
        let num_files = files.len();
        let mut handles = Vec::with_capacity(num_files);
        let completed = Arc::new(AtomicUsize::new(0));

        if show_progress {
            eprintln!("[PAR] Starting parallel analysis of {num_files} files...");
        }

        for file in files {
            let file = file.clone();
            let checks_clone = active_checks.clone();
            let thresh_clone = thresholds.clone();
            let min_sev = min_severity.to_string();
            let completed_clone = completed.clone();
            let show_prog = show_progress;

            handles.push(tokio::task::spawn_blocking(move || {
                let result = analyze_file(&file, &checks_clone, &thresh_clone, &min_sev);
                let done = completed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                if show_prog {
                    eprintln!("[PAR] [{done}/{num_files}] Completed: {file}");
                }
                result
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
                Ok(None) => {}
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
        let num_files = files.len();
        if show_progress && num_files > 0 {
            eprintln!("[SEQ] Starting sequential analysis of {num_files} files...");
        }
        for (idx, file) in files.iter().enumerate() {
            if show_progress {
                eprintln!("[SEQ] [{}/{}] Analyzing: {}...", idx + 1, num_files, file);
            }
            if let Some(result) = analyze_file(file, active_checks, thresholds, min_severity) {
                let issue_count = result.issues.len();
                total_errors += result.errors;
                total_warnings += result.warnings;
                total_info += result.info;
                file_summaries.push(result.summary);
                all_issues.extend(result.issues);
                if show_progress {
                    eprintln!(
                        "[SEQ] [{}/{}] Completed: {} ({} issues)",
                        idx + 1, num_files, file, issue_count
                    );
                }
            }
        }
    }

    Ok((all_issues, file_summaries, total_errors, total_warnings, total_info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::{ActiveChecks, Thresholds};

    #[test]
    fn test_analyze_file_basic() {
        let checks = ActiveChecks::all();
        let thresholds = Thresholds { max_fn_length: 100, max_nesting: 8, max_line_length: 120 };
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
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_analyze_file_io_error() {
        let checks = ActiveChecks::all();
        let thresholds = Thresholds { max_fn_length: 100, max_nesting: 8, max_line_length: 120 };
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

        let result = analyze_file(tmp.to_str().unwrap(), &checks, &thresholds, "error");
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.issues.iter().any(|i| i.severity == Severity::Error), "Should retain errors, got: {:?}", r.issues);
        assert!(r.issues.iter().any(|i| i.message.contains("panic!")), "Should flag panic! as error");
        assert!(r.issues.iter().all(|i| i.severity == Severity::Error), "Only errors should remain");
        let _ = std::fs::remove_file(&tmp);
    }
}
