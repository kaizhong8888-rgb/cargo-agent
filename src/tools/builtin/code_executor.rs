//! Code execution sandbox: run cargo commands in an isolated temp directory.
//!
//! Allows the agent to compile and execute Rust code snippets safely,
//! returning stdout/stderr and exit codes.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::time::Duration;

const MAX_OUTPUT_BYTES: usize = 50_000;
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Kill a process and all its children by PID (Unix).
/// Used to enforce execution timeouts.
fn kill_process_tree(pid: u32) {
    // Send SIGTERM first (graceful)
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    // Small delay then SIGKILL if still alive
    std::thread::sleep(Duration::from_millis(500));
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
}

// ============================================================================
// CodeExecutorTool
// ============================================================================

pub struct CodeExecutorTool;

#[async_trait::async_trait]
impl Tool for CodeExecutorTool {
    fn name(&self) -> &str {
        "code_execute"
    }

    fn description(&self) -> &str {
        "Compile and run Rust code in an isolated temporary directory. Supports cargo run, cargo build, cargo test, and cargo check. Returns stdout, stderr, and exit status."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "code".to_string(),
                description: "Rust source code to execute. Should be a complete program (with main function) or library code.".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "command".to_string(),
                description: "Cargo command to run: run (default), build, test, check, clippy".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "dependencies".to_string(),
                description: "Cargo.toml [dependencies] section as TOML string (e.g. 'serde = \"1.0\"\\ntokio = \"1.0\"')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "edition".to_string(),
                description: "Rust edition to use: 2015, 2018, 2021 (default: 2021)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "timeout_secs".to_string(),
                description: "Execution timeout in seconds (default: 60)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "crate_type".to_string(),
                description: "Project type: binary (default, creates main.rs) or lib (creates lib.rs with tests)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let code = params
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: code")?;

        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("run");

        let deps_toml = params.get("dependencies").and_then(|v| v.as_str());
        let edition = params
            .get("edition")
            .and_then(|v| v.as_str())
            .unwrap_or("2021");

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let crate_type = params
            .get("crate_type")
            .and_then(|v| v.as_str())
            .unwrap_or("binary");

        // Validate command
        match command {
            "run" | "build" | "test" | "check" | "clippy" => {}
            other => return Err(format!("Unsupported cargo command: {other}")),
        }

        // Create temp directory
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-agent-exec-{}", uuid::Uuid::new_v4()));

        let cleanup = TempDirGuard(&temp_dir);

        // Create project structure
        let src_dir = temp_dir.join("src");
        fs::create_dir_all(&src_dir)
            .map_err(|e| format!("Failed to create temp directory: {e}"))?;

        // Write Cargo.toml
        let mut cargo_toml = format!(
            "[package]\n\
             name = \"sandbox\"\n\
             version = \"0.1.0\"\n\
             edition = \"{edition}\"\n\n"
        );

        if let Some(deps) = deps_toml {
            cargo_toml.push_str("[dependencies]\n");
            cargo_toml.push_str(deps);
            cargo_toml.push('\n');
        }

        fs::write(temp_dir.join("Cargo.toml"), &cargo_toml)
            .map_err(|e| format!("Failed to write Cargo.toml: {e}"))?;

        // Write source file
        let source_file = if crate_type == "lib" {
            src_dir.join("lib.rs")
        } else {
            src_dir.join("main.rs")
        };

        fs::write(&source_file, code).map_err(|e| format!("Failed to write source file: {e}"))?;

        // Run cargo command with timeout enforcement
        let (cargo_cmd, extra_args) = match command {
            "clippy" => ("clippy", vec!["--", "-D", "warnings"]),
            _ => (command, vec![]),
        };

        let mut cmd = Command::new("cargo");
        cmd.arg(cargo_cmd)
            .current_dir(&temp_dir)
            .env("CARGO_HOME", "/tmp/cargo-agent-home") // Isolated cargo home
            .env("RUST_BACKTRACE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for arg in &extra_args {
            cmd.arg(arg);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to execute cargo: {e}"))?;

        let child_id = child.id();

        // Enforce the timeout via tokio::time::timeout + spawn_blocking
        let output_result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::task::spawn_blocking(move || child.wait_with_output()),
        )
        .await;

        let output = match output_result {
            Ok(Ok(Ok(output))) => output,
            Ok(Ok(Err(e))) => {
                // Kill the child on error
                kill_process_tree(child_id);
                return Err(format!("Failed to wait for cargo: {e}"));
            }
            Ok(Err(_)) => {
                // spawn_blocking join error
                return Err("Task execution failed".to_string());
            }
            Err(_) => {
                // Timeout — kill the process tree
                kill_process_tree(child_id);
                return Err(format!(
                    "Execution timed out after {timeout_secs}s. Consider simplifying your code or increasing the timeout."
                ));
            }
        };

        let exit_code = output.status.code();
        let success = output.status.success();

        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Truncate large outputs
        let stdout_truncated = stdout.len() > MAX_OUTPUT_BYTES;
        if stdout_truncated {
            stdout.truncate(MAX_OUTPUT_BYTES);
            stdout.push_str("\n... [output truncated, exceeded 50KB limit]");
        }

        let stderr_truncated = stderr.len() > MAX_OUTPUT_BYTES;
        if stderr_truncated {
            stderr.truncate(MAX_OUTPUT_BYTES);
            stderr.push_str("\n... [output truncated, exceeded 50KB limit]");
        }

        // Clean up temp directory
        let _ = fs::remove_dir_all(&temp_dir);
        std::mem::forget(cleanup);

        Ok(serde_json::json!({
            "status": if success { "ok" } else { "error" },
            "command": format!("cargo {cargo_cmd}"),
            "exit_code": exit_code,
            "success": success,
            "stdout": stdout,
            "stderr": stderr,
            "stdout_truncated": stdout_truncated,
            "stderr_truncated": stderr_truncated,
            "temp_dir": temp_dir.to_string_lossy(),
        }))
    }
}

/// RAII guard to clean up temp directory on drop.
struct TempDirGuard<'a>(&'a PathBuf);

impl Drop for TempDirGuard<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0);
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeExecutorTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_executor_tool_metadata() {
        let tool = CodeExecutorTool;
        assert_eq!(tool.name(), "code_execute");
        assert!(tool.description().contains("Rust code"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "code" && p.required));
        assert!(params.iter().any(|p| p.name == "command" && !p.required));
    }

    #[test]
    fn temp_dir_guard_cleans_up() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-agent-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        assert!(temp_dir.exists());

        let guard = TempDirGuard(&temp_dir);
        drop(guard);
        assert!(!temp_dir.exists());
    }
}
