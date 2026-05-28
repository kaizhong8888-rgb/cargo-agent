use anyhow::{Result, bail};
use serde_json::Value;

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
}

impl ModelClient {
    /// Create a new model client.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::model::client::ModelClient;
    ///
    /// // In production, load these from configuration
    /// let client = ModelClient::new(
    ///     "sk-xxx".into(),
    ///     "deepseek-v4-flash".into(),
    ///     "https://api.deepseek.com".into(),
    /// );
    /// ```
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        }
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

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Chat request failed after {} retries", MAX_RETRIES)))
    }

    /// Single attempt at the chat API call (no retry logic).
    async fn chat_once(
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

        // DeepSeek reasoning models: pass reasoning_content back
        let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
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
        Self::parse_response(data)
    }

    fn parse_response(data: Value) -> Result<ModelResponse> {
        let choice = &data["choices"][0];
        let message_data = &choice["message"];

        // DeepSeek reasoning models return reasoning_content alongside content
        let reasoning = message_data["reasoning_content"]
            .as_str()
            .map(|s| s.to_string());

        let content = message_data["content"]
            .as_str()
            .map(|s| s.to_string());

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
        assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
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

        let response = ModelClient::parse_response(data).unwrap();
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

        let response = ModelClient::parse_response(data).unwrap();
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

        let response = ModelClient::parse_response(data).unwrap();
        assert_eq!(response.content.as_deref(), Some("Final answer"));
        assert_eq!(response.reasoning.as_deref(), Some("Let me think about this..."));
    }
}
