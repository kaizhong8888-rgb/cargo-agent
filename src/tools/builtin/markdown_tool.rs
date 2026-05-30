//! Markdown Processor tool: convert, analyze, lint, and transform Markdown documents.
//!
//! # Actions
//!
//! - **to_html**: Convert Markdown to HTML
//! - **to_text**: Extract plain text from Markdown
//! - **toc**: Generate table of contents
//! - **lint**: Check Markdown formatting issues
//! - **stats**: Document statistics (headings, links, images, code blocks, word count)
//! - **transform**: Format transformations (table alignment, link normalization)
//! - **validate_links**: Check link validity (local files)

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

// ============================================================================
// Pre-compiled regex patterns
// ============================================================================

static RE_HEADING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^(#{1,6})\s+(.+)$"#).expect("valid regex"));

static RE_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\[([^\]]*)\]\(([^)]*)\)"#).expect("valid regex"));

static RE_IMAGE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"!\[([^\]]*)\]\(([^)]*)\)"#).expect("valid regex"));

static RE_CODE_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^```(\w*)\s*$"#).expect("valid regex"));

static RE_INLINE_CODE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"`([^`]+)`"#).expect("valid regex"));

static RE_BOLD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\*\*([^*]+)\*\*|__([^_]+)__"#).expect("valid regex"));

static RE_ITALIC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\*([^*]+)\*|_([^_]+)_|__([^_]+)__"#).expect("valid regex"));

static RE_HR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*[-*_]{3,}\s*$"#).expect("valid regex"));

static RE_BLOCKQUOTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*>\s*(.*)$"#).expect("valid regex"));

static RE_LIST_ITEM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*[-*+]\s+"#).expect("valid regex"));

static RE_ORDERED_LIST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*\d+\.\s+"#).expect("valid regex"));

static RE_TABLE_ROW: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\|.+\|.*$"#).expect("valid regex"));

static RE_FOOTNOTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\[\^([^\]]+)\]"#).expect("valid regex"));

static RE_TODO_ITEM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*[-*+]\s+\[[ xX]\]"#).expect("valid regex"));

static RE_CHECKED_TODO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*[-*+]\s+\[[xX]\]"#).expect("valid regex"));

static RE_UNCHECKED_TODO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)^\s*[-*+]\s+\[ \]"#).expect("valid regex"));

// ============================================================================
// MarkdownProcessorTool
// ============================================================================

pub struct MarkdownProcessorTool;

#[async_trait::async_trait]
impl Tool for MarkdownProcessorTool {
    fn name(&self) -> &str {
        "markdown_tool"
    }

    fn description(&self) -> &str {
        "Process Markdown documents: convert to HTML or plain text, generate table of contents, lint formatting issues, compute statistics (headings/links/images/code blocks/word count), transform formats, and validate local links."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: to_html (convert to HTML), to_text (extract plain text), toc (generate table of contents), lint (check formatting), stats (document statistics), transform (format transformations), validate_links (check local link validity)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "content".to_string(),
                description: "Markdown content (or use path parameter to read from file)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to a Markdown file".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "base_path".to_string(),
                description: "Base directory for resolving relative links (default: directory of the file)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "max_depth".to_string(),
                description: "Maximum heading depth for TOC (default: 6)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let content = load_content(params)?;
        let max_depth = params.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(6) as usize;
        let base_path = params.get("base_path").and_then(|v| v.as_str());

        match action {
            "to_html" => {
                let html = md_to_html(&content);
                Ok(json!({
                    "status": "ok",
                    "action": "to_html",
                    "html": html,
                }))
            }
            "to_text" => {
                let text = md_to_text(&content);
                Ok(json!({
                    "status": "ok",
                    "action": "to_text",
                    "text": text,
                }))
            }
            "toc" => {
                let toc = generate_toc(&content, max_depth);
                Ok(json!({
                    "status": "ok",
                    "action": "toc",
                    "toc": toc,
                    "heading_count": toc.len(),
                }))
            }
            "lint" => {
                let issues = lint_markdown(&content);
                Ok(json!({
                    "status": "ok",
                    "action": "lint",
                    "issues": issues,
                    "issue_count": issues.len(),
                    "severity_summary": compute_severity_summary(&issues),
                }))
            }
            "stats" => {
                let stats = compute_md_stats(&content);
                Ok(json!({
                    "status": "ok",
                    "action": "stats",
                    "stats": stats,
                }))
            }
            "transform" => {
                let transformed = transform_markdown(&content);
                Ok(json!({
                    "status": "ok",
                    "action": "transform",
                    "original_length": content.len(),
                    "transformed_length": transformed.len(),
                    "content": transformed,
                }))
            }
            "validate_links" => {
                let links = validate_links(&content, base_path)?;
                Ok(json!({
                    "status": "ok",
                    "action": "validate_links",
                    "total_links": links.len(),
                    "valid": links.iter().filter(|l| l["valid"].as_bool().unwrap_or(false)).count(),
                    "broken": links.iter().filter(|l| !l["valid"].as_bool().unwrap_or(false)).count(),
                    "links": links,
                }))
            }
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: to_html, to_text, toc, lint, stats, transform, validate_links"),
            })),
        }
    }
}

