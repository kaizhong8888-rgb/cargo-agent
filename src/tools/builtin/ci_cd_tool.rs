//! CI/CD Integration Tool: automate test runs, build verification, deploy script generation.
//!
//! # Actions
//!
//! - **generate_ci**: Generate GitHub Actions / GitLab CI config
//! - **run_tests**: Execute cargo test with options
//! - **run_build**: Execute cargo build/check/clippy with options
//! - **coverage**: Generate test coverage report
//! - **benchmark**: Run cargo bench
//! - **audit**: Run cargo audit for security vulnerabilities
//! - **check_prerelease**: Full pre-release checklist
//! - **check_coverage_threshold**: Enforce minimum test coverage for new tools (>80%)
//! - **check_code_health**: Scan code for anti-patterns (unwrap, expect, dbg!, todo!) and detect regressions

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct CiCdTool;

#[async_trait::async_trait]
impl Tool for CiCdTool {
    fn name(&self) -> &str {
        "ci_cd"
    }

    fn description(&self) -> &str {
        "CI/CD integration tool: generate CI configs (GitHub Actions/GitLab CI), run tests/builds/clippy/audit, generate coverage reports, enforce coverage thresholds (>80% for new tools), and pre-release checklists."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: generate_ci, run_tests, run_build, coverage, benchmark, audit, check_prerelease, check_coverage_threshold, check_code_health".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "platform".to_string(),
                description: "CI platform: github, gitlab (for generate_ci, default: github)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "test_pattern".to_string(),
                description: "Test name pattern filter (for run_tests)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "all_features".to_string(),
                description: "Test with all features enabled (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "profile".to_string(),
                description: "Build profile: dev, release, bench (for run_build, default: dev)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "toolchain".to_string(),
                description: "Toolchain: stable, beta, nightly, or specific version (default: stable)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "project_path".to_string(),
                description: "Path to the Rust project (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "min_coverage".to_string(),
                description: "Minimum coverage percentage for check_coverage_threshold (default: 80.0)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "tool_file".to_string(),
                description: "Specific tool file to check coverage for (for check_coverage_threshold)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Sub-path to limit code health scan (for check_code_health)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "baseline".to_string(),
                description: "Baseline JSON string or file path with previous counts (for check_code_health)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_unwrap_increase".to_string(),
                description: "Maximum allowed increase in unwrap count vs baseline (for check_code_health, default: 0)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let project_path = params
            .get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match action {
            "generate_ci" => {
                let platform = params
                    .get("platform")
                    .and_then(|v| v.as_str())
                    .unwrap_or("github");
                generate_ci_config(platform, project_path)
            }
            "run_tests" => {
                let test_pattern = params
                    .get("test_pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let all_features = params
                    .get("all_features")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                run_tests(project_path, test_pattern, all_features)
            }
            "run_build" => {
                let profile = params
                    .get("profile")
                    .and_then(|v| v.as_str())
                    .unwrap_or("dev");
                run_build(project_path, profile)
            }
            "coverage" => generate_coverage_info(project_path),
            "benchmark" => run_benchmark(project_path),
            "audit" => run_audit(project_path),
            "check_prerelease" => check_prerelease(project_path),
            "check_coverage_threshold" => {
                let min_coverage = params
                    .get("min_coverage")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(80.0);
                let tool_file = params
                    .get("tool_file")
                    .and_then(|v| v.as_str());
                check_coverage_threshold(project_path, min_coverage, tool_file)
            }
            "check_code_health" => {
                let baseline = params
                    .get("baseline")
                    .and_then(|v| v.as_str());
                let max_unwrap_increase = params
                    .get("max_unwrap_increase")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let path_filter = params
                    .get("path")
                    .and_then(|v| v.as_str());
                check_code_health(project_path, path_filter, baseline, max_unwrap_increase)
            }
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: generate_ci, run_tests, run_build, coverage, benchmark, audit, check_prerelease, check_coverage_threshold, check_code_health"),
            })),
        }
    }
}

// ============================================================================
// Generate CI Configuration
// ============================================================================

