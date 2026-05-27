//! Code analyzer tool: parse Rust code structure, find patterns, detect issues.
//! Uses regex-based scanning and cargo commands for code analysis.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct CodeAnalyzerTool;

#[async_trait::async_trait]
impl Tool for CodeAnalyzerTool {
    fn name(&self) -> &str {
        "code_analyzer"
    }

    fn description(&self) -> &str {
        "Analyze Rust code structure: extract functions, structs, enums, traits, impl blocks. \
         Run cargo metadata for project info. Find patterns like TODOs, unsafe blocks, unwrap calls."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: analyze_file, analyze_dir, find_pattern, cargo_metadata, cargo_doc, complexity".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "File or directory path to analyze".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "pattern".to_string(),
                description: "Regex pattern to search for (for find_pattern action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "file_types".to_string(),
                description: "Comma-separated file extensions to scan (default: '.rs')".to_string(),
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

        match action {
            "analyze_file" => self.analyze_file(params),
            "analyze_dir" => self.analyze_dir(params),
            "find_pattern" => self.find_pattern(params),
            "cargo_metadata" => self.cargo_metadata(),
            "cargo_doc" => self.cargo_doc(params),
            "complexity" => self.analyze_complexity(params),
            other => Err(format!("Unknown action: {other}")),
        }
    }
}

impl CodeAnalyzerTool {
    fn project_root(&self) -> Result<PathBuf, String> {
        let mut dir = std::env::current_dir().map_err(|e| format!("Cannot get current dir: {e}"))?;
        for _ in 0..10 {
            if dir.join("Cargo.toml").exists() {
                return Ok(dir);
            }
            if !dir.pop() {
                break;
            }
        }
        Err("Could not find project root (Cargo.toml)".to_string())
    }

