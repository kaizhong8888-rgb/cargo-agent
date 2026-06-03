//! Code quality analysis tool: provides comprehensive code quality scoring,
//! duplicate code detection, and dependency visualization.
//!
//! # Features
//!
//! - **quality_score**: Calculate code quality score (0-100) based on multiple metrics
//! - **duplicate_detection**: Find duplicate/similar code blocks
//! - **dependency_viz**: Generate Mermaid dependency diagrams

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Pre-compiled regex patterns
// ============================================================================

static RE_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]"#)
        .expect("valid regex")
});

static RE_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).expect("valid regex")
});

static RE_UNWRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\.unwrap\(\)"#).expect("valid regex"));

static RE_UNSAFE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"unsafe\s*\{"#).expect("valid regex"));

static RE_TODO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)//\s*(TODO|FIXME|HACK|XXX)"#).expect("valid regex"));

static RE_DOC_COMMENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*///"#).expect("valid regex"));

static RE_USE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*use\s+([a-zA-Z_][a-zA-Z0-9_:*{}\s]*(?:;|$))"#).expect("valid regex")
});

static RE_MOD_DECL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\{|;)"#)
        .expect("valid regex")
});

static RE_TEST_ATTR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*#\[test\]"#).expect("valid regex"));

// ============================================================================
// CodeQualityTool
// ============================================================================

pub struct CodeQualityTool;

#[async_trait::async_trait]
impl Tool for CodeQualityTool {
    fn name(&self) -> &str {
        "code_quality"
    }

