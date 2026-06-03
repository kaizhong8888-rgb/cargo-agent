//! Documentation Generator Tool: auto-generate API docs, README, architecture docs.
//!
//! # Actions
//!
//! - **api_doc**: Generate API documentation from Rust source code
//! - **readme**: Generate or update README.md for a project
//! - **architecture**: Generate architecture documentation with Mermaid diagrams
//! - **module_doc**: Generate module-level documentation

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Regex Patterns
// ============================================================================

static RE_PUB_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)(?:\s*->\s*([^\{]+))?")
        .expect("valid regex")
});

static RE_PUB_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+struct\s+([a-zA-Z_][a-zA-Z0-9_]*)").expect("valid regex")
});

static RE_PUB_ENUM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+enum\s+([a-zA-Z_][a-zA-Z0-9_]*)").expect("valid regex")
});

static RE_PUB_TRAIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+(?:unsafe\s+)?trait\s+([a-zA-Z_][a-zA-Z0-9_]*)")
        .expect("valid regex")
});

static RE_PUB_TYPE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+type\s+([a-zA-Z_][a-zA-Z0-9_]*)").expect("valid regex")
});

static RE_PUB_CONST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+const\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*([^\s=]+)")
        .expect("valid regex")
});

#[allow(dead_code)]
static RE_DOC_COMMENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*///\s*(.*)").expect("valid regex"));

static RE_MODULE_DECL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\{|;)").expect("valid regex")
});

static RE_USE_DECL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*pub\s+use\s+(.+)").expect("valid regex"));

// ============================================================================
// DocGeneratorTool
// ============================================================================

pub struct DocGeneratorTool;

#[async_trait::async_trait]
impl Tool for DocGeneratorTool {
    fn name(&self) -> &str {
        "doc_gen"
    }

    fn description(&self) -> &str {
        "Documentation generator: auto-generate API docs from Rust source code, README.md, architecture docs with Mermaid diagrams, and module documentation. Actions: api_doc, readme, architecture, module_doc."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: api_doc, readme, architecture, module_doc".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to Rust file or project directory".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "project_name".to_string(),
                description: "Project name for README (default: from Cargo.toml)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "output".to_string(),
                description: "Output file path (optional, prints to result if omitted)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Scan recursively for project directories (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: markdown, json (default: markdown)".to_string(),
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

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let output = params.get("output").and_then(|v| v.as_str());
        let project_name = params.get("project_name").and_then(|v| v.as_str());
        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");

        let result = match action {
            "api_doc" => generate_api_doc(path, recursive, format)?,
            "readme" => generate_readme(path, project_name)?,
            "architecture" => generate_architecture(path, recursive)?,
            "module_doc" => generate_module_doc(path, recursive, format)?,
            _ => {
                return Ok(json!({
                    "status": "error",
                    "message": format!("Unknown action: {action}. Available: api_doc, readme, architecture, module_doc"),
                }))
            }
        };

        // Write to file if output specified
        if let Some(out_path) = output {
            std::fs::write(out_path, result.to_string())
                .map_err(|e| format!("Failed to write to {out_path}: {e}"))?;
            return Ok(json!({
                "status": "ok",
                "action": action,
                "output": out_path,
            }));
        }

        Ok(result)
    }
}

// ============================================================================
// File Collection
// ============================================================================

fn collect_rust_files(
    dir: &Path,
    files: &mut Vec<String>,
    recursive: bool,
    depth: usize,
) -> Result<(), String> {
    if depth > 10 {
        return Ok(());
    }
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read '{}': {e}", dir.display()))?;
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
                collect_rust_files(&p, files, true, depth + 1)?;
            }
        } else if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
            files.push(p.to_string_lossy().to_string());
        }
    }
    Ok(())
}

fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))
}

// ============================================================================
// Extract doc comments preceding a line
// ============================================================================

