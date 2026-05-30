//! Test generation tool: analyzes Rust source code and generates unit tests,
//! integration tests, and property tests with edge cases and error handling coverage.
//!
//! # Features
//!
//! - **function**: Generate unit tests for individual functions
//! - **struct**: Generate tests for struct methods and derive implementations
//! - **trait_impl**: Generate tests for trait implementations
//! - **module**: Generate tests for an entire module
//! - **edge_cases**: Focus on edge case coverage (empty, boundary, overflow)
//!
//! # Test Types Generated
//!
//! - Standard `#[test]` unit tests with assertions
//! - `#[should_panic]` tests for error conditions
//! - `#[should_panic(expected = "...")]` for specific panic messages
//! - Property-based test stubs (proptest/quickcheck style)
//! - Doc tests

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
    Regex::new(
        r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?(?:const\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)\s*(?:->\s*([^{]+?))?\s*\{?"#
    ).expect("valid regex")
});

static RE_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:pub\s+)(?:struct)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*(?:where\s+[^{]+)?\{?"#
    ).expect("valid regex")
});

#[allow(dead_code)]
static RE_ENUM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:pub\s+)(?:enum)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*(?:where\s+[^{]+)?\{?"#
    ).expect("valid regex")
});

#[allow(dead_code)]
static RE_TRAIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:pub\s+)(?:unsafe\s+)?trait\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*(?::\s*([^{;]+))?\s*(?:where\s+[^{]+)?\{?"#
    ).expect("valid regex")
});

static RE_IMPL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:(?:pub\s+)?(?:unsafe\s+)?impl\s+(?:(?:<[^>]*>)\s+)?([a-zA-Z_][a-zA-Z0-9_<>]*(?:\s*::\s*[a-zA-Z_][a-zA-Z0-9_<>]*)*)\s*(?:for\s+([a-zA-Z_][a-zA-Z0-9_<>]*(?:\s*::\s*[a-zA-Z_][a-zA-Z0-9_<>]*)*))?)"#
    ).expect("valid regex")
});

static RE_FN_SIG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:pub\s+(?:crate\s+)?(?:super\s+)?(?:\(crate\)\s+)?)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)\s*(?:->\s*([^\{;]+?))?\s*\{?\s*$"#
    ).expect("valid regex")
});

static RE_PARAM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*([^,]+)"#).expect("valid regex")
});

#[allow(dead_code)]
static RE_OPTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,])Option\s*<\s*([^>]+)\s*>"#).expect("valid regex")
});

#[allow(dead_code)]
static RE_RESULT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,])Result\s*<\s*([^,>]+)\s*(?:,\s*[^>]+)?\s*>"#).expect("valid regex")
});

#[allow(dead_code)]
static RE_VEC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,])Vec\s*<\s*([^>]+)\s*>"#).expect("valid regex")
});

static RE_STRING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,&])&?\s*str\b"#).expect("valid regex")
});

static RE_INT_TYPES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,])(u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)\b"#)
        .expect("valid regex")
});

static RE_FLOAT_TYPES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|[\s<,])(f32|f64)\b"#).expect("valid regex")
});

static RE_BOOL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:^|[\s<,])bool\b"#).expect("valid regex"));

#[allow(dead_code)]
static RE_TEST_ATTR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*#\[(test|should_panic)"#).expect("valid regex"));

static RE_PUB_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\("#).expect("valid regex")
});

// ============================================================================
// TestGeneratorTool
// ============================================================================

pub struct TestGeneratorTool;

#[async_trait::async_trait]
impl Tool for TestGeneratorTool {
    fn name(&self) -> &str {
        "test_generate"
    }

