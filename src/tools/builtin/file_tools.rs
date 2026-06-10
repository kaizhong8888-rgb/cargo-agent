//! File and filesystem tools: read, write, list directory, grep search.
//!
//! Provides four tools for file operations and filesystem exploration:
//!
//! - **read_file** — read a file's contents
//! - **write_file** — write content to a file
//! - **list_directory** — list directory contents with preview & .gitignore support
//! - **grep_search** — regex search across files with context & file filtering

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ============================================================================
// ReadFile
// ============================================================================

pub struct ReadFile;

#[async_trait::async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "path".to_string(),
            description: "Path to the file".to_string(),
            required: true,
            parameter_type: "string".to_string(),
        }]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        match std::fs::read_to_string(path) {
            Ok(content) => Ok(serde_json::json!({
                "status": "ok",
                "content": content,
                "path": path,
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "message": format!("Failed to read file: {}", e),
            })),
        }
    }
}

// ============================================================================
// WriteFile
// ============================================================================

pub struct WriteFile;

#[async_trait::async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the file".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "content".to_string(),
                description: "Content to write".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: content")?;

        match std::fs::write(path, content) {
            Ok(_) => Ok(serde_json::json!({
                "status": "ok",
                "path": path,
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "message": format!("Failed to write file: {}", e),
            })),
        }
    }
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
    #[allow(dead_code)]
    raw: String,
    re: Regex,
    dir_only: bool,
    negated: bool,
}

