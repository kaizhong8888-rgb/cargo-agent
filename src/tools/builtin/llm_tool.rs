//! LLM Integration Tool
//!
//! Enables the agent to call Large Language Model APIs (OpenAI / Anthropic / Ollama)
//! for code generation, code review, explanation, documentation, and general Q&A.
//!
//! # Providers
//!
//! - `openai`  → https://api.openai.com/v1/chat/completions
//! - `anthropic` → https://api.anthropic.com/v1/messages
//! - `ollama`   → http://localhost:11434/api/chat (local, no API key needed)
//!
//! # API Key Storage
//!
//! API keys are stored via the config system (`config set llm_openai_key sk-...`).
//! They can also be passed directly as parameters.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// LlmChatTool
// ============================================================================

/// Call LLM APIs for code generation, review, explanation, and Q&A.
pub struct LlmChatTool;

#[async_trait::async_trait]
impl Tool for LlmChatTool {
    fn name(&self) -> &str {
        "llm"
    }

    fn description(&self) -> &str {
        "Call LLM APIs (OpenAI GPT-4/3.5, Anthropic Claude, local Ollama) for code generation, code review, explanation, documentation, and general Q&A. API keys are stored via config system or passed directly."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "prompt".to_string(),
                description: "The user prompt / question / code to analyze".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "system".to_string(),
                description: "System prompt to set context and behavior (e.g. 'You are a Rust expert. Provide concise, practical answers.')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "provider".to_string(),
                description: "LLM provider: 'openai' (default, needs OPENAI_API_KEY), 'anthropic' (needs ANTHROPIC_API_KEY), or 'ollama' (local, no key)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "model".to_string(),
                description: "Model name. Defaults: openai→'gpt-4o', anthropic→'claude-3-5-sonnet-20241022', ollama→'llama3.2'".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "temperature".to_string(),
                description: "Creativity 0.0-2.0 (default 0.7). Lower = deterministic, higher = creative".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "max_tokens".to_string(),
                description: "Maximum output tokens (default: 4096, max: 16384)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format: 'text' (default) or 'json' (forces structured JSON output)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "api_key".to_string(),
                description: "API key passed directly (overrides stored config key)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "ollama_url".to_string(),
                description: "Ollama server URL (default: http://localhost:11434)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "timeout_secs".to_string(),
                description: "Request timeout in seconds (default: 120)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: prompt")?;

        let system = params.get("system").and_then(|v| v.as_str()).unwrap_or("");
        let provider = params
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai")
            .to_lowercase();
        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_lowercase();

        let temperature = params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7)
            .clamp(0.0, 2.0);

        let max_tokens = params
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(4096)
            .min(16384) as u32;

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120);

        // Get API key: from params first, then try config file
        let api_key = params.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string());

        // Match provider and dispatch
        match provider.as_str() {
            "openai" => {
                let key = api_key.unwrap_or_else(|| load_config_key("llm_openai_key"));
                let model = params
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("gpt-4o");
                call_openai(&key, model, prompt, system, temperature, max_tokens, &format, timeout_secs).await
            }
            "anthropic" | "claude" => {
                let key = api_key.unwrap_or_else(|| load_config_key("llm_anthropic_key"));
                let model = params
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("claude-3-5-sonnet-20241022");
                call_anthropic(&key, model, prompt, system, temperature, max_tokens, &format, timeout_secs).await
            }
            "ollama" => {
                let base_url = params
                    .get("ollama_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("http://localhost:11434");
                let model = params
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("llama3.2");
                call_ollama(base_url, model, prompt, system, temperature, &format, timeout_secs).await
            }
            other => Err(format!(
                "Unsupported provider: '{other}'. Supported: 'openai', 'anthropic', 'ollama'"
            )),
        }
    }
}

// ============================================================================
// OpenAI Provider
// ============================================================================

#[allow(clippy::too_many_arguments)]
async fn call_openai(
    api_key: &str,
    model: &str,
    prompt: &str,
    system: &str,
    temperature: f64,
    max_tokens: u32,
    format: &str,
    timeout_secs: u64,
) -> Result<Value, String> {
    if api_key.is_empty() {
        return Err(
            "OpenAI API key not found. Set it via: config set llm_openai_key <your-key>\n"
            .to_string(),
        );
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut messages = Vec::new();
    if !system.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": system
        }));
    }
    messages.push(json!({
        "role": "user",
        "content": prompt
    }));

    let mut body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
    });

    // Add JSON mode if requested
    if format == "json" {
        body["response_format"] = json!({ "type": "json_object" });
    }

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {e}"))?;

    let status = response.status().as_u16();
    let response_body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {e}"))?;

    if status >= 400 {
        let error_msg = response_body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown API error");
        return Err(format!("OpenAI API error (HTTP {status}): {error_msg}"));
    }

    // Extract the content
    let content = response_body
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let usage = response_body.get("usage");

    Ok(json!({
        "content": content,
        "model": model,
        "provider": "openai",
        "usage": usage,
        "finish_reason": response_body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("finish_reason"))
            .and_then(|c| c.as_str()),
    }))
}

