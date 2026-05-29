//! Network tools: HTTP requests for fetching web content.
//!
//! Enables the agent to access online documentation, APIs, and other resources
//! to inform decision-making during self-evolution.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// FetchTool
// ============================================================================

/// Perform HTTP GET requests to fetch web content.
pub struct FetchTool;

#[async_trait::async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "fetch_url"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL via HTTP GET. Useful for accessing documentation, APIs, and web resources."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "url".to_string(),
                description: "The URL to fetch (e.g. https://docs.rs/reqwest/latest/reqwest/)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "timeout_secs".to_string(),
                description: "Request timeout in seconds (default: 15)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "max_size_bytes".to_string(),
                description: "Maximum response body size in bytes (default: 100_000, max: 1_000_000)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "headers".to_string(),
                description: "Additional HTTP headers as JSON object (e.g. {\"Accept\": \"application/json\"})".to_string(),
                required: false,
                parameter_type: "object".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: url")?;

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(15);

        let max_size = params
            .get("max_size_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000)
            .min(1_000_000) as usize;

        let extra_headers = params.get("headers").and_then(|v| v.as_object());

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(serde_json::json!({
                "status": "error",
                "message": "URL must start with http:// or https://".to_string(),
            }));
        }

        // Build the client
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .user_agent("cargo-agent/0.1.0 (self-evolving AI assistant)")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        let mut request = client.get(url);

        // Add extra headers if provided
        if let Some(headers) = extra_headers {
            for (key, value) in headers {
                if let Some(val_str) = value.as_str() {
                    if let (Ok(header_name), Ok(header_value)) = (
                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                        reqwest::header::HeaderValue::from_str(val_str),
                    ) {
                        request = request.header(header_name, header_value);
                    }
                }
            }
        }

        // Execute the request
        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                format!("Request timed out after {timeout_secs}s")
            } else if e.is_connect() {
                format!("Connection failed: {e}")
            } else {
                format!("Request failed: {e}")
            }
        })?;

        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Check for non-success status
        if status_code >= 400 {
            let body_text = response.text().await.unwrap_or_default();
            return Ok(serde_json::json!({
                "status": "error",
                "http_status": status_code,
                "content_type": content_type,
                "body": body_text.chars().take(2000).collect::<String>(),
            }));
        }

        // Read body with size limit
        let body_bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        let truncated = body_bytes.len() > max_size;
        let body_text = if truncated {
            let limited = &body_bytes[..max_size];
            String::from_utf8_lossy(limited).to_string()
        } else {
            String::from_utf8_lossy(&body_bytes).to_string()
        };

        // Determine if this looks like JSON
        let is_json = content_type.contains("json") || url.ends_with(".json");

        Ok(serde_json::json!({
            "status": "ok",
            "url": url,
            "http_status": status_code,
            "content_type": content_type,
            "size_bytes": body_bytes.len(),
            "truncated": truncated,
            "body": body_text,
            "is_json": is_json,
        }))
    }
}

// ============================================================================
// HttpClientTool
// ============================================================================

/// Full HTTP client supporting GET/POST/PUT/DELETE/HEAD/PATCH with JSON,
/// custom headers, cookies, and multipart file uploads.
pub struct HttpClientTool;

#[async_trait::async_trait]
impl Tool for HttpClientTool {
    fn name(&self) -> &str {
        "http_client"
    }

    fn description(&self) -> &str {
        "Make HTTP requests (GET/POST/PUT/DELETE/HEAD/PATCH) with JSON body, custom headers, cookies, and multipart file uploads. For API testing and web service integration."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "url".to_string(),
                description: "The URL to request".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "method".to_string(),
                description: "HTTP method: GET, POST, PUT, DELETE, HEAD, PATCH (default: GET)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "headers".to_string(),
                description: "HTTP headers as JSON object (e.g. {\"Authorization\": \"Bearer token\"})".to_string(),
                required: false,
                parameter_type: "object".to_string(),
            },
            ToolParameter {
                name: "body".to_string(),
                description: "Request body. If content_type is JSON, this should be a JSON object (as string). Otherwise, raw text.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "content_type".to_string(),
                description: "Content-Type header: application/json, text/plain, multipart/form-data (default: application/json for POST/PUT/PATCH)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "cookies".to_string(),
                description: "Cookies as JSON object (e.g. {\"session_id\": \"abc123\"})".to_string(),
                required: false,
                parameter_type: "object".to_string(),
            },
            ToolParameter {
                name: "query_params".to_string(),
                description: "URL query parameters as JSON object (e.g. {\"page\": \"1\", \"limit\": \"20\"})".to_string(),
                required: false,
                parameter_type: "object".to_string(),
            },
            ToolParameter {
                name: "timeout_secs".to_string(),
                description: "Request timeout in seconds (default: 30)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "follow_redirects".to_string(),
                description: "Whether to follow redirects (default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "max_size_bytes".to_string(),
                description: "Maximum response body size in bytes (default: 100_000, max: 1_000_000)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: url")?;

        let method = params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let max_size = params
            .get("max_size_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000)
            .min(1_000_000) as usize;

        let follow_redirects = params
            .get("follow_redirects")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let body_str = params.get("body").and_then(|v| v.as_str());
        let content_type = params.get("content_type").and_then(|v| v.as_str());
        let headers_map = params.get("headers").and_then(|v| v.as_object());
        let cookies_map = params.get("cookies").and_then(|v| v.as_object());
        let query_map = params.get("query_params").and_then(|v| v.as_object());

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(serde_json::json!({
                "status": "error",
                "message": "URL must start with http:// or https://",
            }));
        }

