//! Diff tool: compare text/code, generate unified diffs, patches, and side-by-side views.
//!
//! # Actions
//!
//! - **diff**: Compare two files or strings
//! - **unified**: Generate unified diff format
//! - **side_by_side**: Generate side-by-side comparison
//! - **stat**: Generate diffstat summary
//! - **patch**: Generate a patch file
//! - **apply**: Apply a patch to text

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

// ============================================================================
// DiffTool
// ============================================================================

pub struct DiffTool;

#[async_trait::async_trait]
impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff_tool"
    }

    fn description(&self) -> &str {
        "Compare text/code and generate diffs: unified diff format, side-by-side comparison, diffstat summary, patch generation, and patch application. Works with strings or files."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: diff (basic comparison), unified (unified diff), side_by_side (side-by-side view), stat (diffstat summary), patch (generate patch file), apply (apply patch to text)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "old".to_string(),
                description: "Old text or file path (for file comparisons, use @file:/path/to/file)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "new".to_string(),
                description: "New text or file path".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "old_path".to_string(),
                description: "Label for old file (default: 'a/file')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "new_path".to_string(),
                description: "Label for new file (default: 'b/file')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "context_lines".to_string(),
                description: "Number of context lines in unified diff (default: 3)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "ignore_whitespace".to_string(),
                description: "Ignore whitespace differences (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_width".to_string(),
                description: "Max width for side-by-side view (default: 120)".to_string(),
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

        let context_lines = params
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;
        let ignore_whitespace = params
            .get("ignore_whitespace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_width = params
            .get("max_width")
            .and_then(|v| v.as_u64())
            .unwrap_or(120) as usize;

        let old_path_label = params
            .get("old_path")
            .and_then(|v| v.as_str())
            .unwrap_or("a/file");
        let new_path_label = params
            .get("new_path")
            .and_then(|v| v.as_str())
            .unwrap_or("b/file");

        // Load old/new content
        let old_text = load_text(params.get("old").and_then(|v| v.as_str()))?;
        let new_text = load_text(params.get("new").and_then(|v| v.as_str()))?;

        let old_lines = split_lines(&old_text, ignore_whitespace);
        let new_lines = split_lines(&new_text, ignore_whitespace);

        match action {
            "diff" => {
                let changes = compute_diff(&old_lines, &new_lines);
                Ok(json!({
                    "status": "ok",
                    "action": "diff",
                    "identical": old_lines == new_lines,
                    "changes": changes,
                    "stats": compute_stats(&old_lines, &new_lines),
                }))
            }
            "unified" => {
                let unified = generate_unified_diff(
                    &old_lines,
                    &new_lines,
                    old_path_label,
                    new_path_label,
                    context_lines,
                );
                Ok(json!({
                    "status": "ok",
                    "action": "unified",
                    "diff": unified,
                    "stats": compute_stats(&old_lines, &new_lines),
                }))
            }
            "side_by_side" => {
                let side_by_side = generate_side_by_side(&old_lines, &new_lines, max_width);
                Ok(json!({
                    "status": "ok",
                    "action": "side_by_side",
                    "output": side_by_side,
                    "stats": compute_stats(&old_lines, &new_lines),
                }))
            }
            "stat" => {
                let stats = compute_stats(&old_lines, &new_lines);
                let diffstat =
                    generate_diffstat(&old_lines, &new_lines, old_path_label, new_path_label);
                Ok(json!({
                    "status": "ok",
                    "action": "stat",
                    "stats": stats,
                    "diffstat": diffstat,
                }))
            }
            "patch" => {
                let patch = generate_patch(
                    &old_lines,
                    &new_lines,
                    old_path_label,
                    new_path_label,
                    context_lines,
                );
                Ok(json!({
                    "status": "ok",
                    "action": "patch",
                    "patch": patch,
                    "stats": compute_stats(&old_lines, &new_lines),
                }))
            }
            "apply" => {
                let patch_text = params
                    .get("old")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing parameter: old (patch content)")?;
                let target = params
                    .get("new")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing parameter: new (target text)")?;
                let result = apply_patch(target, patch_text)?;
                Ok(json!({
                    "status": "ok",
                    "action": "apply",
                    "result": result,
                }))
            }
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: diff, unified, side_by_side, stat, patch, apply"),
            })),
        }
    }
}

// ============================================================================
// Diff Engine (LCS-based)
// ============================================================================

/// Compute diff using longest common subsequence algorithm.
fn compute_diff(old: &[String], new: &[String]) -> Vec<Value> {
    let m = old.len();
    let n = new.len();

    if m == 0 && n == 0 {
        return vec![];
    }

    // Build LCS table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find diff
    let (mut i, mut j) = (m, n);
    let mut reversed = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            reversed.push(json!({
                "type": "unchanged",
                "old_line": i,
                "new_line": j,
                "content": old[i - 1],
            }));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            reversed.push(json!({
                "type": "added",
                "new_line": j,
                "content": new[j - 1],
            }));
            j -= 1;
        } else {
            reversed.push(json!({
                "type": "removed",
                "old_line": i,
                "content": old[i - 1],
            }));
            i -= 1;
        }
    }

    reversed.reverse();
    reversed
}

