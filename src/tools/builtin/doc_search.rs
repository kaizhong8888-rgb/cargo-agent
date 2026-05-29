//! Documentation search tool: query docs.rs and crates.io for crate information.
//!
//! Fetches crate documentation, type signatures, and example code
//! from the docs.rs and crates.io APIs.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// DocSearchTool
// ============================================================================

pub struct DocSearchTool;

#[async_trait::async_trait]
impl Tool for DocSearchTool {
    fn name(&self) -> &str {
        "doc_search"
    }

    fn description(&self) -> &str {
        "Search and retrieve documentation from docs.rs and crates.io. Actions: info (crate metadata), docs (crate documentation page URL), readme (README content), latest_version (check latest version), examples (fetch example source code)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: info, docs, readme, latest_version, examples".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "crate_name".to_string(),
                description: "Name of the crate (e.g. 'serde', 'tokio', 'axum')".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "version".to_string(),
                description: "Specific version (default: latest)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "module_path".to_string(),
                description: "Specific module/submodule (e.g. 'tokio::fs', 'serde::de'). Used with docs action.".to_string(),
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

        let crate_name = params
            .get("crate_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: crate_name")?;

        let version = params.get("version").and_then(|v| v.as_str());

        match action {
            "info" => action_info(crate_name).await,
            "docs" => action_docs(crate_name, version, params),
            "readme" => action_readme(crate_name, version).await,
            "latest_version" => action_latest_version(crate_name).await,
            "examples" => action_examples(crate_name, version).await,
            other => Err(format!("Unknown action: {other}")),
        }
    }
}

async fn action_info(crate_name: &str) -> Result<Value, String> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("cargo-agent/0.1.0")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = response.status().as_u16();
    if status == 404 {
        return Ok(serde_json::json!({
            "status": "not_found",
            "crate": crate_name,
            "message": format!("Crate '{crate_name}' not found on crates.io"),
        }));
    }

    if status >= 400 {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("crates.io error ({status}): {body}"));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let crate_info = json
        .get("crate")
        .ok_or("Missing 'crate' field in response")?;

    Ok(serde_json::json!({
        "status": "ok",
        "crate": crate_name,
        "latest_version": crate_info.get("newest_version").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "description": crate_info.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "homepage": crate_info.get("homepage").and_then(|v| v.as_str()).unwrap_or(""),
        "repository": crate_info.get("repository").and_then(|v| v.as_str()).unwrap_or(""),
        "downloads": crate_info.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0),
        "documentation_url": format!("https://docs.rs/{crate_name}"),
        "categories": crate_info.get("categories").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
    }))
}

fn action_docs(
    crate_name: &str,
    version: Option<&str>,
    params: &HashMap<String, Value>,
) -> Result<Value, String> {
    let version_segment = version.unwrap_or("latest");
    let module_path = params.get("module_path").and_then(|v| v.as_str());

    let base_url = format!("https://docs.rs/{crate_name}/{version_segment}");
    let full_url = if let Some(mod_path) = module_path {
        let path = mod_path.replace("::", "/");
        format!("{base_url}/{path}/index.html")
    } else {
        format!("{base_url}/{crate_name}/index.html")
    };

    Ok(serde_json::json!({
        "status": "ok",
        "crate": crate_name,
        "version": version.unwrap_or("latest"),
        "module_path": module_path.unwrap_or("root"),
        "url": full_url,
        "hint": "Use fetch_url or http_client to fetch the documentation content.",
    }))
}

async fn action_readme(crate_name: &str, version: Option<&str>) -> Result<Value, String> {
    let version_segment = version.unwrap_or("latest");
    let _url = format!(
        "https://raw.githubusercontent.com/rust-lang/crates.io-index/master/{crate_name}/README.md"
    );

    // Try docs.rs source endpoint
    let docs_url = format!("https://docs.rs/crate/{crate_name}/{version_segment}/source/README.md");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client.get(&docs_url).send().await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let body = resp
                .text()
                .await
                .map_err(|e| format!("Failed to read body: {e}"))?;
            Ok(serde_json::json!({
                "status": "ok",
                "crate": crate_name,
                "readme": body.chars().take(10000).collect::<String>(),
                "truncated": body.len() > 10000,
            }))
        }
        _ => Ok(serde_json::json!({
            "status": "info",
            "crate": crate_name,
            "message": "README not directly available. Check the crate's repository URL for source.",
            "docs_url": format!("https://docs.rs/{crate_name}"),
        })),
    }
}

async fn action_latest_version(crate_name: &str) -> Result<Value, String> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("cargo-agent/0.1.0")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("crates.io error: {}", response.status()));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let versions = json
        .get("crate")
        .and_then(|c| c.get("versions"))
        .and_then(|v| v.as_array())
        .ok_or("No versions found")?;

    let latest = versions
        .first()
        .and_then(|v| v.get("num"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(serde_json::json!({
        "status": "ok",
        "crate": crate_name,
        "latest_version": latest,
        "total_versions": versions.len(),
    }))
}

async fn action_examples(crate_name: &str, _version: Option<&str>) -> Result<Value, String> {
    // Look for examples in the crate's source on docs.rs
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    // Fetch crate info to find repository
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("crates.io error: {}", response.status()));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let repo = json
        .get("crate")
        .and_then(|c| c.get("repository"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok(serde_json::json!({
        "status": "ok",
        "crate": crate_name,
        "repository": repo,
        "hint": "Clone the repository with git_clone and browse examples/ directory, or use code_execute with the crate as a dependency.",
        "docs_examples_url": format!("https://docs.rs/{crate_name}/#examples"),
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DocSearchTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_search_tool_metadata() {
        let tool = DocSearchTool;
        assert_eq!(tool.name(), "doc_search");
        assert!(tool.description().contains("documentation"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
        assert!(params.iter().any(|p| p.name == "crate_name" && p.required));
    }
}
