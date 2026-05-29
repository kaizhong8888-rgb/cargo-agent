//! Filesystem exploration tools: list directories and grep/search across files.
//!
//! These tools allow the agent to navigate and search the project filesystem,
//! enabling autonomous codebase exploration without requiring the user to
//! specify exact file paths.
//!
//! # Enhancements
//!
//! - **File preview**: ListDirTool can show first N lines of text files
//! - **.gitignore support**: Auto-skip git-ignored files when listing
//! - **Fixed recursive paths**: Shows full relative paths in recursive mode
//! - **Files-only mode**: GrepTool can return just matching file names

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Options shared between grep_dir and grep_dir_files_only.
struct GrepOptions {
    re: Regex,
    file_re: Option<Regex>,
    context_lines: usize,
    max_results: usize,
    max_file_size: usize,
    results: Vec<Value>,
    total_matches: usize,
}

// ============================================================================
// .gitignore pattern parser
// ============================================================================

/// A simple .gitignore-style pattern matcher.
#[derive(Debug)]
struct GitIgnoreMatcher {
    patterns: Vec<GitIgnorePattern>,
}

#[derive(Debug)]
struct GitIgnorePattern {
    /// The raw pattern string
    #[allow(dead_code)]
    raw: String,
    /// Compiled regex
    re: Regex,
    /// If true, pattern matches directories only (ends with /)
    dir_only: bool,
    /// If true, pattern is negated (starts with !)
    negated: bool,
}

impl GitIgnoreMatcher {
    /// Load .gitignore patterns from a directory, including parent directories.
    fn load_from_dir(dir: &Path) -> Self {
        let mut patterns = Vec::new();

        // Walk up from `dir` to root, collecting .gitignore files
        let mut current = Some(dir);
        while let Some(path) = current {
            let gitignore_path = path.join(".gitignore");
            if gitignore_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                    let parent = path;
                    for line in content.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some(p) = GitIgnorePattern::parse(line, parent) {
                            patterns.push(p);
                        }
                    }
                }
            }
            current = path.parent();
        }

        GitIgnoreMatcher { patterns }
    }

    /// Check if a path (relative to the repo root) is ignored.
    fn is_ignored(&self, relative_path: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for p in &self.patterns {
            if p.dir_only && !is_dir {
                continue;
            }
            if p.re.is_match(relative_path) || p.re.is_match(relative_path.trim_end_matches('/')) {
                ignored = !p.negated;
            }
        }
        ignored
    }
}

impl GitIgnorePattern {
    #[allow(unused_variables)]
    fn parse(line: &str, base_dir: &Path) -> Option<Self> {
        let mut raw = line.to_string();

        let negated = raw.starts_with('!');
        if negated {
            raw = raw[1..].to_string();
        }

        let dir_only = raw.ends_with('/');
        if dir_only {
            raw = raw.trim_end_matches('/').to_string();
        }

        // Convert .gitignore glob to regex
        let mut regex_str = String::new();
        let mut chars = raw.chars().peekable();

        let anchored = !raw.starts_with('/'); // if starts with /, it's relative to repo root

        if !anchored {
            regex_str.push_str("(^|/)");
            let _ = chars.next(); // consume leading /
        } else if raw.contains('/') {
            // Pattern contains a slash, so it's matched against full path
            regex_str.push_str("(^|/)");
        } else {
            // Otherwise, match basename
            regex_str.push_str("(^|/)");
        }

        // If pattern ends with /** or is a directory pattern, match everything inside
        let is_dir_wild = raw.ends_with("/**");

        while let Some(c) = chars.next() {
            match c {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        if chars.peek() == Some(&'/') {
                            chars.next(); // consume /
                            regex_str.push_str("(.*/)?");
                        } else {
                            regex_str.push_str(".*");
                        }
                    } else {
                        regex_str.push_str("[^/]*");
                    }
                }
                '?' => regex_str.push_str("[^/]"),
                '.' => regex_str.push_str("\\."),
                '[' => {
                    // Character class - pass through
                    regex_str.push('[');
                    for cc in chars.by_ref() {
                        regex_str.push(cc);
                        if cc == ']' {
                            break;
                        }
                    }
                }
                other => regex_str.push(other),
            }
        }

        regex_str.push_str("(/.*)?$");

        let re = Regex::new(&regex_str).ok()?;
        Some(GitIgnorePattern {
            raw: line.to_string(),
            re,
            dir_only,
            negated,
        })
    }
}

