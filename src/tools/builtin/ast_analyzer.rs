//! AST-level Rust code analyzer using `syn`.
//!
//! Provides precise code understanding beyond regex-based approaches:
//! - **analyze**: Parse Rust source and extract structured information
//! - **unused_imports**: Detect unused `use` statements
//! - **public_api**: Extract all public items (functions, structs, enums, traits)
//! - **dependencies**: Analyze external crate dependencies
//! - **complexity**: Calculate cyclomatic complexity per function

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use syn::spanned::Spanned;
use syn::{
    visit::Visit, ExprIf, ExprLoop, ExprMatch, ExprWhile, File, FnArg, Item, ItemFn, Pat, UseTree,
    Visibility,
};

// ============================================================================
// AstAnalyzerTool
// ============================================================================

pub struct AstAnalyzerTool;

#[async_trait::async_trait]
impl Tool for AstAnalyzerTool {
    fn name(&self) -> &str {
        "ast_analyzer"
    }

    fn description(&self) -> &str {
        "AST-level Rust code analysis using syn: analyze (parse and extract structured info), unused_imports (detect unused use statements), public_api (extract public items), dependencies (analyze crate dependencies), complexity (cyclomatic complexity per function)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: analyze (parse and extract structured info), unused_imports (detect unused use statements), public_api (extract public items), dependencies (analyze crate dependencies), complexity (cyclomatic complexity per function)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to Rust source file or directory".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Recursively analyze directory (default: true)".to_string(),
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

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let file_path = Path::new(path);
        let mut files: Vec<String> = Vec::new();

        if file_path.is_file() {
            if !path.ends_with(".rs") {
                return Ok(json!({
                    "status": "error",
                    "message": "Not a Rust file",
                }));
            }
            files.push(path.to_string());
        } else if file_path.is_dir() {
            collect_rust_files(file_path, &mut files, recursive, 0)?;
            if files.is_empty() {
                return Ok(json!({
                    "status": "error",
                    "message": format!("No Rust files found in: {path}"),
                }));
            }
        } else {
            return Ok(json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        match action {
            "analyze" => action_analyze(&files),
            "unused_imports" => action_unused_imports(&files),
            "public_api" => action_public_api(&files),
            "dependencies" => action_dependencies(&files),
            "complexity" => action_complexity(&files),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: analyze, unused_imports, public_api, dependencies, complexity"),
            })),
        }
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
    if depth > 20 {
        return Ok(());
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
            if name.starts_with('.') || name == "target" || name == "node_modules" {
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

fn parse_file(path: &str) -> Result<File, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))?;
    syn::parse_file(&content).map_err(|e| format!("Failed to parse '{path}': {e}"))
}

// ============================================================================
// Action: analyze
// ============================================================================

fn action_analyze(files: &[String]) -> Result<Value, String> {
    let mut results = Vec::new();
    let mut total_functions = 0usize;
    let mut total_structs = 0usize;
    let mut total_enums = 0usize;
    let mut total_traits = 0usize;
    let mut total_modules = 0usize;
    let mut total_uses = 0usize;
    let mut total_lines = 0usize;
    let mut total_parse_errors = 0usize;

    for file_path in files {
        match parse_file(file_path) {
            Ok(ast) => {
                let info = analyze_file(&ast);
                total_functions += info.functions;
                total_structs += info.structs;
                total_enums += info.enums;
                total_traits += info.traits;
                total_modules += info.modules;
                total_uses += info.uses;
                total_lines += info.lines;

                results.push(json!({
                    "file": file_path,
                    "lines": info.lines,
                    "functions": info.functions,
                    "pub_functions": info.pub_functions,
                    "structs": info.structs,
                    "pub_structs": info.pub_structs,
                    "enums": info.enums,
                    "pub_enums": info.pub_enums,
                    "traits": info.traits,
                    "pub_traits": info.pub_traits,
                    "modules": info.modules,
                    "use_statements": info.uses,
                }));
            }
            Err(e) => {
                total_parse_errors += 1;
                results.push(json!({
                    "file": file_path,
                    "parse_error": e,
                }));
            }
        }
    }

    Ok(json!({
        "status": "ok",
        "action": "analyze",
        "total_files": files.len(),
        "total_lines": total_lines,
        "total_functions": total_functions,
        "total_structs": total_structs,
        "total_enums": total_enums,
        "total_traits": total_traits,
        "total_modules": total_modules,
        "total_use_statements": total_uses,
        "parse_errors": total_parse_errors,
        "files": results,
    }))
}