fn extract_doc_comments(content: &str, line_num: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut docs = Vec::new();

    let start = line_num.saturating_sub(10);
    for i in (start..line_num).rev() {
        if let Some(line) = lines.get(i) {
            let trimmed = line.trim();
            if trimmed.starts_with("///") {
                docs.push(
                    trimmed
                        .strip_prefix("///")
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("//!") {
                docs.push(
                    trimmed
                        .strip_prefix("//!")
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                );
            } else if trimmed.is_empty() {
                continue;
            } else {
                break;
            }
        }
    }

    docs.reverse();
    docs.join(" ")
}

// ============================================================================
// API Documentation
// ============================================================================

fn generate_api_doc(path: &str, recursive: bool, format: &str) -> Result<Value, String> {
    let scan_path = Path::new(path);
    let mut files: Vec<String> = Vec::new();

    if scan_path.is_file() {
        files.push(path.to_string());
    } else {
        collect_rust_files(scan_path, &mut files, recursive, 0)?;
    }

    if files.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "No Rust files found",
        }));
    }

    let mut modules: Vec<ModuleDoc> = Vec::new();

    for file_path in &files {
        let content = read_file(file_path)?;
        let module = parse_module(file_path, &content);
        modules.push(module);
    }

    if format == "json" {
        let json_modules: Vec<Value> = modules.iter().map(|m| m.to_json()).collect();
        Ok(json!({
            "status": "ok",
            "action": "api_doc",
            "format": "json",
            "files": files.len(),
            "modules": json_modules,
        }))
    } else {
        let mut doc = String::new();
        doc.push_str("# API Documentation\n\n");
        for module in &modules {
            doc.push_str(&module.to_markdown());
        }
        Ok(json!({
            "status": "ok",
            "action": "api_doc",
            "format": "markdown",
            "files": files.len(),
            "documentation": doc,
        }))
    }
}

struct ModuleDoc {
    file_path: String,
    module_name: String,
    pub_functions: Vec<FnDoc>,
    pub_structs: Vec<StructDoc>,
    pub_enums: Vec<EnumDoc>,
    pub_traits: Vec<TraitDoc>,
    pub_types: Vec<TypeDoc>,
    pub_consts: Vec<ConstDoc>,
}

struct FnDoc {
    name: String,
    params: String,
    return_type: String,
    docs: String,
}

struct StructDoc {
    name: String,
    docs: String,
}

struct EnumDoc {
    name: String,
    docs: String,
}

struct TraitDoc {
    name: String,
    docs: String,
}

struct TypeDoc {
    name: String,
    docs: String,
}

struct ConstDoc {
    name: String,
    type_name: String,
    docs: String,
}

fn parse_module(file_path: &str, content: &str) -> ModuleDoc {
    let module_name = file_path
        .trim_end_matches(".rs")
        .split('/')
        .next_back()
        .unwrap_or(file_path)
        .to_string();

    let mut pub_functions = Vec::new();
    let mut pub_structs = Vec::new();
    let mut pub_enums = Vec::new();
    let mut pub_traits = Vec::new();
    let mut pub_types = Vec::new();
    let mut pub_consts = Vec::new();

    for cap in RE_PUB_FN.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let params = cap
            .get(2)
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let return_type = cap
            .get(3)
            .map(|m| m.as_str())
            .unwrap_or("()")
            .trim()
            .to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_functions.push(FnDoc {
            name,
            params,
            return_type,
            docs,
        });
    }

    for cap in RE_PUB_STRUCT.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_structs.push(StructDoc { name, docs });
    }

    for cap in RE_PUB_ENUM.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_enums.push(EnumDoc { name, docs });
    }

    for cap in RE_PUB_TRAIT.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_traits.push(TraitDoc { name, docs });
    }

    for cap in RE_PUB_TYPE.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_types.push(TypeDoc { name, docs });
    }

    for cap in RE_PUB_CONST.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        let type_name = cap
            .get(2)
            .map(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let line_num = content[..cap.get(0).unwrap().start()].lines().count();
        let docs = extract_doc_comments(content, line_num);
        pub_consts.push(ConstDoc {
            name,
            type_name,
            docs,
        });
    }

    ModuleDoc {
        file_path: file_path.to_string(),
        module_name,
        pub_functions,
        pub_structs,
        pub_enums,
        pub_traits,
        pub_types,
        pub_consts,
    }
}

impl ModuleDoc {
    fn to_json(&self) -> Value {
        json!({
            "file": self.file_path,
            "module": self.module_name,
            "functions": self.pub_functions.iter().map(|f| json!({
                "name": f.name,
                "params": f.params,
                "return_type": f.return_type,
                "docs": f.docs,
            })).collect::<Vec<_>>(),
            "structs": self.pub_structs.iter().map(|s| json!({"name": s.name, "docs": s.docs})).collect::<Vec<_>>(),
            "enums": self.pub_enums.iter().map(|e| json!({"name": e.name, "docs": e.docs})).collect::<Vec<_>>(),
            "traits": self.pub_traits.iter().map(|t| json!({"name": t.name, "docs": t.docs})).collect::<Vec<_>>(),
            "types": self.pub_types.iter().map(|t| json!({"name": t.name, "docs": t.docs})).collect::<Vec<_>>(),
            "consts": self.pub_consts.iter().map(|c| json!({"name": c.name, "type": c.type_name, "docs": c.docs})).collect::<Vec<_>>(),
        })
    }

