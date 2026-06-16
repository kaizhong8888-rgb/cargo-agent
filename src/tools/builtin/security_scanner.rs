//! Advanced Security Scanner: vulnerability detection, dependency audit, code security patterns.
//!
//! # Actions
//!
//! - **scan**: Scan code for security patterns (SQL injection, command injection, hardcoded secrets, etc.)
//! - **audit_deps**: Check dependencies for known vulnerabilities
//! - **check_secrets**: Scan for hardcoded secrets/credentials
//! - **report**: Generate security report

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Security Pattern Definitions
// ============================================================================

// Pre-compiled security patterns
static RE_SQL_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        (?:query|execute|exec)\s*\(\s*
        (?:format!\s*!\s*\(.*?%.*?
        |".*?\{.*\}.*"
        |&?\s*format!\s*\()
    "#,
    )
    .expect("valid regex")
});

static RE_CMD_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        (?:Command::new|std::process::Command)
        .*?(?:format!|concat!|\+.*user|\.arg\(.*\{)
    "#,
    )
    .expect("valid regex")
});

static RE_PATH_TRAVERSAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"File::open\s*\([^)]*(?:\{|\+|format!)"#).expect("valid regex"));

static RE_UNWRAP_ON_IO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:File::open|fs::read|fs::write|fs::read_to_string).*\.unwrap\(\)"#)
        .expect("valid regex")
});

static RE_DEPRECATED_CRYPTO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        # Actual usage patterns (function calls, imports, type annotations)
        (?:
            (?:Md5|Sha1|DES|RC4|Ecb)::           # Type::method() pattern
            |(?:md5|sha1|des|rc4)\s*::           # module::function() pattern
            |use\s+.*(?:md5|sha1|des|rc4)        # use statements
            |::(?:md5|sha1)\s*\(                  # ::md5() function call
        )
    "#,
    )
    .expect("valid regex")
});

static RE_TAR_BOMBING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"Archive::new.*unpack"#).expect("valid regex"));

static RE_RACE_CONDITION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:fs::exists|Path::exists).*\n.*fs::"#).expect("valid regex"));

static RE_LOG_SENSITIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:info!|debug!|warn!|error!|println!).*\{.*(?:password|secret|token|key)"#)
        .expect("valid regex")
});

static RE_UNSAFE_NO_SAFETY: Lazy<Regex> = Lazy::new(|| {
    // Detect unsafe blocks - manual review needed for SAFETY comments
    Regex::new(r#"unsafe\s*\{"#).expect("valid regex")
});

// ============================================================================
// SecurityScannerTool
// ============================================================================

pub struct SecurityScannerTool;

#[async_trait::async_trait]
impl Tool for SecurityScannerTool {
    fn name(&self) -> &str {
        "security_scan"
    }

    fn description(&self) -> &str {
        "Advanced security scanning: code security pattern detection, dependency vulnerability audit, hardcoded secrets detection. Actions: scan, audit_deps, check_secrets, report."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: scan, audit_deps, check_secrets, report".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to scan (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Scan recursively (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "severity".to_string(),
                description: "Minimum severity: critical, high, medium, low (default: low)"
                    .to_string(),
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

        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        match action {
            "scan" => scan_code_security(path, recursive),
            "audit_deps" => audit_dependencies(path),
            "check_secrets" => check_hardcoded_secrets(path, recursive),
            "report" => generate_security_report(path, recursive),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: scan, audit_deps, check_secrets, report"),
            })),
        }
    }
}

// ============================================================================
// File Collection
// ============================================================================

fn collect_files(
    dir: &Path,
    files: &mut Vec<String>,
    recursive: bool,
    extensions: &[&str],
    skip_patterns: &[&str],
) -> Result<(), String> {
    if !dir.exists() {
        return Err(format!("Path does not exist: {}", dir.display()));
    }
    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
    for entry in read_dir.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == ".git"
            {
                continue;
            }
            if recursive {
                collect_files(&p, files, true, extensions, skip_patterns)?;
            }
        } else if p.is_file() {
            // Skip files matching skip patterns (e.g. the scanner's own source file)
            if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                if skip_patterns.iter().any(|pat| fname.contains(pat)) {
                    continue;
                }
            }
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(p.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(())
}

fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))
}

// ============================================================================
// Code Security Scan
// ============================================================================

struct ScanRule {
    name: &'static str,
    regex: &'static Lazy<Regex>,
    severity: &'static str,
    category: &'static str,
    description: &'static str,
    suggestion: &'static str,
}