impl GitIgnoreMatcher {
    fn load_from_dir(dir: &Path) -> Self {
        let mut patterns = Vec::new();
        let mut current = Some(dir);
        while let Some(path) = current {
            let gitignore_path = path.join(".gitignore");
            if gitignore_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some(p) = GitIgnorePattern::parse(line) {
                            patterns.push(p);
                        }
                    }
                }
            }
            current = path.parent();
        }
        GitIgnoreMatcher { patterns }
    }

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
    fn parse(line: &str) -> Option<Self> {
        let mut raw = line.to_string();
        let negated = raw.starts_with('!');
        if negated {
            raw = raw[1..].to_string();
        }
        let dir_only = raw.ends_with('/');
        if dir_only {
            raw = raw.trim_end_matches('/').to_string();
        }

        let mut regex_str = String::new();
        let mut chars = raw.chars().peekable();
        let anchored = !raw.starts_with('/');

        // All branches add the same prefix; only the non-anchored branch consumes a leading '/'
        regex_str.push_str("(^|/)");
        if !anchored {
            let _ = chars.next();
        }

        while let Some(c) = chars.next() {
            match c {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        if chars.peek() == Some(&'/') {
                            chars.next();
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

        let gitignore = if use_gitignore {
            Some(GitIgnoreMatcher::load_from_dir(dir_path))
        } else {
            None
        };

        if recursive {
            let entries = list_dir_recursive(
                dir_path, dir_path, 0, max_depth, max_preview_lines, gitignore.as_ref(),
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

// ============================================================================
// Listing helpers
// ============================================================================

const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "toml", "md", "txt", "json", "yaml", "yml", "xml", "html", "css", "js", "ts", "py",
    "rb", "go", "java", "c", "cpp", "h", "hpp", "sh", "bash", "zsh", "fish", "lua", "sql", "r",
    "scala", "kt", "swift", "zig", "lock", "csv", "cfg", "ini", "env", "gitignore",
    "gitattributes", "editorconfig", "dockerfile", "conf",
];

fn preview_file(path: &Path, max_lines: usize) -> Option<Vec<String>> {
    if max_lines == 0 {
        return None;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_text = TEXT_EXTENSIONS.contains(&ext)
        || ext.is_empty()
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n == "Makefile"
                    || n == "Dockerfile"
                    || n == "Cargo.lock"
                    || n.starts_with(".env")
                    || n.starts_with("CMakeLists")
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

fn list_dir_flat(
    dir: &Path,
    max_preview_lines: usize,
    gitignore: Option<&GitIgnoreMatcher>,
) -> Result<Vec<Value>, String> {
    let mut entries = Vec::new();
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
    let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|e| format!("Failed to get file type: {e}"))?;
        let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata: {e}"))?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        if let Some(gi) = gitignore {
            if gi.is_ignored(&name, file_type.is_dir()) {
                continue;
            }
        }

        let mut item = serde_json::json!({
            "name": name,
            "type": kind,
            "size": metadata.len(),
            "modified": chrono::DateTime::<chrono::Utc>::from(
                metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            ).format("%Y-%m-%d %H:%M:%S").to_string(),
        });

        if file_type.is_file() && max_preview_lines > 0 {
            if let Some(preview) = preview_file(&entry.path(), max_preview_lines) {
                let truncated = preview.len() as u64;
                let total_lines = std::fs::read_to_string(entry.path())
                    .ok()
                    .map(|c| c.lines().count() as u64)
                    .unwrap_or(0);
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("preview".to_string(), Value::Array(preview.into_iter().map(Value::String).collect()));
                    obj.insert("preview_lines".to_string(), Value::Number(serde_json::Number::from(truncated)));
                    obj.insert("total_lines".to_string(), Value::Number(serde_json::Number::from(total_lines)));
                }
            }
        }

        entries.push(item);
    }
    Ok(entries)
}

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
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
    let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in &dir_entries {
        let file_type = entry.file_type().map_err(|e| format!("Failed to get file type: {e}"))?;
        let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata: {e}"))?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        let full_path = entry.path();
        let relative = full_path
            .strip_prefix(root)
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string();

        if let Some(gi) = gitignore {
            if gi.is_ignored(&relative, file_type.is_dir()) {
                continue;
            }
        }

        let mut item = serde_json::json!({
            "name": relative,
            "type": kind,
            "size": metadata.len(),
            "modified": chrono::DateTime::<chrono::Utc>::from(
                metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            ).format("%Y-%m-%d %H:%M:%S").to_string(),
        });

        if file_type.is_dir() {
            let children = list_dir_recursive(
                root, &full_path, depth + 1, max_depth, max_preview_lines, gitignore,
            )?;
            if !children.is_empty() {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("children".to_string(), Value::Array(children));
                }
            }
        }

        if file_type.is_file() && max_preview_lines > 0 {
            if let Some(preview) = preview_file(&full_path, max_preview_lines) {
                let truncated = preview.len() as u64;
                let total_lines = std::fs::read_to_string(&full_path)
                    .ok()
                    .map(|c| c.lines().count() as u64)
                    .unwrap_or(0);
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("preview".to_string(), Value::Array(preview.into_iter().map(Value::String).collect()));
                    obj.insert("preview_lines".to_string(), Value::Number(serde_json::Number::from(truncated)));
                    obj.insert("total_lines".to_string(), Value::Number(serde_json::Number::from(total_lines)));
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

struct GrepOptions {
    re: Regex,
    file_re: Option<Regex>,
    context_lines: usize,
    max_results: usize,
    max_file_size: usize,
    results: Vec<Value>,
    total_matches: usize,
}

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
                description: "Optional file regex pattern to filter (e.g. '*.rs', '*.toml')".to_string(),
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
                description: "Number of context lines before/after each match (default: 0)".to_string(),
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
                description: "If true, only return matching file names without line details (default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_file_size".to_string(),
                description: "Skip files larger than this many bytes (default: 1MB = 1048576)".to_string(),
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
        let max_results = params.get("max_results").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
        let context_lines = params.get("context_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let case_sensitive = params.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let files_only = params.get("files_only").and_then(|v| v.as_bool()).unwrap_or(false);
        let max_file_size = params.get("max_file_size").and_then(|v| v.as_u64()).unwrap_or(1_048_576) as usize;

        let search_dir = Path::new(path);
        if !search_dir.exists() || !search_dir.is_dir() {
            return Ok(serde_json::json!({
                "status": "error",
                "message": format!("Directory does not exist: {path}"),
            }));
        }

        let regex_str = if case_sensitive {
            pattern_str.to_string()
        } else {
            format!("(?i){pattern_str}")
        };
        let re = Regex::new(&regex_str).map_err(|e| format!("Invalid regex pattern '{pattern_str}': {e}"))?;

        let file_re: Option<Regex> = file_pattern.map(|fp| {
            let glob_re = fp.replace('.', "\\.").replace('*', ".*").replace('?', ".");
            Regex::new(&format!("(?i)^{glob_re}$")).unwrap_or_else(|_| Regex::new(".*").expect("fallback"))
        });

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
            "results_shown": if files_only {
                opt.results.len()
            } else {
                opt.results.iter().map(|r| r["matches"].as_array().map(|a| a.len()).unwrap_or(0)).sum::<usize>()
            },
            "truncated": opt.total_matches > opt.results.len(),
            "files_found": opt.results.len(),
            "results": opt.results,
        }))
    }
}

// ============================================================================
// Grep helpers
// ============================================================================

fn is_binary_file(path: &Path) -> bool {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let binary_extensions: HashSet<&str> = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg",
        "woff", "woff2", "ttf", "eot", "otf",
        "mp3", "mp4", "avi", "mov", "wmv", "flv", "webm", "mkv",
        "zip", "tar", "gz", "bz2", "7z", "rar", "xz", "zst",
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
        "o", "so", "dylib", "dll", "exe", "class", "jar", "wasm",
        "iso", "img", "bin", "dat", "db", "sqlite", "ttc", "dfont",
    ]
    .iter()
    .cloned()
    .collect();
    binary_extensions.contains(extension)
}