    fn to_markdown(&self) -> String {
        let bq = "`";
        let mut md = String::new();
        md.push_str(&format!("## Module: {}{}{}\n\n", bq, self.module_name, bq));
        md.push_str(&format!("*Source: {}{}{}*\n\n", bq, self.file_path, bq));

        if !self.pub_functions.is_empty() {
            md.push_str("### Functions\n\n");
            for f in &self.pub_functions {
                md.push_str(&format!("#### {}{}{}\n\n", bq, f.name, bq));
                if !f.docs.is_empty() {
                    md.push_str(&format!("{}\n\n", f.docs));
                }
                let ret = if f.return_type == "()" {
                    String::new()
                } else {
                    format!(" -> {}", f.return_type)
                };
                md.push_str(&format!(
                    "```rust\npub fn {}({}){}\n```\n\n",
                    f.name, f.params, ret
                ));
            }
        }

        if !self.pub_structs.is_empty() {
            md.push_str("### Structs\n\n");
            for s in &self.pub_structs {
                md.push_str(&format!("#### {}{}{}\n\n", bq, s.name, bq));
                if !s.docs.is_empty() {
                    md.push_str(&format!("{}\n\n", s.docs));
                }
            }
        }

        if !self.pub_enums.is_empty() {
            md.push_str("### Enums\n\n");
            for e in &self.pub_enums {
                md.push_str(&format!("#### {}{}{}\n\n", bq, e.name, bq));
                if !e.docs.is_empty() {
                    md.push_str(&format!("{}\n\n", e.docs));
                }
            }
        }

        if !self.pub_traits.is_empty() {
            md.push_str("### Traits\n\n");
            for t in &self.pub_traits {
                md.push_str(&format!("#### {}{}{}\n\n", bq, t.name, bq));
                if !t.docs.is_empty() {
                    md.push_str(&format!("{}\n\n", t.docs));
                }
            }
        }

        if !self.pub_types.is_empty() {
            md.push_str("### Type Aliases\n\n");
            for t in &self.pub_types {
                md.push_str(&format!("- {}{}{}", bq, t.name, bq));
                if !t.docs.is_empty() {
                    md.push_str(&format!(" - {}", t.docs));
                }
                md.push('\n');
            }
            md.push('\n');
        }

        if !self.pub_consts.is_empty() {
            md.push_str("### Constants\n\n");
            for c in &self.pub_consts {
                md.push_str(&format!(
                    "- {}{}{}: {}{}{}",
                    bq, c.name, bq, bq, c.type_name, bq
                ));
                if !c.docs.is_empty() {
                    md.push_str(&format!(" - {}", c.docs));
                }
                md.push('\n');
            }
            md.push('\n');
        }

        md
    }
}

// ============================================================================
// README Generation
// ============================================================================

