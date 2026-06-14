//! Git workflow enhancement tool: branch management, changelog generation, release automation.
//!
//! # Actions
//!
//! - **branch**: List, create, delete, switch branches
//! - **changelog**: Generate changelog from commit history
//! - **release**: Create a release (tag + changelog)
//! - **merge**: Merge branches with conflict detection
//! - **pr_description**: Generate PR description from diff
//! - **blame**: Show who last modified each line of a file
//! - **contributors**: List contributors with commit counts

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;

// ============================================================================
// Regex Patterns
// ============================================================================

static RE_CONVENTIONAL_COMMIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+?\))?:\s*(.+)$",
    )
    .expect("valid regex")
});

static RE_COMMIT_WITH_AUTHOR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([a-f0-9]+)\s*<([^>]+)>\s*(.+)$").expect("valid regex"));

// ============================================================================
// GitWorkflowTool
// ============================================================================

pub struct GitWorkflowTool;

#[async_trait::async_trait]
impl Tool for GitWorkflowTool {
    fn name(&self) -> &str {
        "git_workflow"
    }

    fn description(&self) -> &str {
        "Enhanced Git workflow tools: branch management (list/create/delete/switch), changelog generation from conventional commits, release automation (tag + notes), merge with conflict detection, PR description generation, file blame, and contributor statistics."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description:
                    "Action: branch, changelog, release, merge, pr_description, blame, contributors"
                        .to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "repo_path".to_string(),
                description: "Path to the Git repository (default: current directory)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "branch_name".to_string(),
                description: "Branch name for branch/create/delete/switch/merge actions"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "source_branch".to_string(),
                description: "Source branch for merge action".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "target_branch".to_string(),
                description: "Target branch for merge action".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "version".to_string(),
                description: "Version tag for release action (e.g. 'v1.0.0')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "since".to_string(),
                description: "Start point for changelog (tag/commit/branch, default: last tag)"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_path".to_string(),
                description: "File path for blame action".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: markdown, json (default: markdown)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "force".to_string(),
                description: "Force operation (for delete branch, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let repo_path = params
            .get("repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");

        match action {
            "branch" => handle_branch(params, repo_path),
            "changelog" => handle_changelog(params, repo_path, format),
            "release" => handle_release(params, repo_path, format),
            "merge" => handle_merge(params, repo_path),
            "pr_description" => handle_pr_description(params, repo_path, format),
            "blame" => handle_blame(params, repo_path, format),
            "contributors" => handle_contributors(repo_path, format),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: branch, changelog, release, merge, pr_description, blame, contributors"),
            })),
        }
    }
}

// ============================================================================
// Git Helper Functions
// ============================================================================

fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("Failed to execute git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git error: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn current_branch(repo: &str) -> Result<String, String> {
    run_git(repo, &["branch", "--show-current"])
}

// ============================================================================
// Branch Management
// ============================================================================