fn scan_code_security(path: &str, recursive: bool) -> Result<Value, String> {
    let scan_path = Path::new(path);
    let mut files: Vec<String> = Vec::new();
    collect_files(
        scan_path,
        &mut files,
        recursive,
        &["rs"],
        &["security_scanner.rs"],
    )?;

    // Extra safety: explicitly remove the scanner's own file from the list
    // This prevents the scanner from matching its own regex pattern definitions
    files.retain(|f| !f.contains("security_scanner.rs"));

    if files.is_empty() {
        return Ok(json!({
            "status": "ok",
            "action": "security_scan",
            "message": "No Rust source files found to scan",
            "issues": [],
        }));
    }

    let rules: Vec<ScanRule> = vec![
        ScanRule {
            name: "sql_injection",
            regex: &RE_SQL_INJECTION,
            severity: "critical",
            category: "injection",
            description: "Potential SQL injection via string interpolation",
            suggestion: "Use parameterized queries instead of string formatting",
        },
        ScanRule {
            name: "command_injection",
            regex: &RE_CMD_INJECTION,
            severity: "critical",
            category: "injection",
            description: "Potential command injection via unsanitized input",
            suggestion: "Validate and sanitize all user inputs before passing to Command",
        },
        ScanRule {
            name: "path_traversal",
            regex: &RE_PATH_TRAVERSAL,
            severity: "high",
            category: "file_access",
            description: "Potential path traversal vulnerability",
            suggestion: "Validate file paths and use canonicalize() to resolve symlinks",
        },
        ScanRule {
            name: "unwrap_on_io",
            regex: &RE_UNWRAP_ON_IO,
            severity: "medium",
            category: "error_handling",
            description: "Unwrap on I/O operations will panic on failure",
            suggestion: "Use ? operator or proper error handling with match/if let",
        },
        ScanRule {
            name: "deprecated_crypto",
            regex: &RE_DEPRECATED_CRYPTO,
            severity: "high",
            category: "cryptography",
            description: "Use of deprecated/weak cryptographic algorithm",
            suggestion: "Use SHA-256/SHA-3 for hashing, AES-GCM for encryption",
        },
        ScanRule {
            name: "tar_bombing",
            regex: &RE_TAR_BOMBING,
            severity: "high",
            category: "archive",
            description: "Unvalidated archive extraction may lead to tar bombing",
            suggestion: "Validate archive entries before extraction, check for path traversal",
        },
        ScanRule {
            name: "log_sensitive_data",
            regex: &RE_LOG_SENSITIVE,
            severity: "high",
            category: "data_leak",
            description: "Sensitive data may be logged",
            suggestion: "Mask or redact sensitive data before logging",
        },
        ScanRule {
            name: "unsafe_no_safety",
            regex: &RE_UNSAFE_NO_SAFETY,
            severity: "medium",
            category: "safety",
            description: "Unsafe block without SAFETY comment",
            suggestion: "Add '// SAFETY: ' comment explaining why the unsafe code is sound",
        },
        ScanRule {
            name: "race_condition_toc",
            regex: &RE_RACE_CONDITION,
            severity: "medium",
            category: "concurrency",
            description: "Potential TOCTOU race condition",
            suggestion: "Use atomic file operations or proper locking mechanisms",
        },
    ];

    let mut issues: Vec<Value> = Vec::new();
    let mut stats: HashMap<String, usize> = HashMap::new();

    for file_path in &files {
        let content = read_file(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        for rule in &rules {
            for mat in rule.regex.find_iter(&content) {
                let line_num = content[..mat.start()].lines().count() + 1;
                let matched_text = mat.as_str().lines().next().unwrap_or("").trim();

                // Skip if inside test code
                let is_test = is_in_test_code(&lines, line_num);
                if is_test {
                    continue;
                }

                *stats.entry(rule.severity.to_string()).or_insert(0) += 1;
                *stats.entry("total".to_string()).or_insert(0) += 1;

                issues.push(json!({
                    "severity": rule.severity,
                    "category": rule.category,
                    "rule": rule.name,
                    "file": file_path,
                    "line": line_num,
                    "description": rule.description,
                    "matched": if matched_text.len() > 80 { format!("{}...", &matched_text[..80]) } else { matched_text.to_string() },
                    "suggestion": rule.suggestion,
                }));
            }
        }
    }

    // Sort by severity
    let severity_order = |s: &str| -> u8 {
        match s {
            "critical" => 0,
            "high" => 1,
            "medium" => 2,
            "low" => 3,
            _ => 4,
        }
    };
    issues.sort_by(|a, b| {
        let sa = a["severity"].as_str().unwrap_or("");
        let sb = b["severity"].as_str().unwrap_or("");
        severity_order(sa).cmp(&severity_order(sb))
    });

    let total = stats.get("total").copied().unwrap_or(0);
    let critical = stats.get("critical").copied().unwrap_or(0);
    let high = stats.get("high").copied().unwrap_or(0);

    Ok(json!({
        "status": if critical > 0 || high > 0 { "warning" } else { "ok" },
        "action": "security_scan",
        "files_scanned": files.len(),
        "total_issues": total,
        "summary": {
            "critical": critical,
            "high": high,
            "medium": stats.get("medium").copied().unwrap_or(0),
            "low": stats.get("low").copied().unwrap_or(0),
        },
        "issues": issues,
    }))
}

fn is_in_test_code(lines: &[&str], line_num: usize) -> bool {
    // Check if we're inside a #[cfg(test)] module, #[test] function,
    // or a #[cfg(test)] mod tests { ... } block
    let mut brace_depth = 0isize;
    let mut test_depth = -1isize;
    let mut prev_line_was_test_attr = false;

    for i in 0..line_num.saturating_sub(1) {
        let line = lines.get(i).unwrap_or(&"").trim();

        // Detect test markers — record the depth at which they appear
        if test_depth < 0 {
            let is_test_attr = line.starts_with("#[cfg(test)]")
                || line.starts_with("#[test]")
                || line.starts_with("#[tokio::test]")
                || line.starts_with("#[actix_web::test]")
                || line.starts_with("#[ctor]");
            let is_mod_block = line.starts_with("mod ") && line.contains('{');

            if is_test_attr {
                prev_line_was_test_attr = true;
            } else if prev_line_was_test_attr && is_mod_block {
                // #[cfg(test)]\nmod tests { — record depth before the '{'
                test_depth = brace_depth;
                prev_line_was_test_attr = false;
            } else if is_test_attr && line.contains('{') {
                // Single-line: #[test] fn foo() { or #[cfg(test)] mod tests {
                test_depth = brace_depth;
                prev_line_was_test_attr = false;
            } else {
                prev_line_was_test_attr = false;
            }
        }

        // Count braces to track scope depth
        for ch in line.chars() {
            match ch {
                '{' => {
                    brace_depth += 1;
                }
                '}' => {
                    if brace_depth == test_depth {
                        test_depth = -1;
                    }
                    brace_depth -= 1;
                }
                _ => {}
            }
        }
    }

    test_depth >= 0
}

// ============================================================================
// Dependency Audit
// ============================================================================

fn audit_dependencies(path: &str) -> Result<Value, String> {
    // Try cargo audit
    let audit_output = std::process::Command::new("cargo")
        .arg("audit")
        .arg("--json")
        .current_dir(path)
        .output();

    match audit_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let success = output.status.success();

            // Try to parse JSON output
            let vulns: Value = serde_json::from_str(&stdout).unwrap_or(json!({
                "raw_output": stdout.to_string(),
            }));

            Ok(json!({
                "status": if success { "ok" } else { "warning" },
                "action": "audit_deps",
                "vulnerabilities_found": !success,
                "output": vulns,
                "stderr": stderr.to_string(),
            }))
        }
        Err(_) => {
            // Parse Cargo.toml for dependencies
            let cargo_toml = std::path::Path::new(path).join("Cargo.toml");
            let deps = if cargo_toml.exists() {
                let content = read_file(&cargo_toml.to_string_lossy())?;
                let mut found_deps = Vec::new();
                let mut in_deps = false;
                for line in content.lines() {
                    if line.trim() == "[dependencies]" {
                        in_deps = true;
                        continue;
                    }
                    if line.starts_with('[') {
                        in_deps = false;
                        continue;
                    }
                    if in_deps {
                        if let Some(name) = line.split('=').next() {
                            found_deps.push(name.trim().to_string());
                        }
                    }
                }
                found_deps
            } else {
                Vec::new()
            };

            Ok(json!({
                "status": "info",
                "action": "audit_deps",
                "audit_available": false,
                "message": "cargo-audit is not installed",
                "dependencies_found": deps,
                "recommendation": "Install: cargo install cargo-audit && cargo audit",
            }))
        }
    }
}