fn generate_ci_config(platform: &str, _project_path: &str) -> Result<Value, String> {
    match platform {
        "github" => {
            let config = r#"name: Rust CI

on:
  push:
    branches: [ main, master ]
  pull_request:
    branches: [ main, master ]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --all-targets --all-features

  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-targets --all-features

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --all-targets --all-features -- -D warnings

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: rustsec/audit-check@v1.4.1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

  # Optional: cross-platform testing
  cross-platform:
    name: Cross-platform (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
"#;
            Ok(json!({
                "status": "ok",
                "action": "generate_ci",
                "platform": "github",
                "file_path": ".github/workflows/ci.yml",
                "config": config,
                "description": "Save this content to .github/workflows/ci.yml in your project root",
            }))
        }
        "gitlab" => {
            let config = r#"stages:
  - check
  - test
  - lint
  - security

variables:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

check:
  stage: check
  image: rust:latest
  script:
    - cargo check --all-targets --all-features

test:
  stage: test
  image: rust:latest
  script:
    - cargo test --all-targets --all-features

clippy:
  stage: lint
  image: rust:latest
  script:
    - rustup component add clippy
    - cargo clippy --all-targets --all-features -- -D warnings

fmt:
  stage: lint
  image: rust:latest
  script:
    - rustup component add rustfmt
    - cargo fmt --all -- --check

security:
  stage: security
  image: rust:latest
  script:
    - cargo install cargo-audit
    - cargo audit
"#;
            Ok(json!({
                "status": "ok",
                "action": "generate_ci",
                "platform": "gitlab",
                "file_path": ".gitlab-ci.yml",
                "config": config,
                "description": "Save this content to .gitlab-ci.yml in your project root",
            }))
        }
        _ => Err(format!("Unknown platform: {platform}. Supported: github, gitlab")),
    }
}

// ============================================================================
// Run Tests
// ============================================================================

fn run_tests(project_path: &str, test_pattern: &str, all_features: bool) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("test");

    if all_features {
        cmd.arg("--all-features");
    }
    if !test_pattern.is_empty() {
        cmd.arg(test_pattern);
    }

    let output = cmd
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to run cargo test: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();

    // Parse test results
    let passed = stdout.lines().filter(|l| l.contains("test ... ok")).count();
    let failed = stdout.lines().filter(|l| l.contains("test ... FAILED")).count();
    let ignored = stdout.lines().filter(|l| l.contains("test ... ignored")).count();

    Ok(json!({
        "status": if success { "ok" } else { "error" },
        "action": "run_tests",
        "success": success,
        "exit_code": output.status.code().unwrap_or(-1),
        "results": {
            "passed": passed,
            "failed": failed,
            "ignored": ignored,
            "total": passed + failed + ignored,
        },
        "stderr": stderr.to_string(),
        "stdout": stdout.to_string(),
    }))
}

// ============================================================================
// Run Build
// ============================================================================

fn run_build(project_path: &str, profile: &str) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("cargo");

    match profile {
        "release" | "bench" => {
            cmd.arg("build").arg("--release");
        }
        _ => {
            cmd.arg("build");
        }
    }

    let output = cmd
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to run cargo build: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();

    Ok(json!({
        "status": if success { "ok" } else { "error" },
        "action": "run_build",
        "profile": profile,
        "success": success,
        "exit_code": output.status.code().unwrap_or(-1),
        "stderr": stderr.to_string(),
        "stdout": stdout.to_string(),
    }))
}

// ============================================================================
// Coverage Info
// ============================================================================

fn generate_coverage_info(_project_path: &str) -> Result<Value, String> {
    let info = r#"# Test Coverage

To generate test coverage, install cargo-tarpaulin:

```bash
cargo install cargo-tarpaulin
```

Then run:

```bash
cargo tarpaulin --out Html --output-dir target/tarpaulin
```

Or for XML (CI integration):

```bash
cargo tarpaulin --out Xml
```

Coverage thresholds recommendation:
- Line coverage: >= 80%
- Branch coverage: >= 70%
- Function coverage: >= 85%
"#;

    // Try to check if tarpaulin is installed
    let has_tarpaulin = std::process::Command::new("cargo")
        .arg("tarpaulin")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    Ok(json!({
        "status": "ok",
        "action": "coverage",
        "tarpaulin_installed": has_tarpaulin,
        "instructions": info,
        "recommendation": if has_tarpaulin {
            "cargo-tarpaulin is installed. Run: cargo tarpaulin --out Html"
        } else {
            "Install cargo-tarpaulin: cargo install cargo-tarpaulin"
        },
    }))
}