// ============================================================================
// ListDirTool
// ============================================================================

/// List the contents of a directory, with file type, size, and modification time.
///
/// Supports:
/// - Recursive listing with depth control
/// - File content preview (first N lines of text files)
/// - .gitignore-aware filtering
pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the contents of a directory with file type, size, and modification time. Supports recursive listing, file preview, and .gitignore-aware filtering."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the directory to list (default: current directory)"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Whether to list recursively (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_depth".to_string(),
                description: "Maximum recursion depth when recursive=true (default: 3)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "max_preview_lines".to_string(),
                description: "Number of lines to preview for text files (default: 0, no preview)"
                    .to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "use_gitignore".to_string(),
                description: "Whether to respect .gitignore rules when listing (default: false)"
                    .to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_depth = params
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;
        let max_preview_lines = params
            .get("max_preview_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let use_gitignore = params
            .get("use_gitignore")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dir_path = Path::new(path);

        if !dir_path.exists() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        if !dir_path.is_dir() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Path is not a directory: {path}"),
            }));
        }

        // Load .gitignore rules if requested
        let gitignore = if use_gitignore {
            Some(GitIgnoreMatcher::load_from_dir(dir_path))
        } else {
            None
        };

        if recursive {
            let entries = list_dir_recursive(
                dir_path,
                dir_path,
                0,
                max_depth,
                max_preview_lines,
                gitignore.as_ref(),
            )?;
            Ok(serde_json::json!({
                "status": "ok",
                "path": path,
                "recursive": true,
                "total_entries": entries.len(),
                "entries": entries,
            }))
        } else {
            let entries = list_dir_flat(dir_path, max_preview_lines, gitignore.as_ref())?;
            Ok(serde_json::json!({
                "status": "ok",
                "path": path,
                "recursive": false,
                "total_entries": entries.len(),
                "entries": entries,
            }))
        }
    }
}

/// Known text file extensions for preview.
const TEXT_EXTENSIONS: &[&str] = &[
    "rs",
    "toml",
    "md",
    "txt",
    "json",
    "yaml",
    "yml",
    "xml",
    "html",
    "css",
    "js",
    "ts",
    "py",
    "rb",
    "go",
    "java",
    "c",
    "cpp",
    "h",
    "hpp",
    "sh",
    "bash",
    "zsh",
    "fish",
    "lua",
    "sql",
    "r",
    "scala",
    "kt",
    "swift",
    "zig",
    "lock",
    "csv",
    "cfg",
    "ini",
    "env",
    "gitignore",
    "gitattributes",
    "editorconfig",
    "dockerfile",
    "conf",
];

/// Preview the first N lines of a text file.
fn preview_file(path: &Path, max_lines: usize) -> Option<Vec<String>> {
    if max_lines == 0 {
        return None;
    }

    // Check extension first
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_text = TEXT_EXTENSIONS.contains(&ext)
        || ext.is_empty() // no extension files like Makefile, Dockerfile
        || path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n == "Makefile" || n == "Dockerfile" || n == "Cargo.lock"
                    || n.starts_with(".env") || n.starts_with("CMakeLists")
            })
            .unwrap_or(false);

    if !is_text {
        return None;
    }

    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let preview: Vec<String> = lines
        .iter()
        .take(max_lines)
        .enumerate()
        .map(|(i, l)| format!("{:4}: {}", i + 1, l))
        .collect();

    if preview.is_empty() {
        return None;
    }

    Some(preview)
}