// ============================================================================
// Hardcoded Secrets Detection
// ============================================================================

fn check_hardcoded_secrets(path: &str, recursive: bool) -> Result<Value, String> {
    let scan_path = Path::new(path);
    let mut files: Vec<String> = Vec::new();
    // Scan .rs, .toml, .env, .yaml, .yml, .json, .cfg, .conf
    // Exclude scanner's own file to prevent self-referencing false positives
    collect_files(
        scan_path,
        &mut files,
        recursive,
        &["rs", "toml", "env", "yaml", "yml", "json", "cfg", "conf"],
        &["security_scanner.rs"],
    )?;

    let secret_patterns: Vec<(&str, &str, &str)> = vec![
        (
            r#"(?i)(?:password|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#,
            "hardcoded_password",
            "Hardcoded password detected",
        ),
        (
            r#"(?i)(?:api_key|apikey|api_secret)\s*[=:]\s*["'][A-Za-z0-9]{16,}["']"#,
            "hardcoded_api_key",
            "Hardcoded API key detected",
        ),
        (
            r#"(?i)(?:secret_key|secret)\s*[=:]\s*["'][^"']{8,}["']"#,
            "hardcoded_secret",
            "Hardcoded secret detected",
        ),
        (
            r#"(?i)(?:token|access_token|auth_token)\s*[=:]\s*["'][A-Za-z0-9]{16,}["']"#,
            "hardcoded_token",
            "Hardcoded token detected",
        ),
        (
            r#"(?i)(?:private_key|priv_key)\s*[=:]\s*["']-----BEGIN"#,
            "hardcoded_private_key",
            "Hardcoded private key detected",
        ),
        (
            r#"(?i)aws_access_key_id\s*[=:]\s*["']AKIA"#,
            "aws_access_key",
            "AWS access key detected",
        ),
        (
            r#"(?i)aws_secret_access_key\s*[=:]\s*["'][A-Za-z0-9/+=]{40}"#,
            "aws_secret_key",
            "AWS secret key detected",
        ),
        (
            r#"sk-[a-zA-Z0-9]{20,}"#,
            "openai_key",
            "OpenAI API key detected",
        ),
        (
            r#"ghp_[a-zA-Z0-9]{36}"#,
            "github_token",
            "GitHub personal access token detected",
        ),
    ];

    let mut findings: Vec<Value> = Vec::new();

    for file_path in &files {
        let content = read_file(file_path)?;

        for (pattern, category, description) in &secret_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for mat in re.find_iter(&content) {
                    let line_num = content[..mat.start()].lines().count() + 1;
                    let matched = mat.as_str();

                    // Redact the secret value for safety
                    let redacted = if matched.len() > 20 {
                        format!("{}...{}", &matched[..8], &matched[matched.len() - 4..])
                    } else {
                        "***REDACTED***".to_string()
                    };

                    findings.push(json!({
                        "severity": "critical",
                        "category": category,
                        "file": file_path,
                        "line": line_num,
                        "description": description,
                        "matched_redacted": redacted,
                        "suggestion": "Use environment variables or a secrets manager instead",
                    }));
                }
            }
        }
    }

    let total = findings.len();
    Ok(json!({
        "status": if total > 0 { "warning" } else { "ok" },
        "action": "check_secrets",
        "files_scanned": files.len(),
        "total_findings": total,
        "findings": findings,
    }))
}

