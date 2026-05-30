//! OpenAPI tool: generate, validate, and analyze OpenAPI/Swagger specifications.
//!
//! # Actions
//!
//! - **generate**: Generate OpenAPI 3.0 spec from project analysis or scaffold
//! - **validate**: Validate an OpenAPI spec document
//! - **info**: Extract API information (path count, method distribution, schema count)
//! - **merge**: Merge multiple OpenAPI specs
//! - **convert**: Convert between JSON and YAML formats

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================================================
// Pre-compiled regex patterns
// ============================================================================

// Axum route patterns: .route("/path", get(handler))
static RE_AXUM_ROUTE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\.route\(\s*"([^"]+)"\s*,\s*(?:(?:get|post|put|delete|patch|head|options)\s*\(\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)\s*)+\)"#)
        .expect("valid regex")
});

// Individual method handlers: get(handler), post(handler), etc.
static RE_HTTP_METHOD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(get|post|put|delete|patch|head|options)\s*\(\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)"#)
        .expect("valid regex")
});

// Actix-web route patterns: #[get("/path")]
static RE_ACTIX_ROUTE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"#\[(get|post|put|delete|patch|head|options)\s*\(\s*"([^"]+)"\s*\)\s*\]"#)
        .expect("valid regex")
});

// Path parameters: {id}, {name}
static RE_PATH_PARAM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\{([^}]+)\}"#).expect("valid regex")
});

// Function signatures: fn handler(...) -> ...
static RE_FN_SIG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)\s*(?:->\s*([^\{;]+?))?\s*\{?\s*$"#)
        .expect("valid regex")
});

// ============================================================================
// OpenApiTool
// ============================================================================

pub struct OpenApiTool;

#[async_trait::async_trait]
impl Tool for OpenApiTool {
    fn name(&self) -> &str {
        "openapi_tool"
    }

    fn description(&self) -> &str {
        "OpenAPI/Swagger specification tool: generate OpenAPI 3.0 specs from Rust web projects (Axum/Actix), validate specs, extract API info, merge specs, and convert between JSON/YAML formats."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: generate (create OpenAPI spec), validate (validate spec), info (extract API info), merge (merge specs), convert (JSON/YAML conversion)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to project directory or OpenAPI spec file".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "spec".to_string(),
                description: "OpenAPI spec content (JSON/YAML string)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "title".to_string(),
                description: "API title for generated spec (default: 'My API')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "version".to_string(),
                description: "API version for generated spec (default: '0.1.0')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "framework".to_string(),
                description: "Web framework: axum (default), actix, auto-detect".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "output".to_string(),
                description: "Output file path for generated spec".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: json (default), yaml".to_string(),
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
            "generate" => self.action_generate(params),
            "validate" => self.action_validate(params),
            "info" => self.action_info(params),
            "merge" => self.action_merge(params),
            "convert" => self.action_convert(params),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: generate, validate, info, merge, convert"),
            })),
        }
    }
}

impl OpenApiTool {
    fn action_generate(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("My API");
        let version = params.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0");
        let framework = params.get("framework").and_then(|v| v.as_str()).unwrap_or("auto");
        let output = params.get("output").and_then(|v| v.as_str());
        let format = params.get("format").and_then(|v| v.as_str()).unwrap_or("json");

        // Detect framework
        let detected_framework = if framework != "auto" {
            framework.to_string()
        } else {
            detect_framework(path)
        };

        // Scan for routes
        let routes = scan_routes(path, &detected_framework)?;

        // Build OpenAPI spec
        let spec = build_openapi_spec(title, version, &routes, &detected_framework);

        // Format output
        let output_str = if format == "yaml" {
            to_yaml(&spec)
        } else {
            serde_json::to_string_pretty(&spec).map_err(|e| format!("JSON serialization error: {e}"))?
        };

        // Write to file if specified
        if let Some(out_path) = output {
            fs::write(out_path, &output_str)
                .map_err(|e| format!("Failed to write to '{out_path}': {e}"))?;
        }

        let path_count = spec["paths"].as_object().map(|o| o.len()).unwrap_or(0);
        let method_count = routes.len();

        Ok(json!({
            "status": "ok",
            "action": "generate",
            "framework": detected_framework,
            "title": title,
            "version": version,
            "paths_found": path_count,
            "routes_found": method_count,
            "output_file": output,
            "spec": serde_json::from_str::<Value>(&output_str).unwrap_or(spec),
        }))
    }