/// List a single directory level (non-recursive).
fn list_dir_flat(
    dir: &Path,
    max_preview_lines: usize,
    gitignore: Option<&GitIgnoreMatcher>,
) -> Result<Vec<Value>, String> {
    let mut entries = Vec::new();

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    // Collect and sort by name for deterministic output
    let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {e}"))?;

        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        // Check .gitignore
        if let Some(gi) = gitignore {
            if gi.is_ignored(&name, file_type.is_dir()) {
                continue;
            }
        }

        let mut item = serde_json::json!({
            "name": name,
            "type": kind,
            "size": metadata.len(),
            "modified": chrono::DateTime::<chrono::Utc>::from(metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        });

        // Add preview for text files
        if file_type.is_file() && max_preview_lines > 0 {
            if let Some(preview) = preview_file(&entry.path(), max_preview_lines) {
                let truncated = preview.len() as u64;
                let total_lines = std::fs::read_to_string(entry.path())
                    .ok()
                    .map(|c| c.lines().count() as u64)
                    .unwrap_or(0);
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "preview".to_string(),
                        Value::Array(preview.into_iter().map(Value::String).collect()),
                    );
                }
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "preview_lines".to_string(),
                        Value::Number(serde_json::Number::from(truncated)),
                    );
                }
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "total_lines".to_string(),
                        Value::Number(serde_json::Number::from(total_lines)),
                    );
                }
            }
        }

        entries.push(item);
    }

    Ok(entries)
}

/// Recursively list directory contents up to max_depth, with full relative paths.
fn list_dir_recursive(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    max_preview_lines: usize,
    gitignore: Option<&GitIgnoreMatcher>,
) -> Result<Vec<Value>, String> {
    if depth > max_depth {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in &dir_entries {
        let _name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {e}"))?;

        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        // Compute relative path from root
        let full_path = entry.path();
        let relative = full_path
            .strip_prefix(root)
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string();

        // Check .gitignore
        if let Some(gi) = gitignore {
            if gi.is_ignored(&relative, file_type.is_dir()) {
                continue;
            }
        }

        let mut item = serde_json::json!({
            "name": relative,
            "type": kind,
            "size": metadata.len(),
            "modified": chrono::DateTime::<chrono::Utc>::from(metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        });

        // Recurse into subdirectories
        if file_type.is_dir() {
            let children = list_dir_recursive(
                root,
                &full_path,
                depth + 1,
                max_depth,
                max_preview_lines,
                gitignore,
            )?;
            if !children.is_empty() {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("children".to_string(), Value::Array(children));
                }
            }
        }

        // Add preview for text files
        if file_type.is_file() && max_preview_lines > 0 {
            if let Some(preview) = preview_file(&full_path, max_preview_lines) {
                let truncated = preview.len() as u64;
                let total_lines = std::fs::read_to_string(&full_path)
                    .ok()
                    .map(|c| c.lines().count() as u64)
                    .unwrap_or(0);
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "preview".to_string(),
                        Value::Array(preview.into_iter().map(Value::String).collect()),
                    );
                }
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "preview_lines".to_string(),
                        Value::Number(serde_json::Number::from(truncated)),
                    );
                }
                if let Some(obj) = item.as_object_mut() {
                    obj.insert(
                        "total_lines".to_string(),
                        Value::Number(serde_json::Number::from(total_lines)),
                    );
                }
            }
        }

        entries.push(item);
    }

    Ok(entries)
}

// ============================================================================
// GrepTool
// ============================================================================