fn generate_readme(path: &str, project_name: Option<&str>) -> Result<Value, String> {
    let project_dir = Path::new(path);
    let cargo_toml = project_dir.join("Cargo.toml");

    let name = if let Some(n) = project_name {
        n.to_string()
    } else if cargo_toml.exists() {
        let content = read_file(&cargo_toml.to_string_lossy())?;
        content
            .lines()
            .find(|l| l.starts_with("name"))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| "my-project".to_string())
    } else {
        project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "my-project".to_string())
    };

    let description = if cargo_toml.exists() {
        let content = read_file(&cargo_toml.to_string_lossy()).ok();
        content
            .as_ref()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("description"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "A Rust project".to_string())
    } else {
        "A Rust project".to_string()
    };

    let version = if cargo_toml.exists() {
        let content = read_file(&cargo_toml.to_string_lossy()).ok();
        content
            .as_ref()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("version"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "0.1.0".to_string())
    } else {
        "0.1.0".to_string()
    };

    let edition = if cargo_toml.exists() {
        let content = read_file(&cargo_toml.to_string_lossy()).ok();
        content
            .as_ref()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.starts_with("edition"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "2021".to_string())
    } else {
        "2021".to_string()
    };

    let mut rust_files: Vec<String> = Vec::new();
    collect_rust_files(project_dir, &mut rust_files, true, 0).ok();
    let file_count = rust_files.len();

    let mut deps_section = String::new();
    let mut in_deps = false;
    if cargo_toml.exists() {
        if let Ok(content) = read_file(&cargo_toml.to_string_lossy()) {
            for line in content.lines() {
                if line.trim() == "[dependencies]" {
                    in_deps = true;
                    continue;
                }
                if line.starts_with('[') {
                    in_deps = false;
                    continue;
                }
                if in_deps && !line.trim().is_empty() && !line.starts_with('#') {
                    if let Some((dep_name, _)) = line.split_once('=') {
                        deps_section.push_str(&format!("- **{}**\n", dep_name.trim()));
                    }
                }
            }
        }
    }

    let readme = format!(
        r##"# {name}

{description}

## Overview

- **Version**: {version}
- **Edition**: {edition}
- **Source Files**: {file_count}

## Getting Started

### Prerequisites

- Rust {edition} or later
- Cargo package manager

### Installation

```bash
cargo build --release
```

### Usage

```bash
cargo run
```

## Project Structure

```
{name}/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point
│   └── lib.rs           # Library root
├── tests/               # Integration tests
└── README.md
```

## Dependencies

{deps_section}
## Development

### Running Tests

```bash
cargo test
```

### Code Style

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
```

## License

MIT
"##,
    );

    Ok(json!({
        "status": "ok",
        "action": "readme",
        "project": name,
        "version": version,
        "readme": readme,
    }))
}

// ============================================================================
// Architecture Documentation
// ============================================================================

fn generate_architecture(path: &str, recursive: bool) -> Result<Value, String> {
    let scan_path = Path::new(path);
    let mut files: Vec<String> = Vec::new();

    if scan_path.is_file() {
        files.push(path.to_string());
    } else {
        collect_rust_files(scan_path, &mut files, recursive, 0)?;
    }

    let mut modules: Vec<String> = Vec::new();
    let mut module_deps: Vec<(String, String)> = Vec::new();
    let mut external_crates: std::collections::HashSet<String> = std::collections::HashSet::new();

    for file_path in &files {
        let rel_path = file_path
            .strip_prefix("src/")
            .unwrap_or(file_path)
            .trim_end_matches(".rs")
            .replace('/', "::");

        if !modules.contains(&rel_path) {
            modules.push(rel_path.clone());
        }

        let content = read_file(file_path).unwrap_or_default();

        for cap in RE_MODULE_DECL.captures_iter(&content) {
            let mod_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let dep = format!("{rel_path}::{mod_name}");
            module_deps.push((rel_path.clone(), dep.clone()));
            if !modules.contains(&dep) {
                modules.push(dep);
            }
        }

        for cap in RE_USE_DECL.captures_iter(&content) {
            let use_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(first) = use_path.split("::").next() {
                let crate_name = first.trim();
                if !crate_name.is_empty() && !["self", "super", "crate"].contains(&crate_name) {
                    external_crates.insert(crate_name.to_string());
                }
            }
        }
    }

    let bq = "`";
    let mut mermaid = "```mermaid\ngraph TD\n".to_string();

    for module in &modules {
        let safe_name = module.replace("::", "_");
        mermaid.push_str(&format!("    {}[\"{}{}{}\"]\n", safe_name, bq, module, bq));
    }

    for (from, to) in &module_deps {
        let safe_from = from.replace("::", "_");
        let safe_to = to.replace("::", "_");
        mermaid.push_str(&format!("    {} --> {}\n", safe_from, safe_to));
    }

    if !external_crates.is_empty() {
        mermaid.push_str("\n    subgraph External\n");
        for ext in &external_crates {
            let safe_ext = format!("ext_{ext}");
            mermaid.push_str(&format!(
                "        {}[[\"{}{}{}\"]]\n",
                safe_ext, bq, ext, bq
            ));
        }
        mermaid.push_str("    end\n");
    }

    mermaid.push_str("```\n");

    let mut desc = String::new();
    desc.push_str("# Architecture Documentation\n\n");
    desc.push_str("## Module Overview\n\n");
    for module in &modules {
        let file = files
            .iter()
            .find(|f| {
                f.strip_prefix("src/")
                    .unwrap_or(f)
                    .trim_end_matches(".rs")
                    .replace('/', "::")
                    == *module
            })
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let dep_count = module_deps.iter().filter(|(src, _)| src == module).count();
        desc.push_str(&format!(
            "- **{}** (`{}`) - {} submodules\n",
            module, file, dep_count
        ));
    }

    if !external_crates.is_empty() {
        desc.push_str("\n## External Dependencies\n\n");
        for ext in &external_crates {
            desc.push_str(&format!("- `{}`\n", ext));
        }
    }

    desc.push_str("\n## Summary\n\n");
    desc.push_str(&format!("- Total modules: {}\n", modules.len()));
    desc.push_str(&format!("- Module dependencies: {}\n", module_deps.len()));
    desc.push_str(&format!("- External crates: {}\n", external_crates.len()));

    Ok(json!({
        "status": "ok",
        "action": "architecture",
        "modules": modules.len(),
        "dependencies": module_deps.len(),
        "external_crates": external_crates.into_iter().collect::<Vec<_>>(),
        "mermaid_diagram": mermaid,
        "documentation": desc,
    }))
}