// ============================================================================
// Markdown to HTML
// ============================================================================

fn md_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_code_block = false;
    let mut _code_lang = String::new();
    let mut code_content = String::new();

    for line in md.lines() {
        if in_code_block {
            if line.starts_with("```") {
                html.push_str(&format!("</code></pre>\n"));
                in_code_block = false;
                code_content.clear();
            } else {
                code_content.push_str(&escape_html(line));
                code_content.push('\n');
            }
            continue;
        }

        if line.starts_with("```") {
            in_code_block = true;
            _code_lang = line.trim_start_matches('`').trim().to_string();
            html.push_str(&format!("<pre><code class=\"language-{}\">\n", _code_lang));
            continue;
        }

        // Headings
        if let Some(cap) = RE_HEADING.captures(line) {
            let level = cap.get(1).map(|m| m.as_str().len()).unwrap_or(1);
            let text = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let text_html = inline_md_to_html(text);
            let id = text.to_lowercase().replace(' ', "-").replace(|c: char| !c.is_alphanumeric() && c != '-', "");
            html.push_str(&format!("<h{level} id=\"{id}\">{text_html}</h{level}>\n"));
            continue;
        }

        // Horizontal rule
        if RE_HR.is_match(line) {
            html.push_str("<hr>\n");
            continue;
        }

        // Blockquote
        if let Some(cap) = RE_BLOCKQUOTE.captures(line) {
            let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            html.push_str(&format!("<blockquote><p>{}</p></blockquote>\n", inline_md_to_html(text)));
            continue;
        }

        // Unordered list
        if RE_LIST_ITEM.is_match(line) {
            let text = line.trim_start_matches(|c| c == '-' || c == '*' || c == '+' || c == ' ');
            // Handle checkboxes
            let text = if text.starts_with("[ ] ") {
                &text[4..]
            } else if text.starts_with("[x] ") || text.starts_with("[X] ") {
                &text[4..]
            } else {
                text
            };
            html.push_str(&format!("<li>{}</li>\n", inline_md_to_html(text)));
            continue;
        }

        // Ordered list
        if RE_ORDERED_LIST.is_match(line) {
            let text = line.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
            html.push_str(&format!("<li>{}</li>\n", inline_md_to_html(text)));
            continue;
        }

        // Empty line
        if line.trim().is_empty() {
            html.push('\n');
            continue;
        }

        // Table row
        if RE_TABLE_ROW.is_match(line) {
            if line.contains("---") || line.contains(":::") {
                // Separator row - skip
                continue;
            }
            let cells: Vec<&str> = line.split('|').filter(|c| !c.trim().is_empty()).collect();
            html.push_str("<tr>");
            for cell in cells {
                html.push_str(&format!("<td>{}</td>", inline_md_to_html(cell.trim())));
            }
            html.push_str("</tr>\n");
            continue;
        }

        // Regular paragraph
        html.push_str(&format!("<p>{}</p>\n", inline_md_to_html(line)));
    }

    if in_code_block {
        html.push_str("</code></pre>\n");
    }

    html
}