fn handle_branch(params: &HashMap<String, Value>, repo: &str) -> Result<Value, String> {
    let branch_name = params
        .get("branch_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let force = params
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // If no branch name, list all branches
    if branch_name.is_empty() {
        let branches_raw = run_git(repo, &["branch", "-vva"])?;
        let current = current_branch(repo).unwrap_or_default();

        let mut branches = Vec::new();
        for line in branches_raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let is_current = trimmed.starts_with("* ");
            let name = if is_current {
                trimmed[2..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                trimmed.split_whitespace().next().unwrap_or("").to_string()
            };

            branches.push(json!({
                "name": name,
                "current": is_current,
                "remote": name.starts_with("remotes/"),
            }));
        }

        return Ok(json!({
            "status": "ok",
            "action": "branch_list",
            "current": current,
            "branches": branches,
            "count": branches.len(),
        }));
    }

    // Determine sub-action based on other parameters
    let sub_action = if params.contains_key("source_branch") {
        "switch"
    } else if params.contains_key("target_branch") {
        "create"
    } else {
        // Check if branch exists
        match run_git(repo, &["rev-parse", "--verify", branch_name]) {
            Ok(_) => "delete",  // Branch exists, delete it
            Err(_) => "create", // Branch doesn't exist, create it
        }
    };

    match sub_action {
        "create" => {
            let base = params
                .get("target_branch")
                .and_then(|v| v.as_str())
                .unwrap_or("HEAD");
            run_git(repo, &["branch", branch_name, base])?;
            Ok(json!({
                "status": "ok",
                "action": "branch_create",
                "branch": branch_name,
                "based_on": base,
                "message": format!("Created branch '{branch_name}' from {base}"),
            }))
        }
        "switch" => {
            let source = params
                .get("source_branch")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if source.is_empty() {
                run_git(repo, &["checkout", branch_name])?;
            } else {
                run_git(repo, &["checkout", "-b", branch_name, source])?;
            }
            Ok(json!({
                "status": "ok",
                "action": "branch_switch",
                "branch": branch_name,
                "message": format!("Switched to branch '{branch_name}'"),
            }))
        }
        "delete" => {
            let flag = if force { "-D" } else { "-d" };
            run_git(repo, &[flag, branch_name])?;
            Ok(json!({
                "status": "ok",
                "action": "branch_delete",
                "branch": branch_name,
                "force": force,
                "message": format!("Deleted branch '{branch_name}'"),
            }))
        }
        _ => Err(format!("Unknown branch sub-action: {sub_action}")),
    }
}

// ============================================================================
// Changelog Generation
// ============================================================================

fn handle_changelog(
    params: &HashMap<String, Value>,
    repo: &str,
    format: &str,
) -> Result<Value, String> {
    let since = params.get("since").and_then(|v| v.as_str()).unwrap_or("");

    // Get the range
    let range = if since.is_empty() {
        // Try to find the last tag
        match run_git(repo, &["describe", "--tags", "--abbrev=0"]) {
            Ok(last_tag) => format!("{last_tag}..HEAD"),
            Err(_) => "HEAD".to_string(),
        }
    } else {
        format!("{since}..HEAD")
    };

    // Get commits with type prefix
    let format_str = "%H%n%s%n%an%n%ai";
    let commits_raw = run_git(repo, &["log", &range, "--pretty=format:", format_str])?;

    // Parse commits
    let mut lines = commits_raw.lines();
    let mut commits = Vec::new();
    while let (Some(hash), Some(msg), Some(author), Some(date)) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    {
        commits.push((
            hash.to_string(),
            msg.to_string(),
            author.to_string(),
            date.to_string(),
        ));
    }

    // Categorize by conventional commit type
    let mut categorized: HashMap<String, Vec<(String, String, String, String)>> = HashMap::new();
    let mut uncategorized = Vec::new();

    for (hash, msg, author, date) in &commits {
        if let Some(cap) = RE_CONVENTIONAL_COMMIT.captures(msg) {
            let commit_type = cap
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("other")
                .to_string();
            let scope = cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            let description = cap.get(3).map(|m| m.as_str()).unwrap_or(msg).to_string();

            categorized.entry(commit_type).or_default().push((
                hash.clone(),
                msg.clone(),
                description,
                scope,
            ));
        } else {
            uncategorized.push((hash.clone(), msg.clone(), author.clone(), date.clone()));
        }
    }

    // Build changelog
    let type_labels = [
        ("feat", "✨ Features"),
        ("fix", "🐛 Bug Fixes"),
        ("perf", "⚡ Performance"),
        ("refactor", "♻️ Code Refactoring"),
        ("docs", "📝 Documentation"),
        ("style", "💄 Styles"),
        ("test", "✅ Tests"),
        ("build", "🔨 Build System"),
        ("ci", "👷 CI/CD"),
        ("chore", "🔧 Chores"),
        ("revert", "⏪ Reverts"),
    ];

    let mut changelog_sections = Vec::new();
    for (type_key, label) in &type_labels {
        if let Some(entries) = categorized.get(*type_key) {
            let mut items = Vec::new();
            for (hash, _full, desc, scope) in entries {
                let short_hash = &hash[..8];
                let item = if scope.is_empty() {
                    format!("- {desc} ({short_hash})")
                } else {
                    format!("- **{scope}**: {desc} ({short_hash})")
                };
                items.push(item);
            }
            changelog_sections.push(format!("### {label}\n\n{}", items.join("\n")));
        }
    }

    if !uncategorized.is_empty() {
        let mut items = Vec::new();
        for (hash, msg, _author, _date) in &uncategorized {
            let short_hash = &hash[..8];
            items.push(format!("- {msg} ({short_hash})"));
        }
        changelog_sections.push(format!("### Other Changes\n\n{}", items.join("\n")));
    }

    let changelog = if changelog_sections.is_empty() {
        "No commits found.".to_string()
    } else {
        changelog_sections.join("\n\n")
    };

    if format == "json" {
        let mut json_sections = serde_json::Map::new();
        for (type_key, _label) in &type_labels {
            if let Some(entries) = categorized.get(*type_key) {
                let items: Vec<Value> = entries
                    .iter()
                    .map(|(hash, _full, desc, scope)| {
                        json!({
                            "hash": &hash[..8],
                            "full_hash": hash,
                            "description": desc,
                            "scope": if scope.is_empty() { Value::Null } else { json!(scope) },
                        })
                    })
                    .collect();
                json_sections.insert(type_key.to_string(), json!(items));
            }
        }

        Ok(json!({
            "status": "ok",
            "action": "changelog",
            "range": range,
            "total_commits": commits.len(),
            "sections": json_sections,
        }))
    } else {
        Ok(json!({
            "status": "ok",
            "action": "changelog",
            "range": range,
            "total_commits": commits.len(),
            "changelog": changelog,
        }))
    }
}

// ============================================================================
// Release Automation
// ============================================================================

fn handle_release(
    params: &HashMap<String, Value>,
    repo: &str,
    _format: &str,
) -> Result<Value, String> {
    let version = params
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: version for release action")?;

    let tag = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };

    // Generate changelog since last tag
    let last_tag = run_git(repo, &["describe", "--tags", "--abbrev=0"]).ok();
    let changelog_since = if let Some(ref lt) = last_tag {
        format!("{lt}..HEAD")
    } else {
        "HEAD".to_string()
    };

    let commits_raw = run_git(repo, &["log", &changelog_since, "--pretty=format:%s"])?;

    let mut features = Vec::new();
    let mut fixes = Vec::new();
    let mut others = Vec::new();

    for line in commits_raw.lines() {
        if let Some(cap) = RE_CONVENTIONAL_COMMIT.captures(line) {
            let commit_type = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let desc = cap.get(3).map(|m| m.as_str()).unwrap_or(line);
            match commit_type {
                "feat" => features.push(desc.to_string()),
                "fix" => fixes.push(desc.to_string()),
                _ => others.push(format!("{commit_type}: {desc}")),
            }
        } else {
            others.push(line.to_string());
        }
    }

    // Create release notes
    let mut release_notes = String::new();
    release_notes.push_str(&format!("# Release {tag}\n\n"));

    if !features.is_empty() {
        release_notes.push_str("## ✨ New Features\n\n");
        for f in &features {
            release_notes.push_str(&format!("- {f}\n"));
        }
        release_notes.push('\n');
    }

    if !fixes.is_empty() {
        release_notes.push_str("## 🐛 Bug Fixes\n\n");
        for f in &fixes {
            release_notes.push_str(&format!("- {f}\n"));
        }
        release_notes.push('\n');
    }

    if !others.is_empty() {
        release_notes.push_str("## Other Changes\n\n");
        for o in &others {
            release_notes.push_str(&format!("- {o}\n"));
        }
        release_notes.push('\n');
    }

    // Create tag
    run_git(repo, &["tag", "-a", &tag, "-m", &format!("Release {tag}")])?;

    let push_tag = run_git(repo, &["push", "origin", &tag]).ok();

    Ok(json!({
        "status": "ok",
        "action": "release",
        "tag": tag,
        "version": version,
        "last_tag": last_tag.unwrap_or_else(|| "(none)".to_string()),
        "summary": {
            "features": features.len(),
            "fixes": fixes.len(),
            "other_changes": others.len(),
        },
        "release_notes": release_notes,
        "tag_pushed": push_tag.is_some(),
    }))
}

