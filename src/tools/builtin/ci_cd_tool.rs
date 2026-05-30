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
        "CI/CD integration tool: generate CI configs (GitHub Actions/GitLab CI), run tests/builds/clippy/audit, generate coverage reports, and pre-release checklists."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: generate_ci, run_tests, run_build, coverage, benchmark, audit, check_prerelease".to_string(),
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
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: generate_ci, run_tests, run_build, coverage, benchmark, audit, check_prerelease"),
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
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CiCdTool));
}
