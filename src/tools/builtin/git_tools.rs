//! Git integration tools: clone, status, diff, log, commit, push.
//!
//! Gives the agent full Git capabilities to analyze repositories,
//! track changes, and commit improvements.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

// ============================================================================
// GitStatusTool
// ============================================================================

pub struct GitStatusTool;

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }

    fn description(&self) -> &str {
        "Show the working tree status of a Git repository. Lists modified, staged, and untracked files."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "repo_path".to_string(),
            description: "Path to the Git repository (default: current directory)".to_string(),
            required: false,
            parameter_type: "string".to_string(),
        }]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        run_git_cmd_with_output(repo_path, &["status", "--short", "--branch"])
            .map(|output| {
                let (branch, changes) = parse_status_output(&output);
                serde_json::json!({
                    "status": "ok",
                    "repo": repo_path,
                    "branch": branch,
                    "changes": changes,
                    "raw": output,
                })
            })
    }
}

fn parse_status_output(output: &str) -> (String, Vec<Value>) {
    let mut branch = String::from("unknown");
    let mut changes = Vec::new();

    for line in output.lines() {
        if line.starts_with("## ") {
            branch = line.strip_prefix("## ").unwrap_or(line).to_string();
        } else if line.len() >= 3 {
            let status = &line[..2].trim();
            let path = line[3..].to_string();
            changes.push(serde_json::json!({ "status": status, "path": path }));
        }
    }

    (branch, changes)
}

// ============================================================================
// GitDiffTool
// ============================================================================

pub struct GitDiffTool;

#[async_trait::async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str { "git_diff" }

    fn description(&self) -> &str {
        "Show changes between commits, commit and working tree, or a branch and working tree."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "repo_path".to_string(),
                description: "Path to the Git repository (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "target".to_string(),
                description: "Commit, branch, or range to diff (e.g. 'HEAD', 'main..HEAD', 'v1.0..v2.0'). Defaults to unstaged changes.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_path".to_string(),
                description: "Limit diff to a specific file".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "stat".to_string(),
                description: "Show diffstat summary instead of full diff (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let stat = params.get("stat").and_then(|v| v.as_bool()).unwrap_or(false);
        let target = params.get("target").and_then(|v| v.as_str());
        let file_path = params.get("file_path").and_then(|v| v.as_str());

        let mut args = vec!["diff".to_string()];
        if stat {
            args.push("--stat".to_string());
        } else {
            args.push("--unified=3".to_string());
        }

        if let Some(t) = target {
            args.push(t.to_string());
        }

        if let Some(fp) = file_path {
            args.push("--".to_string());
            args.push(fp.to_string());
        }

        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_git_cmd_with_output(repo_path, &str_args)
            .map(|output| {
                serde_json::json!({
                    "status": "ok",
                    "repo": repo_path,
                    "target": target.unwrap_or("working tree"),
                    "diff": output,
                })
            })
    }
}

// ============================================================================
// GitLogTool
// ============================================================================

pub struct GitLogTool;

#[async_trait::async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str { "git_log" }

    fn description(&self) -> &str {
        "Show commit history. Supports filtering by author, date range, and file path."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "repo_path".to_string(),
                description: "Path to the Git repository (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_count".to_string(),
                description: "Maximum number of commits to show (default: 20)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "author".to_string(),
                description: "Filter commits by author name or email".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_path".to_string(),
                description: "Show only commits that touched a specific file".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let max_count = params
            .get("max_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(20);

        let author = params.get("author").and_then(|v| v.as_str());
        let file_path = params.get("file_path").and_then(|v| v.as_str());

        let max_count_val = format!("--max-count={max_count}");
        let author_val = author.map(|a| format!("--author={a}"));

        let mut args: Vec<String> = vec![
            "log".to_string(),
            max_count_val,
            "--pretty=format:%h | %ai | %s | %an".to_string(),
        ];

        if let Some(a) = &author_val {
            args.push(a.clone());
        }

        let str_args: Vec<&str> = if let Some(fp) = file_path {
            let mut a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            a.push("--");
            a.push(fp);
            a
        } else {
            args.iter().map(|s| s.as_str()).collect()
        };

        run_git_cmd_with_output(repo_path, &str_args)
            .map(|output| {
                let commits: Vec<Value> = output
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|line| {
                        let parts: Vec<&str> = line.splitn(4, " | ").collect();
                        if parts.len() == 4 {
                            serde_json::json!({
                                "hash": parts[0].trim(),
                                "date": parts[1].trim(),
                                "subject": parts[2].trim(),
                                "author": parts[3].trim(),
                            })
                        } else {
                            serde_json::json!({ "raw": line })
                        }
                    })
                    .collect();

                serde_json::json!({
                    "status": "ok",
                    "repo": repo_path,
                    "commits": commits,
                    "count": commits.len(),
                })
            })
    }
}

// ============================================================================
// GitCloneTool
// ============================================================================

pub struct GitCloneTool;

#[async_trait::async_trait]
impl Tool for GitCloneTool {
    fn name(&self) -> &str { "git_clone" }

