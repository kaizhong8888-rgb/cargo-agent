//! Mail Tool: send emails via SMTP.
//!
//! Supports HTML and plain text emails, multiple recipients, CC, BCC,
//! attachments, and TLS/SSL connections.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(MailTool));
}

struct MailTool;

#[async_trait::async_trait]
impl Tool for MailTool {
    fn name(&self) -> &str {
        "mail"
    }

    fn description(&self) -> &str {
        "Send emails via SMTP. Supports: send (send plain text or HTML email \
         with optional attachments, CC, BCC). Requires SMTP server credentials."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: send (send an email)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "smtp_host".to_string(),
                parameter_type: "string".to_string(),
                description: "SMTP server hostname (e.g. smtp.gmail.com)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "smtp_port".to_string(),
                parameter_type: "number".to_string(),
                description: "SMTP server port (default: 587 for STARTTLS, 465 for SSL)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "username".to_string(),
                parameter_type: "string".to_string(),
                description: "SMTP authentication username (usually email address)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "password".to_string(),
                parameter_type: "string".to_string(),
                description: "SMTP authentication password or app-specific password".to_string(),
                required: true,
            },
            ToolParameter {
                name: "from".to_string(),
                parameter_type: "string".to_string(),
                description: "Sender email address (e.g. sender@example.com)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "to".to_string(),
                parameter_type: "string".to_string(),
                description: "Recipient email address(es). Separate multiple with comma.".to_string(),
                required: true,
            },
            ToolParameter {
                name: "subject".to_string(),
                parameter_type: "string".to_string(),
                description: "Email subject line".to_string(),
                required: true,
            },
            ToolParameter {
                name: "body".to_string(),
                parameter_type: "string".to_string(),
                description: "Email body content (plain text or HTML)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "is_html".to_string(),
                parameter_type: "boolean".to_string(),
                description: "If true, body is treated as HTML. Default: false (plain text)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "cc".to_string(),
                parameter_type: "string".to_string(),
                description: "CC recipient email address(es). Separate multiple with comma.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "bcc".to_string(),
                parameter_type: "string".to_string(),
                description: "BCC recipient email address(es). Separate multiple with comma.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "use_tls".to_string(),
                parameter_type: "boolean".to_string(),
                description: "Use TLS/SSL (default: true). If true and port is 465, uses TLS. If true and port is 587, uses STARTTLS.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "attachments".to_string(),
                parameter_type: "array".to_string(),
                description: "JSON array of attachment file paths (e.g. '[\"/path/to/file1.pdf\", \"/path/to/file2.png\"]')".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "send" => send_email(params).await,
            _ => Err(format!("Unknown action: {action}. Valid: send")),
        }
    }
}