fn inline_md_to_html(text: &str) -> String {
    let mut result = text.to_string();

    // Images (must be before links)
    result = RE_IMAGE.replace_all(&result, |caps: &regex::Captures| {
        let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let src = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("<img src=\"{}\" alt=\"{}\">", escape_html(src), escape_html(alt))
    }).to_string();

    // Links
    result = RE_LINK.replace_all(&result, |caps: &regex::Captures| {
        let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("<a href=\"{}\">{}</a>", escape_html(href), escape_html(text))
    }).to_string();

    // Bold
    result = RE_BOLD.replace_all(&result, |caps: &regex::Captures| {
        let text = caps.get(1).or(caps.get(2)).map(|m| m.as_str()).unwrap_or("");
        format!("<strong>{}</strong>", text)
    }).to_string();

    // Italic
    result = RE_ITALIC.replace_all(&result, |caps: &regex::Captures| {
        let text = caps.get(1).or(caps.get(2)).or(caps.get(3)).map(|m| m.as_str()).unwrap_or("");
        format!("<em>{}</em>", text)
    }).to_string();

    // Inline code
    result = RE_INLINE_CODE.replace_all(&result, |caps: &regex::Captures| {
        let code = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        format!("<code>{}</code>", escape_html(code))
    }).to_string();

    result
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ============================================================================
// Markdown to Plain Text
// ============================================================================

fn md_to_text(md: &str) -> String {
    let mut text = String::new();
    let mut in_code_block = false;

    for line in md.lines() {
        if in_code_block {
            if line.starts_with("```") {
                in_code_block = false;
            } else {
                text.push_str(line);
            }
            text.push('\n');
            continue;
        }

        if line.starts_with("```") {
            in_code_block = true;
            continue;
        }

        // Headings
        if let Some(cap) = RE_HEADING.captures(line) {
            let level = cap.get(1).map(|m| m.as_str().len()).unwrap_or(1);
            let content = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            text.push_str(&format!("{}\n{}\n", content, "=".repeat(level * 4)));
            continue;
        }

        // Remove inline formatting
        let mut clean = line.to_string();
        clean = RE_IMAGE.replace_all(&clean, "[Image: $1]").to_string();
        clean = RE_LINK.replace_all(&clean, "$1 ($2)").to_string();
        clean = RE_BOLD.replace_all(&clean, "$1").to_string();
        clean = RE_ITALIC.replace_all(&clean, "$1").to_string();
        clean = RE_INLINE_CODE.replace_all(&clean, "$1").to_string();

        // Remove blockquote markers
        clean = clean.trim_start_matches("> ").trim_start_matches('>').to_string();

        // Remove list markers
        if RE_LIST_ITEM.is_match(&clean) {
            clean = RE_LIST_ITEM.replace_all(&clean, "• ").to_string();
        }
        if RE_ORDERED_LIST.is_match(&clean) {
            clean = RE_ORDERED_LIST.replace_all(&clean, "• ").to_string();
        }

        // Remove horizontal rules
        if RE_HR.is_match(&clean) {
            continue;
        }

        // Remove table formatting
        clean = clean.replace('|', " | ").trim().to_string();

        text.push_str(&clean);
        text.push('\n');
    }

    // Clean up excessive newlines
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }

    text.trim().to_string()
}

// ============================================================================
// Table of Contents
// ============================================================================

fn generate_toc(md: &str, max_depth: usize) -> Vec<Value> {
    let mut toc = Vec::new();

    for cap in RE_HEADING.captures_iter(md) {
        let level = cap.get(1).map(|m| m.as_str().len()).unwrap_or(1);
        if level > max_depth {
            continue;
        }
        let text = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let anchor = text.to_lowercase().replace(' ', "-").replace(|c: char| !c.is_alphanumeric() && c != '-', "");

        toc.push(json!({
            "level": level,
            "text": text,
            "anchor": anchor,
            "indent": "  ".repeat(level - 1),
        }));
    }

    toc
}