    fn description(&self) -> &str {
        "Generate unit tests, integration tests, and property tests for Rust code. Analyzes function signatures, struct definitions, and trait implementations to produce comprehensive test suites with edge cases, error handling, and boundary conditions."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to a Rust source file or directory to analyze".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "mode".to_string(),
                description: "Generation mode: function (tests for functions), struct (tests for struct methods), trait_impl (tests for trait implementations), module (tests for entire module), all (everything) (default: all)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "test_type".to_string(),
                description: "Test type: unit (default), integration, property (property-based test stubs)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "coverage".to_string(),
                description: "Coverage focus: normal (default), edge_cases (focus on boundary/empty/overflow), full (comprehensive)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "include_doc_tests".to_string(),
                description: "Include doc test stubs (true/false, default: false)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "output".to_string(),
                description: "Optional output file path to write generated tests".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let mode = params
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let test_type = params
            .get("test_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unit");

        let coverage = params
            .get("coverage")
            .and_then(|v| v.as_str())
            .unwrap_or("normal");

        let include_doc_tests = params
            .get("include_doc_tests")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let output = params.get("output").and_then(|v| v.as_str());

        let file_path = Path::new(path);
        if !file_path.exists() {
            return Ok(json!({
                "status": "error",
                "message": format!("Path does not exist: {path}"),
            }));
        }

        let content = read_file_content(path)?;
        let mut generated_tests = Vec::new();

        let modes: Vec<&str> = if mode == "all" {
            vec!["function", "struct", "trait_impl"]
        } else {
            vec![mode]
        };

        for m in modes {
            match m {
                "function" => {
                    generated_tests.extend(generate_function_tests(
                        &content, path, test_type, coverage, include_doc_tests,
                    )?);
                }
                "struct" => {
                    generated_tests.extend(generate_struct_tests(
                        &content, path, test_type, coverage,
                    )?);
                }
                "trait_impl" => {
                    generated_tests.extend(generate_trait_impl_tests(
                        &content, path, test_type, coverage,
                    )?);
                }
                "module" => {
                    generated_tests.extend(generate_module_tests(
                        &content, path, test_type, coverage,
                    )?);
                }
                _ => {
                    return Ok(json!({
                        "status": "error",
                        "message": format!("Unknown mode: {m}. Available: function, struct, trait_impl, module, all"),
                    }));
                }
            }
        }

        let total = generated_tests.len();
        let test_code = generated_tests.join("\n\n");

        if let Some(out_path) = output {
            write_file_content(out_path, &test_code)?;
        }

        Ok(json!({
            "status": "ok",
            "mode": mode,
            "test_type": test_type,
            "coverage_focus": coverage,
            "file_analyzed": path,
            "tests_generated": total,
            "output_file": output,
            "generated_code": test_code,
        }))
    }
}

// ============================================================================
// Test Generation Functions
// ============================================================================

/// Generate tests for all functions in the source.
fn generate_function_tests(
    content: &str,
    _path: &str,
    test_type: &str,
    coverage: &str,
    include_doc_tests: bool,
) -> Result<Vec<String>, String> {
    let mut tests = Vec::new();

    for cap in RE_FN.captures_iter(content) {
        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let params_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let return_type = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("");

        let test_name = format!("test_{fn_name}");
        let mut test_body = String::new();

        let fn_info = FunctionInfo {
            name: fn_name.to_string(),
            params: parse_params(params_str),
            return_type: return_type.to_string(),
        };

        match test_type {
            "unit" => {
                test_body = generate_unit_test_body(&fn_info, coverage);
            }
            "integration" => {
                test_body = generate_integration_test_stub(&fn_info);
            }
            "property" => {
                test_body = generate_property_test_stub(&fn_info);
            }
            _ => {}
        }

        if !test_body.is_empty() {
            let test_attr = "#[test]".to_string();
            if fn_info.return_type.contains("Result")
                && coverage != "normal"
                && !fn_info.params.is_empty()
            {
                // Add a should_panic variant for Result-returning functions
                let mut panic_test = "#[test]\n".to_string();
                panic_test.push_str("#[should_panic]\n");
                panic_test.push_str(&format!("fn {test_name}_error() {{\n"));
                panic_test.push_str(&generate_test_args(&fn_info, "error"));
                panic_test.push_str(&format!("    {fn_name}(...); // TODO: trigger error case\n"));
                panic_test.push_str("}\n");
                tests.push(panic_test);
            }

            let mut test = String::new();
            test.push_str(&test_attr);
            test.push('\n');
            test.push_str(&format!("fn {test_name}() {{\n"));
            test.push_str(&generate_test_args(&fn_info, "normal"));
            if fn_info.return_type.contains("Result") {
                test.push_str(&format!(
                    "    let result = {fn_name}(...);\n"
                ));
                test.push_str("    assert!(result.is_ok());\n");
                test.push_str("    // TODO: verify result value\n");
            } else if !fn_info.return_type.is_empty() && fn_info.return_type != "()" {
                test.push_str(&format!(
                    "    let result = {fn_name}(...);\n"
                ));
                test.push_str(&format!(
                    "    // TODO: assert expected value for {test_name}\n"
                ));
                test.push_str(&format!("    // let expected = ...;\n"));
                test.push_str(&format!("    // assert_eq!(result, expected);\n"));
            } else {
                test.push_str(&format!(
                    "    {fn_name}(...);\n"
                ));
                test.push_str("    // TODO: add assertions\n");
            }
            test.push_str("}");

            tests.push(test);
        }

        // Generate doc tests if requested
        if include_doc_tests && RE_PUB_FN.is_match(&cap.get(0).unwrap().as_str()) {
            let doc_test = generate_doc_test_stub(&fn_info);
            if !doc_test.is_empty() {
                tests.push(doc_test);
            }
        }
    }

    Ok(tests)
}

