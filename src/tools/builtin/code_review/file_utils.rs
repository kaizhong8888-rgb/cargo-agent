//! File collection and git diff support.

use serde_json::Value;
use std::path::Path;

/// Recursively collect .rs files from a directory.
fn collect_rust_files(dir: &Path, files: &mut Vec<String>, recursive: bool, depth: usize) -> Result<(), String> {
    if depth > 20 { return Ok(()); }
    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" { continue; }
            if recursive {
                collect_rust_files(&path, files, true, depth + 1)?;
            }
        } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

/// Run `git diff` to get a list of changed files in the working tree.
pub(super) fn get_git_diff_files() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only"])
        .output()
        .map_err(|e| format!("Failed to run git diff: {e}"))?;

    let mut files: Vec<String> = Vec::new();
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if !line.is_empty() { files.push(line.to_string()); }
        }
    }

    // Also get staged changes
    let staged_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()
        .map_err(|e| format!("Failed to run git diff --cached: {e}"))?;

    if staged_output.status.success() {
        let stdout = String::from_utf8_lossy(&staged_output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if !line.is_empty() && !files.contains(&line.to_string()) {
                files.push(line.to_string());
            }
        }
    }

    files.retain(|f| f.ends_with(".rs"));
    files.sort();
    files.dedup();
    Ok(files)
}

/// Discover Rust files from path, applying filters.
pub(super) fn collect_files(
    path: &str,
    max_results: usize,
    git_diff: bool,
    recursive: bool,
) -> Result<Vec<String>, String> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(format!("Path does not exist: {path}"));
    }

    let mut files: Vec<String> = Vec::new();
    if file_path.is_file() {
        if !path.ends_with(".rs") {
            return Err(format!("Not a Rust file: {path}"));
        }
        files.push(path.to_string());
    } else if file_path.is_dir() {
        collect_rust_files(file_path, &mut files, recursive, 0)?;
        if files.is_empty() {
            return Err(format!("No Rust files found in: {path}"));
        }
    }

    if files.len() > max_results {
        files.truncate(max_results);
    }

    // Filter to only git-changed files if git_diff is enabled
    if git_diff {
        let changed = get_git_diff_files()?;
        if changed.is_empty() { return Ok(vec![]); }
        let original_count = files.len();
        files.retain(|f| {
            let normalized = f.strip_prefix("./").unwrap_or(f.as_str());
            changed.iter().any(|c| c == normalized || f.ends_with(c))
        });
        let _filtered_total = original_count - files.len();
        if files.is_empty() { return Ok(vec![]); }
    }

    Ok(files)
}

/// Build empty result for early-exit cases (no files, git diff only, etc.)
pub(super) fn build_empty_result(git_diff: bool) -> Value {
    if git_diff {
        serde_json::json!({
            "status": "ok",
            "message": "No git changes detected. No files to review.",
            "git_diff": true,
            "summary": { "files": 0, "total_issues": 0, "errors": 0, "warnings": 0, "info": 0 },
        })
    } else {
        serde_json::json!({
            "status": "ok",
            "message": "No files to review.",
            "summary": { "files": 0, "total_issues": 0, "errors": 0, "warnings": 0, "info": 0 },
        })
    }
}
