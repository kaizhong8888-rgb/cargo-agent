//! Report generation — text, JSON, GitHub Actions, and GitLab CI formats.

use super::config::{ActiveChecks, ReviewIssue, Severity};
use serde_json::Value;
use std::collections::HashMap;

/// Resolve the output format, auto-detecting CI environment if set to "auto".
pub(super) fn resolve_format(requested: &str) -> String {
    match requested.trim().to_lowercase().as_str() {
        "auto" => {
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

#[inline]
pub(super) fn shorten_path(path: &str) -> &str {
    if let Some(pos) = path.rfind('/') {
        &path[pos + 1..]
    } else {
        path
    }
}

#[inline]
pub(super) fn plural(count: u64, word: &str) -> String {
    if count == 1 {
        format!("{count} {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// Generate a human-readable text report.
pub(super) fn generate_text_report(
    issues: &[ReviewIssue],
    file_summaries: &[Value],
    files: &[String],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
) -> Result<Value, String> {
    let total_issues = issues.len();
    let mut report = String::new();
    report.push_str(
        "╔══════════════════════════════════════════════════════════════════╗\n\
         ║                       CODE REVIEW REPORT                       ║\n\
         ╚══════════════════════════════════════════════════════════════════╝\n\n",
    );
    report.push_str(&format!("Files reviewed: {}\n", files.len()));
    report.push_str(&format!("Total issues found: {total_issues}\n"));
    report.push_str(&format!(
        "  {e} Errors | {w} Warnings | {i} Info\n\n",
        e = total_errors,
        w = total_warnings,
        i = total_info
    ));

    report.push_str("─── File Summary ──────────────────────────────────────────────\n");
    for fs in file_summaries {
        let file = fs["file"].as_str().unwrap_or("unknown");
        let errs = fs["errors"].as_u64().unwrap_or(0);
        let warns = fs["warnings"].as_u64().unwrap_or(0);
        let infos = fs["info"].as_u64().unwrap_or(0);
        let lines = fs["code_lines"].as_u64().unwrap_or(0);
        report.push_str(&format!(
            "  {:<40} {:>4} issues ({}, {}, {}) - {} lines\n",
            file,
            errs + warns + infos,
            plural(errs, "error"),
            plural(warns, "warning"),
            plural(infos, "info"),
            lines,
        ));
    }
    report.push('\n');

    if !issues.is_empty() {
        report.push_str("─── Issues ────────────────────────────────────────────────────\n\n");
        let errors: Vec<&ReviewIssue> = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Error))
            .collect();
        let warnings: Vec<&ReviewIssue> = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Warning))
            .collect();
        let infos: Vec<&ReviewIssue> = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Info))
            .collect();

        for (label, icon, group) in [
            ("ERRORS", "🔴", &errors),
            ("WARNINGS", "🟡", &warnings),
            ("INFO", "🔵", &infos),
        ] {
            if group.is_empty() {
                continue;
            }
            report.push_str(&format!("{icon} {label} ({})\n", group.len()));
            report.push_str("   ──────────────────────────────────────────────────────\n");
            for issue in group.iter() {
                report.push_str(&format!(
                    "   [{:<6}] {}:{}\n   {:>10} {}\n",
                    issue.check,
                    shorten_path(&issue.file),
                    issue.line,
                    "",
                    issue.message,
                ));
                if let Some(rec) = &issue.recommendation {
                    report.push_str(&format!("   {:>10} 💡 {}\n\n", "", rec));
                } else {
                    report.push('\n');
                }
            }
        }

        report.push_str("\n─── Check Summary ───────────────────────────────────────────\n");
        report.push_str(&format!(
            "  {:<20} {:>5} {:>5} {:>5}\n",
            "Check", "Errors", "Warn.", "Info"
        ));
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
        sorted.sort_by_key(|b| std::cmp::Reverse((b.1 .0, b.1 .1, b.1 .2)));
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

/// Generate a structured JSON report.
pub(super) fn generate_json_report(
    issues: &[ReviewIssue],
    file_summaries: &[Value],
    files: &[String],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
    _checks: &ActiveChecks,
) -> Result<Value, String> {
    let json_issues: Vec<Value> = issues
        .iter()
        .map(|i| {
            serde_json::json!({
                "severity": i.severity.as_str(),
                "check": i.check,
                "file": i.file,
                "line": i.line,
                "column": i.column,
                "message": i.message,
                "recommendation": i.recommendation,
            })
        })
        .collect();

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

/// Generate GitHub Actions annotation report.
pub(super) fn generate_github_actions_report(
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
        let message = issue
            .message
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
            "files": "N/A", "total_issues": total_issues,
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
    }))
}

/// Generate GitLab CI Code Quality report.
pub(super) fn generate_gitlab_ci_report(
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
            "content": { "body": issue.recommendation.as_deref().unwrap_or("") },
            "categories": ["Code Style"],
            "location": {
                "path": issue.file,
                "lines": { "begin": issue.line, "end": issue.line },
            },
            "severity": severity,
            "fingerprint": format!("{}:{}:{}", issue.check, issue.file, issue.line),
        }));
    }

    let summary = format!(
        "Code Review Summary: {total_issues} issues found ({total_errors} errors, {total_warnings} warnings, {total_info} info)\n"
    );

    Ok(serde_json::json!({
        "status": "ok", "format": "gitlab-ci",
        "report": gitlab_issues,
        "summary_text": summary,
        "summary": {
            "files": "N/A", "total_issues": total_issues,
            "errors": total_errors, "warnings": total_warnings, "info": total_info,
        },
    }))
}

