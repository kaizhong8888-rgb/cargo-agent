//! Benchmark tool: analyze Rust code and generate micro-benchmarks, memory benchmarks,
//! and criterion-compatible benchmark code. Compare multiple implementations.
//!
//! # Actions
//!
//! - **micro_benchmark**: Run quick micro-benchmarks comparing code snippets
//! - **compare**: Compare multiple implementations side by side
//! - **generate_criterion**: Generate criterion benchmark code for a project
//! - **analyze_hotspots**: Analyze code for potential performance hotspots
//! - **estimate_complexity**: Estimate algorithmic complexity from code patterns

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

// ============================================================================
// Pre-compiled regex patterns
// ============================================================================

static RE_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)\s*(?:->\s*([^{]+?))?\s*\{?"#
    ).expect("valid regex")
});

static RE_LOOP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*(?:for|while|loop)\s"#).expect("valid regex"));

#[allow(dead_code)]
static RE_NESTED_LOOP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*(?:for|while|loop)\s.*\{[\s\S]*?(?:for|while|loop)\s"#)
        .expect("valid regex"));

#[allow(dead_code)]
static RE_RECURSIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)[^{]*\{[^}]*\1\s*\("#)
        .expect("valid regex")
});

static RE_COLLECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\.collect::<Vec"#).expect("valid regex"));

static RE_CLONE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\.clone\(\)"#).expect("valid regex"));

static RE_ALLOC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:String::new|Vec::new|Box::new|HashMap::new|BTreeMap::new)\s*\(?\s*\)"#)
        .expect("valid regex")
});

static RE_FORMAT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:format!|println!|eprintln!)\s*\("#).expect("valid regex"));

#[allow(dead_code)]
static RE_UNWRAP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\.unwrap\(\)"#).expect("valid regex"));

#[allow(dead_code)]
static RE_REF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*(?:let\s+)?[a-zA-Z_]\w*\s*:\s*&"#).expect("valid regex"));

static RE_STRING_CONCAT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:\+\s*"[^"]*"|\+\s*\w+)"#).expect("valid regex"));

// ============================================================================
// BenchmarkTool
// ============================================================================

pub struct BenchmarkTool;

#[async_trait::async_trait]
impl Tool for BenchmarkTool {
    fn name(&self) -> &str {
        "benchmark"
    }

    fn description(&self) -> &str {
        "Rust performance benchmarking tool: analyze code for hotspots, generate criterion benchmark code, run micro-benchmarks comparing implementations, and estimate algorithmic complexity."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: micro_benchmark (run quick benchmarks), compare (compare implementations), generate_criterion (generate criterion code), analyze_hotspots (find performance hotspots), estimate_complexity (estimate algorithmic complexity)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to Rust source file or project directory".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "code".to_string(),
                description: "Rust code snippet to benchmark (for micro_benchmark/compare)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "implementations".to_string(),
                description: "JSON array of {name, code} objects for comparison".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "iterations".to_string(),
                description: "Number of iterations for micro-benchmark (default: 10000)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "output".to_string(),
                description: "Output file path for generated criterion code".to_string(),
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
            "micro_benchmark" => self.action_micro_benchmark(params),
            "compare" => self.action_compare(params),
            "generate_criterion" => self.action_generate_criterion(params),
            "analyze_hotspots" => self.action_analyze_hotspots(params),
            "estimate_complexity" => self.action_estimate_complexity(params),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: micro_benchmark, compare, generate_criterion, analyze_hotspots, estimate_complexity"),
            })),
        }
    }
}