// ============================================================================
// Benchmark
// ============================================================================

fn run_benchmark(project_path: &str) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("bench");

    let output = cmd
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to run cargo bench: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();

    // Parse benchmark names and times
    let mut benchmarks = Vec::new();
    for line in stdout.lines() {
        if line.contains("... ") {
            let parts: Vec<&str> = line.splitn(2, "... ").collect();
            if parts.len() == 2 {
                let name = parts[0].trim();
                let result = parts[1].trim();
                benchmarks.push(json!({
                    "name": name,
                    "result": result,
                }));
            }
        }
    }

    Ok(json!({
        "status": if success { "ok" } else { "error" },
        "action": "benchmark",
        "success": success,
        "exit_code": output.status.code().unwrap_or(-1),
        "benchmarks": benchmarks,
        "stderr": stderr.to_string(),
    }))
}

// ============================================================================
// Security Audit
// ============================================================================

fn run_audit(project_path: &str) -> Result<Value, String> {
    // Try cargo audit first
    let audit_output = std::process::Command::new("cargo")
        .arg("audit")
        .current_dir(project_path)
        .output();

    match audit_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let success = output.status.success();

            Ok(json!({
                "status": if success { "ok" } else { "warning" },
                "action": "audit",
                "tool": "cargo-audit",
                "success": success,
                "vulnerabilities_found": !success,
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }
        Err(_) => {
            // cargo-audit not installed, check Cargo.lock manually
            let cargo_lock = std::path::Path::new(project_path).join("Cargo.lock");
            let has_lock = cargo_lock.exists();

            Ok(json!({
                "status": "warning",
                "action": "audit",
                "tool": "cargo-audit",
                "audit_available": false,
                "cargo_lock_exists": has_lock,
                "message": "cargo-audit is not installed. Run: cargo install cargo-audit",
                "recommendation": "Install cargo-audit for automated vulnerability scanning",
            }))
        }
    }
}

// ============================================================================
// Pre-release Checklist
// ============================================================================

fn check_prerelease(project_path: &str) -> Result<Value, String> {
    let mut checklist = Vec::new();

    // 1. cargo check
    let check_ok = std::process::Command::new("cargo")
        .arg("check")
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    checklist.push(json!({
        "item": "cargo check",
        "passed": check_ok,
        "description": "Code compiles without errors",
    }));

    // 2. cargo test
    let test_ok = std::process::Command::new("cargo")
        .arg("test")
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    checklist.push(json!({
        "item": "cargo test",
        "passed": test_ok,
        "description": "All tests pass",
    }));

    // 3. cargo clippy
    let clippy_ok = std::process::Command::new("cargo")
        .arg("clippy")
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    checklist.push(json!({
        "item": "cargo clippy",
        "passed": clippy_ok,
        "description": "No clippy warnings",
    }));

    // 4. cargo fmt check
    let fmt_ok = std::process::Command::new("cargo")
        .arg("fmt")
        .arg("--")
        .arg("--check")
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    checklist.push(json!({
        "item": "cargo fmt --check",
        "passed": fmt_ok,
        "description": "Code is properly formatted",
    }));

    // 5. Check for debug artifacts
    let src_path = std::path::Path::new(project_path).join("src");
    let mut debug_artifacts = Vec::new();
    if src_path.exists() {
        for entry in walkdir::WalkDir::new(&src_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains("dbg!(") || content.contains("todo!()") || content.contains("unimplemented!()") {
                    debug_artifacts.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }
    checklist.push(json!({
        "item": "no debug artifacts",
        "passed": debug_artifacts.is_empty(),
        "description": "No dbg!, todo!, or unimplemented! in production code",
        "details": if debug_artifacts.is_empty() { Value::Null } else { json!(debug_artifacts) },
    }));

    // 6. Cargo.lock exists
    let lock_exists = std::path::Path::new(project_path).join("Cargo.lock").exists();
    checklist.push(json!({
        "item": "Cargo.lock exists",
        "passed": lock_exists,
        "description": "Lock file committed for reproducible builds",
    }));

    let all_passed = checklist.iter().all(|c| c["passed"].as_bool().unwrap_or(false));
    let passed_count = checklist.iter().filter(|c| c["passed"].as_bool().unwrap_or(false)).count();

    Ok(json!({
        "status": if all_passed { "ok" } else { "warning" },
        "action": "check_prerelease",
        "all_passed": all_passed,
        "passed": passed_count,
        "total": checklist.len(),
        "checklist": checklist,
    }))
}

// ============================================================================
// Check Coverage Threshold
// ============================================================================

/// Check that tool files meet minimum test coverage threshold (default 80%).
fn check_coverage_threshold(
    project_path: &str,
    min_coverage: f64,
    tool_file: Option<&str>,
) -> Result<Value, String> {
    // Check if cargo-llvm-cov is installed
    let has_llvm_cov = std::process::Command::new("cargo")
        .arg("llvm-cov")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_llvm_cov {
        return Ok(json!({
            "status": "error",
            "action": "check_coverage_threshold",
            "min_coverage_pct": format!("{:.0}", min_coverage),
            "passed": false,
            "llvm_cov_installed": false,
            "message": "cargo-llvm-cov is not installed. Run: cargo install cargo-llvm-cov",
        }));
    }

    // Run coverage analysis
    let output = std::process::Command::new("cargo")
        .arg("llvm-cov")
        .arg("--lib")
        .arg("--json")
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to run cargo llvm-cov: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(json!({
            "status": "error",
            "action": "check_coverage_threshold",
            "min_coverage": min_coverage,
            "passed": false,
            "message": "cargo llvm-cov failed",
            "stderr": stderr.to_string(),
        }));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let coverage_data: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse coverage JSON: {e}"))?;

    // Parse per-file coverage from llvm-cov JSON output
    let mut file_results = Vec::new();
    let mut all_passed = true;

    // llvm-cov JSON format: {"data": [{"files": [...], "totals": {...}}]}
    if let Some(data_arr) = coverage_data.get("data").and_then(|d| d.as_array()) {
        for data_item in data_arr {
            if let Some(files) = data_item.get("files").and_then(|f| f.as_array()) {
                for file_entry in files {
                    let filename = file_entry
                        .get("filename")
                        .and_then(|f| f.as_str())
                        .unwrap_or("unknown");

                    // Skip if a specific file is requested and this isn't it
                    if let Some(target) = tool_file {
                        if !filename.contains(target) {
                            continue;
                        }
                    }

                    // Only check tool files in src/tools/builtin
                    if !filename.contains("tools/builtin") {
                        continue;
                    }

                    let line_percent = file_entry
                        .get("summary")
                        .and_then(|s| s.get("lines"))
                        .and_then(|l| l.get("percent"))
                        .and_then(|p| p.as_f64())
                        .unwrap_or(0.0);

                    let branch_percent = file_entry
                        .get("summary")
                        .and_then(|s| s.get("branches"))
                        .and_then(|b| b.get("percent"))
                        .and_then(|p| p.as_f64())
                        .unwrap_or(0.0);

                    let passes = line_percent >= min_coverage;
                    if !passes {
                        all_passed = false;
                    }

                    file_results.push(json!({
                        "file": filename,
                        "line_coverage_pct": format!("{:.1}", line_percent),
                        "branch_coverage_pct": format!("{:.1}", branch_percent),
                        "passes": passes,
                        "threshold_pct": format!("{:.0}", min_coverage),
                    }));
                }
            }
        }
    }

    // Extract overall project coverage
    let total_line_pct = coverage_data
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("totals"))
        .and_then(|t| t.get("lines"))
        .and_then(|l| l.get("percent"))
        .and_then(|p| p.as_f64());

    let files_passing = file_results.iter().filter(|f| f["passes"].as_bool().unwrap_or(false)).count();
    let files_failing = file_results.len() - files_passing;

    Ok(json!({
        "status": if all_passed { "ok" } else { "error" },
        "action": "check_coverage_threshold",
        "min_coverage_pct": format!("{:.0}", min_coverage),
        "passed": all_passed,
        "total_line_coverage_pct": total_line_pct.map(|p| format!("{:.1}", p)),
        "file_results": file_results,
        "files_checked": file_results.len(),
        "files_passing": files_passing,
        "files_failing": files_failing,
    }))
}

// ============================================================================
// Check Code Health
// ============================================================================

/// Scan code for anti-patterns and detect regressions vs baseline.
/// Monitors: .unwrap(), .expect(), dbg!(), todo!(), unimplemented!(), clone()
fn check_code_health(
    project_path: &str,
    path_filter: Option<&str>,
    baseline: Option<&str>,
    max_unwrap_increase: u64,
) -> Result<Value, String> {
    let scan_path = match path_filter {
        Some(p) => std::path::Path::new(project_path).join(p),
        None => std::path::Path::new(project_path).join("src"),
    };

    if !scan_path.exists() {
        return Err(format!("Scan path does not exist: {:?}", scan_path));
    }

    // Patterns to detect (anti-patterns in production code)
    let patterns = [("unwrap", r"\.unwrap\(\)"),
        ("expect", r"\.expect\("),
        ("dbg_macro", r"dbg!\("),
        ("todo_macro", r"todo!\("),
        ("unimplemented_macro", r"unimplemented!\("),
        ("clone", r"\.clone\(\)"),
        ("unsafe", r"\bunsafe\s*\{"),
        ("panic", r"\bpanic!\(")];

    // Compile all regexes once
    let compiled_patterns: Vec<(String, regex::Regex)> = patterns
        .iter()
        .filter_map(|(name, pattern)| {
            regex::Regex::new(pattern)
                .ok()
                .map(|re| (name.to_string(), re))
        })
        .collect();

    // Count occurrences per file
    let mut total_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut file_results = Vec::new();

    for entry in walkdir::WalkDir::new(&scan_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "rs")
        })
    {
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let relative_path = entry
            .path()
            .strip_prefix(project_path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let mut file_patterns = std::collections::HashMap::new();

        for (name, re) in &compiled_patterns {
            let count = re.find_iter(&content).count();
            if count > 0 {
                file_patterns.insert(name.clone(), count);
                *total_counts.entry(name.clone()).or_insert(0) += count;
            }
        }

        // Distinguish test vs production code
        let is_test_file = relative_path.contains("/tests/")
            || entry.path().file_name().is_some_and(|n| {
                n.to_string_lossy().starts_with("test_")
            });

        // For production code, separate unwrap/expect in #[cfg(test)] blocks
        let mut prod_counts = file_patterns.clone();
        if !is_test_file {
            // Subtract patterns that appear only in #[cfg(test)] blocks
            for (name, _re) in &compiled_patterns {
                if let Some(&total) = file_patterns.get(name) {
                    if total > 0 {
                        // Simple heuristic: count occurrences in lines between #[cfg(test)] and end of block
                        let test_block_count = count_in_test_blocks(&content, name);
                        let prod = total.saturating_sub(test_block_count);
                        prod_counts.insert(name.clone(), prod);
                        total_counts.entry(name.clone()).and_modify(|c| {
                            *c = c.saturating_sub(test_block_count) + prod;
                        });
                    }
                }
            }
        }

        if !prod_counts.is_empty() {
            file_results.push(json!({
                "file": relative_path,
                "is_test": is_test_file,
                "counts": prod_counts,
            }));
        }
    }

    // Compare against baseline
    let mut regression_detected = false;
    let mut regression_details = Vec::new();

    if let Some(baseline_str) = baseline {
        let baseline_data: Value = if baseline_str.starts_with('{') {
            serde_json::from_str(baseline_str).map_err(|e| format!("Invalid baseline JSON: {e}"))?
        } else {
            let baseline_path = std::path::Path::new(baseline_str);
            let content = std::fs::read_to_string(baseline_path)
                .map_err(|e| format!("Cannot read baseline file: {e}"))?;
            serde_json::from_str(&content).map_err(|e| format!("Invalid baseline JSON in file: {e}"))?
        };

        if let Some(baseline_counts) = baseline_data.get("total_counts").and_then(|v| v.as_object()) {
            for (key, current_val) in &total_counts {
                let baseline_val = baseline_counts.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let increase = current_val.saturating_sub(baseline_val);

                if increase > 0 {
                    // Check if this is unwrap/expect and exceeds threshold
                    if (key == "unwrap" || key == "expect") && increase as u64 > max_unwrap_increase {
                        regression_detected = true;
                        regression_details.push(json!({
                            "pattern": key,
                            "baseline": baseline_val,
                            "current": current_val,
                            "increase": increase,
                            "max_allowed_increase": max_unwrap_increase,
                            "status": "REGRESSION",
                        }));
                    } else {
                        regression_details.push(json!({
                            "pattern": key,
                            "baseline": baseline_val,
                            "current": current_val,
                            "increase": increase,
                            "status": "increased",
                        }));
                    }
                }
            }
        }
    }

    // Check for critical anti-patterns
    let critical_unwraps = total_counts.get("unwrap").copied().unwrap_or(0);
    let critical_expects = total_counts.get("expect").copied().unwrap_or(0);
    let critical_dbg = total_counts.get("dbg_macro").copied().unwrap_or(0);
    let critical_todo = total_counts.get("todo_macro").copied().unwrap_or(0);
    let critical_unimplemented = total_counts.get("unimplemented_macro").copied().unwrap_or(0);

    let health_score = calculate_health_score(&total_counts);

    Ok(json!({
        "status": if regression_detected { "error" } else { "ok" },
        "action": "check_code_health",
        "scan_path": scan_path.to_string_lossy().to_string(),
        "health_score": health_score,
        "total_counts": total_counts,
        "file_results": file_results,
        "files_scanned": file_results.len(),
        "regression_detected": regression_detected,
        "regression_details": regression_details,
        "summary": {
            "production_unwraps": critical_unwraps,
            "production_expects": critical_expects,
            "production_dbg": critical_dbg,
            "production_todo": critical_todo,
            "production_unimplemented": critical_unimplemented,
            "clone_count": total_counts.get("clone").copied().unwrap_or(0),
        },
        "recommendations": generate_health_recommendations(&total_counts),
    }))
}

/// Count pattern occurrences within #[cfg(test)] blocks
fn count_in_test_blocks(content: &str, _pattern_name: &str) -> usize {
    let mut count = 0;
    let mut in_test_block = false;
    let mut brace_depth = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[test]") {
            in_test_block = true;
            brace_depth = 0;
            continue;
        }

        if in_test_block {
            brace_depth += line.matches('{').count();
            brace_depth -= line.matches('}').count();

            if brace_depth == 0 && line.contains('}') {
                in_test_block = false;
            } else {
                // Count .unwrap() in test blocks
                if _pattern_name == "unwrap" {
                    count += line.matches(".unwrap()").count();
                } else if _pattern_name == "expect" {
                    count += line.matches(".expect(").count();
                }
            }
        }
    }

    count
}