#[allow(clippy::too_many_arguments)]
/// Generate the final report based on format.
pub(super) fn generate_report(
    all_issues: &[ReviewIssue],
    file_summaries: &[Value],
    files: &[String],
    total_errors: u32,
    total_warnings: u32,
    total_info: u32,
    active_checks: &ActiveChecks,
    effective_format: &str,
) -> Result<Value, String> {
    let mut sorted_issues = all_issues.to_vec();
    sorted_issues.sort_by(|a, b| {
        let sev_cmp = (b.severity as u8).cmp(&(a.severity as u8));
        if sev_cmp != std::cmp::Ordering::Equal {
            sev_cmp
        } else {
            a.file.cmp(&b.file)
        }
    });

    match effective_format {
        "json" => generate_json_report(
            &sorted_issues,
            file_summaries,
            files,
            total_errors,
            total_warnings,
            total_info,
            active_checks,
        ),
        "github-actions" | "github_actions" => {
            generate_github_actions_report(&sorted_issues, total_errors, total_warnings, total_info)
        }
        "gitlab-ci" | "gitlab_ci" => {
            generate_gitlab_ci_report(&sorted_issues, total_errors, total_warnings, total_info)
        }
        _ => generate_text_report(
            &sorted_issues,
            file_summaries,
            files,
            total_errors,
            total_warnings,
            total_info,
        ),
    }
}

/// Write report to file.
pub(super) fn write_report(
    result: &mut Value,
    out_path: &str,
    effective_format: &str,
) -> Result<(), String> {
    let report_content = if result.get("report").and_then(|v| v.as_str()).is_some() {
        result["report"].as_str().unwrap_or_default().to_string()
    } else if effective_format == "json" {
        serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
    } else if effective_format == "gitlab-ci" || effective_format == "gitlab_ci" {
        serde_json::to_string_pretty(&result["report"]).unwrap_or_else(|_| "[]".to_string())
    } else {
        "No report content available".to_string()
    };

    match std::fs::write(out_path, &report_content) {
        Ok(_) => {
            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "output_file".to_string(),
                    Value::String(out_path.to_string()),
                );
                obj.insert("output_saved".to_string(), Value::Bool(true));
            }
        }
        Err(e) => {
            if let Some(obj) = result.as_object_mut() {
                obj.insert(
                    "output_error".to_string(),
                    Value::String(format!("Failed to write report to '{}': {}", out_path, e)),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::config::{ReviewIssue, Severity};
    use super::*;

    #[test]
    fn test_resolve_format_text_default() {
        assert_eq!(resolve_format("text"), "text");
        assert_eq!(resolve_format("json"), "json");
        assert_eq!(resolve_format("github-actions"), "github-actions");
        assert_eq!(resolve_format("gitlab-ci"), "gitlab-ci");
    }

    #[test]
    fn test_resolve_format_auto_text() {
        let old_gh = std::env::var("GITHUB_ACTIONS").ok();
        let old_gl = std::env::var("GITLAB_CI").ok();
        std::env::remove_var("GITHUB_ACTIONS");
        std::env::remove_var("GITLAB_CI");
        let result = resolve_format("auto");
        assert_eq!(result, "text");
        if let Some(v) = old_gh {
            std::env::set_var("GITHUB_ACTIONS", v);
        }
        if let Some(v) = old_gl {
            std::env::set_var("GITLAB_CI", v);
        }
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
        assert!(
            report.contains(
                "::error file=src/main.rs,line=10,col=5,title=unsafe/error::Unsafe block detected."
            ),
            "Should contain GitHub error annotation, got: {}",
            report
        );
        assert!(
            report.contains(
                "::warning file=src/lib.rs,line=25,col=1,title=style/warning::Long line."
            ),
            "Should contain GitHub warning annotation, got: {}",
            report
        );
        assert!(
            report.contains("::notice title=Code Review Summary"),
            "Should contain summary notice, got: {}",
            report
        );
    }

    #[test]
    fn test_generate_github_actions_report_empty() {
        let result = generate_github_actions_report(&[], 0, 0, 0).unwrap();
        let report = result["report"].as_str().unwrap();
        assert!(report.contains("0 issues"));
        let error_count = report.matches("::error").count();
        let warning_count = report.matches("::warning").count();
        let notice_count = report.matches("::notice").count();
        assert_eq!(error_count, 0, "No error annotations for empty issues");
        assert_eq!(warning_count, 0, "No warning annotations for empty issues");
        assert_eq!(notice_count, 1, "Should have exactly 1 notice (summary)");
    }

    #[test]
    fn test_generate_gitlab_ci_report_basic() {
        let issues = vec![ReviewIssue {
            severity: Severity::Error,
            check: "unsafe".to_string(),
            file: "src/main.rs".to_string(),
            line: 10,
            column: 1,
            message: "Unsafe block.".to_string(),
            recommendation: Some("Fix it.".to_string()),
        }];
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
            ReviewIssue {
                severity: Severity::Error,
                check: "err".to_string(),
                file: "a.rs".to_string(),
                line: 1,
                column: 1,
                message: "E".to_string(),
                recommendation: None,
            },
            ReviewIssue {
                severity: Severity::Warning,
                check: "warn".to_string(),
                file: "a.rs".to_string(),
                line: 2,
                column: 1,
                message: "W".to_string(),
                recommendation: None,
            },
            ReviewIssue {
                severity: Severity::Info,
                check: "info".to_string(),
                file: "a.rs".to_string(),
                line: 3,
                column: 1,
                message: "I".to_string(),
                recommendation: None,
            },
        ];
        let result = generate_gitlab_ci_report(&issues, 1, 1, 1).unwrap();
        let report = result["report"].as_array().unwrap();
        assert_eq!(report[0]["severity"], "blocker");
        assert_eq!(report[1]["severity"], "major");
        assert_eq!(report[2]["severity"], "minor");
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
}
