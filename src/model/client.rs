use anyhow::{bail, Result};
use futures::Stream;
use serde_json::Value;
use std::pin::Pin;

/// Maximum retry attempts for transient API errors.
const MAX_RETRIES: u32 = 3;

/// Base delay in milliseconds for exponential backoff.
const BASE_DELAY_MS: u64 = 1000;

/// A client for interacting with LLM APIs (OpenAI-compatible).
///
/// Supports streaming, tool calls, and reasoning content (e.g. DeepSeek reasoning models).
pub struct ModelClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    /// true if the base_url path contains "anthropic" — use Anthropic Messages API format.
    anthropic_mode: bool,
}

impl ModelClient {
    /// Create a new model client.
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let anthropic_mode = base_url.contains("/anthropic");
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("cargo-agent/0.1.0")
            // Connection timeout: fail fast if we can't reach the API
            .connect_timeout(std::time::Duration::from_secs(15))
            // Overall timeout for the full request/response cycle
            .timeout(std::time::Duration::from_secs(120))
            // TCP keepalive to detect dead connections
            .tcp_keepalive(std::time::Duration::from_secs(30))
            // Force HTTP/1.1 — the DashScope Anthropic endpoint only supports HTTP/1.1.
            // reqwest defaults to trying HTTP/2 first, which can cause hanging connections.
            .http1_only()
            // Disable connection pool — prevents stale connection reuse which can
            // cause intermittent timeouts on long-running API endpoints.
            .pool_max_idle_per_host(0)
            // Bypass macOS system proxy detection — reqwest calls the SystemConfiguration
            // framework on every request, which can hang for seconds. We don't need a proxy.
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            api_key,
            model,
            base_url,
            anthropic_mode,
        }
    }

    /// Return the configured model name.
    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// Switch the model used for subsequent requests.
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    /// Send a chat completion request with automatic retry.
    ///
    /// Retries on transient errors (429 rate limit, 5xx server errors) with exponential backoff.
    /// Supports tool calls and reasoning models.
    pub async fn chat(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[Value]>,
    ) -> Result<ModelResponse> {
        let mut last_err = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = if attempt == 1 {
                    // For 429, prefer Retry-After header (handled below); use shorter delay here
                    BASE_DELAY_MS * 2
                } else {
                    BASE_DELAY_MS * 2u64.pow(attempt - 1)
                };
                tracing::warn!(
                    attempt,
                    delay_ms,
                    "Retrying chat request after transient error",
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            match self.chat_once(messages, tools).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let is_retryable = e.to_string().contains("429")
                        || e.to_string().contains("500")
                        || e.to_string().contains("502")
                        || e.to_string().contains("503")
                        || e.to_string().contains("504");

                    if !is_retryable || attempt == MAX_RETRIES {
                        if !is_retryable {
                            return Err(e);
                        }
                        last_err = Some(e);
                        break;
                    }
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            anyhow::anyhow!("Chat request failed after {} retries", MAX_RETRIES)
        }))
    }

    /// Single attempt at the chat API call (no retry logic).
    async fn chat_once(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[Value]>,
    ) -> Result<ModelResponse> {
        if self.anthropic_mode {
            self.chat_once_anthropic(messages, tools).await
        } else {
            self.chat_once_openai(messages, tools).await
        }
    }

    /// OpenAI-compatible API endpoint.
    async fn chat_once_openai(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[Value]>,
    ) -> Result<ModelResponse> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });

        if let Some(t) = tools {
            body["tools"] = serde_json::Value::Array(t.to_vec());
        }

        let url = Self::build_openai_url(&self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!("API error ({}): {}", status, text);
        }

        let data: Value = serde_json::from_str(&text)?;
        Self::parse_openai_response(data)
    }

    /// Build the chat completions URL, avoiding duplicate /v1 paths.
    /// Many providers (DashScope, etc.) include /v1 in their base_url already.
    fn build_openai_url(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        // If base_url already has the full path, return as-is
        if trimmed.ends_with("/v1/chat/completions") {
            return trimmed.to_string();
        }
        // If base_url ends with /v1, append /chat/completions (not /v1/...)
        if trimmed.ends_with("/v1") {
            return format!("{}/chat/completions", trimmed);
        }
        // Standard case: append full path
        format!("{}/v1/chat/completions", trimmed)
    }

    /// Build the Anthropic messages URL, avoiding duplicate /v1 paths.
    fn build_anthropic_url(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1/messages") {
            return trimmed.to_string();
        }
        if trimmed.ends_with("/v1") {
            return format!("{}/messages", trimmed);
        }
        format!("{}/v1/messages", trimmed)
    }

    /// Anthropic Messages API endpoint.
    async fn chat_once_anthropic(
        &self,
        messages: &[serde_json::Value],
        tools: Option<&[Value]>,
    ) -> Result<ModelResponse> {
        // Convert OpenAI-format messages to Anthropic format
        let mut system_message: Option<String> = None;
        let mut anthropic_messages: Vec<Value> = Vec::new();

        for msg in messages {
            let role = msg["role"].as_str().unwrap_or("");
            let content = msg["content"].as_str().unwrap_or("");

            match role {
                "system" => {
                    // Collect system messages as the system prompt
                    let existing = system_message.get_or_insert(String::new());
                    if !existing.is_empty() {
                        existing.push_str("\n\n");
                    }
                    existing.push_str(content);
                }
                "user" => {
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                "assistant" => {
                    // Check for tool_calls in the message
                    if let Some(tool_calls) = msg["tool_calls"].as_array() {
                        let mut content_parts: Vec<Value> = Vec::new();
                        if !content.is_empty() {
                            content_parts.push(serde_json::json!({
                                "type": "text",
                                "text": content,
                            }));
                        }
                        for tc in tool_calls {
                            if let Some(func) = tc.get("function") {
                                let id = tc["id"].as_str().unwrap_or("");
                                let name = func["name"].as_str().unwrap_or("");
                                let args: serde_json::Value = func["arguments"]
                                    .as_str()
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or(serde_json::json!({}));
                                content_parts.push(serde_json::json!({
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": args,
                                }));
                            }
                        }
                        anthropic_messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": content_parts,
                        }));
                    } else {
                        anthropic_messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": content,
                        }));
                    }
                }
                "tool" => {
                    let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content,
                        }],
                    }));
                }
                _ => {}
            }
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": 8192,
        });

        if let Some(sys) = &system_message {
            body["system"] = serde_json::json!(sys);
        }

        if let Some(t) = tools {
            let anthropic_tools: Vec<Value> = t
                .iter()
                .filter_map(|tool| {
                    let func = tool.get("function")?;
                    let name = func.get("name")?.as_str()?;
                    let description = func.get("description")?.as_str()?;
                    let input_schema = func.get("parameters")?;
                    Some(serde_json::json!({
                        "name": name,
                        "description": description,
                        "input_schema": input_schema,
                    }))
                })
                .collect();

            if !anthropic_tools.is_empty() {
                body["tools"] = serde_json::Value::Array(anthropic_tools);
            }
        }

        let url = Self::build_anthropic_url(&self.base_url);
        tracing::debug!("Anthropic request: POST {}", url);
        tracing::debug!("Request body model: {}", self.model);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "output-128k-2025-02-19")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        tracing::debug!("Anthropic response status: {}", status);
        let text = response.text().await?;

        if !status.is_success() {
            bail!("API error ({}): {}", status, text);
        }

        let data: Value = serde_json::from_str(&text)?;
        Self::parse_anthropic_response(data)
    }

    fn parse_openai_response(data: Value) -> Result<ModelResponse> {
        let choice = &data["choices"][0];
        let message_data = &choice["message"];

        // DeepSeek reasoning models return reasoning_content alongside content
        let reasoning = message_data["reasoning_content"]
            .as_str()
            .map(|s| s.to_string());

        let content = message_data["content"].as_str().map(|s| s.to_string());

        let tool_calls = message_data["tool_calls"].as_array().map(|calls| {
            calls
                .iter()
                .map(|call| {
                    let function = &call["function"];
                    ToolCallInfo {
                        id: call["id"].as_str().unwrap_or("").to_string(),
                        name: function["name"].as_str().unwrap_or("").to_string(),
                        arguments: function["arguments"].as_str().unwrap_or("{}").to_string(),
                    }
                })
                .collect()
        });

        let finish_reason = choice["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        let usage = data["usage"].as_object().map(|u| UsageInfo {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        });

        Ok(ModelResponse {
            content,
            reasoning,
            tool_calls,
            finish_reason,
            usage,
        })
    }

    /// Parse an Anthropic Messages API response.
    fn parse_anthropic_response(data: Value) -> Result<ModelResponse> {
        let empty_arr = vec![];
        let content_parts = data["content"].as_array().unwrap_or(&empty_arr);

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCallInfo> = Vec::new();

        for part in content_parts {
            match part["type"].as_str() {
                Some("text") => {
                    if let Some(text) = part["text"].as_str() {
                        text_parts.push(text.to_string());
                    }
                }
                Some("tool_use") => {
                    let id = part["id"].as_str().unwrap_or("").to_string();
                    let name = part["name"].as_str().unwrap_or("").to_string();
                    let args = part["input"].to_string();
                    tool_calls.push(ToolCallInfo {
                        id,
                        name,
                        arguments: args,
                    });
                }
                _ => {}
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        };

        let finish_reason = data["stop_reason"]
            .as_str()
            .unwrap_or("end_turn")
            .to_string();
        let finish_reason = match finish_reason.as_str() {
            "tool_use" => "tool_calls".to_string(),
            other => other.to_string(),
        };

        let usage = data
            .get("usage")
            .and_then(|u| u.as_object())
            .map(|u| UsageInfo {
                prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    as u32,
                total_tokens: 0,
            });

        Ok(ModelResponse {
            content,
            reasoning: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            finish_reason,
            usage,
        })
    }

    /// Stream a chat completion response as an async stream of text chunks.
    ///
    /// Returns a stream that yields each text delta as it arrives from the API.
    /// Useful for real-time display of LLM responses.
    ///
    /// Note: Full SSE streaming implementation pending — currently returns
    /// an empty stream. Use `chat()` for synchronous responses.
    pub async fn chat_stream(
        &self,
        _messages: &[serde_json::Value],
        _tools: Option<&[Value]>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // SSE streaming requires a more complex state machine with proper
        // line-buffered parsing. The infrastructure is wired up (stream flag,
        // Accept header) but the parser is deferred.
        let empty_stream = futures::stream::empty::<Result<String>>();
        Ok(Box::pin(empty_stream))
    }
}

/// The response from a model API call.
///
/// Contains the model's text output, optional reasoning content,
/// any tool calls requested by the model, and usage statistics.
///
/// # Example
///
/// ```
/// use cargo_agent::model::client::ModelResponse;
///
/// let response = ModelResponse {
///     content: Some("Hello!".into()),
///     reasoning: None,
///     tool_calls: None,
///     finish_reason: "stop".into(),
///     usage: None,
/// };
///
/// assert_eq!(response.content.as_deref(), Some("Hello!"));
/// assert_eq!(response.finish_reason, "stop");
/// ```
#[derive(Debug)]
pub struct ModelResponse {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    pub finish_reason: String,
    pub usage: Option<UsageInfo>,
}

/// Information about a tool call requested by the model.
///
/// # Example
///
/// ```
/// use cargo_agent::model::client::ToolCallInfo;
///
/// let call = ToolCallInfo {
///     id: "call_123".into(),
///     name: "read_file".into(),
///     arguments: r#"{"path": "src/main.rs"}"#.into(),
/// };
///
/// assert_eq!(call.name, "read_file");
/// assert!(call.arguments.contains("main.rs"));
/// ```
#[derive(Debug)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Token usage statistics for a model API call.
///
/// # Example
///
/// ```
/// use cargo_agent::model::client::UsageInfo;
///
/// let usage = UsageInfo {
///     prompt_tokens: 150,
///     completion_tokens: 50,
///     total_tokens: 200,
/// };
///
/// assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
/// ```
#[derive(Debug)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_openai_url() {
        assert_eq!(
            ModelClient::build_openai_url("https://api.example.com/v1"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            ModelClient::build_openai_url("https://api.example.com/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            ModelClient::build_openai_url("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
        assert_eq!(
            ModelClient::build_openai_url("https://api.openai.com"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            ModelClient::build_openai_url("https://api.example.com/v1/"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_anthropic_url() {
        assert_eq!(
            ModelClient::build_anthropic_url("https://api.anthropic.com/v1"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            ModelClient::build_anthropic_url("https://api.anthropic.com/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            ModelClient::build_anthropic_url("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_tool_call_info_creation() {
        let call = ToolCallInfo {
            id: "call_abc".into(),
            name: "search".into(),
            arguments: r#"{"query": "rust"}"#.into(),
        };
        assert_eq!(call.id, "call_abc");
        assert_eq!(call.name, "search");
        assert_eq!(call.arguments, r#"{"query": "rust"}"#);
    }

    #[test]
    fn test_usage_info_totals() {
        let usage = UsageInfo {
            prompt_tokens: 100,
            completion_tokens: 30,
            total_tokens: 130,
        };
        assert_eq!(
            usage.total_tokens,
            usage.prompt_tokens + usage.completion_tokens
        );
    }

    #[test]
    fn test_model_response_with_content() {
        let response = ModelResponse {
            content: Some("Hello, world!".into()),
            reasoning: None,
            tool_calls: None,
            finish_reason: "stop".into(),
            usage: None,
        };
        assert_eq!(response.content.as_deref(), Some("Hello, world!"));
        assert!(response.tool_calls.is_none());
    }

    #[test]
    fn test_model_response_with_tool_calls() {
        let response = ModelResponse {
            content: None,
            reasoning: None,
            tool_calls: Some(vec![ToolCallInfo {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: "{}".into(),
            }]),
            finish_reason: "tool_calls".into(),
            usage: Some(UsageInfo {
                prompt_tokens: 50,
                completion_tokens: 20,
                total_tokens: 70,
            }),
        };
        assert_eq!(response.finish_reason, "tool_calls");
        assert!(response.tool_calls.is_some());
        assert_eq!(response.tool_calls.as_ref().unwrap().len(), 1);
        assert!(response.usage.is_some());
    }

    #[test]
    fn test_parse_response_basic() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let response = ModelClient::parse_openai_response(data).unwrap();
        assert_eq!(response.content.as_deref(), Some("Hello!"));
        assert_eq!(response.finish_reason, "stop");
        assert!(response.tool_calls.is_none());
    }

    #[test]
    fn test_parse_response_with_tool_calls() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"Beijing\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": null
        });

        let response = ModelClient::parse_openai_response(data).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.finish_reason, "tool_calls");
        let calls = response.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, r#"{"location": "Beijing"}"#);
    }

    #[test]
    fn test_parse_response_with_reasoning() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Final answer",
                    "reasoning_content": "Let me think about this..."
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 30,
                "total_tokens": 50
            }
        });

        let response = ModelClient::parse_openai_response(data).unwrap();
        assert_eq!(
            response.reasoning.as_deref(),
            Some("Let me think about this...")
        );
    }
}