        // Build the client
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .user_agent("cargo-agent/0.1.0 (self-evolving AI assistant)");

        if !follow_redirects {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
        } else {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::limited(10));
        }

        let client = client_builder
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        // Build the request URL with query params
        let final_url = if let Some(qp) = query_map {
            let url_str = url.to_string();
            let pairs: Vec<String> = qp
                .iter()
                .map(|(k, v)| {
                    let val = v.as_str().unwrap_or("true");
                    format!("{}={}", url_escape(k), url_escape(val),)
                })
                .collect();
            if pairs.is_empty() {
                url_str
            } else {
                let sep = if url_str.contains('?') { "&" } else { "?" };
                format!("{}{}{}", url_str, sep, pairs.join("&"))
            }
        } else {
            url.to_string()
        };

        // Build headers
        let mut headers = HeaderMap::new();
        if let Some(h) = headers_map {
            for (key, value) in h {
                if let Some(val_str) = value.as_str() {
                    if let (Ok(h_name), Ok(h_val)) = (
                        HeaderName::from_bytes(key.as_bytes()),
                        HeaderValue::from_str(val_str),
                    ) {
                        headers.insert(h_name, h_val);
                    }
                }
            }
        }

        // Add cookies as Cookie header
        if let Some(c) = cookies_map {
            let cookie_parts: Vec<String> = c
                .iter()
                .map(|(k, v)| {
                    let val = v.as_str().unwrap_or("true");
                    format!("{k}={val}")
                })
                .collect();
            if !cookie_parts.is_empty() {
                headers.insert(
                    reqwest::header::COOKIE,
                    HeaderValue::from_str(&cookie_parts.join("; "))
                        .map_err(|e| format!("Invalid cookie: {e}"))?,
                );
            }
        }

        // Build request
        let request = match method.as_str() {
            "GET" => client.get(&final_url),
            "POST" => client.post(&final_url),
            "PUT" => client.put(&final_url),
            "DELETE" => client.delete(&final_url),
            "HEAD" => client.head(&final_url),
            "PATCH" => client.patch(&final_url),
            other => return Err(format!("Unsupported HTTP method: {other}")),
        };

        let request = request.headers(headers);

        // Add body for methods that support it
        let request = match method.as_str() {
            "POST" | "PUT" | "PATCH" => {
                let ct = content_type.unwrap_or("application/json");

                if ct == "application/json" {
                    if let Some(body) = body_str {
                        // Try to parse as JSON value for proper serialization
                        let json_body: Value = serde_json::from_str(body)
                            .map_err(|e| format!("Invalid JSON body: {e}"))?;
                        request.json(&json_body)
                    } else {
                        request
                    }
                } else if ct.starts_with("multipart/form-data") {
                    // Multipart: body should be JSON array of {name, value} or {name, file_path}
                    let mut form = reqwest::multipart::Form::new();
                    if let Some(body) = body_str {
                        let parts: Vec<MultipartPart> = serde_json::from_str(body)
                            .map_err(|e| format!("Invalid multipart body: {e}"))?;
                        for part in parts {
                            if let Some(file_path) = part.file_path {
                                let content = std::fs::read(&file_path)
                                    .map_err(|e| format!("Failed to read file {file_path}: {e}"))?;
                                let file_part = reqwest::multipart::Part::bytes(content)
                                    .file_name(file_path.clone());
                                form = form.part(part.name, file_part);
                            } else {
                                form = form.text(part.name, part.value.unwrap_or_default());
                            }
                        }
                    }
                    request.multipart(form)
                } else {
                    if let Some(body) = body_str {
                        request.body(body.to_string())
                    } else {
                        request
                    }
                }
            }
            _ => request,
        };

        // Execute
        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                format!("Request timed out after {timeout_secs}s")
            } else if e.is_connect() {
                format!("Connection failed: {e}")
            } else {
                format!("Request failed: {e}")
            }
        })?;

        let status_code = response.status().as_u16();
        let headers_out: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();

        // For HEAD requests, return headers only
        if method == "HEAD" {
            return Ok(serde_json::json!({
                "status": "ok",
                "url": final_url,
                "http_status": status_code,
                "method": method,
                "headers": headers_out,
            }));
        }

        // Read body
        let body_bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;
        let truncated = body_bytes.len() > max_size;
        let body_text = if truncated {
            String::from_utf8_lossy(&body_bytes[..max_size]).to_string()
        } else {
            String::from_utf8_lossy(&body_bytes).to_string()
        };

        let is_json = headers_out
            .get("content-type")
            .is_some_and(|ct| ct.contains("json"));

        let parsed_json = if is_json {
            serde_json::from_str::<Value>(&body_text).ok()
        } else {
            None
        };

        Ok(serde_json::json!({
            "status": "ok",
            "url": final_url,
            "method": method,
            "http_status": status_code,
            "headers": headers_out,
            "size_bytes": body_bytes.len(),
            "truncated": truncated,
            "body": body_text,
            "json": parsed_json,
        }))
    }
}

#[derive(serde::Deserialize)]
struct MultipartPart {
    name: String,
    value: Option<String>,
    file_path: Option<String>,
}

fn url_escape(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(FetchTool));
    registry.register(Box::new(HttpClientTool));
}