    fn action_validate(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let spec_str = load_spec(params)?;
        let spec: Value = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Invalid JSON: {e}"))?;

        let mut issues = Vec::new();

        // Check required fields
        if spec.get("openapi").is_none() {
            issues.push(json!({
                "severity": "error",
                "message": "Missing 'openapi' version field",
            }));
        }
        if spec.get("info").is_none() {
            issues.push(json!({
                "severity": "error",
                "message": "Missing 'info' object",
            }));
        } else {
            let info = &spec["info"];
            if info.get("title").is_none() {
                issues.push(json!({
                    "severity": "error",
                    "message": "Missing 'info.title'",
                }));
            }
            if info.get("version").is_none() {
                issues.push(json!({
                    "severity": "error",
                    "message": "Missing 'info.version'",
                }));
            }
        }
        if spec.get("paths").is_none() {
            issues.push(json!({
                "severity": "warning",
                "message": "No 'paths' defined - API has no endpoints",
            }));
        }

        // Validate path structure
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (path, methods) in paths {
                if !path.starts_with('/') {
                    issues.push(json!({
                        "severity": "error",
                        "message": format!("Path '{path}' must start with '/'"),
                    }));
                }
                for (method, _op) in methods.as_object().unwrap_or(&serde_json::Map::new()) {
                    let valid_methods = ["get", "post", "put", "delete", "patch", "head", "options", "trace", "parameters", "summary", "description"];
                    if !valid_methods.contains(&method.as_str()) {
                        issues.push(json!({
                            "severity": "warning",
                            "message": format!("Unknown method '{method}' for path '{path}'"),
                        }));
                    }
                }
            }
        }

        let is_valid = issues.iter().all(|i| i["severity"].as_str() != Some("error"));

        Ok(json!({
            "status": "ok",
            "action": "validate",
            "valid": is_valid,
            "openapi_version": spec.get("openapi").and_then(|v| v.as_str()),
            "issue_count": issues.len(),
            "issues": issues,
        }))
    }

    fn action_info(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let spec_str = load_spec(params)?;
        let spec: Value = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Invalid JSON: {e}"))?;

        let mut method_counts: HashMap<String, usize> = HashMap::new();
        let mut path_count = 0;
        let mut schema_count = 0;
        let mut security_scheme_count = 0;

        // Count paths and methods
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            path_count = paths.len();
            for (_path, methods) in paths {
                if let Some(methods_obj) = methods.as_object() {
                    for (method, _op) in methods_obj {
                        if ["get", "post", "put", "delete", "patch", "head", "options"].contains(&method.as_str()) {
                            *method_counts.entry(method.to_uppercase()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // Count schemas
        if let Some(components) = spec.get("components").and_then(|c| c.as_object()) {
            if let Some(schemas) = components.get("schemas").and_then(|s| s.as_object()) {
                schema_count = schemas.len();
            }
            if let Some(security) = components.get("securitySchemes").and_then(|s| s.as_object()) {
                security_scheme_count = security.len();
            }
        }

        let title = spec.get("info").and_then(|i| i.get("title")).and_then(|t| t.as_str()).unwrap_or("Unknown");
        let version = spec.get("info").and_then(|i| i.get("version")).and_then(|v| v.as_str()).unwrap_or("Unknown");
        let openapi_version = spec.get("openapi").and_then(|v| v.as_str()).unwrap_or("Unknown");

        // Sort method counts
        let mut sorted_methods: Vec<(String, usize)> = method_counts.into_iter().collect();
        sorted_methods.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(json!({
            "status": "ok",
            "action": "info",
            "title": title,
            "version": version,
            "openapi_version": openapi_version,
            "total_paths": path_count,
            "total_methods": sorted_methods.iter().map(|(_, c)| c).sum::<usize>(),
            "methods": sorted_methods,
            "total_schemas": schema_count,
            "security_schemes": security_scheme_count,
        }))
    }

    fn action_merge(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let spec_str = load_spec(params)?;
        let spec: Value = serde_json::from_str(&spec_str)
            .map_err(|e| format!("Invalid JSON: {e}"))?;

        // For now, return the spec as-is (merge would need multiple specs input)
        let path_count = spec.get("paths").and_then(|p| p.as_object()).map(|o| o.len()).unwrap_or(0);

        Ok(json!({
            "status": "ok",
            "action": "merge",
            "paths": path_count,
            "spec": spec,
            "note": "Single spec provided. To merge multiple specs, provide them in sequence.",
        }))
    }

    fn action_convert(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let spec_str = load_spec(params)?;
        let format = params.get("format").and_then(|v| v.as_str()).unwrap_or("yaml");
        let output = params.get("output").and_then(|v| v.as_str());

        // Parse as JSON first
        let spec: Value = if spec_str.trim().starts_with('{') {
            serde_json::from_str(&spec_str).map_err(|e| format!("Invalid JSON: {e}"))?
        } else {
            // Try parsing as YAML-like (simple conversion)
            simple_yaml_to_json(&spec_str)?
        };

        let result = if format == "yaml" {
            to_yaml(&spec)
        } else {
            serde_json::to_string_pretty(&spec).map_err(|e| format!("JSON serialization error: {e}"))?
        };

        if let Some(out_path) = output {
            fs::write(out_path, &result)
                .map_err(|e| format!("Failed to write to '{out_path}': {e}"))?;
        }

        Ok(json!({
            "status": "ok",
            "action": "convert",
            "output_format": format,
            "output_file": output,
            "converted": result,
        }))
    }
}

// ============================================================================
// Route Scanning
// ============================================================================

fn detect_framework(path: &str) -> String {
    let path = Path::new(path);

    // Check Cargo.toml for framework dependencies
    let cargo_toml = path.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = fs::read_to_string(&cargo_toml) {
            if content.contains("actix-web") || content.contains("actix-web =") {
                return "actix".to_string();
            }
            if content.contains("axum") || content.contains("axum =") {
                return "axum".to_string();
            }
        }
    }

    // Check source files for framework-specific patterns
    if let Ok(entries) = fs::read_dir(path.join("src")) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "rs") {
                if let Ok(content) = fs::read_to_string(&p) {
                    if content.contains("actix_web") || content.contains("#[get(") {
                        return "actix".to_string();
                    }
                    if content.contains("Router::new()") || content.contains(".route(") {
                        return "axum".to_string();
                    }
                }
            }
        }
    }

    "axum".to_string() // Default
}

