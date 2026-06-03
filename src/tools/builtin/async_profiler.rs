//! Async/Tokio profiler and analyzer for Rust projects.
//!
//! Detects blocking I/O in async contexts, analyzes tokio::spawn patterns,
//! identifies unawaited futures, and suggests runtime configurations.
//!
//! Actions: analyze (full async analysis), blocking (detect blocking calls),
//! spawn_analysis (analyze tokio::spawn usage), runtime_config (suggest runtime settings)

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(AsyncProfilerTool));
}

struct AsyncProfilerTool;

#[async_trait::async_trait]
impl Tool for AsyncProfilerTool {
    fn name(&self) -> &str {
        "async_profiler"
    }

    fn description(&self) -> &str {
        "Async/Tokio profiler for Rust projects. Actions: analyze (full async code analysis), \
         blocking (detect blocking I/O in async contexts), spawn_analysis (analyze tokio::spawn patterns), \
         runtime_config (suggest optimal Tokio runtime configuration), unawaited (find unawaited futures)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: analyze, blocking, spawn_analysis, runtime_config, unawaited"
                    .to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to Rust source file or directory (default: current directory)"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "recursive".to_string(),
                parameter_type: "boolean".to_string(),
                description: "Recursively analyze directory (default: true)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "max_results".to_string(),
                parameter_type: "number".to_string(),
                description: "Maximum number of issues to report (default: 50)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "severity".to_string(),
                parameter_type: "string".to_string(),
                description: "Minimum severity to report: critical, warning, info (default: info)"
                    .to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        match action {
            "analyze" => self.full_analysis(path, recursive, max_results),
            "blocking" => self.detect_blocking(path, recursive, max_results),
            "spawn_analysis" => self.analyze_spawn_patterns(path, recursive, max_results),
            "runtime_config" => self.suggest_runtime_config(path),
            "unawaited" => self.detect_unawaited(path, recursive, max_results),
            _ => Err(format!(
                "Unknown action: {action}. Valid: analyze, blocking, spawn_analysis, runtime_config, unawaited"
            )),
        }
    }
}

impl AsyncProfilerTool {
    fn collect_rust_files(&self, path: &str, recursive: bool) -> Result<Vec<String>, String> {
        let path = Path::new(path);
        let mut files = Vec::new();

        if path.is_file() {
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                files.push(path.to_string_lossy().to_string());
            }
        } else if path.is_dir() {
            self.walk_dir(path, &mut files, recursive)?;
        } else {
            return Err(format!("Path not found: {}", path.display()));
        }

        // Filter out target/ and common non-source directories
        files.retain(|f| {
            !f.contains("/target/")
                && !f.contains("\\target\\")
                && !f.starts_with("target/")
                && !f.starts_with("target\\")
        });