fn is_likely_binary(path: &Path) -> bool {
    if let Ok(content) = std::fs::read(path) {
        let check_len = content.len().min(8192);
        content[..check_len].contains(&0)
    } else {
        true
    }
}

fn should_skip_entry(dir_name: &str, _path: &Path) -> bool {
    let name_lower = dir_name.to_lowercase();
    if name_lower.starts_with('.') && name_lower != "." {
        return true;
    }
    matches!(
        name_lower.as_str(),
        "node_modules" | "target" | "vendor" | ".git" | ".svn"
            | "__pycache__" | ".venv" | "venv" | ".idea" | ".vscode" | ".DS_Store"
    )
}

fn grep_dir(dir: &Path, opt: &mut GrepOptions, depth: usize) -> Result<(), String> {
    if depth > 30 || opt.total_matches >= opt.max_results {
        return Ok(());
    }
    let dir_name = dir.file_name().map(|n| n.to_string_lossy());
    if let Some(name) = &dir_name {
        if should_skip_entry(name, dir) {
            return Ok(());
        }
    }
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
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
            if is_binary_file(&path) || is_likely_binary(&path) {
                continue;
            }
            if let Ok(meta) = path.metadata() {
                if meta.len() as usize > opt.max_file_size {
                    continue;
                }
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

fn grep_dir_files_only(dir: &Path, opt: &mut GrepOptions, depth: usize) -> Result<(), String> {
    if depth > 30 || opt.total_matches >= opt.max_results {
        return Ok(());
    }
    let dir_name = dir.file_name().map(|n| n.to_string_lossy());
    if let Some(name) = &dir_name {
        if should_skip_entry(name, dir) {
            return Ok(());
        }
    }
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?;
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
            if is_binary_file(&path) || is_likely_binary(&path) {
                continue;
            }
            if let Ok(meta) = path.metadata() {
                if meta.len() as usize > opt.max_file_size {
                    continue;
                }
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
    registry.register(Box::new(ReadFile));
    registry.register(Box::new(WriteFile));
    registry.register(Box::new(ListDirTool));
    registry.register(Box::new(GrepTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use std::io::Write;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("file_tools_test_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn create_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn create_subdir(parent: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = parent.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    // ── ReadFile tests ──────────────────────────────────────

    #[test]
    fn test_read_file_success() {
        let dir = temp_dir("read_success");
        let file = create_file(&dir, "test.txt", "hello world");
        let tool = ReadFile;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(file.to_string_lossy().to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["content"], "hello world");
        cleanup(&dir);
    }

    #[test]
    fn test_read_file_missing_path() {
        let tool = ReadFile;
        let params = HashMap::new();
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_err(), "expected error for missing path");
    }

    #[test]
    fn test_read_file_not_found() {
        let tool = ReadFile;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String("/nonexistent/file.txt".to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["status"], "error");
    }

    // ── WriteFile tests ─────────────────────────────────────

    #[test]
    fn test_write_file_success() {
        let dir = temp_dir("write_success");
        let file_path = dir.join("output.txt");
        let tool = WriteFile;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(file_path.to_string_lossy().to_string()));
        params.insert("content".to_string(), Value::String("written content".to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["status"], "ok");
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "written content");
        cleanup(&dir);
    }

    #[test]
    fn test_write_file_missing_params() {
        let tool = WriteFile;
        let params = HashMap::new();
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_err(), "expected error for missing parameters");
    }

    // ── ListDirTool tests ───────────────────────────────────

    #[test]
    fn test_list_directory_non_recursive() {
        let dir = temp_dir("list_flat");
        create_file(&dir, "a.txt", "aaa");
        create_file(&dir, "b.rs", "bbb");
        create_subdir(&dir, "sub");
        let tool = ListDirTool;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["status"], "ok");
        let entries = v["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        cleanup(&dir);
    }

    #[test]
    fn test_list_directory_recursive() {
        let dir = temp_dir("list_rec");
        create_file(&dir, "root.txt", "root");
        let sub = create_subdir(&dir, "sub");
        create_file(&sub, "nested.txt", "nested");
        let tool = ListDirTool;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        params.insert("recursive".to_string(), Value::Bool(true));
        params.insert("max_depth".to_string(), Value::Number(serde_json::Number::from(5)));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        let entries = v["entries"].as_array().unwrap();
        assert!(entries.len() >= 2);
        cleanup(&dir);
    }

    #[test]
    fn test_list_directory_nonexistent() {
        let tool = ListDirTool;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String("/nonexistent_path_xyz".to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["status"], "error");
    }

    #[test]
    fn test_list_directory_with_gitignore() {
        let dir = temp_dir("list_gitignore");
        create_file(&dir, ".gitignore", "*.log\n");
        create_file(&dir, "keep.rs", "ok");
        create_file(&dir, "ignore.log", "should be ignored");
        let tool = ListDirTool;
        let mut params = HashMap::new();
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        params.insert("use_gitignore".to_string(), Value::Bool(true));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        let entries = v["entries"].as_array().unwrap();
        let names: Vec<&str> = entries.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(names.iter().any(|n| n.contains("keep.rs")));
        assert!(!names.iter().any(|n| n.contains("ignore.log")));
        cleanup(&dir);
    }

    // ── GitIgnorePattern tests ──────────────────────────────

    #[test]
    fn test_gitignore_parse_simple_extension() {
        let pattern = GitIgnorePattern::parse("*.log").unwrap();
        assert!(pattern.re.is_match("debug.log"));
        assert!(pattern.re.is_match("src/debug.log"));
        assert!(!pattern.re.is_match("debug.txt"));
    }

    #[test]
    fn test_gitignore_parse_negated() {
        let pattern = GitIgnorePattern::parse("!important.log").unwrap();
        assert!(pattern.negated);
        assert!(pattern.re.is_match("important.log"));
    }

    #[test]
    fn test_gitignore_parse_dir_only() {
        let pattern = GitIgnorePattern::parse("target/").unwrap();
        assert!(pattern.dir_only);
    }

    #[test]
    fn test_gitignore_is_ignored() {
        let mut matcher = GitIgnoreMatcher { patterns: Vec::new() };
        if let Some(p) = GitIgnorePattern::parse("*.log") {
            matcher.patterns.push(p);
        }
        assert!(matcher.is_ignored("debug.log", false));
        assert!(matcher.is_ignored("src/app.log", false));
        assert!(!matcher.is_ignored("src/app.rs", false));
    }

    #[test]
    fn test_gitignore_negation_overrides() {
        let mut matcher = GitIgnoreMatcher { patterns: Vec::new() };
        if let Some(p) = GitIgnorePattern::parse("*.log") {
            matcher.patterns.push(p);
        }
        if let Some(p) = GitIgnorePattern::parse("!keep.log") {
            matcher.patterns.push(p);
        }
        assert!(matcher.is_ignored("debug.log", false));
        assert!(!matcher.is_ignored("keep.log", false));
    }

    #[test]
    fn test_gitignore_dir_only_skips_files() {
        let mut matcher = GitIgnoreMatcher { patterns: Vec::new() };
        if let Some(p) = GitIgnorePattern::parse("target/") {
            matcher.patterns.push(p);
        }
        assert!(matcher.is_ignored("target", true));
        assert!(!matcher.is_ignored("target", false));
    }

    #[test]
    fn test_gitignore_matcher_from_dir() {
        let dir = temp_dir("gitignore_from_dir");
        create_file(&dir, ".gitignore", "*.log\n");
        let matcher = GitIgnoreMatcher::load_from_dir(&dir);
        assert!(matcher.is_ignored("test.log", false));
        assert!(!matcher.is_ignored("test.rs", false));
        cleanup(&dir);
    }

    #[test]
    fn test_gitignore_pattern_complex_glob() {
        let pattern = GitIgnorePattern::parse("build/**/output").unwrap();
        assert!(pattern.re.is_match("build/output"));
        assert!(pattern.re.is_match("build/x86/output"));
    }

    // ── GrepTool tests ──────────────────────────────────────

    #[test]
    fn test_grep_search_basic() {
        let dir = temp_dir("grep_basic");
        create_file(&dir, "a.rs", "fn hello() {}\nfn world() {}\n");
        create_file(&dir, "b.py", "def hello(): pass");
        let tool = GrepTool;
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String("hello".to_string()));
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["status"], "ok");
        assert!(v["files_found"].as_u64().unwrap_or(0) >= 2);
        cleanup(&dir);
    }

    #[test]
    fn test_grep_search_case_insensitive_default() {
        let dir = temp_dir("grep_case");
        create_file(&dir, "test.txt", "Hello World\n");
        let tool = GrepTool;
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String("hello".to_string()));
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["status"], "ok");
        assert!(v["total_matches"].as_u64().unwrap_or(0) >= 1);
        cleanup(&dir);
    }

    #[test]
    fn test_grep_search_files_only() {
        let dir = temp_dir("grep_files_only");
        create_file(&dir, "a.rs", "fn test() {}");
        create_file(&dir, "b.rs", "fn other() {}");
        let tool = GrepTool;
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String("test".to_string()));
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        params.insert("files_only".to_string(), Value::Bool(true));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["files_found"].as_u64().unwrap_or(0), 1);
        cleanup(&dir);
    }

    #[test]
    fn test_grep_search_no_matches() {
        let dir = temp_dir("grep_no_match");
        create_file(&dir, "test.txt", "nothing here");
        let tool = GrepTool;
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String("zzz_nonexistent_zzz".to_string()));
        params.insert("path".to_string(), Value::String(dir.to_string_lossy().to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["total_matches"].as_u64().unwrap_or(0), 0);
        cleanup(&dir);
    }

    #[test]
    fn test_grep_search_invalid_regex() {
        let tool = GrepTool;
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), Value::String("[invalid".to_string()));
        params.insert("path".to_string(), Value::String(".".to_string()));
        let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute(&params));
        assert!(result.is_err(), "expected error for invalid regex");
    }

    // ── Helper function tests ───────────────────────────────

    #[test]
    fn test_is_binary_file() {
        let dir = temp_dir("is_binary");
        let png = create_file(&dir, "test.png", "");
        let rs = create_file(&dir, "main.rs", "");
        assert!(is_binary_file(&png));
        assert!(!is_binary_file(&rs));
        cleanup(&dir);
    }

    #[test]
    fn test_should_skip_entry() {
        let p = Path::new("/tmp");
        assert!(should_skip_entry("node_modules", p));
        assert!(should_skip_entry(".git", p));
        assert!(should_skip_entry("target", p));
        assert!(should_skip_entry(".DS_Store", p));
        assert!(!should_skip_entry("src", p));
    }
}