// ============================================================================
// Module Documentation
// ============================================================================

fn generate_module_doc(path: &str, recursive: bool, format: &str) -> Result<Value, String> {
    let scan_path = Path::new(path);
    let mut files: Vec<String> = Vec::new();

    if scan_path.is_file() {
        files.push(path.to_string());
    } else {
        collect_rust_files(scan_path, &mut files, recursive, 0)?;
    }

    let mut module_docs: Vec<Value> = Vec::new();

    for file_path in &files {
        let content = read_file(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut module_docs_text = Vec::new();
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with("//!") {
                module_docs_text.push(
                    trimmed
                        .strip_prefix("//!")
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string(),
                );
            } else if trimmed.is_empty() {
                continue;
            } else {
                break;
            }
        }

        let fn_count = RE_PUB_FN.captures_iter(&content).count();
        let struct_count = RE_PUB_STRUCT.captures_iter(&content).count();
        let enum_count = RE_PUB_ENUM.captures_iter(&content).count();
        let trait_count = RE_PUB_TRAIT.captures_iter(&content).count();
        let test_count = content.lines().filter(|l| l.contains("#[test]")).count();

        let rel_path = file_path
            .strip_prefix("src/")
            .unwrap_or(file_path)
            .to_string();

        module_docs.push(json!({
            "file": rel_path,
            "module_docs": module_docs_text.join("\n"),
            "stats": {
                "functions": fn_count,
                "structs": struct_count,
                "enums": enum_count,
                "traits": trait_count,
                "tests": test_count,
            },
        }));
    }

    if format == "json" {
        Ok(json!({
            "status": "ok",
            "action": "module_doc",
            "files": files.len(),
            "modules": module_docs,
        }))
    } else {
        let mut md = String::new();
        md.push_str("# Module Documentation\n\n");
        for m in &module_docs {
            let file = m["file"].as_str().unwrap_or("");
            md.push_str(&format!("## `{file}`\n\n"));
            let docs = m["module_docs"].as_str().unwrap_or("");
            if !docs.is_empty() {
                md.push_str(&format!("{docs}\n\n"));
            }
            let stats = &m["stats"];
            md.push_str("| Item | Count |\n|------|-------|\n");
            md.push_str(&format!(
                "| Functions | {} |\n",
                stats["functions"].as_u64().unwrap_or(0)
            ));
            md.push_str(&format!(
                "| Structs | {} |\n",
                stats["structs"].as_u64().unwrap_or(0)
            ));
            md.push_str(&format!(
                "| Enums | {} |\n",
                stats["enums"].as_u64().unwrap_or(0)
            ));
            md.push_str(&format!(
                "| Traits | {} |\n",
                stats["traits"].as_u64().unwrap_or(0)
            ));
            md.push_str(&format!(
                "| Tests | {} |\n\n",
                stats["tests"].as_u64().unwrap_or(0)
            ));
        }

        Ok(json!({
            "status": "ok",
            "action": "module_doc",
            "files": files.len(),
            "documentation": md,
        }))
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DocGeneratorTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_pub_fn() {
        assert!(RE_PUB_FN.is_match("pub fn hello() -> String"));
        assert!(RE_PUB_FN.is_match("pub async fn fetch(url: &str) -> Result<Response>"));
        assert!(!RE_PUB_FN.is_match("fn private()"));
    }

    #[test]
    fn regex_pub_struct() {
        assert!(RE_PUB_STRUCT.is_match("pub struct User {"));
        assert!(!RE_PUB_STRUCT.is_match("struct Private"));
    }

    #[test]
    fn regex_doc_comment() {
        assert!(RE_DOC_COMMENT.is_match("/// This is a doc comment"));
        assert!(RE_DOC_COMMENT.is_match("    /// Indented doc"));
    }
}