// ============================================================================
// Lint
// ============================================================================

fn lint_markdown(md: &str) -> Vec<Value> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = md.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;

        // Trailing whitespace
        if line.ends_with(' ') || line.ends_with('\t') {
            issues.push(json!({
                "line": line_num,
                "rule": "no_trailing_whitespace",
                "severity": "warning",
                "message": "Line has trailing whitespace",
            }));
        }

        // Multiple consecutive blank lines
        if line.trim().is_empty() && idx > 0 && lines[idx - 1].trim().is_empty() {
            issues.push(json!({
                "line": line_num,
                "rule": "no_multiple_blank_lines",
                "severity": "info",
                "message": "Multiple consecutive blank lines",
            }));
        }

        // Heading without space after #
        if regex::Regex::new(r#"^#{2,}[^\s#]"#).ok().and_then(|r| r.captures(line)).is_some() {
            issues.push(json!({
                "line": line_num,
                "rule": "heading_space",
                "severity": "error",
                "message": "Missing space after heading markers",
            }));
        }

        // Line too long
        if line.len() > 120 {
            issues.push(json!({
                "line": line_num,
                "rule": "line_length",
                "severity": "info",
                "message": format!("Line exceeds 120 characters ({} chars)", line.len()),
            }));
        }
    }

    // Check for unclosed code blocks
    let code_block_count = RE_CODE_BLOCK.find_iter(md).count();
    if code_block_count % 2 != 0 {
        issues.push(json!({
            "line": 0,
            "rule": "unclosed_code_block",
            "severity": "error",
            "message": "Unclosed code block (odd number of ``` markers)",
        }));
    }

    // Check for empty links
    for cap in RE_LINK.captures_iter(md) {
        let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if text.is_empty() {
            issues.push(json!({
                "line": 0,
                "rule": "empty_link_text",
                "severity": "warning",
                "message": format!("Link with empty text: {}", href),
            }));
        }
        if href.is_empty() {
            issues.push(json!({
                "line": 0,
                "rule": "empty_link_href",
                "severity": "error",
                "message": "Link with empty URL",
            }));
        }
    }

    // Check heading hierarchy (skip levels)
    let headings: Vec<_> = RE_HEADING.captures_iter(md)
        .filter_map(|c| c.get(1).map(|m| m.as_str().len()))
        .collect();

    for i in 1..headings.len() {
        if headings[i] > headings[i - 1] + 1 {
            issues.push(json!({
                "line": 0,
                "rule": "heading_hierarchy",
                "severity": "warning",
                "message": format!("Heading level jumps from h{} to h{} (skipped h{})", headings[i - 1], headings[i], headings[i - 1] + 1),
            }));
        }
    }

    issues
}

fn compute_severity_summary(issues: &[Value]) -> Value {
    let mut summary: HashMap<String, usize> = HashMap::new();
    for issue in issues {
        let sev = issue["severity"].as_str().unwrap_or("unknown");
        *summary.entry(sev.to_string()).or_insert(0) += 1;
    }
    json!(summary)
}

// ============================================================================
// Statistics
// ============================================================================