    fn description(&self) -> &str {
        "Comprehensive code quality analysis: quality scoring (0-100), duplicate code detection, dependency visualization. Actions: score (quality score with breakdown), duplicates (find similar code), viz_deps (Mermaid dependency diagram)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Analysis type: score, duplicates, viz_deps".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to Rust file or directory".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "recursive".to_string(),
                description: "Recursively analyze directory (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "min_dup_lines".to_string(),
                description: "Minimum lines for duplicate detection (default: 5)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "title".to_string(),
                description: "Title for dependency diagram (default: 'Dependencies')".to_string(),
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

        let recursive = params
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let min_dup_lines = params
            .get("min_dup_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Dependencies");

        let file_path = Path::new(path);
        if !file_path.exists() {
            return Ok(json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        let mut files: Vec<String> = Vec::new();
        if file_path.is_file() {
            if !path.ends_with(".rs") {
                return Ok(json!({
                    "status": "error",
                    "message": "Not a Rust file",
                }));
            }
            files.push(path.to_string());
        } else {
            collect_rust_files(file_path, &mut files, recursive, 0)?;
            if files.is_empty() {
                return Ok(json!({
                    "status": "error",
                    "message": format!("No Rust files found in: {path}"),
                }));
            }
        }

        match action {
            "score" => analyze_quality_score(&files),
            "duplicates" => detect_duplicates(&files, min_dup_lines),
            "viz_deps" => generate_dependency_viz(&files, title),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: score, duplicates, viz_deps"),
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

fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{path}': {e}"))
}

// ============================================================================
// Quality Scoring
// ============================================================================

struct QualityMetrics {
    doc_coverage: f64,     // 0-1
    test_coverage: f64,    // 0-1
    unwrap_ratio: f64,     // 0-1 (lower is better)
    unsafe_ratio: f64,     // 0-1 (lower is better)
    todo_ratio: f64,       // 0-1 (lower is better)
    avg_fn_length: f64,    // lines per function
    complexity_score: f64, // 0-1 (lower is better)
    comment_ratio: f64,    // 0-1
}

fn calculate_metrics(content: &str) -> QualityMetrics {
    let total_lines = content.lines().count() as f64;
    if total_lines == 0.0 {
        return QualityMetrics {
            doc_coverage: 0.0,
            test_coverage: 0.0,
            unwrap_ratio: 0.0,
            unsafe_ratio: 0.0,
            todo_ratio: 0.0,
            avg_fn_length: 0.0,
            complexity_score: 0.0,
            comment_ratio: 0.0,
        };
    }

    let fn_count = RE_FN.find_iter(content).count() as f64;
    let struct_count = RE_STRUCT.find_iter(content).count() as f64;
    let doc_lines = RE_DOC_COMMENT.find_iter(content).count() as f64;
    let test_count = RE_TEST_ATTR.find_iter(content).count() as f64;
    let unwrap_count = RE_UNWRAP.find_iter(content).count() as f64;
    let unsafe_count = RE_UNSAFE.find_iter(content).count() as f64;
    let todo_count = RE_TODO.find_iter(content).count() as f64;

    // Doc coverage: ratio of documented items (estimate)
    let total_items = fn_count + struct_count;
    let doc_coverage = if total_items > 0.0 {
        (doc_lines / total_items).min(1.0)
    } else {
        0.0
    };

    // Test coverage: tests per function
    let test_coverage = if fn_count > 0.0 {
        (test_count / fn_count).min(1.0)
    } else {
        0.0
    };

    // Unwrap ratio
    let unwrap_ratio = if fn_count > 0.0 {
        unwrap_count / fn_count
    } else {
        0.0
    };

    // Unsafe ratio
    let unsafe_ratio = if total_lines > 0.0 {
        unsafe_count / total_lines
    } else {
        0.0
    };

    // TODO ratio
    let todo_ratio = if total_lines > 0.0 {
        todo_count / total_lines
    } else {
        0.0
    };

    // Average function length (rough estimate)
    let avg_fn_length = if fn_count > 0.0 {
        total_lines / fn_count
    } else {
        0.0
    };

    // Complexity score (based on nesting)
    let mut max_nesting = 0usize;
    let mut current = 0usize;
    for ch in content.chars() {
        match ch {
            '{' | '[' => {
                current += 1;
                max_nesting = max_nesting.max(current);
            }
            '}' | ']' => {
                current = current.saturating_sub(1);
            }
            _ => {}
        }
    }
    let complexity_score = (max_nesting as f64 / 10.0).min(1.0);

    // Comment ratio
    let comment_lines = content
        .lines()
        .filter(|l| l.trim().starts_with("//"))
        .count() as f64;
    let comment_ratio = (comment_lines / total_lines).min(1.0);

    QualityMetrics {
        doc_coverage,
        test_coverage,
        unwrap_ratio,
        unsafe_ratio,
        todo_ratio,
        avg_fn_length,
        complexity_score,
        comment_ratio,
    }
}

fn score_from_metrics(m: &QualityMetrics) -> f64 {
    // Weighted scoring (100 points total)
    let mut score = 0.0;

    // Documentation (20 points)
    score += m.doc_coverage * 20.0;

    // Testing (25 points)
    score += m.test_coverage * 25.0;

    // Safety - unwrap (15 points)
    let unwrap_score = if m.unwrap_ratio < 0.1 {
        1.0
    } else if m.unwrap_ratio < 0.3 {
        0.7
    } else if m.unwrap_ratio < 0.5 {
        0.4
    } else {
        0.1
    };
    score += unwrap_score * 15.0;

    // Safety - unsafe (10 points)
    let unsafe_score = if m.unsafe_ratio < 0.01 {
        1.0
    } else if m.unsafe_ratio < 0.05 {
        0.7
    } else {
        0.3
    };
    score += unsafe_score * 10.0;

    // Completeness - TODOs (10 points)
    let todo_score = if m.todo_ratio < 0.01 {
        1.0
    } else if m.todo_ratio < 0.05 {
        0.6
    } else {
        0.2
    };
    score += todo_score * 10.0;

    // Maintainability - function length (10 points)
    let fn_len_score = if m.avg_fn_length < 20.0 {
        1.0
    } else if m.avg_fn_length < 50.0 {
        0.7
    } else if m.avg_fn_length < 100.0 {
        0.4
    } else {
        0.1
    };
    score += fn_len_score * 10.0;

    // Complexity (10 points)
    score += (1.0 - m.complexity_score) * 10.0;

    score.round()
}

fn analyze_quality_score(files: &[String]) -> Result<Value, String> {
    let mut file_scores = Vec::new();
    let mut total_score = 0.0;
    let mut total_lines = 0usize;

    for file_path in files {
        let content = read_file(file_path)?;
        let metrics = calculate_metrics(&content);
        let score = score_from_metrics(&metrics);
        let lines = content.lines().count();
        total_score += score;
        total_lines += lines;

        file_scores.push(json!({
            "file": file_path,
            "score": score,
            "lines": lines,
            "metrics": {
                "doc_coverage": format!("{:.0}%", metrics.doc_coverage * 100.0),
                "test_coverage": format!("{:.0}%", metrics.test_coverage * 100.0),
                "unwrap_count": RE_UNWRAP.find_iter(&content).count(),
                "unsafe_blocks": RE_UNSAFE.find_iter(&content).count(),
                "todo_count": RE_TODO.find_iter(&content).count(),
                "avg_function_length": format!("{:.1} lines", metrics.avg_fn_length),
                "max_nesting_depth": format!("{:.0}", metrics.complexity_score * 10.0),
                "comment_ratio": format!("{:.0}%", metrics.comment_ratio * 100.0),
            },
            "grade": match score as u64 {
                90..=100 => "A+",
                80..=89 => "A",
                70..=79 => "B",
                60..=69 => "C",
                50..=59 => "D",
                _ => "F",
            },
        }));
    }

    let avg_score = if !file_scores.is_empty() {
        total_score / file_scores.len() as f64
    } else {
        0.0
    };

    let overall_grade = match avg_score as u64 {
        90..=100 => "A+",
        80..=89 => "A",
        70..=79 => "B",
        60..=69 => "C",
        50..=59 => "D",
        _ => "F",
    };

    Ok(json!({
        "status": "ok",
        "action": "quality_score",
        "total_files": files.len(),
        "total_lines": total_lines,
        "overall_score": avg_score.round(),
        "overall_grade": overall_grade,
        "files": file_scores,
    }))
}

// ============================================================================
// Duplicate Code Detection
// ============================================================================

fn detect_duplicates(files: &[String], min_lines: usize) -> Result<Value, String> {
    // Normalize and hash code blocks
    let mut block_hashes: HashMap<String, Vec<(String, usize)>> = HashMap::new();

    for file_path in files {
        let content = read_file(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        // Sliding window of min_lines
        for i in 0..lines.len().saturating_sub(min_lines - 1) {
            let window = &lines[i..i + min_lines];
            // Normalize: trim and remove blank lines
            let normalized: String = window
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");

            if normalized.lines().count() >= 3 {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                normalized.hash(&mut hasher);
                let hash = format!("{:x}", hasher.finish());

                block_hashes
                    .entry(hash)
                    .or_default()
                    .push((file_path.clone(), i + 1));
            }
        }
    }

    // Filter to duplicates only
    let mut duplicates = Vec::new();
    for (hash, locations) in block_hashes {
        if locations.len() > 1 {
            // Check if locations are in different files or far apart in same file
            let unique_files: std::collections::HashSet<_> =
                locations.iter().map(|(f, _)| f).collect();
            if unique_files.len() > 1 {
                duplicates.push(json!({
                    "hash": hash,
                    "occurrences": locations.len(),
                    "locations": locations.iter().map(|(f, l)| json!({"file": f, "line": l})).collect::<Vec<_>>(),
                }));
            }
        }
    }

    duplicates.sort_by(|a, b| {
        b["occurrences"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["occurrences"].as_u64().unwrap_or(0))
    });

    Ok(json!({
        "status": "ok",
        "action": "duplicate_detection",
        "min_lines": min_lines,
        "total_files_analyzed": files.len(),
        "duplicate_blocks": duplicates.len(),
        "duplicates": duplicates,
    }))
}

// ============================================================================
// Dependency Visualization
// ============================================================================

fn generate_dependency_viz(files: &[String], title: &str) -> Result<Value, String> {
    // Build module dependency graph
    let mut modules: Vec<String> = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut external_crates: std::collections::HashSet<String> = std::collections::HashSet::new();

    for file_path in files {
        let content = read_file(file_path)?;
        let rel_path = if file_path.contains("src/") {
            file_path
                .split("src/")
                .nth(1)
                .unwrap_or(file_path)
                .to_string()
        } else {
            file_path.clone()
        };

        // Extract module name from file path
        let module_name = rel_path
            .trim_end_matches(".rs")
            .replace('/', "::")
            .trim_end_matches("::mod")
            .to_string();

        modules.push(module_name.clone());

        // Find use statements to determine dependencies
        for cap in RE_USE.captures_iter(&content) {
            let use_path = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if let Some(first) = use_path.split("::").next() {
                let crate_name = first.trim();
                if !crate_name.is_empty()
                    && crate_name != "self"
                    && crate_name != "super"
                    && crate_name != "crate"
                {
                    external_crates.insert(crate_name.to_string());
                }
            }
        }

        // Find mod declarations
        for cap in RE_MOD_DECL.captures_iter(&content) {
            if let Some(mod_name) = cap.get(1) {
                let dep = format!("{module_name}::{}", mod_name.as_str());
                edges.push((module_name.clone(), dep));
            }
        }
    }

    // Generate Mermaid diagram
    let mut mermaid = format!("---\ntitle: {title}\n---\ngraph TD\n");

    // Add module nodes
    for module in &modules {
        let safe_name = module.replace("::", "_");
        mermaid.push_str(&format!("    {safe_name}[\"{module}\"]\n"));
    }

    // Add external crate nodes
    for ext in &external_crates {
        let safe_name = format!("ext_{ext}");
        mermaid.push_str(&format!("    {safe_name}[[\"{ext}\"]]\n"));
    }

    // Add edges
    for (from, to) in &edges {
        let safe_from = from.replace("::", "_");
        let safe_to = to.replace("::", "_");
        mermaid.push_str(&format!("    {safe_from} --> {safe_to}\n"));
    }

    // Add external dependencies
    if let Some(ext) = external_crates.iter().next() {
        let safe_ext = format!("ext_{ext}");
        for module in &modules {
            let safe_module = module.replace("::", "_");
            mermaid.push_str(&format!("    {safe_module} -.-> {safe_ext}\n"));
        }
    }

    Ok(json!({
        "status": "ok",
        "action": "dependency_visualization",
        "title": title,
        "total_modules": modules.len(),
        "external_crates": external_crates.into_iter().collect::<Vec<_>>(),
        "mermaid": mermaid,
        "description": "Render this Mermaid code in any Mermaid-compatible viewer",
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CodeQualityTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool() -> CodeQualityTool {
        CodeQualityTool
    }

    #[tokio::test]
    async fn test_score_action_on_dir() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("score".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("src/tools/builtin".to_string()),
        );
        params.insert("recursive".to_string(), Value::Bool(false));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["overall_score"].as_f64().is_some());
        let files = result["files"].as_array().unwrap();
        assert!(!files.is_empty());
    }

    #[tokio::test]
    async fn test_score_action_single_file() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("score".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("src/tools/builtin/hello_tool.rs".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["total_files"], 1);
        let grade = result["overall_grade"].as_str().unwrap();
        assert!(["A+", "A", "B", "C", "D", "F"].contains(&grade));
    }

    #[tokio::test]
    async fn test_viz_deps_action() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("viz_deps".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("src/tools/builtin/hello_tool.rs".to_string()),
        );
        params.insert(
            "title".to_string(),
            Value::String("Test Diagram".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["mermaid"].as_str().unwrap().contains("graph TD"));
        assert!(result["mermaid"].as_str().unwrap().contains("Test Diagram"));
    }

    #[tokio::test]
    async fn test_duplicates_action() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert(
            "action".to_string(),
            Value::String("duplicates".to_string()),
        );
        params.insert(
            "path".to_string(),
            Value::String("src/tools/builtin".to_string()),
        );
        params.insert("recursive".to_string(), Value::Bool(false));
        params.insert("min_dup_lines".to_string(), Value::Number(5.into()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["duplicate_blocks"].as_u64().is_some());
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert(
            "action".to_string(),
            Value::String("nonexistent".to_string()),
        );
        params.insert("path".to_string(), Value::String("src".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains("nonexistent"));
    }

    #[tokio::test]
    async fn test_missing_path() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("score".to_string()));

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonexistent_path() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("score".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("/nonexistent/path/xyz".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("does not exist"));
    }

    #[tokio::test]
    async fn test_not_a_rust_file() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("score".to_string()));
        params.insert("path".to_string(), Value::String("Cargo.toml".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("Not a Rust file"));
    }

    #[test]
    fn test_calculate_metrics_empty() {
        let metrics = calculate_metrics("");
        assert_eq!(metrics.doc_coverage, 0.0);
        assert_eq!(metrics.test_coverage, 0.0);
        assert_eq!(metrics.unwrap_ratio, 0.0);
        assert_eq!(metrics.unsafe_ratio, 0.0);
        assert_eq!(metrics.todo_ratio, 0.0);
        assert_eq!(metrics.avg_fn_length, 0.0);
        assert_eq!(metrics.complexity_score, 0.0);
        assert_eq!(metrics.comment_ratio, 0.0);
    }

    #[test]
    fn test_calculate_metrics_with_code() {
        let code = r#"
/// A simple function
/// with docs
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

struct Point {
    x: i32,
    y: i32,
}

// TODO: implement this
fn dangerous() {
    let _x = None.unwrap();
}

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}

#[test]
fn test_add_negative() {
    assert_eq!(add(-1, -2), -3);
}
"#;
        let metrics = calculate_metrics(code);
        assert!(metrics.doc_coverage > 0.0);
        assert!(metrics.test_coverage > 0.0);
        assert!(metrics.unwrap_ratio > 0.0);
        assert!(metrics.todo_ratio > 0.0);
    }

    #[test]
    fn test_score_from_metrics_perfect() {
        let m = QualityMetrics {
            doc_coverage: 1.0,
            test_coverage: 1.0,
            unwrap_ratio: 0.0,
            unsafe_ratio: 0.0,
            todo_ratio: 0.0,
            avg_fn_length: 10.0,
            complexity_score: 0.0,
            comment_ratio: 1.0,
        };
        let score = score_from_metrics(&m);
        assert!(score >= 90.0);
    }

    #[test]
    fn test_score_from_metrics_terrible() {
        let m = QualityMetrics {
            doc_coverage: 0.0,
            test_coverage: 0.0,
            unwrap_ratio: 1.0,
            unsafe_ratio: 0.1,
            todo_ratio: 0.1,
            avg_fn_length: 200.0,
            complexity_score: 1.0,
            comment_ratio: 0.0,
        };
        let score = score_from_metrics(&m);
        assert!(score < 30.0);
    }

    #[test]
    fn test_collect_rust_files() {
        let mut files = Vec::new();
        let result = collect_rust_files(Path::new("src"), &mut files, true, 0);
        assert!(result.is_ok());
        assert!(!files.is_empty());
        // Should not include target directory
        for f in &files {
            assert!(!f.contains("/target/"));
        }
    }

    #[test]
    fn test_collect_rust_files_depth_limit() {
        let mut files = Vec::new();
        let result = collect_rust_files(Path::new("src"), &mut files, true, 25);
        assert!(result.is_ok());
        assert!(files.is_empty()); // depth > 20 returns early
    }

    #[test]
    fn test_read_file_nonexistent() {
        let result = read_file("/nonexistent_file_xyz.rs");
        assert!(result.is_err());
    }
}