/// Generate tests for structs and their methods.
fn generate_struct_tests(
    content: &str,
    _path: &str,
    _test_type: &str,
    coverage: &str,
) -> Result<Vec<String>, String> {
    let mut tests = Vec::new();

    for cap in RE_STRUCT.captures_iter(content) {
        let struct_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");

        let si = StructInfo {
            name: struct_name.to_string(),
            fields: extract_struct_fields(content, struct_name),
        };

        // New / constructor test
        let mut new_test = String::new();
        new_test.push_str("#[test]\n");
        new_test.push_str(&format!("fn test_{}_new() {{\n", si.name.to_lowercase()));
        new_test.push_str(&generate_struct_construction(&si));
        new_test.push_str("    // TODO: verify constructor creates valid instance\n");
        new_test.push_str("}\n");
        tests.push(new_test);

        // Default test if struct might impl Default
        let mut default_test = String::new();
        default_test.push_str("#[test]\n");
        default_test.push_str(&format!(
            "fn test_{}_default() {{\n",
            si.name.to_lowercase()
        ));
        default_test.push_str(&format!("    let instance = {}::default();\n", si.name));
        default_test.push_str("    // TODO: verify default values are sensible\n");
        default_test.push_str("}\n");
        tests.push(default_test);

        // Edge case: empty/minimal construction
        if coverage == "edge_cases" || coverage == "full" {
            let mut edge_test = String::new();
            edge_test.push_str("#[test]\n");
            edge_test.push_str(&format!(
                "fn test_{}_edge_cases() {{\n",
                si.name.to_lowercase()
            ));
            edge_test.push_str(&format!("    // Test with minimal/empty values\n"));
            edge_test.push_str(&format!("    // let minimal = {} {{ ... }};\n", si.name));
            edge_test.push_str(&format!(
                "    // Test with boundary values\n"
            ));
            edge_test.push_str(&format!("    // let boundary = {} {{ ... }};\n", si.name));
            edge_test.push_str("}\n");
            tests.push(edge_test);
        }

        // Debug/Display test stub
        let mut debug_test = String::new();
        debug_test.push_str("#[test]\n");
        debug_test.push_str(&format!(
            "fn test_{}_debug_display() {{\n",
            si.name.to_lowercase()
        ));
        debug_test.push_str(&format!(
            "    let instance = {} {{ /* ... */ }}; // TODO: construct\n",
            si.name
        ));
        debug_test.push_str("    let debug_str = format!(\"{:?}\", instance);\n");
        debug_test.push_str(
            "    assert!(!debug_str.is_empty()); // Debug should produce non-empty output\n",
        );
        debug_test.push_str("}\n");
        tests.push(debug_test);
    }

    Ok(tests)
}