// ============================================================================
// Merge
// ============================================================================

fn handle_merge(params: &HashMap<String, Value>, repo: &str) -> Result<Value, String> {
    let source = params
        .get("source_branch")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: source_branch for merge action")?;

    let target = params.get("target_branch").and_then(|v| v.as_str());

    // Switch to target branch first
    let target_branch = target.unwrap_or("main");
    run_git(repo, &["checkout", target_branch])?;

    // Try merge
    let merge_result = run_git(repo, &["merge", "--no-edit", source]);

    match merge_result {
        Ok(output) => Ok(json!({
            "status": "ok",
            "action": "merge",
            "source": source,
            "target": target_branch,
            "success": true,
            "conflicts": false,
            "output": output,
        })),
        Err(e) => {
            // Check if it's a conflict
            let is_conflict = e.contains("CONFLICT");
            let status_output = run_git(repo, &["status", "--short"]).unwrap_or_default();

            // Get conflicting files
            let conflicting_files: Vec<String> = status_output
                .lines()
                .filter(|l| l.starts_with("UU") || l.starts_with("AA") || l.starts_with("DU"))
                .map(|l| l[3..].trim().to_string())
                .collect();

            Ok(json!({
                "status": "warning",
                "action": "merge",
                "source": source,
                "target": target_branch,
                "success": false,
                "conflicts": is_conflict,
                "conflicting_files": conflicting_files,
                "error": e,
                "hint": "Resolve conflicts and run: git add <files> && git commit",
            }))
        }
    }
}