/// Calculate a health score from 0-100 based on anti-pattern counts
fn calculate_health_score(counts: &std::collections::HashMap<String, usize>) -> f64 {
    let mut score = 100.0;

    // Deduct for each anti-pattern type
    let unwrap_count = counts.get("unwrap").copied().unwrap_or(0);
    let expect_count = counts.get("expect").copied().unwrap_or(0);
    let dbg_count = counts.get("dbg_macro").copied().unwrap_or(0);
    let todo_count = counts.get("todo_macro").copied().unwrap_or(0);
    let unimplemented_count = counts.get("unimplemented_macro").copied().unwrap_or(0);

    // unwrap/expect in production code is most concerning
    score -= (unwrap_count as f64) * 2.0;
    score -= (expect_count as f64) * 1.5;

    // dbg! and todo! are serious issues
    score -= (dbg_count as f64) * 5.0;
    score -= (todo_count as f64) * 3.0;
    score -= (unimplemented_count as f64) * 5.0;

    // clone is less concerning but still notable
    let clone_count = counts.get("clone").copied().unwrap_or(0);
    score -= (clone_count as f64) * 0.2;

    // unsafe blocks
    let unsafe_count = counts.get("unsafe").copied().unwrap_or(0);
    score -= (unsafe_count as f64) * 3.0;

    score.clamp(0.0, 100.0)
}