        Ok(files)
    }

    fn walk_dir(&self, dir: &Path, files: &mut Vec<String>, recursive: bool) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();

            if path.is_dir() {
                if recursive {
                    self.walk_dir(&path, files, true)?;
                }
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                files.push(path.to_string_lossy().to_string());
            }
        }
        Ok(())
    }

    fn full_analysis(
        &self,
        path: &str,
        recursive: bool,
        max_results: usize,
    ) -> Result<Value, String> {
        let files = self.collect_rust_files(path, recursive)?;
        let mut all_issues: Vec<AsyncIssue> = Vec::new();
        let mut stats = AnalysisStats {
            total_files: files.len(),
            ..Default::default()
        };

        for file_path in &files {
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
            stats.total_lines += content.lines().count();

            let file_issues = self.analyze_file(file_path, &content);
            stats.total_async_fns += file_issues.async_fn_count;
            stats.total_spawn_calls += file_issues.spawn_count;
            stats.total_blocking += file_issues.blocking_count;

            for issue in file_issues.issues {
                all_issues.push(issue);
            }
        }

        all_issues.sort_by(|a, b| a.severity.cmp(&b.severity));
        all_issues.truncate(max_results);

        let severity_counts = self.count_severities(&all_issues);

        Ok(serde_json::json!({
            "action": "analyze",
            "files_analyzed": stats.total_files,
            "total_lines": stats.total_lines,
            "async_functions": stats.total_async_fns,
            "spawn_calls": stats.total_spawn_calls,
            "issues": all_issues.iter().map(|i| i.to_json()).collect::<Vec<_>>(),
            "summary": {
                "total_issues": all_issues.len(),
                "critical": severity_counts.critical,
                "warning": severity_counts.warning,
                "info": severity_counts.info,
            },
            "recommendations": self.generate_recommendations(&stats, &all_issues),
        }))
    }

    fn detect_blocking(
        &self,
        path: &str,
        recursive: bool,
        max_results: usize,
    ) -> Result<Value, String> {
        let files = self.collect_rust_files(path, recursive)?;
        let mut blocking_issues: Vec<AsyncIssue> = Vec::new();

        for file_path in &files {
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

            let blocking = self.find_blocking_calls(file_path, &content);
            blocking_issues.extend(blocking);
        }

        blocking_issues.truncate(max_results);

        Ok(serde_json::json!({
            "action": "blocking",
            "files_analyzed": files.len(),
            "blocking_issues": blocking_issues.iter().map(|i| i.to_json()).collect::<Vec<_>>(),
            "total_blocking": blocking_issues.len(),
            "suggestion": "Replace std::fs/std::net calls with tokio::fs/tokio::net, \
                          or wrap blocking calls in tokio::task::spawn_blocking()",
        }))
    }

    fn analyze_spawn_patterns(
        &self,
        path: &str,
        recursive: bool,
        max_results: usize,
    ) -> Result<Value, String> {
        let files = self.collect_rust_files(path, recursive)?;
        let mut spawn_issues: Vec<AsyncIssue> = Vec::new();
        let mut spawn_patterns: Vec<SpawnPattern> = Vec::new();

        for file_path in &files {
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

            let (patterns, issues) = self.analyze_spawn_in_file(file_path, &content);
            spawn_patterns.extend(patterns);
            spawn_issues.extend(issues);
        }

        spawn_issues.truncate(max_results);

        Ok(serde_json::json!({
            "action": "spawn_analysis",
            "files_analyzed": files.len(),
            "spawn_patterns": spawn_patterns.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
            "issues": spawn_issues.iter().map(|i| i.to_json()).collect::<Vec<_>>(),
            "total_issues": spawn_issues.len(),
            "suggestions": self.generate_spawn_suggestions(&spawn_patterns),
        }))
    }

    fn suggest_runtime_config(&self, path: &str) -> Result<Value, String> {
        let cargo_path = Path::new(path).join("Cargo.toml");
        let has_tokio = if cargo_path.exists() {
            let content = fs::read_to_string(&cargo_path).unwrap_or_default();
            content.contains("tokio")
        } else {
            false
        };

        if !has_tokio {
            return Ok(serde_json::json!({
                "action": "runtime_config",
                "message": "Tokio not found in Cargo.toml dependencies",
                "suggestion": "Add tokio = {{ version = \"1\", features = [\"full\"] }} to [dependencies]",
            }));
        }

        let files = self.collect_rust_files(path, true)?;
        let mut async_fn_count = 0;
        let mut has_io = false;
        let mut has_compute_heavy = false;
        let mut has_timers = false;
        let mut spawn_count = 0;

        for file_path in &files {
            let content = fs::read_to_string(file_path).unwrap_or_default();
            async_fn_count += content.matches("async fn").count();
            has_io = has_io
                || content.contains("tokio::fs")
                || content.contains("tokio::net")
                || content.contains("tokio::io");
            has_compute_heavy = has_compute_heavy
                || content.contains("spawn_blocking")
                || content.contains("rayon")
                || content.contains("std::thread");
            has_timers = has_timers
                || content.contains("tokio::time")
                || content.contains("sleep")
                || content.contains("timeout")
                || content.contains("interval");
            spawn_count += content.matches("tokio::spawn").count();
        }

        let mut config_suggestions: Vec<String> = Vec::new();
        let mut code_example = String::new();

        if spawn_count > 10 {
            config_suggestions.push(
                "High spawn count detected. Consider using a worker pool or task limiting."
                    .to_string(),
            );
        }

        if has_compute_heavy {
            config_suggestions.push(
                "Blocking work detected. Ensure spawn_blocking is used for CPU-intensive tasks."
                    .to_string(),
            );
        }

        if has_io && !has_timers {
            config_suggestions.push(
                "I/O heavy workload. Default multi-threaded runtime is suitable.".to_string(),
            );
        }

        if async_fn_count == 0 {
            config_suggestions
                .push("No async functions found. Consider if async runtime is needed.".to_string());
        }

        // Generate recommended runtime builder code
        code_example.push_str("// Recommended Tokio runtime configuration\n");
        code_example.push_str("#[tokio::main]\n");
        code_example.push_str("async fn main() {\n");
        code_example.push_str("    // Your async code here\n");
        code_example.push_str("}\n\n");
        code_example.push_str("// Or with custom builder:\n");
        code_example.push_str("tokio::runtime::Builder::new_multi_thread()\n");
        code_example.push_str("    .worker_threads(num_cpus::get())\n");

        if has_io {
            code_example.push_str("    .max_blocking_threads(512)\n");
        }

        code_example.push_str("    .enable_all()\n");
        code_example.push_str("    .build()\n");
        code_example.push_str("    .unwrap()\n");
        code_example.push_str("    .block_on(async {\n");
        code_example.push_str("        // Your async code\n");
        code_example.push_str("    });\n");

        Ok(serde_json::json!({
            "action": "runtime_config",
            "async_functions": async_fn_count,
            "spawn_calls": spawn_count,
            "has_io": has_io,
            "has_compute_heavy": has_compute_heavy,
            "has_timers": has_timers,
            "config_suggestions": config_suggestions,
            "code_example": code_example,
            "recommended_features": {
                "worker_threads": "num_cpus::get()",
                "max_blocking_threads": if has_io { 512 } else { 51 },
                "enable_io": has_io,
                "enable_time": has_timers,
            },
        }))
    }

    fn detect_unawaited(
        &self,
        path: &str,
        recursive: bool,
        max_results: usize,
    ) -> Result<Value, String> {
        let files = self.collect_rust_files(path, recursive)?;
        let mut unawaited_issues: Vec<AsyncIssue> = Vec::new();

        for file_path in &files {
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

            let issues = self.find_unawaited_futures(file_path, &content);
            unawaited_issues.extend(issues);
        }

        unawaited_issues.truncate(max_results);

        Ok(serde_json::json!({
            "action": "unawaited",
            "files_analyzed": files.len(),
            "unawaited_issues": unawaited_issues.iter().map(|i| i.to_json()).collect::<Vec<_>>(),
            "total_unawaited": unawaited_issues.len(),
            "suggestion": "Use `.await` on futures, or explicitly drop them with `let _ = ...` \
                          or use `tokio::spawn` to run in background",
        }))
    }

    fn analyze_file(&self, file_path: &str, content: &str) -> FileAnalysis {
        let mut issues = Vec::new();
        let mut async_fn_count = 0;
        let mut spawn_count = 0;
        let mut blocking_count = 0;

        let lines: Vec<(usize, &str)> = content.lines().enumerate().collect();

        for (line_num, line) in &lines {
            let trimmed = line.trim();

            // Count async functions
            if trimmed.starts_with("async fn") || trimmed.contains("async fn") {
                async_fn_count += 1;
            }

            // Count spawn calls
            if trimmed.contains("tokio::spawn") {
                spawn_count += 1;
            }

            // Check for blocking calls inside async functions
            if self.is_in_async_context(&lines, *line_num) {
                if let Some(issue) = self.check_blocking_line(file_path, *line_num, line) {
                    issues.push(issue);
                    blocking_count += 1;
                }
            }
        }

        FileAnalysis {
            issues,
            async_fn_count,
            spawn_count,
            blocking_count,
        }
    }

    fn is_in_async_context(&self, lines: &[(usize, &str)], current_line: usize) -> bool {
        // Look backwards for async fn declaration
        let mut brace_depth = 0;
        for i in (0..=current_line).rev() {
            let (_, line) = &lines[i];
            let trimmed = line.trim();

            if trimmed.contains('}') {
                brace_depth += 1;
            }
            if trimmed.contains('{') && brace_depth > 0 {
                brace_depth -= 1;
            }

            if (trimmed.starts_with("async fn") || trimmed.contains("async fn"))
                && brace_depth == 0
            {
                return true;
            }

            if trimmed.starts_with("fn ") && !trimmed.contains("async")
                && brace_depth == 0
            {
                return false;
            }
        }
        false
    }

    fn find_blocking_calls(&self, file_path: &str, content: &str) -> Vec<AsyncIssue> {
        let mut issues = Vec::new();
        let lines: Vec<(usize, &str)> = content.lines().enumerate().collect();

        for (line_num, line) in &lines {
            if !self.is_in_async_context(&lines, *line_num) {
                continue;
            }

            if let Some(issue) = self.check_blocking_line(file_path, *line_num, line) {
                issues.push(issue);
            }
        }

        issues
    }

    fn check_blocking_line(
        &self,
        file_path: &str,
        line_num: usize,
        line: &str,
    ) -> Option<AsyncIssue> {
        let trimmed = line.trim();

        // Skip comments and attributes
        if trimmed.starts_with("//") || trimmed.starts_with("#[") {
            return None;
        }

        let blocking_patterns = [
            ("std::fs::", "Use tokio::fs::* instead", "critical"),
            (
                "std::io::stdin",
                "Use tokio::io::stdin() instead",
                "warning",
            ),
            (
                "std::io::stdout",
                "Use tokio::io::stdout() instead",
                "warning",
            ),
            ("std::net::", "Use tokio::net::* instead", "critical"),
            (
                "std::thread::sleep",
                "Use tokio::time::sleep() instead",
                "critical",
            ),
            (
                "std::thread::spawn",
                "Use tokio::task::spawn() instead",
                "critical",
            ),
            (
                "reqwest::blocking::",
                "Use async reqwest instead",
                "warning",
            ),
            (
                "serde_json::from_reader",
                "Use serde_json::from_slice/async equivalent",
                "warning",
            ),
            (
                "std::process::Command",
                "Use tokio::process::Command instead",
                "warning",
            ),
            (
                "std::sync::Mutex",
                "Use tokio::sync::Mutex if held across .await",
                "warning",
            ),
            (
                "std::sync::RwLock",
                "Use tokio::sync::RwLock if held across .await",
                "warning",
            ),
        ];

        for (pattern, suggestion, severity) in &blocking_patterns {
            if trimmed.contains(pattern) {
                return Some(AsyncIssue {
                    file: file_path.to_string(),
                    line: line_num + 1,
                    severity: match *severity {
                        "critical" => IssueSeverity::Critical,
                        "warning" => IssueSeverity::Warning,
                        _ => IssueSeverity::Info,
                    },
                    category: "blocking_io".to_string(),
                    message: format!("Blocking call detected: {}", trimmed),
                    suggestion: suggestion.to_string(),
                    code_snippet: trimmed.to_string(),
                });
            }
        }

        None
    }

    fn analyze_spawn_in_file(
        &self,
        file_path: &str,
        content: &str,
    ) -> (Vec<SpawnPattern>, Vec<AsyncIssue>) {
        let mut patterns = Vec::new();
        let mut issues = Vec::new();
        let lines: Vec<(usize, &str)> = content.lines().enumerate().collect();

        for (line_num, line) in &lines {
            let trimmed = line.trim();

            if trimmed.contains("tokio::spawn") {
                patterns.push(SpawnPattern {
                    file: file_path.to_string(),
                    line: line_num + 1,
                    pattern_type: if trimmed.contains("tokio::spawn(async") {
                        "async_block".to_string()
                    } else if trimmed.contains("tokio::spawn(Self::")
                        || trimmed.contains("tokio::spawn(")
                    {
                        "function".to_string()
                    } else {
                        "unknown".to_string()
                    },
                    code_snippet: trimmed.to_string(),
                    has_join_handle: false,
                });

                // Check if spawn result is ignored
                if !trimmed.starts_with("let ") && !trimmed.starts_with("let mut ") {
                    issues.push(AsyncIssue {
                        file: file_path.to_string(),
                        line: line_num + 1,
                        severity: IssueSeverity::Warning,
                        category: "unhandled_spawn".to_string(),
                        message: "tokio::spawn handle is not stored".to_string(),
                        suggestion:
                            "Consider storing the JoinHandle to await or abort the task later"
                                .to_string(),
                        code_snippet: trimmed.to_string(),
                    });
                }
            }

            // Check for nested spawns (potential task explosion)
            if trimmed.contains("tokio::spawn") && self.is_in_async_context(&lines, *line_num) {
                // Check if this is inside a loop
                let mut in_loop = false;
                for i in (0..=*line_num).rev() {
                    let (_, prev_line) = &lines[i];
                    let prev_trimmed = prev_line.trim();
                    if prev_trimmed.starts_with("for ")
                        || prev_trimmed.starts_with("while ")
                        || prev_trimmed.starts_with("loop")
                    {
                        in_loop = true;
                        break;
                    }
                    if prev_trimmed.starts_with("async fn") || prev_trimmed.starts_with("fn ") {
                        break;
                    }
                }

                if in_loop {
                    issues.push(AsyncIssue {
                        file: file_path.to_string(),
                        line: line_num + 1,
                        severity: IssueSeverity::Critical,
                        category: "spawn_in_loop".to_string(),
                        message: "tokio::spawn called inside a loop - potential task explosion"
                            .to_string(),
                        suggestion:
                            "Use Semaphore to limit concurrent tasks, or use a worker pool pattern"
                                .to_string(),
                        code_snippet: trimmed.to_string(),
                    });
                }
            }
        }

        (patterns, issues)
    }

    fn find_unawaited_futures(&self, file_path: &str, content: &str) -> Vec<AsyncIssue> {
        let mut issues = Vec::new();
        let lines: Vec<(usize, &str)> = content.lines().enumerate().collect();

        for (line_num, line) in &lines {
            let trimmed = line.trim();

            // Skip if already awaited or in let binding
            if !self.is_in_async_context(&lines, *line_num) {
                continue;
            }

            if trimmed.starts_with("//") || trimmed.starts_with("#[") {
                continue;
            }

            // Look for async method calls without .await
            let async_call_patterns = [
                ".send()",
                ".fetch()",
                ".execute()",
                ".query()",
                ".read_to_string()",
                ".read_line()",
                ".write_all()",
                ".flush()",
                ".accept()",
                ".connect()",
            ];

            for pattern in &async_call_patterns {
                if trimmed.contains(pattern)
                    && !trimmed.contains(".await")
                    && !trimmed.starts_with("let ")
                    && !trimmed.starts_with("return")
                {
                    // Only flag if it looks like it's being discarded
                    if !trimmed.starts_with("let ")
                        && !trimmed.starts_with("let mut ")
                        && !trimmed.starts_with("return")
                        && !trimmed.starts_with("_ =")
                    {
                        issues.push(AsyncIssue {
                            file: file_path.to_string(),
                            line: line_num + 1,
                            severity: IssueSeverity::Warning,
                            category: "unawaited".to_string(),
                            message: format!("Potentially unawaited async call: {}", trimmed),
                            suggestion:
                                "Add .await to the call, or explicitly drop with `let _ = ...`"
                                    .to_string(),
                            code_snippet: trimmed.to_string(),
                        });
                    }
                }
            }
        }

        issues
    }

    fn generate_recommendations(
        &self,
        stats: &AnalysisStats,
        issues: &[AsyncIssue],
    ) -> Vec<String> {
        let mut recs = Vec::new();

        if stats.total_blocking > 0 {
            recs.push(format!(
                "Found {} blocking calls in async contexts. Replace with async equivalents or use spawn_blocking().",
                stats.total_blocking
            ));
        }

        if stats.total_spawn_calls > 20 {
            recs.push("High number of spawn calls. Consider using a task pool or worker pattern for better resource management.".to_string());
        }

        if stats.total_async_fns > 50 {
            recs.push("Large async codebase. Consider using tracing/tracing-subscriber for better async debugging.".to_string());
        }

        let critical_count = issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Critical))
            .count();
        if critical_count > 0 {
            recs.push(format!("{} critical issues found that may cause runtime performance degradation or deadlocks.", critical_count));
        }

        if recs.is_empty() {
            recs.push("No significant issues found. Your async code looks healthy!".to_string());
        }

        recs
    }

    fn generate_spawn_suggestions(&self, patterns: &[SpawnPattern]) -> Vec<String> {
        let mut suggestions = Vec::new();

        let async_block_count = patterns
            .iter()
            .filter(|p| p.pattern_type == "async_block")
            .count();
        let function_count = patterns
            .iter()
            .filter(|p| p.pattern_type == "function")
            .count();

        if async_block_count > function_count {
            suggestions.push("Most spawns use async blocks. Consider extracting to named functions for better error handling and debugging.".to_string());
        }

        if patterns.len() > 10 {
            suggestions.push(
                "Consider using `tokio::sync::Semaphore` to limit concurrent task execution."
                    .to_string(),
            );
            suggestions.push(
                "For producer-consumer patterns, consider `tokio::sync::mpsc` channels."
                    .to_string(),
            );
        }

        if suggestions.is_empty() {
            suggestions.push("Spawn patterns look reasonable.".to_string());
        }

        suggestions
    }

    fn count_severities(&self, issues: &[AsyncIssue]) -> SeverityCounts {
        let mut counts = SeverityCounts::default();
        for issue in issues {
            match issue.severity {
                IssueSeverity::Critical => counts.critical += 1,
                IssueSeverity::Warning => counts.warning += 1,
                IssueSeverity::Info => counts.info += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone)]
struct AsyncIssue {
    file: String,
    line: usize,
    severity: IssueSeverity,
    category: String,
    message: String,
    suggestion: String,
    code_snippet: String,
}

impl AsyncIssue {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "file": self.file,
            "line": self.line,
            "severity": format!("{:?}", self.severity).to_lowercase(),
            "category": self.category,
            "message": self.message,
            "suggestion": self.suggestion,
            "code": self.code_snippet,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IssueSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
struct SpawnPattern {
    file: String,
    line: usize,
    pattern_type: String,
    code_snippet: String,
    has_join_handle: bool,
}

impl SpawnPattern {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "file": self.file,
            "line": self.line,
            "pattern_type": self.pattern_type,
            "code": self.code_snippet,
            "has_join_handle": self.has_join_handle,
        })
    }
}

#[derive(Debug, Default)]
struct AnalysisStats {
    total_files: usize,
    total_lines: usize,
    total_async_fns: usize,
    total_spawn_calls: usize,
    total_blocking: usize,
}

#[derive(Debug, Default)]
struct SeverityCounts {
    critical: usize,
    warning: usize,
    info: usize,
}

struct FileAnalysis {
    issues: Vec<AsyncIssue>,
    async_fn_count: usize,
    spawn_count: usize,
    blocking_count: usize,
}