    fn resolve_path(&self, rel_path: &str) -> Result<PathBuf, String> {
        let root = self.project_root()?;
        Ok(root.join(rel_path))
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))
    }

    /// Extract Rust items from source using regex patterns.
    fn extract_items(content: &str) -> RustStructure {
        let mut items = RustStructure::default();

        // Extract functions
        for cap in regex::Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*<[^>]*>\s*\(")
            .unwrap()
            .captures_iter(content)
            .filter(|c| c.get(1).is_some())
        {
            items.functions.push(cap[1].to_string());
        }
        // Fallback: simple fn match
        if items.functions.is_empty() {
            for cap in regex::Regex::new(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(")
                .unwrap()
                .captures_iter(content)
            {
                items.functions.push(cap[1].to_string());
            }
        }

        // Extract structs
        for cap in regex::Regex::new(r"(?:pub\s+)?struct\s+(\w+)")
            .unwrap()
            .captures_iter(content)
        {
            items.structs.push(cap[1].to_string());
        }

        // Extract enums
        for cap in regex::Regex::new(r"(?:pub\s+)?enum\s+(\w+)")
            .unwrap()
            .captures_iter(content)
        {
            items.enums.push(cap[1].to_string());
        }

        // Extract traits
        for cap in regex::Regex::new(r"(?:pub\s+)?trait\s+(\w+)")
            .unwrap()
            .captures_iter(content)
        {
            items.traits.push(cap[1].to_string());
        }

        // Extract impl blocks
        for cap in regex::Regex::new(r"impl(?:\s+<[^>]*>)?\s+(\w+)")
            .unwrap()
            .captures_iter(content)
        {
            items.impls.push(cap[1].to_string());
        }

        // Count unwrap calls
        items.unwrap_count = regex::Regex::new(r"\.unwrap\(\)")
            .unwrap()
            .captures_iter(content)
            .count();

        // Count unsafe blocks
        items.unsafe_count = regex::Regex::new(r"\bunsafe\b")
            .unwrap()
            .captures_iter(content)
            .count();

        // Count TODO comments
        items.todo_count = regex::Regex::new(r"//.*TODO|//.*todo")
            .unwrap()
            .captures_iter(content)
            .count();

        // Count lines
        items.line_count = content.lines().count();

        items
    }

    fn analyze_file(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let full_path = self.resolve_path(path)?;
        let content = Self::read_file(&full_path)?;
        let items = Self::extract_items(&content);

        Ok(serde_json::json!({
            "status": "ok",
            "file": path,
            "lines": items.line_count,
            "functions": items.functions,
            "structs": items.structs,
            "enums": items.enums,
            "traits": items.traits,
            "impls": items.impls,
            "unwrap_calls": items.unwrap_count,
            "unsafe_blocks": items.unsafe_count,
            "todos": items.todo_count,
        }))
    }

    fn analyze_dir(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("src");

        let full_path = self.resolve_path(path)?;
        let file_types_param = params
            .get("file_types")
            .and_then(|v| v.as_str())
            .unwrap_or(".rs");
        let extensions: Vec<&str> = file_types_param.split(',').map(|s| s.trim()).collect();

        let mut total = FileSummary::default();
        let mut files: Vec<Value> = Vec::new();

        if !full_path.exists() {
            return Err(format!("Path does not exist: {}", full_path.display()));
        }

        for entry in walk_dir(&full_path)? {
            if !entry.is_file() {
                continue;
            }
            let ext = entry.extension().and_then(|e| e.to_str()).unwrap_or("");
            let ext_with_dot = format!(".{ext}");
            if !extensions.contains(&ext_with_dot.as_str()) {
                continue;
            }

            if let Ok(content) = Self::read_file(&entry) {
                let items = Self::extract_items(&content);
                let rel_path = entry.strip_prefix(self.project_root()?)
                    .unwrap_or(&entry)
                    .to_string_lossy()
                    .to_string();

                total.functions += items.functions.len();
                total.structs += items.structs.len();
                total.enums += items.enums.len();
                total.traits += items.traits.len();
                total.impls += items.impls.len();
                total.unwrap_calls += items.unwrap_count;
                total.unsafe_blocks += items.unsafe_count;
                total.todo_count += items.todo_count;
                total.line_count += items.line_count;
                total.file_count += 1;

                if !items.functions.is_empty() || !items.structs.is_empty() {
                    files.push(serde_json::json!({
                        "file": rel_path,
                        "lines": items.line_count,
                        "functions": items.functions,
                        "structs": items.structs,
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "status": "ok",
            "path": path,
            "summary": {
                "files_scanned": total.file_count,
                "total_lines": total.line_count,
                "total_functions": total.functions,
                "total_structs": total.structs,
                "total_enums": total.enums,
                "total_traits": total.traits,
                "total_impls": total.impls,
                "unwrap_calls": total.unwrap_calls,
                "unsafe_blocks": total.unsafe_blocks,
                "todos": total.todo_count,
            },
            "files": files,
        }))
    }

    fn find_pattern(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: pattern")?;
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("src");

        let full_path = self.resolve_path(path)?;
        let re = regex::Regex::new(pattern)
            .map_err(|e| format!("Invalid regex pattern: {e}"))?;

        let mut matches: Vec<Value> = Vec::new();

        if !full_path.exists() {
            return Err(format!("Path does not exist: {}", full_path.display()));
        }

        for entry in walk_dir(&full_path)? {
            if !entry.is_file() {
                continue;
            }
            let ext = entry.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }

            if let Ok(content) = Self::read_file(&entry) {
                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        let rel_path = entry.strip_prefix(self.project_root()?)
                            .unwrap_or(&entry)
                            .to_string_lossy()
                            .to_string();
                        matches.push(serde_json::json!({
                            "file": rel_path,
                            "line": line_num + 1,
                            "content": line.trim(),
                        }));
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "status": "ok",
            "pattern": pattern,
            "count": matches.len(),
            "matches": matches.iter().take(50).cloned().collect::<Vec<_>>(),
        }))
    }

    fn cargo_metadata(&self) -> Result<Value, String> {
        let root = self.project_root()?;
        let output = Command::new("cargo")
            .arg("metadata")
            .arg("--no-deps")
            .arg("--format-version=1")
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run cargo metadata: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let metadata: Value = serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse cargo metadata: {e}"))?;

        let packages = metadata["packages"].as_array().map(|pkgs| {
            pkgs.iter().map(|p| {
                serde_json::json!({
                    "name": p["name"].as_str().unwrap_or(""),
                    "version": p["version"].as_str().unwrap_or(""),
                    "dependencies_count": p["dependencies"].as_array().map(|d| d.len()).unwrap_or(0),
                })
            }).collect::<Vec<_>>()
        }).unwrap_or_default();

        Ok(serde_json::json!({
            "status": "ok",
            "workspace_root": metadata["workspace_root"].as_str().unwrap_or(""),
            "packages": packages,
        }))
    }

    fn cargo_doc(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let no_deps = params
            .get("no_deps")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let root = self.project_root()?;
        let mut cmd = Command::new("cargo");
        cmd.arg("doc").arg("--no-deps");

        if no_deps {
            cmd.arg("--no-deps");
        }

        let output = cmd
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run cargo doc: {e}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(serde_json::json!({
                "status": "ok",
                "output": "Documentation generated in target/doc/",
                "log": stderr.lines().take(20).collect::<Vec<_>>(),
            }))
        } else {
            Ok(serde_json::json!({
                "status": "error",
                "log": stderr.lines().take(20).collect::<Vec<_>>(),
            }))
        }
    }

    fn analyze_complexity(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("src");

        let full_path = self.resolve_path(path)?;
        let mut complexities: Vec<Value> = Vec::new();

        if !full_path.exists() {
            return Err(format!("Path does not exist: {}", full_path.display()));
        }

        for entry in walk_dir(&full_path)? {
            if !entry.is_file() {
                continue;
            }
            let ext = entry.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }

            if let Ok(content) = Self::read_file(&entry) {
                let lines = content.lines().collect::<Vec<_>>();
                let mut in_function = false;
                let mut func_name = String::new();
                let mut func_start = 0;
                let mut brace_depth = 0;
                let mut mut_count = 0;

                for (i, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();

                    if let Some(cap) = regex::Regex::new(r"fn\s+(\w+)")
                        .unwrap().captures(trimmed)
                    {
                        if in_function && brace_depth == 0 {
                            // End of previous function
                            let complexity = (mut_count as f64 / (func_start..i).len() as f64 * 100.0).round();
                            complexities.push(serde_json::json!({
                                "file": entry.strip_prefix(self.project_root()?).unwrap_or(&entry).to_string_lossy(),
                                "function": func_name,
                                "lines": i - func_start,
                                "mut_count": mut_count,
                            });
                        }
                        in_function = true;
                        func_name = cap[1].to_string();
                        func_start = i;
                        mut_count = 0;
                    }

                    mut_count += regex::Regex::new(r"\bmut\b").unwrap().captures_iter(trimmed).count();

                    brace_depth += trimmed.matches('{').count();
                    brace_depth -= trimmed.matches('}').count();
                }

                if in_function && brace_depth == 0 {
                    complexities.push(serde_json::json!({
                        "file": entry.strip_prefix(self.project_root()?).unwrap_or(&entry).to_string_lossy(),
                        "function": func_name,
                        "lines": lines.len() - func_start,
                        "mut_count": mut_count,
                    }));
                }
            }
        }

        complexities.sort_by(|a, b| {
            b["lines"].as_u64().unwrap_or(0).cmp(&a["lines"].as_u64().unwrap_or(0))
        });

        Ok(serde_json::json!({
            "status": "ok",
            "path": path,
            "functions": complexities.iter().take(20).cloned().collect::<Vec<_>>(),
            "total_functions": complexities.len(),
        }))
    }
}

#[derive(Default)]
struct RustStructure {
    functions: Vec<String>,
    structs: Vec<String>,
    enums: Vec<String>,
    traits: Vec<String>,
    impls: Vec<String>,
    unwrap_count: usize,
    unsafe_count: usize,
    todo_count: usize,
    line_count: usize,
}

#[derive(Default)]
struct FileSummary {
    file_count: usize,
    line_count: usize,
    functions: usize,
    structs: usize,
    enums: usize,
    traits: usize,
    impls: usize,
    unwrap_calls: usize,
    unsafe_blocks: usize,
    todo_count: usize,
}

fn walk_dir(dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut entries = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("Cannot read entry: {e}"))?;
            let path = entry.path();
            if path.is_dir() {
                // Skip target and .git directories
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "target" || name == ".git" {
                        continue;
                    }
                }
                let mut sub = walk_dir(&path)?;
                entries.append(&mut sub);
            } else {
                entries.push(path);
            }
        }
    }
    Ok(entries)
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeAnalyzerTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_items_from_sample() {
        let sample = r#"
            pub struct User {
                name: String,
            }

            pub enum Status { Active, Inactive }

            pub trait Repository {
                fn find(&self, id: u64) -> Option<User>;
            }

            impl User {
                pub fn new(name: &str) -> Self {
                    Self { name: name.to_string() }
                }
            }

            async fn fetch_user(id: u64) -> Result<User, Error> {
                let conn = get_conn().unwrap();
                unsafe { conn.raw_query() }
                // TODO: implement caching
                Ok(user)
            }
        "#;

        let items = CodeAnalyzerTool::extract_items(sample);
        assert_eq!(items.functions.len(), 2); // new, fetch_user
        assert_eq!(items.structs.len(), 1);
        assert_eq!(items.enums.len(), 1);
        assert_eq!(items.traits.len(), 1);
        assert_eq!(items.impls.len(), 1);
        assert!(items.unwrap_count > 0);
        assert!(items.unsafe_count > 0);
        assert!(items.todo_count > 0);
    }

    #[test]
    fn test_walk_dir_skips_target() {
        let root = std::env::current_dir().unwrap();
        let entries = walk_dir(&root).unwrap();
        // Should not contain any target/ entries
        for entry in &entries {
            let path_str = entry.to_string_lossy();
            assert!(!path_str.contains("/target/"), "Found target dir in: {}", path_str);
        }
    }
}