// ============================================================================
// PR Description
// ============================================================================

fn handle_pr_description(
    params: &HashMap<String, Value>,
    repo: &str,
    _format: &str,
) -> Result<Value, String> {
    let source = params
        .get("source_branch")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: source_branch for pr_description")?;

    let target = params
        .get("target_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("main");

    // Get diff stats
    let diff_stat = run_git(repo, &["diff", "--stat", &format!("{target}..{source}")])
        .ok()
        .unwrap_or_default();

    // Get commits
    let commits_raw = run_git(
        repo,
        &[
            "log",
            &format!("{target}..{source}"),
            "--pretty=format:%H %s",
        ],
    )?;

    let mut commits = Vec::new();
    for line in commits_raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(cap) = RE_COMMIT_WITH_AUTHOR.captures(line) {
            let hash = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            commits.push(format!(
                "- {} {} ({})",
                &hash[..8.min(hash.len())],
                cap.get(3).map(|m| m.as_str()).unwrap_or(""),
                cap.get(2).map(|m| m.as_str()).unwrap_or("unknown"),
            ));
        } else {
            commits.push(format!("- {line}"));
        }
    }

    // Get changed files
    let changed_files_raw = run_git(
        repo,
        &["diff", "--name-only", &format!("{target}..{source}")],
    )
    .ok();
    let changed_files: Vec<String> = changed_files_raw
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build PR description
    let pr_description = format!(
        r#"## Summary

<!-- Describe the purpose of this PR -->

## Changes

{diff_stat}

### Commits ({})

{}

### Files Changed ({})

{}

## Testing

<!-- Describe how to test these changes -->

## Checklist

- [ ] Code follows project conventions
- [ ] Tests added/updated
- [ ] Documentation updated
"#,
        commits.len(),
        commits.join("\n"),
        changed_files.len(),
        changed_files
            .iter()
            .map(|f| format!("- `{f}`"))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    Ok(json!({
        "status": "ok",
        "action": "pr_description",
        "source": source,
        "target": target,
        "commits": commits.len(),
        "files_changed": changed_files.len(),
        "pr_description": pr_description,
    }))
}

// ============================================================================
// Blame
// ============================================================================

fn handle_blame(
    params: &HashMap<String, Value>,
    repo: &str,
    format: &str,
) -> Result<Value, String> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: file_path for blame action")?;

    let blame_raw = run_git(repo, &["blame", "--line-porcelain", file_path])?;

    let mut blame_entries = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;

    for line in blame_raw.lines() {
        if let Some(hash) = line.strip_prefix("author ") {
            if let Some(entry) = current.take() {
                blame_entries.push(entry);
            }
            current = Some(HashMap::from([("author".to_string(), hash.to_string())]));
        } else if let Some(time) = line.strip_prefix("author-time ") {
            if let Some(ref mut entry) = current {
                entry.insert("time".to_string(), time.to_string());
            }
        } else if let Some(_content) = line.strip_prefix("\t") {
            // This is the actual line content
        }
    }

    if let Some(entry) = current {
        blame_entries.push(entry);
    }

    // Get summary: who contributed most
    let mut author_counts: HashMap<String, usize> = HashMap::new();
    for entry in &blame_entries {
        if let Some(author) = entry.get("author") {
            *author_counts.entry(author.clone()).or_insert(0) += 1;
        }
    }

    let mut contributors: Vec<(String, usize)> = author_counts.into_iter().collect();
    contributors.sort_by_key(|b| std::cmp::Reverse(b.1));

    if format == "json" {
        Ok(json!({
            "status": "ok",
            "action": "blame",
            "file": file_path,
            "total_lines": blame_entries.len(),
            "contributors": contributors.iter().map(|(a, c)| json!({"author": a, "lines": c})).collect::<Vec<_>>(),
        }))
    } else {
        let summary: String = contributors
            .iter()
            .map(|(author, count)| {
                let pct = (*count as f64 / blame_entries.len() as f64 * 100.0).round() as u64;
                format!("- {author}: {count} lines ({pct}%)")
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(json!({
            "status": "ok",
            "action": "blame",
            "file": file_path,
            "total_lines": blame_entries.len(),
            "summary": summary,
            "contributors": contributors.iter().map(|(a, c)| json!({"author": a, "lines": c})).collect::<Vec<_>>(),
        }))
    }
}

// ============================================================================
// Contributors
// ============================================================================

fn handle_contributors(repo: &str, format: &str) -> Result<Value, String> {
    // Get all commits with author info
    let shortlog = run_git(repo, &["shortlog", "-sne", "HEAD"])?;

    let mut contributors = Vec::new();
    for line in shortlog.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Format: "  123\tAuthor Name <email>"
        if let Some((count, rest)) = trimmed.split_once('\t') {
            let count = count.trim().parse::<usize>().unwrap_or(0);
            let name = rest.split('<').next().unwrap_or(rest).trim();
            let email = rest
                .split('<')
                .nth(1)
                .and_then(|e| e.strip_suffix('>'))
                .unwrap_or("");

            contributors.push(json!({
                "name": name,
                "email": email,
                "commits": count,
            }));
        }
    }

    // Sort by commit count
    contributors.sort_by(|a, b| {
        b["commits"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["commits"].as_u64().unwrap_or(0))
    });

    let total_commits: u64 = contributors
        .iter()
        .map(|c| c["commits"].as_u64().unwrap_or(0))
        .sum();

    if format == "json" {
        Ok(json!({
            "status": "ok",
            "action": "contributors",
            "total_contributors": contributors.len(),
            "total_commits": total_commits,
            "contributors": contributors,
        }))
    } else {
        let summary: String = contributors
            .iter()
            .map(|c| {
                let name = c["name"].as_str().unwrap_or("");
                let commits = c["commits"].as_u64().unwrap_or(0);
                let pct = (commits as f64 / total_commits.max(1) as f64 * 100.0).round() as u64;
                format!("- {name}: {commits} commits ({pct}%)")
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(json!({
            "status": "ok",
            "action": "contributors",
            "total_contributors": contributors.len(),
            "total_commits": total_commits,
            "summary": summary,
            "contributors": contributors,
        }))
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(GitWorkflowTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_commit_regex() {
        assert!(RE_CONVENTIONAL_COMMIT.is_match("feat: add new feature"));
        assert!(RE_CONVENTIONAL_COMMIT.is_match("fix(core): resolve bug"));
        assert!(RE_CONVENTIONAL_COMMIT.is_match("docs(readme): update docs"));
        assert!(!RE_CONVENTIONAL_COMMIT.is_match("random commit message"));
    }

    #[test]
    fn commit_with_author_regex() {
        assert!(RE_COMMIT_WITH_AUTHOR.is_match("abc1234 <john@example.com> feat: add new feature"));
    }
}