// ============================================================================
// Security Report
// ============================================================================

fn generate_security_report(path: &str, recursive: bool) -> Result<Value, String> {
    let code_scan = scan_code_security(path, recursive)?;
    let secrets_scan = check_hardcoded_secrets(path, recursive)?;
    let dep_audit = audit_dependencies(path)?;

    let total_issues = code_scan["total_issues"].as_u64().unwrap_or(0) as usize
        + secrets_scan["total_findings"].as_u64().unwrap_or(0) as usize;

    let has_critical = code_scan["summary"]["critical"].as_u64().unwrap_or(0) > 0
        || secrets_scan["total_findings"].as_u64().unwrap_or(0) > 0;

    Ok(json!({
        "status": if has_critical { "critical" } else if total_issues > 0 { "warning" } else { "ok" },
        "action": "security_report",
        "summary": {
            "total_issues": total_issues,
            "code_issues": code_scan["total_issues"],
            "secret_findings": secrets_scan["total_findings"],
            "dependency_audit": dep_audit["status"],
            "risk_level": if has_critical { "HIGH" } else if total_issues > 5 { "MEDIUM" } else { "LOW" },
        },
        "code_scan": code_scan,
        "secrets_scan": secrets_scan,
        "dependency_audit": dep_audit,
        "recommendations": [
            "Run `cargo audit` regularly to check for known vulnerabilities",
            "Use `cargo-deny` for dependency policy enforcement",
            "Use `cargo-geiger` to find unsafe code",
            "Use environment variables for secrets, never hardcode them",
            "Add `// SAFETY:` comments to all unsafe blocks",
            "Use parameterized queries for all database operations",
        ],
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(SecurityScannerTool));
}