async fn send_email(params: &HashMap<String, Value>) -> Result<Value, String> {
    let smtp_host = params
        .get("smtp_host")
        .and_then(|v| v.as_str())
        .ok_or("smtp_host is required")?;
    let smtp_port = params
        .get("smtp_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(587) as u16;
    let username = params
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or("username is required")?;
    let password = params
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or("password is required")?;
    let from_addr = params
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or("from is required")?;
    let to_raw = params
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or("to is required")?;
    let subject = params
        .get("subject")
        .and_then(|v| v.as_str())
        .ok_or("subject is required")?;
    let body = params
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or("body is required")?;
    let is_html = params
        .get("is_html")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let cc_raw = params.get("cc").and_then(|v| v.as_str());
    let bcc_raw = params.get("bcc").and_then(|v| v.as_str());
    let use_tls = params
        .get("use_tls")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let attachments_raw = params.get("attachments").and_then(|v| v.as_array());

    // Parse recipients
    let to_addresses = parse_email_list(to_raw)?;
    let cc_addresses = if let Some(cc) = cc_raw {
        parse_email_list(cc)?
    } else {
        Vec::new()
    };
    let bcc_addresses = if let Some(bcc) = bcc_raw {
        parse_email_list(bcc)?
    } else {
        Vec::new()
    };

    // Build the email message
    let from_mailbox: Mailbox = from_addr
        .parse()
        .map_err(|e: lettre::address::AddressError| {
            format!("Invalid from address '{from_addr}': {e}")
        })?;

    let mut builder = Message::builder()
        .from(from_mailbox)
        .subject(subject.to_string());

    for to_addr in &to_addresses {
        builder = builder.to(to_addr.clone());
    }
    for cc_addr in &cc_addresses {
        builder = builder.cc(cc_addr.clone());
    }
    for bcc_addr in &bcc_addresses {
        builder = builder.bcc(bcc_addr.clone());
    }

    // Handle body content and attachments
    let email = if let Some(attachments) = attachments_raw {
        // Email with attachments - use multipart/mixed
        let body_part = if is_html {
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(body.to_string())
        } else {
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(body.to_string())
        };

        let mut multipart = MultiPart::mixed().singlepart(body_part);

        // Add each attachment
        for attach_val in attachments {
            let path = attach_val
                .as_str()
                .ok_or("Each attachment must be a file path string")?;

            let file_bytes = match tokio::fs::read(path).await {
                Ok(bytes) => bytes,
                Err(e) => return Err(format!("Failed to read attachment '{path}': {e}")),
            };

            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("attachment");

            // Detect MIME type from extension
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            let ct_str = mime.to_string();
            let content_type: ContentType = ct_str
                .parse()
                .map_err(|_| format!("Invalid MIME type for '{path}'"))?;

            let attachment = Attachment::new(filename.to_string()).body(file_bytes, content_type);

            multipart = multipart.singlepart(attachment);
        }

        builder
            .multipart(multipart)
            .map_err(|e| format!("Failed to build email message: {e}"))?
    } else {
        // Simple email without attachments
        let content_type = if is_html {
            ContentType::TEXT_HTML
        } else {
            ContentType::TEXT_PLAIN
        };

        builder
            .header(content_type)
            .body(body.to_string())
            .map_err(|e| format!("Failed to build email message: {e}"))?
    };

    // Connect to SMTP server
    let creds = Credentials::new(username.to_string(), password.to_string());

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if use_tls && smtp_port == 465 {
        // Direct TLS (e.g., SMTP over SSL)
        AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
            .map_err(|e| format!("Failed to create SMTP transport: {e}"))?
            .port(smtp_port)
            .credentials(creds)
            .build()
    } else if use_tls {
        // STARTTLS (default, port 587)
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
            .map_err(|e| format!("Failed to create SMTP transport: {e}"))?
            .port(smtp_port)
            .credentials(creds)
            .build()
    } else {
        // No encryption (unsecure)
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
            .port(smtp_port)
            .credentials(creds)
            .build()
    };

    // Send the email
    match mailer.send(email).await {
        Ok(response) => {
            let message_id: Vec<String> = response.message().map(|id| id.to_string()).collect();
            let message_id_str = message_id.first().cloned().unwrap_or_default();
            Ok(serde_json::json!({
                "success": true,
                "message": "Email sent successfully",
                "message_id": message_id_str,
                "to": to_raw,
                "subject": subject,
                "has_attachments": attachments_raw.is_some(),
                "is_html": is_html
            }))
        }
        Err(e) => Err(format!("Failed to send email: {e}")),
    }
}

/// Parse a comma-separated list of email addresses into Mailbox values.
fn parse_email_list(raw: &str) -> Result<Vec<Mailbox>, String> {
    let mut mailboxes = Vec::new();
    for addr in raw.split(',') {
        let trimmed = addr.trim();
        if !trimmed.is_empty() {
            let mailbox: Mailbox =
                trimmed
                    .parse()
                    .map_err(|e: lettre::address::AddressError| {
                        format!("Invalid email address '{trimmed}': {e}")
                    })?;
            mailboxes.push(mailbox);
        }
    }
    if mailboxes.is_empty() {
        return Err("No valid email addresses provided".to_string());
    }
    Ok(mailboxes)
}