// ============================================================================
// Unified Diff
// ============================================================================

fn generate_unified_diff(
    old: &[String],
    new: &[String],
    old_label: &str,
    new_label: &str,
    context: usize,
) -> String {
    if old == new {
        return String::new();
    }

    let changes = compute_diff(old, new);

    // Group changes into hunks
    let mut hunks = Vec::new();
    let mut current_hunk = Vec::new();
    let mut in_hunk = false;

    for change in &changes {
        let change_type = change["type"].as_str().unwrap_or("");
        if change_type != "unchanged" {
            // Found a change - add context before
            if !in_hunk {
                // Look back and add context lines
                let ctx_start = current_hunk.len().saturating_sub(context);
                let context_lines: Vec<Value> = current_hunk.drain(ctx_start..).collect();
                current_hunk.extend(context_lines);
                in_hunk = true;
            }
            current_hunk.push(change.clone());
        } else if in_hunk {
            current_hunk.push(change.clone());
            // Check if we've seen enough context after the change
            let trailing_unchanged = current_hunk
                .iter()
                .rev()
                .take_while(|c| c["type"].as_str() == Some("unchanged"))
                .count();
            if trailing_unchanged > context {
                // Remove excess context and close hunk
                let excess = trailing_unchanged - context;
                current_hunk.truncate(current_hunk.len() - excess);
                hunks.push(current_hunk.clone());
                current_hunk = Vec::new();
                in_hunk = false;
            }
        } else {
            current_hunk.push(change.clone());
            // Limit buffer size
            if current_hunk.len() > context * 2 {
                current_hunk.drain(..current_hunk.len() - context);
            }
        }
    }

    if in_hunk && !current_hunk.is_empty() {
        // Trim trailing context
        let trailing = current_hunk
            .iter()
            .rev()
            .take_while(|c| c["type"].as_str() == Some("unchanged"))
            .count();
        if trailing > context {
            current_hunk.truncate(current_hunk.len() - (trailing - context));
        }
        hunks.push(current_hunk);
    }

    if hunks.is_empty() {
        return String::new();
    }

    // Build unified diff output
    let mut output = String::new();
    output.push_str(&format!("--- {old_label}\n"));
    output.push_str(&format!("+++ {new_label}\n"));

    let mut old_line = 1usize;
    let mut new_line = 1usize;

    // Calculate hunk headers first to get line numbers right
    for hunk in &hunks {
        let mut hunk_old_start = 0usize;
        let mut hunk_new_start = 0usize;
        let mut hunk_old_count = 0usize;
        let mut hunk_new_count = 0usize;

        for change in hunk {
            match change["type"].as_str() {
                Some("unchanged") => {
                    if hunk_old_start == 0 {
                        hunk_old_start = old_line;
                        hunk_new_start = new_line;
                    }
                    hunk_old_count += 1;
                    hunk_new_count += 1;
                    old_line += 1;
                    new_line += 1;
                }
                Some("removed") => {
                    if hunk_old_start == 0 {
                        hunk_old_start = old_line;
                        hunk_new_start = new_line;
                    }
                    hunk_old_count += 1;
                    old_line += 1;
                }
                Some("added") => {
                    if hunk_old_start == 0 {
                        hunk_old_start = old_line;
                        hunk_new_start = new_line;
                    }
                    hunk_new_count += 1;
                    new_line += 1;
                }
                _ => {}
            }
        }

        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk_old_start, hunk_old_count, hunk_new_start, hunk_new_count
        ));

        for change in hunk {
            match change["type"].as_str() {
                Some("unchanged") => {
                    output.push_str(&format!(" {}\n", change["content"]));
                }
                Some("removed") => {
                    output.push_str(&format!("-{}\n", change["content"]));
                }
                Some("added") => {
                    output.push_str(&format!("+{}\n", change["content"]));
                }
                _ => {}
            }
        }
    }

    output
}

// ============================================================================
// Side-by-Side View
// ============================================================================

fn generate_side_by_side(old: &[String], new: &[String], max_width: usize) -> String {
    let changes = compute_diff(old, new);
    let half_width = max_width / 2 - 2;

    let mut output = String::new();

    // Header
    let header = format!(
        "{:<width$} | {:<width$}",
        "< old",
        "> new",
        width = half_width
    );
    output.push_str(&header);
    output.push_str(&format!("\n{}\n", "-".repeat(max_width)));

    for change in &changes {
        match change["type"].as_str() {
            Some("unchanged") => {
                let line = format!(
                    "{:<width$} | {:<width$}",
                    change["content"],
                    change["content"],
                    width = half_width
                );
                output.push_str(&line);
                output.push('\n');
            }
            Some("removed") => {
                let line = format!(
                    "-{:<width$} | {}",
                    change["content"],
                    "",
                    width = half_width - 1
                );
                output.push_str(&line);
                output.push('\n');
            }
            Some("added") => {
                let line = format!(
                    "{:<width$} | +{:<width$}",
                    "",
                    change["content"],
                    width = half_width - 1
                );
                output.push_str(&line);
                output.push('\n');
            }
            _ => {}
        }
    }

    output
}