/// Generate tests for trait implementations.
fn generate_trait_impl_tests(
    content: &str,
    _path: &str,
    _test_type: &str,
    _coverage: &str,
) -> Result<Vec<String>, String> {
    let mut tests = Vec::new();

    for cap in RE_IMPL.captures_iter(content) {
        let trait_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let for_type = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        if for_type.is_empty() {
            continue;
        }

        // Find methods in this impl block
        let impl_start = cap.get(0).unwrap().start();
        let impl_content = &content[impl_start..];
        let methods = extract_impl_methods(impl_content);

        if methods.is_empty() {
            continue;
        }

        let mut test = String::new();
        test.push_str("#[test]\n");
        test.push_str(&format!(
            "fn test_{}_impl_{}() {{\n",
            for_type.to_lowercase(),
            trait_name.to_lowercase()
        ));
        test.push_str(&format!(
            "    let instance = {}::default(); // TODO: construct\n",
            for_type
        ));
        test.push_str("\n");

        for method in methods {
            test.push_str(&format!(
                "    // Test {} trait method: {}\n",
                trait_name, method
            ));
            test.push_str(&format!("    // instance.{}(...);\n", method));
        }

        test.push_str("}\n");
        tests.push(test);
    }

    Ok(tests)
}

/// Generate module-level tests.
fn generate_module_tests(
    content: &str,
    path: &str,
    _test_type: &str,
    _coverage: &str,
) -> Result<Vec<String>, String> {
    let mut tests = Vec::new();

    // Get module name from path
    let module_name = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mod".to_string());

    let pub_fns: Vec<_> = RE_PUB_FN
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    if pub_fns.is_empty() {
        return Ok(tests);
    }

    let mut test = String::new();
    test.push_str(&format!("// Integration tests for module '{module_name}'\n"));
    test.push_str(&format!("use crate::{module_name};\n\n"));

    for fn_name in &pub_fns {
        test.push_str("#[test]\n");
        test.push_str(&format!("fn test_{}_{fn_name}() {{\n", module_name));
        test.push_str(&format!("    let result = {module_name}::{fn_name}(...);\n"));
        test.push_str("    // TODO: add assertions\n");
        test.push_str("}\n\n");
    }

    tests.push(test);
    Ok(tests)
}

// ============================================================================
// Helper Functions
// ============================================================================

struct FunctionInfo {
    name: String,
    params: Vec<(String, String)>, // (name, type)
    return_type: String,
}

struct StructInfo {
    name: String,
    fields: Vec<(String, String)>, // (name, type)
}

fn parse_params(params_str: &str) -> Vec<(String, String)> {
    let mut params = Vec::new();
    for cap in RE_PARAM.captures_iter(params_str) {
        let name = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let ptype = cap.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        if !name.is_empty() {
            params.push((name, ptype));
        }
    }
    params
}

fn extract_struct_fields(content: &str, struct_name: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    // Simple heuristic: find lines between struct { and }
    let pattern = format!(
        r#"(?m)^\s*(?:pub\s+)?struct\s+{}\s*(?:<[^>]*>)?\s*(?:where\s+[^{{]+)?\{{([^}}]*)\}}"#,
        regex::escape(struct_name)
    );
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(cap) = re.captures(content) {
            let body = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            for line in body.lines() {
                let line = line.trim();
                if line.starts_with("//") || line.is_empty() {
                    continue;
                }
                if let Some(colon_pos) = line.find(':') {
                    let name = line[..colon_pos].trim().trim_start_matches("pub").trim();
                    let ftype = line[colon_pos + 1..].trim().trim_end_matches(',').trim();
                    if !name.is_empty() && !ftype.is_empty() {
                        fields.push((name.to_string(), ftype.to_string()));
                    }
                }
            }
        }
    }
    fields
}