impl BenchmarkTool {
    /// Run a micro-benchmark on a code snippet.
    fn action_micro_benchmark(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let code = params
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: code")?;

        let iterations = params
            .get("iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000);

        // Compile and run the benchmark using code_execute
        let benchmark_code = self.wrap_for_benchmark(code, iterations);

        let result = self.run_benchmark_code(&benchmark_code)?;

        Ok(json!({
            "status": "ok",
            "action": "micro_benchmark",
            "iterations": iterations,
            "result": result,
        }))
    }

    /// Compare multiple implementations.
    fn action_compare(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let implementations_str = params
            .get("implementations")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: implementations (JSON array of {name, code})")?;

        let iterations = params
            .get("iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000);

        let implementations: Vec<(String, String)> =
            serde_json::from_str(implementations_str)
                .map_err(|e| format!("Invalid JSON for implementations: {e}"))?;

        if implementations.is_empty() {
            return Ok(json!({
                "status": "error",
                "message": "No implementations provided",
            }));
        }

        let mut results = Vec::new();
        let mut fastest: Option<(String, u128)> = None;

        for (name, code) in &implementations {
            let benchmark_code = self.wrap_for_benchmark(code, iterations);
            match self.run_benchmark_code(&benchmark_code) {
                Ok(result) => {
                    let elapsed = result["elapsed_ns"].as_u64().unwrap_or(0) as u128;
                    results.push(json!({
                        "name": name,
                        "elapsed_ns": elapsed,
                        "elapsed_ms": elapsed as f64 / 1_000_000.0,
                    }));
                    if fastest.is_none() || elapsed < fastest.as_ref().unwrap().1 {
                        fastest = Some((name.clone(), elapsed));
                    }
                }
                Err(e) => {
                    results.push(json!({
                        "name": name,
                        "error": e,
                    }));
                }
            }
        }

        // Calculate relative speeds
        if let Some((fastest_name, fastest_time)) = &fastest {
            for r in results.iter_mut() {
                if let Some(elapsed) = r.get("elapsed_ns").and_then(|v| v.as_u64()) {
                    let ratio = elapsed as f64 / *fastest_time as f64;
                    r["speed_ratio"] = json!(format!("{ratio:.2}x"));
                    if fastest_name == r.get("name").and_then(|v| v.as_str()).unwrap_or("") {
                        r["is_fastest"] = json!(true);
                    }
                }
            }
        }

        Ok(json!({
            "status": "ok",
            "action": "compare",
            "iterations": iterations,
            "fastest": fastest.as_ref().map(|(n, t)| json!({"name": n, "elapsed_ms": *t as f64 / 1_000_000.0})),
            "results": results,
        }))
    }

    /// Generate criterion benchmark code.
    fn action_generate_criterion(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let output = params.get("output").and_then(|v| v.as_str());

        // Analyze the project for public functions
        let mut bench_code = String::new();
        bench_code.push_str("//! Auto-generated criterion benchmarks\n");
        bench_code.push_str("//!\n");
        bench_code.push_str("//! Add to Cargo.toml:\n");
        bench_code.push_str("//! ```toml\n");
        bench_code.push_str("//! [[bench]]\n");
        bench_code.push_str("//! name = \"benchmarks\"\n");
        bench_code.push_str("//! harness = false\n");
        bench_code.push_str("//!\n");
        bench_code.push_str("//! [dev-dependencies]\n");
        bench_code.push_str("//! criterion = \"0.5\"\n");
        bench_code.push_str("//! ```\n\n");
        bench_code.push_str("use criterion::{black_box, criterion_group, criterion_main, Criterion};\n\n");

        let path = Path::new(path);
        let mut pub_fns = Vec::new();
        self.collect_pub_functions(path, &mut pub_fns)?;

        for (module, fn_name) in &pub_fns {
            let bench_name = format!("bench_{module}_{fn_name}");
            bench_code.push_str(&format!("fn {bench_name}(c: &mut Criterion) {{\n"));
            bench_code.push_str(&format!(
                "    c.bench_function(\"{module}::{fn_name}\", |b| b.iter(|| {{\n"
            ));
            bench_code.push_str(&format!(
                "        // TODO: set up input parameters\n"
            ));
            bench_code.push_str(&format!(
                "        black_box({module}::{fn_name}()); // TODO: pass arguments\n"
            ));
            bench_code.push_str("    }));\n");
            bench_code.push_str("}\n\n");
        }

        // Generate main function
        bench_code.push_str("criterion_group!(benches,");
        for (module, fn_name) in &pub_fns {
            bench_code.push_str(&format!(" bench_{module}_{fn_name},"));
        }
        bench_code.push_str(");\ncriterion_main!(benches);\n");

        let generated = if pub_fns.is_empty() {
            "// No public functions found to benchmark.\n// Add public functions to your library to auto-generate benchmarks.".to_string()
        } else {
            bench_code
        };

        if let Some(out_path) = output {
            std::fs::write(out_path, &generated)
                .map_err(|e| format!("Failed to write benchmark file: {e}"))?;
        }

        Ok(json!({
            "status": "ok",
            "action": "generate_criterion",
            "functions_found": pub_fns.len(),
            "output_file": output,
            "generated_code": generated,
        }))
    }

    /// Analyze code for performance hotspots.
    fn action_analyze_hotspots(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let file_path = Path::new(path);
        let mut files = Vec::new();

        if file_path.is_file() {
            files.push(path.to_string());
        } else if file_path.is_dir() {
            self.collect_rust_files(file_path, &mut files, true, 0)?;
        }

        let mut hotspots = Vec::new();

        for file in &files {
            let content = std::fs::read_to_string(file)
                .map_err(|e| format!("Failed to read {file}: {e}"))?;

            let mut file_hotspots = Vec::new();

            // Detect expensive patterns
            let clone_count = RE_CLONE.find_iter(&content).count();
            if clone_count > 5 {
                file_hotspots.push(json!({
                    "type": "excessive_clone",
                    "count": clone_count,
                    "severity": "warning",
                    "suggestion": "Consider using references (&T) or Cow<T> to avoid unnecessary clones",
                }));
            }

            let format_count = RE_FORMAT.find_iter(&content).count();
            if format_count > 10 {
                file_hotspots.push(json!({
                    "type": "excessive_formatting",
                    "count": format_count,
                    "severity": "warning",
                    "suggestion": "Consider using write! macro or pre-formatted strings in hot paths",
                }));
            }

            let alloc_count = RE_ALLOC.find_iter(&content).count();
            if alloc_count > 5 {
                file_hotspots.push(json!({
                    "type": "frequent_allocation",
                    "count": alloc_count,
                    "severity": "info",
                    "suggestion": "Consider using with_capacity() or object pooling",
                }));
            }

            let loop_count = RE_LOOP.find_iter(&content).count();
            if loop_count > 3 {
                file_hotspots.push(json!({
                    "type": "multiple_loops",
                    "count": loop_count,
                    "severity": "info",
                    "suggestion": "Consider fusing loops or using iterators for better optimization",
                }));
            }

            let nested_count = count_nested_loops(&content);
            if nested_count > 0 {
                file_hotspots.push(json!({
                    "type": "nested_loops",
                    "count": nested_count,
                    "severity": "warning",
                    "suggestion": "O(n²) or worse complexity. Consider using hashmaps or sorting",
                }));
            }

            let recursive_count = count_recursive_functions(&content);
            if recursive_count > 0 {
                file_hotspots.push(json!({
                    "type": "recursive_functions",
                    "count": recursive_count,
                    "severity": "info",
                    "suggestion": "Consider iterative approach or memoization to avoid stack overflow",
                }));
            }

            let string_concat = count_string_concatenation(&content);
            if string_concat > 3 {
                file_hotspots.push(json!({
                    "type": "string_concatenation",
                    "count": string_concat,
                    "severity": "warning",
                    "suggestion": "Use format! or String::with_capacity() + push_str() instead",
                }));
            }

            let collect_vec = RE_COLLECT.find_iter(&content).count();
            if collect_vec > 3 {
                file_hotspots.push(json!({
                    "type": "collect_to_vec",
                    "count": collect_vec,
                    "severity": "info",
                    "suggestion": "Consider using iterators directly instead of collecting to Vec",
                }));
            }

            if !file_hotspots.is_empty() {
                hotspots.push(json!({
                    "file": file,
                    "hotspots": file_hotspots,
                    "total_issues": file_hotspots.len(),
                }));
            }
        }

        let total_issues: usize = hotspots.iter().map(|h| h["total_issues"].as_u64().unwrap_or(0) as usize).sum();

        Ok(json!({
            "status": "ok",
            "action": "analyze_hotspots",
            "files_analyzed": files.len(),
            "total_issues": total_issues,
            "hotspots": hotspots,
        }))
    }

    /// Estimate algorithmic complexity from code patterns.
    fn action_estimate_complexity(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let code = params
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: code")?;

        let mut complexity_score = 0;
        let mut factors = Vec::new();

        let loop_count = RE_LOOP.find_iter(code).count();
        if loop_count > 0 {
            complexity_score += loop_count * 2;
            factors.push(json!({
                "factor": "loops",
                "count": loop_count,
                "impact": format!("O(n^{loop_count}) or worse depending on nesting"),
            }));
        }

        let nested_count = count_nested_loops(code);
        if nested_count > 0 {
            complexity_score += nested_count * 5;
            factors.push(json!({
                "factor": "nested_loops",
                "count": nested_count,
                "impact": format!("O(n^{}) complexity detected", nested_count + 1),
            }));
        }

        let recursive_count = count_recursive_functions(code);
        if recursive_count > 0 {
            complexity_score += recursive_count * 3;
            factors.push(json!({
                "factor": "recursive_calls",
                "count": recursive_count,
                "impact": "Potential exponential complexity without memoization",
            }));
        }

        let clone_count = RE_CLONE.find_iter(code).count();
        if clone_count > 0 {
            complexity_score += clone_count;
            factors.push(json!({
                "factor": "clone_operations",
                "count": clone_count,
                "impact": "O(n) per clone for sized collections",
            }));
        }

        let estimated = if complexity_score == 0 {
            "O(1) - constant time"
        } else if complexity_score <= 5 {
            "O(n) - linear time"
        } else if complexity_score <= 15 {
            "O(n log n) or O(n²) - likely quadratic"
        } else {
            "O(n³) or worse - exponential/polynomial"
        };

        Ok(json!({
            "status": "ok",
            "action": "estimate_complexity",
            "estimated_complexity": estimated,
            "complexity_score": complexity_score,
            "factors": factors,
        }))
    }

    /// Wrap code for benchmarking in a compilable program.
    fn wrap_for_benchmark(&self, code: &str, iterations: u64) -> String {
        format!(
            r#"
use std::time::Instant;

fn main() {{
    let iterations = {iterations}u64;
    let start = Instant::now();

    for _ in 0..iterations {{
        {code}
    }}

    let elapsed = start.elapsed();
    println!("{{}}", elapsed.as_nanos());
}}
"#
        )
    }

    /// Run benchmark code using code_execute.
    fn run_benchmark_code(&self, code: &str) -> Result<Value, String> {
        let output = Command::new("rustc")
            .args(["--edition", "2021", "-o", "/tmp/bench_temp", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn rustc: {e}"))?;

        output
            .stdin
            .as_ref()
            .unwrap()
            .write_all(code.as_bytes())
            .map_err(|e| format!("Failed to write code: {e}"))?;

        let compile_result = output.wait_with_output().map_err(|e| e.to_string())?;

        if !compile_result.status.success() {
            let stderr = String::from_utf8_lossy(&compile_result.stderr);
            return Err(format!("Compilation failed: {stderr}"));
        }

        // Run the compiled benchmark
        let run_start = Instant::now();
        let run_output = Command::new("/tmp/bench_temp")
            .output()
            .map_err(|e| format!("Failed to run benchmark: {e}"))?;
        let run_elapsed = run_start.elapsed();

        let _ = std::fs::remove_file("/tmp/bench_temp");

        let elapsed_ns = String::from_utf8_lossy(&run_output.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(run_elapsed.as_nanos() as u64);

        Ok(json!({
            "elapsed_ns": elapsed_ns,
            "elapsed_ms": elapsed_ns as f64 / 1_000_000.0,
            "real_elapsed_ms": run_elapsed.as_micros() as f64 / 1000.0,
            "success": run_output.status.success(),
        }))
    }

    /// Collect public functions from a Rust project.
    fn collect_pub_functions(&self, path: &Path, functions: &mut Vec<(String, String)>) -> Result<(), String> {
        if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {path:?}: {e}"))?;
            let module = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            for cap in RE_FN.captures_iter(&content) {
                let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                functions.push((module.clone(), fn_name.to_string()));
            }
        } else if path.is_dir() {
            self.collect_rust_files(path, &mut Vec::new(), true, 0)?;
            let mut files = Vec::new();
            self.collect_rust_files(path, &mut files, true, 0)?;
            for file in files {
                let p = Path::new(&file);
                if p.is_file() {
                    self.collect_pub_functions(p, functions)?;
                }
            }
        }
        Ok(())
    }

    /// Recursively collect .rs files.
    fn collect_rust_files(
        &self,
        dir: &Path,
        files: &mut Vec<String>,
        recursive: bool,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 10 {
            return Ok(());
        }
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read dir {:?}: {e}", dir))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                if recursive {
                    self.collect_rust_files(&path, files, true, depth + 1)?;
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path.to_string_lossy().to_string());
            }
        }
        Ok(())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn count_nested_loops(content: &str) -> usize {
    let mut count = 0;
    let lines: Vec<&str> = content.lines().collect();
    let mut in_loop = false;
    let mut brace_depth = 0;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with("for ") || trimmed.starts_with("while ") || trimmed.starts_with("loop") {
            if in_loop && brace_depth > 0 {
                count += 1;
            }
            in_loop = true;
            brace_depth = 0;
        }
        brace_depth += line.chars().filter(|&c| c == '{').count();
        brace_depth = brace_depth.saturating_sub(line.chars().filter(|&c| c == '}').count());
        if brace_depth == 0 && in_loop {
            in_loop = false;
        }
    }
    count
}

fn count_recursive_functions(content: &str) -> usize {
    let mut count = 0;
    for cap in RE_FN.captures_iter(content) {
        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let start = cap.get(0).unwrap().end();
        // Find matching brace
        let rest = &content[start..];
        let mut depth = 0;
        let mut fn_body = String::new();
        for ch in rest.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            if depth > 0 {
                fn_body.push(ch);
            }
        }
        // Check if function calls itself
        let self_call = format!("{fn_name}(");
        if fn_body.contains(&self_call) {
            count += 1;
        }
    }
    count
}

fn count_string_concatenation(content: &str) -> usize {
    RE_STRING_CONCAT.find_iter(content).count()
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(BenchmarkTool));
}