/// Generate recommendations based on anti-pattern counts
fn generate_health_recommendations(counts: &std::collections::HashMap<String, usize>) -> Vec<String> {
    let mut recs = Vec::new();

    if let Some(&count) = counts.get("unwrap") {
        if count > 0 {
            recs.push(format!(
                "Found {} .unwrap() calls in production code. Replace with ? operator or proper error handling.",
                count
            ));
        }
    }

    if let Some(&count) = counts.get("expect") {
        if count > 0 {
            recs.push(format!(
                "Found {} .expect() calls. Consider using .context() from anyhow for better error messages.",
                count
            ));
        }
    }

    if let Some(&count) = counts.get("dbg_macro") {
        if count > 0 {
            recs.push(format!(
                "Found {} dbg!() macro calls. Remove before committing to production.",
                count
            ));
        }
    }

    if let Some(&count) = counts.get("todo_macro") {
        if count > 0 {
            recs.push(format!(
                "Found {} todo!() macros. These will panic at runtime - complete or remove them.",
                count
            ));
        }
    }

    if let Some(&count) = counts.get("unimplemented_macro") {
        if count > 0 {
            recs.push(format!(
                "Found {} unimplemented!() macros. These will panic at runtime.",
                count
            ));
        }
    }

    if recs.is_empty() {
        recs.push("No significant anti-patterns detected. Code health looks good!".to_string());
    }

    recs
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CiCdTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_ci_github() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("generate_ci"));
        params.insert("platform".to_string(), json!("github"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["platform"], "github");
        assert_eq!(result["file_path"], ".github/workflows/ci.yml");
        let config = result["config"].as_str().unwrap();
        assert!(config.contains("name: Rust CI"));
        assert!(config.contains("cargo check"));
        assert!(config.contains("cargo test"));
        assert!(config.contains("cargo clippy"));
        assert!(config.contains("cargo fmt"));
    }

    #[tokio::test]
    async fn test_generate_ci_gitlab() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("generate_ci"));
        params.insert("platform".to_string(), json!("gitlab"));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["platform"], "gitlab");
        assert_eq!(result["file_path"], ".gitlab-ci.yml");
        let config = result["config"].as_str().unwrap();
        assert!(config.contains("stages:"));
        assert!(config.contains("cargo check"));
        assert!(config.contains("cargo clippy"));
    }

    #[tokio::test]
    async fn test_generate_ci_unknown_platform() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("generate_ci"));
        params.insert("platform".to_string(), json!("jenkins"));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown platform"));
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = CiCdTool;
        let params = HashMap::new();
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter: action"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("nonexistent"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_coverage_info() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("coverage"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["instructions"].as_str().unwrap().contains("cargo-llvm-cov"));
        assert!(result["recommendation"].as_str().unwrap().contains("cargo-llvm-cov"));
    }

    #[tokio::test]
    async fn test_check_coverage_threshold_no_llvm_cov() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_coverage_threshold"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("min_coverage".to_string(), json!(80.0));

        let result = tool.execute(&params).await.unwrap();
        // If cargo-llvm-cov is not installed, returns error status with helpful message
        let status = result["status"].as_str().unwrap();
        assert!(status == "error" || status == "ok");
        assert_eq!(result["action"], "check_coverage_threshold");
        assert_eq!(result["min_coverage_pct"], "80");
    }

    #[tokio::test]
    async fn test_check_coverage_threshold_custom_threshold() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_coverage_threshold"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("min_coverage".to_string(), json!(90.0));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["min_coverage_pct"], "90");
    }

    #[tokio::test]
    async fn test_check_coverage_threshold_specific_file() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_coverage_threshold"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("min_coverage".to_string(), json!(80.0));
        params.insert("tool_file".to_string(), json!("ci_cd_tool.rs"));

        let result = tool.execute(&params).await.unwrap();
        // Should at least check this file
        assert_eq!(result["action"], "check_coverage_threshold");
    }

    #[tokio::test]
    async fn test_audit_no_audit_installed() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("audit"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        // If cargo-audit is not installed, status is warning
        let status = result["status"].as_str().unwrap();
        assert!(status == "ok" || status == "warning");
    }

    #[tokio::test]
    async fn test_run_tests_in_current_project() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("run_tests"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        let results = &result["results"];
        // At least some tests should have run
        let total = results["total"].as_u64().unwrap_or(0);
        let passed = results["passed"].as_u64().unwrap_or(0);
        assert!(total > 0, "Expected at least some tests to run, got total={}", total);
        assert!(passed > 0, "Expected at least some tests to pass, got passed={}", passed);
    }

    #[tokio::test]
    async fn test_run_build_in_current_project() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("run_build"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["profile"], "dev");
        let success = result["success"].as_bool().unwrap();
        assert!(success, "Build should succeed in current project");
    }

    #[tokio::test]
    async fn test_run_build_release_profile() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("run_build"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("profile".to_string(), json!("release"));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["profile"], "release");
        let success = result["success"].as_bool().unwrap();
        assert!(success, "Release build should succeed");
    }

    #[tokio::test]
    async fn test_run_tests_with_pattern() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("run_tests"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("test_pattern".to_string(), json!("test_format_duration"));

        let result = tool.execute(&params).await.unwrap();
        // The pattern filter may or may not match tests depending on current test suite
        // Just verify it doesn't crash
        let _total = result["results"]["total"].as_u64().unwrap_or(0);
    }

    #[tokio::test]
    async fn test_run_build_bench_profile() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("run_build"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("profile".to_string(), json!("bench"));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["profile"], "bench");
        let success = result["success"].as_bool().unwrap();
        assert!(success, "Bench build should succeed");
    }

    #[tokio::test]
    async fn test_check_code_health_src_directory() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_code_health"));
        params.insert("project_path".to_string(), json!("."));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "check_code_health");
        // Should have scanned files
        let files_scanned = result["files_scanned"].as_u64().unwrap_or(0);
        assert!(files_scanned > 0, "Should have scanned some files, got {}", files_scanned);
        // Should have total_counts
        assert!(result["total_counts"].is_object());
        // Should have a health_score
        assert!(result["health_score"].is_number());
    }

    #[tokio::test]
    async fn test_check_code_health_invalid_path() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_code_health"));
        params.insert("project_path".to_string(), json!("/nonexistent/path"));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_code_health_with_baseline() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_code_health"));
        params.insert("project_path".to_string(), json!("."));
        // Use a baseline with zero counts
        params.insert("baseline".to_string(), json!(r#"{"total_counts": {}}"#));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "check_code_health");
        // With empty baseline, all patterns should show as increased
        let details = result["regression_details"].as_array().unwrap();
        // Should have regression details since current > baseline (0)
        assert!(!details.is_empty() || result["total_counts"].as_object().map(|o| o.is_empty()).unwrap_or(true));
    }

    #[tokio::test]
    async fn test_check_code_health_regression_detection() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_code_health"));
        params.insert("project_path".to_string(), json!("."));
        // Use a baseline with very high counts (higher than current) to test no regression
        let baseline = r#"{"total_counts": {"unwrap": 99999, "expect": 99999, "dbg_macro": 99999}}"#;
        params.insert("baseline".to_string(), json!(baseline));

        let result = tool.execute(&params).await.unwrap();
        // Current counts should be lower than baseline, so no regressions
        assert_eq!(result["regression_detected"], false);
    }

    #[tokio::test]
    async fn test_check_code_health_specific_subpath() {
        let tool = CiCdTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("check_code_health"));
        params.insert("project_path".to_string(), json!("."));
        params.insert("path".to_string(), json!("src/tools/builtin/ci_cd_tool.rs"));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "check_code_health");
        // Should find results for this specific file
        let files_scanned = result["files_scanned"].as_u64().unwrap_or(0);
        assert!(files_scanned >= 1);
    }
}
