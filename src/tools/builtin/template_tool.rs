//! Template Tool: render Jinja2 templates using minijinja.
//!
//! Supports variables, loops, conditionals, filters, and includes.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use minijinja::Environment;
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(TemplateTool));
}

struct TemplateTool;

#[async_trait::async_trait]
impl Tool for TemplateTool {
    fn name(&self) -> &str {
        "template"
    }

    fn description(&self) -> &str {
        "Render Jinja2 templates with variables, loops, conditionals, and filters. \
         Actions: render (render a template string), \
         render_file (render a template file), \
         render_with_data (render with context from JSON file)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            tp(
                "action",
                "Action: render, render_file, render_with_data",
                true,
            ),
            tp(
                "template",
                "Template string content (for render action)",
                false,
            ),
            tp(
                "template_file",
                "Path to template file (for render_file)",
                false,
            ),
            tp("data", "JSON string with template variables", false),
            tp("data_file", "Path to JSON data file", false),
            tp(
                "output",
                "Output file path to write result (optional)",
                false,
            ),
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        match action {
            "render" => render(params),
            "render_file" => render_file(params),
            "render_with_data" => render_with_data(params),
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

fn tp(name: &str, desc: &str, required: bool) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: desc.to_string(),
        required,
        parameter_type: "string".to_string(),
    }
}

fn render(params: &HashMap<String, Value>) -> Result<Value, String> {
    let template = params
        .get("template")
        .and_then(|v| v.as_str())
        .ok_or("template is required")?;
    let data_raw = params.get("data").and_then(|v| v.as_str()).unwrap_or("{}");
    let output = params.get("output").and_then(|v| v.as_str());

    let data: Value =
        serde_json::from_str(data_raw).map_err(|e| format!("Invalid JSON data: {e}"))?;

    let result = render_template(template, &data)?;

    if let Some(path) = output {
        std::fs::write(path, &result).map_err(|e| format!("Failed to write output: {e}"))?;
    }

    Ok(serde_json::json!({
        "success": true,
        "output": result,
        "length": result.len(),
        "written_to_file": output.is_some(),
    }))
}

fn render_file(params: &HashMap<String, Value>) -> Result<Value, String> {
    let template_file = params
        .get("template_file")
        .and_then(|v| v.as_str())
        .ok_or("template_file is required")?;
    let data_raw = params.get("data").and_then(|v| v.as_str()).unwrap_or("{}");
    let output = params.get("output").and_then(|v| v.as_str());

    let template = std::fs::read_to_string(template_file)
        .map_err(|e| format!("Failed to read template file '{template_file}': {e}"))?;

    let data: Value =
        serde_json::from_str(data_raw).map_err(|e| format!("Invalid JSON data: {e}"))?;

    let result = render_template(&template, &data)?;

    if let Some(path) = output {
        std::fs::write(path, &result).map_err(|e| format!("Failed to write output: {e}"))?;
    }

    Ok(serde_json::json!({
        "success": true,
        "template_file": template_file,
        "output": result,
        "length": result.len(),
        "written_to_file": output.is_some(),
    }))
}

fn render_with_data(params: &HashMap<String, Value>) -> Result<Value, String> {
    let template = params
        .get("template")
        .and_then(|v| v.as_str())
        .ok_or("template is required")?;
    let data_file = params
        .get("data_file")
        .and_then(|v| v.as_str())
        .ok_or("data_file is required")?;
    let output = params.get("output").and_then(|v| v.as_str());

    let data_raw = std::fs::read_to_string(data_file)
        .map_err(|e| format!("Failed to read data file '{data_file}': {e}"))?;

    let data: Value =
        serde_json::from_str(&data_raw).map_err(|e| format!("Invalid JSON in data file: {e}"))?;

    let result = render_template(template, &data)?;

    if let Some(path) = output {
        std::fs::write(path, &result).map_err(|e| format!("Failed to write output: {e}"))?;
    }

    Ok(serde_json::json!({
        "success": true,
        "data_file": data_file,
        "output": result,
        "length": result.len(),
        "written_to_file": output.is_some(),
    }))
}

fn render_template(template_str: &str, data: &Value) -> Result<String, String> {
    let env = Environment::new();

    // Convert serde_json::Value to a context that minijinja can use
    let context = json_value_to_context(data);

    match env.render_str(template_str, context) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("Template render error: {e}")),
    }
}

/// Convert a serde_json Value into a minijinja-compatible context.
fn json_value_to_context(value: &Value) -> serde_json::Value {
    // minijinja's render_str accepts serde_json::Value directly
    value.clone()
}