/// Search across files using a regex pattern, with file type filtering.
///
/// Supports:
/// - Regex pattern matching with case sensitivity control
/// - File name filtering via glob patterns
/// - Context lines before/after matches
/// - Files-only mode (just return matching file names)
pub struct GrepTool;

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Search across project files using regex patterns. Finds matching lines with context. Supports files-only mode to just list matching files."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "pattern".to_string(),
                description: "Regex pattern to search for".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Directory to search in (default: project root)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_pattern".to_string(),
                description: "Optional file regex pattern to filter (e.g. '*.rs', '*.toml')"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_results".to_string(),
                description: "Maximum number of results to return (default: 30)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "context_lines".to_string(),
                description: "Number of context lines before/after each match (default: 0)"
                    .to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "case_sensitive".to_string(),
                description: "Whether the search is case-sensitive (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "files_only".to_string(),
                description:
                    "If true, only return matching file names without line details (default: false)"
                        .to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_file_size".to_string(),
                description: "Skip files larger than this many bytes (default: 1MB = 1048576)"
                    .to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let pattern_str = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: pattern")?;

        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let file_pattern = params.get("file_pattern").and_then(|v| v.as_str());

        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as usize;

        let context_lines = params
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let case_sensitive = params
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let files_only = params
            .get("files_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_file_size = params
            .get("max_file_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize; // default 1MB

        let search_dir = Path::new(path);
        if !search_dir.exists() || !search_dir.is_dir() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Directory does not exist: {path}"),
            }));
        }

        // Build the regex
        let regex_str = if case_sensitive {
            pattern_str.to_string()
        } else {
            format!("(?i){pattern_str}")
        };

        let re = Regex::new(&regex_str)
            .map_err(|e| format!("Invalid regex pattern '{pattern_str}': {e}"))?;

        // Build file filter regex
        let file_re: Option<Regex> = file_pattern.map(|fp| {
            // Convert simple glob-like patterns to regex
            let glob_re = fp.replace('.', "\\.").replace('*', ".*").replace('?', ".");
            Regex::new(&format!("(?i)^{glob_re}$"))
                .unwrap_or_else(|_| Regex::new(".*").expect("invalid regex: fallback .*"))
        });

        // Walk the directory and search
        let mut opt = GrepOptions {
            re,
            file_re,
            context_lines,
            max_results,
            max_file_size,
            results: Vec::new(),
            total_matches: 0,
        };

        if files_only {
            grep_dir_files_only(search_dir, &mut opt, 0)?;
        } else {
            grep_dir(search_dir, &mut opt, 0)?;
        }

        Ok(serde_json::json!({
            "status": "ok",
            "pattern": pattern_str,
            "case_sensitive": case_sensitive,
            "total_matches": opt.total_matches,
            "results_shown": if files_only { opt.results.len() } else { opt.results.iter().map(|r| r["matches"].as_array().map(|a| a.len()).unwrap_or(0)).sum::<usize>() },
            "truncated": opt.total_matches > opt.results.len(),
            "files_found": opt.results.len(),
            "results": opt.results,
        }))
    }
}

/// Check if a file should be skipped based on extension (binary check).
fn is_binary_file(path: &Path) -> bool {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let binary_extensions: HashSet<&str> = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "woff", "woff2", "ttf", "eot", "otf",
        "mp3", "mp4", "avi", "mov", "wmv", "flv", "webm", "mkv", "zip", "tar", "gz", "bz2", "7z",
        "rar", "xz", "zst", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "o", "so", "dylib",
        "dll", "exe", "class", "jar", "wasm", "iso", "img", "bin", "dat", "db", "sqlite", "ttc",
        "dfont",
    ]
    .iter()
    .cloned()
    .collect();
    binary_extensions.contains(extension)
}

/// Check if a file is likely binary by scanning the first few bytes for null bytes.
fn is_likely_binary(path: &Path) -> bool {
    if let Ok(content) = std::fs::read(path) {
        // Check first 8KB for null bytes (binary indicator)
        let check_len = content.len().min(8192);
        content[..check_len].contains(&0)
    } else {
        true // Can't read = skip
    }
}

/// Standard file/directory skip check.
fn should_skip_entry(dir_name: &str, _path: &Path) -> bool {
    let name_lower = dir_name.to_lowercase();
    if name_lower.starts_with('.') && name_lower != "." {
        return true;
    }
    matches!(
        name_lower.as_str(),
        "node_modules"
            | "target"
            | "vendor"
            | ".git"
            | ".svn"
            | "__pycache__"
            | ".venv"
            | "venv"
            | ".idea"
            | ".vscode"
            | ".DS_Store"
    )
}