fn compute_md_stats(md: &str) -> Value {
    let heading_count = RE_HEADING.find_iter(md).count();
    let link_count = RE_LINK.find_iter(md).count();
    let image_count = RE_IMAGE.find_iter(md).count();
    let code_block_count = RE_CODE_BLOCK.find_iter(md).count() / 2;
    let inline_code_count = RE_INLINE_CODE.find_iter(md).count();
    let bold_count = RE_BOLD.find_iter(md).count();
    let italic_count = RE_ITALIC.find_iter(md).count();
    let hr_count = RE_HR.find_iter(md).count();
    let blockquote_count = RE_BLOCKQUOTE.find_iter(md).count();
    let list_count = RE_LIST_ITEM.find_iter(md).count() + RE_ORDERED_LIST.find_iter(md).count();
    let footnote_count = RE_FOOTNOTE.find_iter(md).count();
    let todo_count = RE_TODO_ITEM.find_iter(md).count();
    let checked_count = RE_CHECKED_TODO.find_iter(md).count();
    let unchecked_count = RE_UNCHECKED_TODO.find_iter(md).count();

    // Word count (exclude code blocks)
    let mut in_code = false;
    let mut word_count = 0;
    for line in md.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            word_count += line.split_whitespace().count();
        }
    }

    let line_count = md.lines().count();

    // Heading level distribution
    let mut heading_levels: HashMap<String, usize> = HashMap::new();
    for cap in RE_HEADING.captures_iter(md) {
        let level = cap.get(1).map(|m| m.as_str().len()).unwrap_or(1);
        *heading_levels.entry(format!("h{}", level)).or_insert(0) += 1;
    }

    json!({
        "lines": line_count,
        "words": word_count,
        "headings": heading_count,
        "heading_levels": heading_levels,
        "links": link_count,
        "images": image_count,
        "code_blocks": code_block_count,
        "inline_code": inline_code_count,
        "bold": bold_count,
        "italic": italic_count,
        "horizontal_rules": hr_count,
        "blockquotes": blockquote_count,
        "list_items": list_count,
        "footnotes": footnote_count,
        "todos": todo_count,
        "todos_checked": checked_count,
        "todos_unchecked": unchecked_count,
    })
}

// ============================================================================
// Transform
// ============================================================================

fn transform_markdown(md: &str) -> String {
    let mut result = md.to_string();

    // Normalize links: remove trailing spaces in URLs
    result = RE_LINK.replace_all(&result, |caps: &regex::Captures| {
        let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim_end();
        format!("[{}]({})", text, href)
    }).to_string();

    // Normalize images
    result = RE_IMAGE.replace_all(&result, |caps: &regex::Captures| {
        let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let src = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim_end();
        format!("![{}]({})", alt, src)
    }).to_string();

    // Fix heading spacing (ensure exactly one space after #)
    result = regex::Regex::new(r#"^(#+)\s+(.+)$"#)
        .ok()
        .map(|re| re.replace_all(&result, |caps: &regex::Captures| {
            let hashes = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let text = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            format!("{} {}", hashes, text)
        }).to_string())
        .unwrap_or(result);

    result
}

// ============================================================================
// Link Validation
// ============================================================================

fn validate_links(md: &str, base_path: Option<&str>) -> Result<Vec<Value>, String> {
    let mut results = Vec::new();

    // Get base directory
    let base_dir = if let Some(bp) = base_path {
        std::path::PathBuf::from(bp)
    } else {
        std::env::current_dir().unwrap_or_default()
    };

    for cap in RE_LINK.captures_iter(md) {
        let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        // Skip external URLs and anchors
        if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") || href.starts_with('#') {
            results.push(json!({
                "text": text,
                "href": href,
                "type": "external",
                "valid": true,
                "note": "External link (not validated)",
            }));
            continue;
        }

        // Resolve relative path
        let link_path = base_dir.join(href);
        let exists = link_path.exists();
        let is_file = exists && link_path.is_file();
        let is_dir = exists && link_path.is_dir();

        results.push(json!({
            "text": text,
            "href": href,
            "type": "local",
            "valid": exists,
            "is_file": is_file,
            "is_directory": is_dir,
        }));
    }

    // Also check image links
    for cap in RE_IMAGE.captures_iter(md) {
        let alt = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let src = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            results.push(json!({
                "text": format!("[Image: {}]", alt),
                "href": src,
                "type": "external_image",
                "valid": true,
                "note": "External image (not validated)",
            }));
            continue;
        }

        let img_path = base_dir.join(src);
        let exists = img_path.exists();

        results.push(json!({
            "text": format!("[Image: {}]", alt),
            "href": src,
            "type": "local_image",
            "valid": exists,
        }));
    }

    Ok(results)
}

// ============================================================================
// Utility
// ============================================================================

fn load_content(params: &HashMap<String, Value>) -> Result<String, String> {
    if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
        fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
    } else if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
        Ok(content.to_string())
    } else {
        Err("Missing required parameter: content or path".to_string())
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(MarkdownProcessorTool));
}