// ============================================================================
// Stats and Diffstat
// ============================================================================

fn compute_stats(old: &[String], new: &[String]) -> Value {
    let changes = compute_diff(old, new);
    let additions = changes
        .iter()
        .filter(|c| c["type"].as_str() == Some("added"))
        .count();
    let deletions = changes
        .iter()
        .filter(|c| c["type"].as_str() == Some("removed"))
        .count();
    let unchanged = changes
        .iter()
        .filter(|c| c["type"].as_str() == Some("unchanged"))
        .count();

    json!({
        "additions": additions,
        "deletions": deletions,
        "unchanged": unchanged,
        "total_changes": additions + deletions,
        "old_lines": old.len(),
        "new_lines": new.len(),
    })
}

fn generate_diffstat(old: &[String], new: &[String], old_label: &str, new_label: &str) -> String {
    let changes = compute_diff(old, new);
    let additions = changes
        .iter()
        .filter(|c| c["type"].as_str() == Some("added"))
        .count();
    let deletions = changes
        .iter()
        .filter(|c| c["type"].as_str() == Some("removed"))
        .count();
    let total = additions + deletions;

    let max_bar = 50;
    let bar_add = (additions * max_bar)
        .checked_div(total)
        .unwrap_or(0)
        .min(max_bar);
    let bar_del = (deletions * max_bar)
        .checked_div(total)
        .unwrap_or(0)
        .min(max_bar);

    let bar = format!("{}{}", "+".repeat(bar_add), "-".repeat(bar_del));

    format!(
        " {} -> {} | {} {}{}{}\n {} file{} changed, {} insertion{}(+), {} deletion{}(-)",
        old_label,
        new_label,
        total,
        bar,
        if total > 0 { " " } else { "" },
        "",
        1,
        if total > 0 { "s" } else { "" },
        additions,
        if additions != 1 { "s" } else { "" },
        deletions,
        if deletions != 1 { "s" } else { "" },
    )
}

// ============================================================================
// Patch Generation
// ============================================================================

fn generate_patch(
    old: &[String],
    new: &[String],
    old_label: &str,
    new_label: &str,
    context: usize,
) -> String {
    let unified = generate_unified_diff(old, new, old_label, new_label, context);
    if unified.is_empty() {
        return "No differences found.".to_string();
    }
    unified
}

// ============================================================================
// Patch Application
// ============================================================================

fn apply_patch(target: &str, patch: &str) -> Result<String, String> {
    let target_lines: Vec<&str> = target.lines().collect();
    let patch_lines: Vec<&str> = patch.lines().collect();

    let mut result = target_lines
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut offset: i32 = 0;

    // Parse hunks from patch
    let mut i = 0;
    while i < patch_lines.len() {
        let line = patch_lines[i];

        if line.starts_with("@@") {
            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
            let parts: Vec<&str> = line.split("@@").collect();
            if parts.len() >= 2 {
                let old_spec = parts[1].trim().split(' ').next().unwrap_or("-0,0");
                let old_start: usize = old_spec
                    .trim_start_matches('-')
                    .split(',')
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let mut add_lines = Vec::new();
                let mut remove_count = 0usize;
                i += 1;

                while i < patch_lines.len() && !patch_lines[i].starts_with("@@") {
                    let hunk_line = patch_lines[i];
                    if let Some(stripped) = hunk_line.strip_prefix('+') {
                        add_lines.push(stripped.to_string());
                    } else if hunk_line.starts_with('-') {
                        remove_count += 1;
                    } else if hunk_line.starts_with(' ') || hunk_line.is_empty() {
                        // Context line
                    }
                    i += 1;
                }

                // Apply the hunk
                let adjusted_start = (old_start as i32 + offset - 1).max(0) as usize;
                if adjusted_start < result.len() {
                    // Remove old lines
                    let end = (adjusted_start + remove_count).min(result.len());
                    result.drain(adjusted_start..end);

                    // Insert new lines
                    for (idx, line) in add_lines.iter().enumerate() {
                        result.insert(adjusted_start + idx, line.clone());
                    }

                    offset += add_lines.len() as i32 - remove_count as i32;
                }
            }
        } else {
            i += 1;
        }
    }

    Ok(result.join("\n"))
}

// ============================================================================
// Utility Functions
// ============================================================================

fn load_text(param: Option<&str>) -> Result<String, String> {
    match param {
        Some(s) if s.starts_with("@file:") => {
            let path = &s[6..];
            fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
        }
        Some(s) => Ok(s.to_string()),
        None => Ok(String::new()),
    }
}

fn split_lines(text: &str, ignore_whitespace: bool) -> Vec<String> {
    let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if ignore_whitespace {
        lines
            .into_iter()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    } else {
        lines
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DiffTool));
}