fn extract_impl_methods(impl_content: &str) -> Vec<String> {
    let mut methods = Vec::new();
    for cap in RE_FN_SIG.captures_iter(impl_content) {
        if let Some(name) = cap.get(1).map(|m| m.as_str()) {
            // Skip constructors
            if name != "new" {
                methods.push(name.to_string());
            }
        }
    }
    methods
}

/// Generate test argument stubs based on parameter types.
fn generate_test_args(fn_info: &FunctionInfo, case: &str) -> String {
    let mut lines = String::new();

    for (name, ptype) in &fn_info.params {
        let value = generate_default_value(ptype, case);
        lines.push_str(&format!("    let {name} = {value};\n"));
    }

    lines
}

/// Generate a sensible default value for a given Rust type.
fn generate_default_value(type_str: &str, case: &str) -> String {
    let type_str = type_str.trim();

    // Handle Option<T>
    if type_str.starts_with("Option") {
        match case {
            "normal" => {
                if let Some(inner) = extract_inner_type(type_str, "Option") {
                    format!("Some({})", generate_default_value(&inner, case))
                } else {
                    "Some(Default::default())".to_string()
                }
            }
            "edge_cases" | "error" => "None".to_string(),
            _ => "None".to_string(),
        }
    }
    // Handle Result<T, E>
    else if type_str.starts_with("Result") {
        if let Some(inner) = extract_inner_type(type_str, "Result") {
            format!("Ok({})", generate_default_value(&inner, case))
        } else {
            "Ok(Default::default())".to_string()
        }
    }
    // Handle Vec<T>
    else if type_str.starts_with("Vec") {
        match case {
            "normal" => {
                if let Some(inner) = extract_inner_type(type_str, "Vec") {
                    format!("vec![{}]", generate_default_value(&inner, case))
                } else {
                    "vec![]".to_string()
                }
            }
            "edge_cases" => "vec![]".to_string(),
            _ => "vec![]".to_string(),
        }
    }
    // Handle String / &str
    else if RE_STRING.is_match(type_str) || type_str.contains("String") {
        match case {
            "normal" => r#""test".to_string()"#.to_string(),
            "edge_cases" => r#""".to_string()"#.to_string(),
            "error" => r#"".to_string()"#.to_string(),
            _ => r#""test".to_string()"#.to_string(),
        }
    }
    // Handle integer types
    else if RE_INT_TYPES.is_match(type_str) {
        match case {
            "normal" => "0".to_string(),
            "edge_cases" => {
                if type_str.starts_with('u') {
                    "u32::MAX".to_string() // generic max for unsigned
                } else {
                    "i32::MAX".to_string()
                }
            }
            "error" => "0".to_string(),
            _ => "0".to_string(),
        }
    }
    // Handle float types
    else if RE_FLOAT_TYPES.is_match(type_str) {
        match case {
            "normal" => "0.0".to_string(),
            "edge_cases" => "f64::INFINITY".to_string(),
            _ => "0.0".to_string(),
        }
    }
    // Handle bool
    else if RE_BOOL.is_match(type_str) {
        match case {
            "normal" => "true".to_string(),
            "edge_cases" => "false".to_string(),
            _ => "true".to_string(),
        }
    }
    // Fallback
    else {
        "Default::default()".to_string()
    }
}

/// Extract the inner type from a generic like Option<T> or Vec<T>.
fn extract_inner_type(type_str: &str, wrapper: &str) -> Option<String> {
    let prefix = format!("{wrapper}<");
    if type_str.starts_with(&prefix) && type_str.ends_with('>') {
        let inner = &type_str[prefix.len()..type_str.len() - 1];
        Some(inner.trim().to_string())
    } else {
        None
    }
}

/// Generate the body of a unit test.
fn generate_unit_test_body(fn_info: &FunctionInfo, _coverage: &str) -> String {
    let mut body = String::new();
    for (name, ptype) in &fn_info.params {
        let value = generate_default_value(ptype, "normal");
        body.push_str(&format!("    let {name} = {value};\n"));
    }
    body
}