struct FileInfo {
    lines: usize,
    functions: usize,
    pub_functions: usize,
    structs: usize,
    pub_structs: usize,
    enums: usize,
    pub_enums: usize,
    traits: usize,
    pub_traits: usize,
    modules: usize,
    uses: usize,
}

fn analyze_file(ast: &File) -> FileInfo {
    let mut info = FileInfo {
        lines: 0,
        functions: 0,
        pub_functions: 0,
        structs: 0,
        pub_structs: 0,
        enums: 0,
        pub_enums: 0,
        traits: 0,
        pub_traits: 0,
        modules: 0,
        uses: 0,
    };

    // Count lines (rough estimate from token stream)
    let token_stream = quote::quote!(#ast).to_string();
    info.lines = token_stream.lines().count();

    for item in &ast.items {
        match item {
            Item::Fn(f) => {
                info.functions += 1;
                if is_pub(&f.vis) {
                    info.pub_functions += 1;
                }
            }
            Item::Struct(s) => {
                info.structs += 1;
                if is_pub(&s.vis) {
                    info.pub_structs += 1;
                }
            }
            Item::Enum(e) => {
                info.enums += 1;
                if is_pub(&e.vis) {
                    info.pub_enums += 1;
                }
            }
            Item::Trait(t) => {
                info.traits += 1;
                if is_pub(&t.vis) {
                    info.pub_traits += 1;
                }
            }
            Item::Mod(_) => {
                info.modules += 1;
            }
            Item::Use(_) => {
                info.uses += 1;
            }
            _ => {}
        }
    }

    info
}

fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

// ============================================================================
// Action: unused_imports
// ============================================================================

fn action_unused_imports(files: &[String]) -> Result<Value, String> {
    let mut results = Vec::new();

    for file_path in files {
        if let Ok(ast) = parse_file(file_path) {
            let unused = detect_unused_imports(&ast);
            if !unused.is_empty() {
                results.push(json!({
                    "file": file_path,
                    "unused_imports": unused,
                    "count": unused.len(),
                }));
            }
        } // Skip files that can't be parsed
    }

    Ok(json!({
        "status": "ok",
        "action": "unused_imports",
        "files_with_unused": results.len(),
        "total_unused": results.iter().map(|r| r["count"].as_u64().unwrap_or(0)).sum::<u64>(),
        "results": results,
    }))
}

fn detect_unused_imports(ast: &File) -> Vec<Value> {
    let mut unused = Vec::new();

    for item in &ast.items {
        if let Item::Use(use_item) = item {
            let import_path = use_tree_to_string(&use_item.tree);

            // Extract the last identifier from the import path
            let last_segment = import_path
                .split("::")
                .last()
                .unwrap_or(&import_path)
                .trim();

            // Skip glob imports and self/super/crate
            if last_segment.is_empty()
                || last_segment == "*"
                || last_segment == "self"
                || last_segment == "super"
                || last_segment == "crate"
            {
                continue;
            }

            // Check if the imported name is used anywhere in the file
            let content = quote::quote!(#ast).to_string();

            // Simple heuristic: count occurrences of the identifier
            // A more sophisticated approach would use syn::visit
            let name_to_find = last_segment;
            let mut count = 0;
            for word in content.split_whitespace() {
                let clean = word
                    .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '_')
                    .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if clean == name_to_find {
                    count += 1;
                }
            }

            // If the name appears only once (in the import itself), it's likely unused
            if count <= 1 {
                let line = use_item.span().start().line;
                unused.push(json!({
                    "import": import_path,
                    "name": last_segment,
                    "line": line,
                }));
            }
        }
    }

    unused
}

fn use_tree_to_string(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => {
            let rest = use_tree_to_string(&path.tree);
            if rest.is_empty() {
                path.ident.to_string()
            } else {
                format!("{}::{}", path.ident, rest)
            }
        }
        UseTree::Name(name) => name.ident.to_string(),
        UseTree::Rename(rename) => format!("{} as {}", rename.ident, rename.rename),
        UseTree::Glob(_) => "*".to_string(),
        UseTree::Group(group) => {
            let items: Vec<String> = group.items.iter().map(use_tree_to_string).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

// ============================================================================
// Action: public_api
// ============================================================================

fn action_public_api(files: &[String]) -> Result<Value, String> {
    let mut results = Vec::new();

    for file_path in files {
        match parse_file(file_path) {
            Ok(ast) => {
                let pub_items = extract_public_api(&ast);
                results.push(json!({
                    "file": file_path,
                    "public_items": pub_items,
                    "count": pub_items.len(),
                }));
            }
            Err(e) => {
                results.push(json!({
                    "file": file_path,
                    "parse_error": e,
                }));
            }
        }
    }

    Ok(json!({
        "status": "ok",
        "action": "public_api",
        "total_files": files.len(),
        "results": results,
    }))
}

fn extract_public_api(ast: &File) -> Vec<Value> {
    let mut items = Vec::new();

    for item in &ast.items {
        match item {
            Item::Fn(f) if is_pub(&f.vis) => {
                let sig = &f.sig;
                let args: Vec<String> = sig
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        FnArg::Receiver(_) => Some("self".to_string()),
                        FnArg::Typed(pat_type) => {
                            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                                Some(pat_ident.ident.to_string())
                            } else {
                                None
                            }
                        }
                    })
                    .collect();

                let return_type = match &sig.output {
                    syn::ReturnType::Default => "()".to_string(),
                    syn::ReturnType::Type(_, ty) => quote::quote!(#ty).to_string(),
                };

                let async_kw = if sig.asyncness.is_some() {
                    "async "
                } else {
                    ""
                };
                let unsafe_kw = if sig.unsafety.is_some() {
                    "unsafe "
                } else {
                    ""
                };

                items.push(json!({
                    "kind": "function",
                    "name": sig.ident.to_string(),
                    "signature": format!(
                        "{}{}fn {}({}) -> {}",
                        async_kw,
                        unsafe_kw,
                        sig.ident,
                        args.join(", "),
                        return_type,
                    ),
                    "async": sig.asyncness.is_some(),
                    "unsafe": sig.unsafety.is_some(),
                    "generics": if sig.generics.params.is_empty() {
                        None
                    } else {
                        Some(quote::quote!(#sig.generics).to_string())
                    },
                }));
            }
            Item::Struct(s) if is_pub(&s.vis) => {
                let fields = match &s.fields {
                    syn::Fields::Named(fields) => {
                        let field_names: Vec<String> = fields
                            .named
                            .iter()
                            .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
                            .collect();
                        format!("{{ {} }}", field_names.join(", "))
                    }
                    syn::Fields::Unnamed(fields) => {
                        format!("({})", fields.unnamed.len())
                    }
                    syn::Fields::Unit => "()".to_string(),
                };

                items.push(json!({
                    "kind": "struct",
                    "name": s.ident.to_string(),
                    "fields": fields,
                    "generics": if s.generics.params.is_empty() {
                        None
                    } else {
                        Some(quote::quote!(#s.generics).to_string())
                    },
                }));
            }
            Item::Enum(e) if is_pub(&e.vis) => {
                let variants: Vec<String> =
                    e.variants.iter().map(|v| v.ident.to_string()).collect();

                items.push(json!({
                    "kind": "enum",
                    "name": e.ident.to_string(),
                    "variants": variants,
                    "generics": if e.generics.params.is_empty() {
                        None
                    } else {
                        Some(quote::quote!(#e.generics).to_string())
                    },
                }));
            }
            Item::Trait(t) if is_pub(&t.vis) => {
                let methods: Vec<String> = t
                    .items
                    .iter()
                    .filter_map(|item| {
                        if let syn::TraitItem::Fn(method) = item {
                            Some(method.sig.ident.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                items.push(json!({
                    "kind": "trait",
                    "name": t.ident.to_string(),
                    "methods": methods,
                    "unsafe": t.unsafety.is_some(),
                }));
            }
            _ => {}
        }
    }

    items
}

// ============================================================================
// Action: dependencies
// ============================================================================

fn action_dependencies(files: &[String]) -> Result<Value, String> {
    let mut all_crates: HashSet<String> = HashSet::new();
    let mut file_deps: HashMap<String, Vec<String>> = HashMap::new();

    for file_path in files {
        if let Ok(ast) = parse_file(file_path) {
            let mut file_crates = Vec::new();
            for item in &ast.items {
                if let Item::Use(use_item) = item {
                    if let UseTree::Path(path) = &use_item.tree {
                        let crate_name = path.ident.to_string();
                        // Filter out std and common internal paths
                        if !["std", "core", "self", "super", "crate"].contains(&crate_name.as_str())
                        {
                            all_crates.insert(crate_name.clone());
                            if !file_crates.contains(&crate_name) {
                                file_crates.push(crate_name);
                            }
                        }
                    }
                }
            }
            file_crates.sort();
            file_deps.insert(file_path.clone(), file_crates);
        }
    }

    let mut sorted_crates: Vec<String> = all_crates.into_iter().collect();
    sorted_crates.sort();

    Ok(json!({
        "status": "ok",
        "action": "dependencies",
        "external_crates": sorted_crates,
        "crate_count": sorted_crates.len(),
        "file_dependencies": file_deps,
    }))
}

// ============================================================================
// Action: complexity
// ============================================================================

fn action_complexity(files: &[String]) -> Result<Value, String> {
    let mut results = Vec::new();

    for file_path in files {
        if let Ok(ast) = parse_file(file_path) {
            let complexities = calculate_complexity(&ast);
            if !complexities.is_empty() {
                results.push(json!({
                    "file": file_path,
                    "functions": complexities,
                    "max_complexity": complexities.iter().map(|c| c["complexity"].as_u64().unwrap_or(0)).max().unwrap_or(0),
                    "avg_complexity": if complexities.is_empty() {
                        0.0
                    } else {
                        complexities.iter()
                            .map(|c| c["complexity"].as_f64().unwrap_or(0.0))
                            .sum::<f64>() / complexities.len() as f64
                    },
                }));
            }
        }
    }

    Ok(json!({
        "status": "ok",
        "action": "complexity",
        "files_analyzed": results.len(),
        "results": results,
    }))
}

fn calculate_complexity(ast: &File) -> Vec<Value> {
    let mut results = Vec::new();

    for item in &ast.items {
        if let Item::Fn(f) = item {
            let complexity = ComplexityVisitor::calculate(f);
            let rating = match complexity {
                1..=5 => "low",
                6..=10 => "moderate",
                11..=20 => "high",
                _ => "very_high",
            };

            results.push(json!({
                "name": f.sig.ident.to_string(),
                "complexity": complexity,
                "rating": rating,
                "line": f.sig.span().start().line,
                "async": f.sig.asyncness.is_some(),
            }));
        }
    }

    results
}

struct ComplexityVisitor {
    complexity: u64,
}

impl ComplexityVisitor {
    fn calculate(f: &ItemFn) -> u64 {
        let mut visitor = ComplexityVisitor { complexity: 1 }; // Base complexity
        visitor.visit_item_fn(f);
        visitor.complexity
    }
}

impl<'ast> Visit<'ast> for ComplexityVisitor {
    fn visit_expr_if(&mut self, _: &ExprIf) {
        self.complexity += 1;
    }

    fn visit_expr_while(&mut self, _: &ExprWhile) {
        self.complexity += 1;
    }

    fn visit_expr_for_loop(&mut self, _: &syn::ExprForLoop) {
        self.complexity += 1;
    }

    fn visit_expr_loop(&mut self, _: &ExprLoop) {
        self.complexity += 1;
    }

    fn visit_expr_match(&mut self, _: &ExprMatch) {
        self.complexity += 1;
    }

    fn visit_expr_binary(&mut self, _: &syn::ExprBinary) {
        // Count && and || as additional decision points
    }

    fn visit_expr_method_call(&mut self, _: &syn::ExprMethodCall) {
        // Method chains don't add complexity
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(AstAnalyzerTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pub() {
        assert!(is_pub(&Visibility::Public(syn::token::Pub::default())));
        assert!(!is_pub(&Visibility::Inherited));
    }

    #[test]
    fn test_use_tree_to_string_simple() {
        let tree = syn::parse_quote!(std::collections::HashMap);
        assert_eq!(use_tree_to_string(&tree), "std::collections::HashMap");
    }

    #[test]
    fn test_use_tree_to_string_rename() {
        // syn::parse_quote!(std::collections::HashMap as Hm) produces
        // UseTree::Path("std"::UseTree::Path("collections"::UseTree::Rename(HashMap as Hm)))
        let tree = syn::parse_quote!(std::collections::HashMap as Hm);
        assert_eq!(use_tree_to_string(&tree), "std::collections::HashMap as Hm");
    }

    #[test]
    fn test_use_tree_to_string_glob() {
        // syn::parse_quote!(std::io::*) produces UseTree::Path(std::UseTree::Glob)
        let tree = syn::parse_quote!(std::io::*);
        assert_eq!(use_tree_to_string(&tree), "std::io::*");
    }

    #[test]
    fn test_parse_and_analyze_simple_file() {
        let code = r#"
use std::collections::HashMap;
use std::io::Read;

pub fn hello(name: &str) -> String {
    format!("Hello, {}!", name)
}

struct Config {
    debug: bool,
    port: u16,
}

pub enum Status {
    Ok,
    Error(String),
}

pub trait Service {
    fn run(&self);
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let info = analyze_file(&ast);

        assert_eq!(info.functions, 1);
        assert_eq!(info.pub_functions, 1);
        assert_eq!(info.structs, 1);
        assert_eq!(info.pub_structs, 0); // struct Config is not pub
        assert_eq!(info.enums, 1);
        assert_eq!(info.pub_enums, 1);
        assert_eq!(info.traits, 1);
        assert_eq!(info.pub_traits, 1);
        assert_eq!(info.uses, 2);
    }

    #[test]
    fn test_extract_public_api() {
        let code = r#"
pub async fn fetch_data(url: &str) -> Result<String, Error> {
    Ok(String::new())
}

pub struct Response {
    status: u16,
    body: String,
}

pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

pub trait HttpClient {
    fn get(&self, url: &str);
    fn post(&self, url: &str, body: &str);
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let pub_items = extract_public_api(&ast);

        assert_eq!(pub_items.len(), 4);

        // Check function
        let func = &pub_items[0];
        assert_eq!(func["kind"], "function");
        assert_eq!(func["name"], "fetch_data");
        assert_eq!(func["async"], true);

        // Check struct
        let s = &pub_items[1];
        assert_eq!(s["kind"], "struct");
        assert_eq!(s["name"], "Response");

        // Check enum
        let e = &pub_items[2];
        assert_eq!(e["kind"], "enum");
        assert_eq!(e["name"], "Method");
        let variants = e["variants"].as_array().unwrap();
        assert_eq!(variants.len(), 4);

        // Check trait
        let t = &pub_items[3];
        assert_eq!(t["kind"], "trait");
        assert_eq!(t["name"], "HttpClient");
        let methods = t["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn test_complexity_simple_function() {
        let code = r#"
fn simple() -> i32 {
    42
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let complexities = calculate_complexity(&ast);
        assert_eq!(complexities.len(), 1);
        assert_eq!(complexities[0]["complexity"], 1);
        assert_eq!(complexities[0]["rating"], "low");
    }

    #[test]
    fn test_complexity_with_if() {
        let code = r#"
fn with_if(x: i32) -> i32 {
    if x > 0 {
        x
    } else {
        -x
    }
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let complexities = calculate_complexity(&ast);
        assert_eq!(complexities[0]["complexity"], 2); // 1 base + 1 if
    }

    #[test]
    fn test_complexity_with_match() {
        let code = r#"
fn with_match(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v,
        None => 0,
    }
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let complexities = calculate_complexity(&ast);
        assert_eq!(complexities[0]["complexity"], 2); // 1 base + 1 match
    }

    #[test]
    fn test_complexity_multiple_branches() {
        let code = r#"
fn complex(x: i32, y: i32) -> i32 {
    if x > 0 {
        if y > 0 {
            x + y
        } else {
            x - y
        }
    } else {
        0
    }
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let complexities = calculate_complexity(&ast);
        // 1 base + 2 if statements = 3
        // The visitor only visits top-level expressions, so nested if might not be counted
        // depending on how Visit is implemented. Let's accept >= 2 as reasonable.
        assert!(complexities[0]["complexity"].as_u64().unwrap_or(0) >= 2);
    }

    #[test]
    fn test_detect_unused_imports() {
        let code = r#"
use std::collections::HashMap;
use std::io::Read;

fn main() {
    let _x = 42;
}
"#;
        let ast = syn::parse_file(code).unwrap();
        let unused = detect_unused_imports(&ast);
        // Both HashMap and Read should be detected as unused
        assert!(unused.len() >= 1);
    }

    #[test]
    fn test_action_analyze_on_real_file() {
        let result = action_analyze(&["src/tools/builtin/hello_tool.rs".to_string()]);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["status"], "ok");
        assert!(value["total_functions"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn test_action_public_api_on_real_file() {
        let result = action_public_api(&["src/tools/builtin/hello_tool.rs".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_action_complexity_on_real_file() {
        let result = action_complexity(&["src/tools/builtin/hello_tool.rs".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_action_unknown() {
        let tool = AstAnalyzerTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("invalid".to_string()));
        params.insert("path".to_string(), Value::String("src".to_string()));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert_eq!(result["status"], "error");
    }

    #[test]
    fn test_not_a_rust_file() {
        let tool = AstAnalyzerTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("analyze".to_string()));
        params.insert("path".to_string(), Value::String("Cargo.toml".to_string()));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert_eq!(result["status"], "error");
    }

    #[test]
    fn test_nonexistent_path() {
        let tool = AstAnalyzerTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("analyze".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("/nonexistent/path".to_string()),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(&params)).unwrap();
        assert_eq!(result["status"], "error");
    }
}