    fn description(&self) -> &str {
        "Clone a Git repository to a local directory."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "url".to_string(),
                description: "Repository URL (e.g. https://github.com/user/repo.git)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "target_dir".to_string(),
                description: "Directory to clone into (defaults to repo name)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "branch".to_string(),
                description: "Specific branch to clone (default: repository default branch)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "depth".to_string(),
                description: "Create a shallow clone with history truncated to N commits".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: url")?;

        let target_dir = params.get("target_dir").and_then(|v| v.as_str());
        let branch = params.get("branch").and_then(|v| v.as_str());
        let depth = params.get("depth").and_then(|v| v.as_u64());

        let mut args = vec!["clone".to_string()];

        if let Some(b) = branch {
            args.push("--branch".to_string());
            args.push(b.to_string());
        }
        if let Some(d) = depth {
            args.push(format!("--depth={d}"));
        }
        args.push(url.to_string());
        if let Some(dir) = target_dir {
            args.push(dir.to_string());
        }

        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_git_cmd_no_cwd(".", &str_args)
            .map(|output| {
                let dir_name = target_dir.unwrap_or_else(|| {
                    url.split('/').next_back().unwrap_or("repo")
                        .trim_end_matches(".git")
                });
                serde_json::json!({
                    "status": "ok",
                    "url": url,
                    "directory": dir_name,
                    "output": output,
                })
            })
    }
}

// ============================================================================
// GitCommitTool
// ============================================================================

pub struct GitCommitTool;

#[async_trait::async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str { "git_commit" }

    fn description(&self) -> &str {
        "Stage files and create a Git commit. Optionally pushes to remote after committing."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "repo_path".to_string(),
                description: "Path to the Git repository (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "message".to_string(),
                description: "Commit message (conventional commit format recommended: 'type: description')".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "files".to_string(),
                description: "Specific files to stage (JSON array). If omitted, stages all changes.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "push".to_string(),
                description: "Push to remote after committing (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: message")?;

        let do_push = params.get("push").and_then(|v| v.as_bool()).unwrap_or(false);

        // Stage files
        let stage_result = if let Some(files_str) = params.get("files").and_then(|v| v.as_str()) {
            // Parse JSON array of file paths
            match serde_json::from_str::<Vec<String>>(files_str) {
                Ok(files) => {
                    let mut output = String::new();
                    for file in &files {
                        match run_git_cmd_with_output(repo_path, &["add", file]) {
                            Ok(out) => output.push_str(&out),
                            Err(e) => return Err(format!("Failed to stage {file}: {e}")),
                        }
                    }
                    Ok(output)
                }
                Err(e) => Err(format!("Failed to parse files JSON array: {e}")),
            }
        } else {
            // Stage all changes
            run_git_cmd_with_output(repo_path, &["add", "-A"])
        };

        stage_result?;

        // Create commit
        let commit_output = run_git_cmd_with_output(repo_path, &["commit", "-m", message])?;

        // Get commit hash
        let hash = run_git_cmd_with_output(repo_path, &["log", "-1", "--pretty=format:%h"])
            .unwrap_or_default();

        // Push if requested
        let push_output = if do_push {
            run_git_cmd_with_output(repo_path, &["push"])
                .ok()
                .map(|out| format!("Push output:\n{out}"))
        } else {
            None
        };

        Ok(serde_json::json!({
            "status": "ok",
            "repo": repo_path,
            "commit_hash": hash.trim(),
            "message": message,
            "staged": commit_output,
            "pushed": push_output,
        }))
    }
}

// ============================================================================
// GitPushTool
// ============================================================================

pub struct GitPushTool;

#[async_trait::async_trait]
impl Tool for GitPushTool {
    fn name(&self) -> &str { "git_push" }

    fn description(&self) -> &str {
        "Push local commits to a remote repository. Supports specifying remote and branch."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "repo_path".to_string(),
                description: "Path to the Git repository (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "remote".to_string(),
                description: "Remote name (default: origin)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "branch".to_string(),
                description: "Branch name (default: current branch)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let remote = params.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
        let branch = params.get("branch").and_then(|v| v.as_str());

        let args: Vec<&str> = if let Some(b) = branch {
            vec!["push", remote, b]
        } else {
            vec!["push", "-u", remote]
        };

        run_git_cmd_with_output(repo_path, &args)
            .map(|output| {
                serde_json::json!({
                    "status": "ok",
                    "repo": repo_path,
                    "remote": remote,
                    "branch": branch.unwrap_or("current"),
                    "output": output,
                })
            })
    }
}

// ============================================================================
// Git helpers
// ============================================================================

/// Run a git command in the given working directory, return stdout.
fn run_git_cmd_with_output(work_dir: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(work_dir)
        .output()
        .map_err(|e| format!("Failed to execute git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git error: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a git command without changing cwd (for clone, etc).
fn run_git_cmd_no_cwd(work_dir: &str, args: &[&str]) -> Result<String, String> {
    run_git_cmd_with_output(work_dir, args)
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(GitStatusTool));
    registry.register(Box::new(GitDiffTool));
    registry.register(Box::new(GitLogTool));
    registry.register(Box::new(GitCloneTool));
    registry.register(Box::new(GitCommitTool));
    registry.register(Box::new(GitPushTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_output_parses_branch_and_changes() {
        let input = "## master\n M src/main.rs\n?? Cargo.lock\n";
        let (branch, changes) = parse_status_output(input);
        assert_eq!(branch, "master");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0]["status"], "M");
        assert_eq!(changes[0]["path"], "src/main.rs");
        assert_eq!(changes[1]["status"], "??");
        assert_eq!(changes[1]["path"], "Cargo.lock");
    }

    #[test]
    fn parse_status_output_empty() {
        let input = "## main";
        let (branch, changes) = parse_status_output(input);
        assert_eq!(branch, "main");
        assert!(changes.is_empty());
    }

    #[test]
    fn git_tools_register() {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry);
        let names: Vec<_> = registry.list_tools().iter().map(|t| t.name()).collect();
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"git_log"));
        assert!(names.contains(&"git_clone"));
        assert!(names.contains(&"git_commit"));
        assert!(names.contains(&"git_push"));
        assert_eq!(registry.list_tools().len(), 6);
    }
}