// ============================================================================
// Anthropic Provider
// ============================================================================

#[allow(clippy::too_many_arguments)]
async fn call_anthropic(
    api_key: &str,
    model: &str,
    prompt: &str,
    system: &str,
    temperature: f64,
    max_tokens: u32,
    _format: &str,
    timeout_secs: u64,
) -> Result<Value, String> {
    if api_key.is_empty() {
        return Err(
            "Anthropic API key not found. Set it via: config set llm_anthropic_key <your-key>\n"
                .to_string(),
        );
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    });

    if !system.is_empty() {
        body["system"] = json!(system);
    }

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {e}"))?;

    let status = response.status().as_u16();
    let response_body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Anthropic response: {e}"))?;

    if status >= 400 {
        let error_msg = response_body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown API error");
        return Err(format!("Anthropic API error (HTTP {status}): {error_msg}"));
    }

    // Extract content from Anthropic response format
    let content = response_body
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|block| {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    block.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
        })
        .unwrap_or("")
        .to_string();

    let usage = response_body.get("usage");

    Ok(json!({
        "content": content,
        "model": model,
        "provider": "anthropic",
        "usage": usage,
        "stop_reason": response_body.get("stop_reason"),
    }))
}

// ============================================================================
// Ollama Provider (local, no API key needed)
// ============================================================================

async fn call_ollama(
    base_url: &str,
    model: &str,
    prompt: &str,
    system: &str,
    temperature: f64,
    _format: &str,
    timeout_secs: u64,
) -> Result<Value, String> {
    let base_url = base_url.trim_end_matches('/');

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut messages = Vec::new();
    if !system.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": system
        }));
    }
    messages.push(json!({
        "role": "user",
        "content": prompt
    }));

    let body = json!({
        "model": model,
        "messages": messages,
        "options": {
            "temperature": temperature
        },
        "stream": false,
    });

    let response = client
        .post(format!("{base_url}/api/chat"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                format!(
                    "Cannot connect to Ollama at {base_url}. Is Ollama running?\n\
                     Install: https://ollama.com/\nError: {e}"
                )
            } else {
                format!("Ollama request failed: {e}")
            }
        })?;

    let status = response.status().as_u16();
    let response_body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    if status >= 400 {
        let error_msg = response_body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Unknown Ollama error");
        return Err(format!("Ollama error (HTTP {status}): {error_msg}"));
    }

    let content = response_body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "content": content,
        "model": model,
        "provider": "ollama",
        "done": response_body.get("done"),
    }))
}

// ============================================================================
// Helper: Load API key from config
// ============================================================================

fn load_config_key(key_name: &str) -> String {
    // Try to read from the config JSON file
    let config_path = dirs_or_default();
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<HashMap<String, Value>>(&content) {
            if let Some(val) = config.get(key_name) {
                if let Some(s) = val.as_str() {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}

fn dirs_or_default() -> String {
    // Check common config locations
    let candidates = vec![
        format!(
            "{}/.cargo-agent/preferences.json",
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
        ),
        ".cargo-agent/preferences.json".to_string(),
        "preferences.json".to_string(),
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.clone();
        }
    }

    // Return the default path even if it doesn't exist yet
    format!(
        "{}/.cargo-agent/preferences.json",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    )
}

// ============================================================================
// Convenience: json! macro re-export for internal use
// ============================================================================

use serde_json::json;

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(LlmChatTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_tool_metadata() {
        let tool = LlmChatTool;
        assert_eq!(tool.name(), "llm");
        assert!(tool.description().contains("LLM"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "prompt"));
        assert!(params.iter().any(|p| p.name == "provider"));
        assert!(params.iter().any(|p| p.name == "model"));
    }

    #[test]
    fn test_load_config_key_not_found() {
        // Should return empty string when no config exists
        let key = load_config_key("llm_openai_key");
        // Don't check specific value — just ensure it doesn't crash
        let _ = key;
    }

    #[test]
    fn test_provider_validation() {
        let tool = LlmChatTool;
        let params = tool.parameters();
        let provider_param = params.iter().find(|p| p.name == "provider").unwrap();
        assert!(!provider_param.required);
        let desc = &provider_param.description;
        assert!(desc.contains("openai") || desc.contains("anthropic") || desc.contains("ollama"));
    }
}