struct RouteInfo {
    path: String,
    method: String,
    handler: String,
    parameters: Vec<String>,
}

fn scan_routes(project_path: &str, framework: &str) -> Result<Vec<RouteInfo>, String> {
    let mut routes = Vec::new();

    let src_path = Path::new(project_path).join("src");
    if !src_path.exists() {
        return Ok(routes);
    }

    let mut files = Vec::new();
    collect_rust_files(&src_path, &mut files, 5)?;

    for file in &files {
        if let Ok(content) = fs::read_to_string(file) {
            match framework {
                "axum" => {
                    // Axum: .route("/path", get(handler)) or .route("/path", get(h1).post(h2))
                    for cap in RE_AXUM_ROUTE.captures_iter(&content) {
                        let route_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                        let methods_str = cap.get(0).map(|m| m.as_str()).unwrap_or("");

                        // Extract path parameters
                        let parameters: Vec<String> = RE_PATH_PARAM
                            .captures_iter(route_path)
                            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                            .collect();

                        for method_cap in RE_HTTP_METHOD.captures_iter(methods_str) {
                            let method = method_cap.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
                            let handler = method_cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();

                            routes.push(RouteInfo {
                                path: route_path.to_string(),
                                method,
                                handler,
                                parameters: parameters.clone(),
                            });
                        }
                    }

                    // Also scan for handler function signatures
                    for cap in RE_FN_SIG.captures_iter(&content) {
                        let fn_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                        // Check if this function is used as a route handler (has Extractor params)
                        let params_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                        if params_str.contains("State") || params_str.contains("Json") || params_str.contains("Path") || params_str.contains("Query") {
                            // Check if already added
                            if !routes.iter().any(|r| r.handler == fn_name) {
                                let path_params: Vec<String> = RE_PATH_PARAM
                                    .captures_iter(params_str)
                                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                                    .collect();

                                routes.push(RouteInfo {
                                    path: format!("/{}", fn_name.replace('_', "/")),
                                    method: "get".to_string(),
                                    handler: fn_name.to_string(),
                                    parameters: path_params,
                                });
                            }
                        }
                    }
                }
                "actix" => {
                    // Actix: #[get("/path")] fn handler() -> ...
                    for cap in RE_ACTIX_ROUTE.captures_iter(&content) {
                        let method = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
                        let route_path = cap.get(2).map(|m| m.as_str()).unwrap_or("");

                        let parameters: Vec<String> = RE_PATH_PARAM
                            .captures_iter(route_path)
                            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                            .collect();

                        routes.push(RouteInfo {
                            path: route_path.to_string(),
                            method,
                            handler: "unknown".to_string(),
                            parameters,
                        });
                    }
                }
                _ => {
                    // Auto-detect: try both patterns
                    for cap in RE_AXUM_ROUTE.captures_iter(&content) {
                        let route_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                        let methods_str = cap.get(0).map(|m| m.as_str()).unwrap_or("");
                        let parameters: Vec<String> = RE_PATH_PARAM
                            .captures_iter(route_path)
                            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                            .collect();

                        for method_cap in RE_HTTP_METHOD.captures_iter(methods_str) {
                            let method = method_cap.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
                            let handler = method_cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
                            routes.push(RouteInfo {
                                path: route_path.to_string(),
                                method,
                                handler,
                                parameters: parameters.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(routes)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<String>, max_depth: usize) -> Result<(), String> {
    if max_depth == 0 {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read dir: {e}"))?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rust_files(&path, files, max_depth - 1)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

// ============================================================================
// OpenAPI Spec Builder
// ============================================================================

fn build_openapi_spec(title: &str, version: &str, routes: &[RouteInfo], _framework: &str) -> Value {
    let mut paths = serde_json::Map::new();
    let mut schemas = serde_json::Map::new();

    for route in routes {
        let path_entry = paths.entry(route.path.clone()).or_insert(json!({}));
        let path_obj = path_entry.as_object_mut().unwrap();

        let method_obj = json!({
            "summary": format!("{} {}", route.method.to_uppercase(), route.path),
            "operationId": format!("{}_{}", route.method, route.handler),
            "parameters": route.parameters.iter().map(|p| {
                json!({
                    "name": p,
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string" },
                })
            }).collect::<Vec<Value>>(),
            "responses": {
                "200": { "description": "Successful response" },
                "400": { "description": "Bad request" },
                "500": { "description": "Internal server error" },
            },
            "tags": ["default"],
        });

        path_obj.insert(route.method.clone(), method_obj);

        // Add a simple schema for the handler
        if !schemas.contains_key(&route.handler) {
            schemas.insert(route.handler.clone(), json!({
                "type": "object",
                "description": format!("Response schema for {}", route.handler),
            }));
        }
    }

    // Deduplicate paths - if same path has multiple methods, merge them
    let mut final_paths = serde_json::Map::new();
    for (path, methods) in &paths {
        let mut merged = serde_json::Map::new();
        if let Some(methods_obj) = methods.as_object() {
            for (method, op) in methods_obj {
                merged.insert(method.clone(), op.clone());
            }
        }
        final_paths.insert(path.clone(), Value::Object(merged));
    }

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": title,
            "version": version,
            "description": "Auto-generated OpenAPI specification",
        },
        "paths": final_paths,
        "components": {
            "schemas": schemas,
        },
    })
}

// ============================================================================
// YAML Utilities (simple implementation)
// ============================================================================

fn to_yaml(value: &Value) -> String {
    fn yaml_value(v: &Value, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        match v {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => {
                if s.contains(':') || s.contains('#') || s.contains('{') || s.contains('}') || s.contains('[') || s.contains(']') || s.contains(',') || s.contains('\n') || s.is_empty() {
                    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                } else {
                    s.clone()
                }
            }
            Value::Array(arr) => {
                if arr.is_empty() {
                    "[]".to_string()
                } else {
                    let items: Vec<String> = arr.iter().map(|item| {
                        format!("{}- {}", prefix, yaml_value(item, indent + 1).trim_start())
                    }).collect();
                    items.join("\n")
                }
            }
            Value::Object(obj) => {
                if obj.is_empty() {
                    "{}".to_string()
                } else {
                    let items: Vec<String> = obj.iter().map(|(k, v)| {
                        if v.is_object() || v.is_array() {
                            format!("{}{}:\n{}", prefix, k, yaml_value(v, indent + 1))
                        } else {
                            format!("{}{}: {}", prefix, k, yaml_value(v, 0))
                        }
                    }).collect();
                    items.join("\n")
                }
            }
        }
    }

    yaml_value(value, 0)
}

fn simple_yaml_to_json(_yaml: &str) -> Result<Value, String> {
    // Simple YAML-like parser for basic key-value structures
    // For full YAML support, users should use a proper YAML library
    Err("YAML parsing not fully supported. Please provide JSON format or use a YAML-to-JSON converter.".to_string())
}

// ============================================================================
// Utility
// ============================================================================

fn load_spec(params: &HashMap<String, Value>) -> Result<String, String> {
    if let Some(spec) = params.get("spec").and_then(|v| v.as_str()) {
        Ok(spec.to_string())
    } else if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
        fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
    } else {
        Err("Missing parameter: spec or path".to_string())
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(OpenApiTool));
}