/// Recursively search a directory for matching lines (full detail mode).
fn grep_dir(dir: &Path, opt: &mut GrepOptions, depth: usize) -> Result<(), String> {
    if depth > 30 {
        return Ok(());
    }
    if opt.total_matches >= opt.max_results {
        return Ok(());
    }

    let dir_name = dir.file_name().map(|n| n.to_string_lossy());
    if let Some(name) = &dir_name {
        if should_skip_entry(name, dir) {
            return Ok(());
        }
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    let mut entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        if opt.total_matches >= opt.max_results {
            break;
        }

        let path = entry.path();

        if path.is_dir() {
            grep_dir(&path, opt, depth + 1)?;
        } else if path.is_file() {
            if let Some(re) = &opt.file_re {
                let fname = path.file_name().unwrap_or_default().to_string_lossy();
                if !re.is_match(&fname) {
                    continue;
                }
            }

            if is_binary_file(&path) {
                continue;
            }

            if let Ok(meta) = path.metadata() {
                if meta.len() as usize > opt.max_file_size {
                    continue;
                }
            }

            if is_likely_binary(&path) {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let relative_path = path.to_string_lossy().to_string();
            let mut file_matches = Vec::new();

            for (line_num, line) in content.lines().enumerate() {
                if opt.total_matches >= opt.max_results {
                    break;
                }

                if opt.re.is_match(line) {
                    opt.total_matches += 1;

                    let context: Vec<String> = if opt.context_lines > 0 {
                        let lines: Vec<&str> = content.lines().collect();
                        let start = line_num.saturating_sub(opt.context_lines);
                        let end = (line_num + opt.context_lines + 1).min(lines.len());

                        (start..end)
                            .map(|i| {
                                let prefix = if i == line_num { ">" } else { " " };
                                format!("{prefix}{}: {}", i + 1, lines[i])
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    file_matches.push(serde_json::json!({
                        "line": line_num + 1,
                        "content": line,
                        "context": context,
                    }));
                }
            }

            if !file_matches.is_empty() {
                opt.results.push(serde_json::json!({
                    "file": relative_path,
                    "matches": file_matches,
                    "match_count": file_matches.len(),
                }));
            }
        }
    }

    Ok(())
}

/// Recursively search a directory for matching files (files-only mode).
fn grep_dir_files_only(dir: &Path, opt: &mut GrepOptions, depth: usize) -> Result<(), String> {
    if depth > 30 {
        return Ok(());
    }
    if opt.total_matches >= opt.max_results {
        return Ok(());
    }

    let dir_name = dir.file_name().map(|n| n.to_string_lossy());
    if let Some(name) = &dir_name {
        if should_skip_entry(name, dir) {
            return Ok(());
        }
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;

    let mut entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        if opt.total_matches >= opt.max_results {
            break;
        }

        let path = entry.path();

        if path.is_dir() {
            grep_dir_files_only(&path, opt, depth + 1)?;
        } else if path.is_file() {
            if let Some(re) = &opt.file_re {
                let fname = path.file_name().unwrap_or_default().to_string_lossy();
                if !re.is_match(&fname) {
                    continue;
                }
            }

            if is_binary_file(&path) {
                continue;
            }

            if let Ok(meta) = path.metadata() {
                if meta.len() as usize > opt.max_file_size {
                    continue;
                }
            }

            if is_likely_binary(&path) {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let relative_path = path.to_string_lossy().to_string();
            let mut match_count = 0usize;

            for line in content.lines() {
                if opt.total_matches >= opt.max_results {
                    break;
                }
                if opt.re.is_match(line) {
                    opt.total_matches += 1;
                    match_count += 1;
                }
            }

            if match_count > 0 {
                opt.results.push(serde_json::json!({
                    "file": relative_path,
                    "match_count": match_count,
                }));
            }
        }
    }

    Ok(())
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ListDirTool));
    registry.register(Box::new(GrepTool));
}
