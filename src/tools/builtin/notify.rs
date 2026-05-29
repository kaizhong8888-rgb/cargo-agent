//! Notification tool: send alerts via webhooks (Slack, DingTalk, etc).
//!
//! Supports multiple notification channels through webhook URLs.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(NotifyTool));
}

struct NotifyTool;

#[async_trait::async_trait]
impl Tool for NotifyTool {
    fn name(&self) -> &str {
        "notify"
    }

    fn description(&self) -> &str {
        "Send notifications via webhooks (Slack, DingTalk, custom). \
         Actions: send (POST JSON to webhook URL)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: send".to_string(),
                required: true,
            },
            ToolParameter {
                name: "webhook_url".to_string(),
                parameter_type: "string".to_string(),
                description: "Webhook URL to POST to".to_string(),
                required: true,
            },
            ToolParameter {
                name: "message".to_string(),
                parameter_type: "string".to_string(),
                description: "Message content to send".to_string(),
                required: true,
            },
            ToolParameter {
                name: "channel".to_string(),
                parameter_type: "string".to_string(),
                description: "Channel type: slack, dingtalk, custom (default: custom)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let webhook_url = params
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .ok_or("webhook_url is required".to_string())?;
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or("message is required".to_string())?;
        let channel = params
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("custom");

        if action != "send" {
            return Err(format!("Unknown action: {action}. Valid: send"));
        }

        let payload = build_payload(channel, message);

        let client = reqwest::Client::new();
        match client.post(webhook_url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    Ok(Value::String(format!(
                        "Notification sent to {webhook_url} via {channel}"
                    )))
                } else {
                    Ok(Value::String(format!(
                        "Notification failed: HTTP {}",
                        resp.status()
                    )))
                }
            }
            Err(e) => Err(format!("Failed to send notification: {e}")),
        }
    }
}

fn build_payload(channel: &str, message: &str) -> Value {
    match channel {
        "slack" => serde_json::json!({ "text": message }),
        "dingtalk" => serde_json::json!({
            "msgtype": "text",
            "text": { "content": message }
        }),
        _ => serde_json::json!({ "message": message }),
    }
}