/// Generate an integration test stub.
fn generate_integration_test_stub(fn_info: &FunctionInfo) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "    // Integration test setup for {}\n",
        fn_info.name
    ));
    body.push_str("    // TODO: set up external dependencies (DB, network, etc.)\n");
    for (name, _ptype) in &fn_info.params {
        body.push_str(&format!("    let {name} = /* setup */;\n"));
    }
    body
}

/// Generate a property-based test stub.
fn generate_property_test_stub(fn_info: &FunctionInfo) -> String {
    let mut body = String::new();
    body.push_str("proptest! {\n");
    body.push_str(&format!(
        "    #[test]\n"
    ));
    body.push_str(&format!(
        "    fn prop_{}(\n",
        fn_info.name
    ));
    for (name, ptype) in &fn_info.params {
        let strat = generate_prop_strategy(ptype);
        body.push_str(&format!("        {name} in {strat},\n"));
    }
    body.push_str("    ) {\n");
    body.push_str(&format!(
        "        let result = {}({});\n",
        fn_info.name,
        fn_info
            .params
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    body.push_str("        // TODO: add property assertions\n");
    body.push_str("        // Example: prop_assert!(result.is_ok());\n");
    body.push_str("    }\n");
    body.push_str("}\n");
    body
}

/// Generate a proptest strategy for a given type.
fn generate_prop_strategy(type_str: &str) -> String {
    let type_str = type_str.trim();

    if type_str.starts_with("Option") {
        if let Some(inner) = extract_inner_type(type_str, "Option") {
            format!("prop::option::of({})", generate_prop_strategy(&inner))
        } else {
            "prop::option::of(any::<String>())".to_string()
        }
    } else if type_str.starts_with("Vec") {
        if let Some(inner) = extract_inner_type(type_str, "Vec") {
            format!("prop::collection::vec({}, 0..10)", generate_prop_strategy(&inner))
        } else {
            "prop::collection::vec(any::<String>(), 0..10)".to_string()
        }
    } else if RE_STRING.is_match(type_str) || type_str.contains("String") {
        r#"r#"[a-zA-Z0-9_]{0,50}"#.to_string()
    } else if RE_INT_TYPES.is_match(type_str) {
        "0..1000i32".to_string()
    } else if RE_FLOAT_TYPES.is_match(type_str) {
        "-1000.0..1000.0f64".to_string()
    } else if RE_BOOL.is_match(type_str) {
        "bool::ANY".to_string()
    } else {
        "any::<T>() // TODO: specify strategy".to_string()
    }
}

/// Generate a doc test stub.
fn generate_doc_test_stub(fn_info: &FunctionInfo) -> String {
    let mut doc = String::new();
    doc.push_str(&format!(
        "/// ```\n"
    ));
    doc.push_str(&format!(
        "/// use my_crate::{};\n",
        fn_info.name
    ));

    let args: Vec<String> = fn_info
        .params
        .iter()
        .map(|(_name, ptype)| generate_default_value(ptype, "normal"))
        .collect();

    if !fn_info.return_type.is_empty() && fn_info.return_type != "()" {
        doc.push_str(&format!(
            "/// let result = {}({});\n",
            fn_info.name,
            args.join(", ")
        ));
        doc.push_str("/// assert!(true); // TODO: verify result\n");
    } else {
        doc.push_str(&format!(
            "/// {}({});\n",
            fn_info.name,
            args.join(", ")
        ));
    }

    doc.push_str("/// ```\n");
    doc
}

/// Generate struct construction code.
fn generate_struct_construction(si: &StructInfo) -> String {
    let mut code = String::new();
    code.push_str(&format!("    let instance = {} {{\n", si.name));
    for (field_name, field_type) in &si.fields {
        let value = generate_default_value(field_type, "normal");
        code.push_str(&format!("        {field_name}: {value},\n"));
    }
    code.push_str("    };\n");
    code
}

// ============================================================================
// Utility Functions
// ============================================================================

#[inline]
fn read_file_content(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
}

fn write_file_content(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("Failed to write file '{path}': {e}"))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(TestGeneratorTool));
}
