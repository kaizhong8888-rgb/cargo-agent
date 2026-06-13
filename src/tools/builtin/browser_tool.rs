//! Browser/Scraper tool - HTML parsing and web scraping
//! Uses reqwest + scraper for headless browsing capabilities.
//! Supports: navigate, extract, links, title, table, search, meta, forms, images, structured_data, headings, readability, diff

use crate::tools::ToolParameter;
use async_trait::async_trait;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

pub struct BrowserTool;

// ============================================================================
// Helper: Build a realistic browser-like HTTP client
// ============================================================================

fn build_client(_cookies: Option<&str>, proxy: Option<&str>) -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true);

    if let Some(proxy_url) = proxy {
        let proxy = reqwest::Proxy::all(proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn build_headers(custom_headers: Option<&HashMap<String, String>>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );

    if let Some(custom) = custom_headers {
        for (key, value) in custom {
            if let Ok(hv) = HeaderValue::from_str(value) {
                if let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                    headers.insert(name, hv);
                }
            }
        }
    }

    headers
}

// ============================================================================
// Helper: Extract text from an element
// ============================================================================

fn element_text(el: &scraper::ElementRef) -> String {
    let mut text = String::new();
    for node in el.descendants() {
        if let scraper::Node::Text(t) = node.value() {
            text.push_str(&t.text);
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ============================================================================
// Helper: Get attribute safely
// ============================================================================

fn get_attr(el: &scraper::ElementRef, attr: &str) -> String {
    el.value().attr(attr).unwrap_or("").to_string()
}

// ============================================================================
// Helper: Get first non-empty attribute from a list
// ============================================================================

fn get_first_attr(el: &scraper::ElementRef, attrs: &[&str]) -> String {
    for &attr in attrs {
        let val = get_attr(el, attr);
        if !val.is_empty() {
            return val;
        }
    }
    String::new()
}

// ============================================================================
// Tool Implementation
// ============================================================================

#[async_trait]
impl crate::tools::Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Headless browser/scraper for web pages: navigate (fetch URL), extract (CSS selector), \
         links (all URLs), title (page title), table (HTML tables), search (text search), \
         meta (page metadata). Supports cookies, custom headers, and proxy."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "Action: navigate, extract, links, title, table, search, meta, forms, images, structured_data, headings, readability, diff".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "url".into(),
                description: "URL to fetch (for navigate/links/title/table/meta/forms/images/headings/readability/diff)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "html".into(),
                description: "Raw HTML string to parse (alternative to url)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "html2".into(),
                description: "Second HTML string for diff action (alternative to url2)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "url2".into(),
                description: "Second URL for diff action (compare two pages)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "selector".into(),
                description: "CSS selector (for extract action, e.g. 'div.price', 'a[href]')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "attribute".into(),
                description: "Attribute to extract (for extract action, e.g. 'href', 'src', 'class'). Default: text".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "query".into(),
                description: "Search query text (for search action)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "headers".into(),
                description: "JSON object of custom headers (e.g. '{\"Authorization\": \"Bearer token\"}')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "cookies".into(),
                description: "Cookie string (e.g. 'session=abc; token=xyz')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "proxy".into(),
                description: "Proxy URL (e.g. 'http://127.0.0.1:7890')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "limit".into(),
                description: "Maximum number of results (default: 50)".into(),
                required: false,
                parameter_type: "number".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        match action {
            "navigate" => navigate(params).await,
            "extract" => extract(params).await,
            "links" => links(params).await,
            "title" => title(params).await,
            "table" => table(params).await,
            "search" => search(params).await,
            "meta" => meta(params).await,
            "forms" => forms(params).await,
            "images" => images(params).await,
            "structured_data" => structured_data(params).await,
            "headings" => headings(params).await,
            "readability" => readability(params).await,
            "diff" => diff_pages(params).await,
            _ => Err(format!(
                "Unknown action: {}. Supported: navigate, extract, links, title, table, search, meta, forms, images, structured_data, headings, readability, diff",
                action
            )),
        }
    }
}

// ============================================================================
// Helper: Fetch URL or parse raw HTML
// ============================================================================

async fn fetch_html(params: &HashMap<String, Value>) -> Result<(Html, String), String> {
    if let Some(raw_html) = params.get("html").and_then(|v| v.as_str()) {
        let document = Html::parse_document(raw_html);
        return Ok((document, raw_html.to_string()));
    }

    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: url or html".to_string())?;

    let custom_headers: Option<HashMap<String, String>> = params
        .get("headers")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok());

    let cookies = params.get("cookies").and_then(|v| v.as_str());
    let proxy = params.get("proxy").and_then(|v| v.as_str());

    let client = build_client(cookies, proxy)?;
    let headers = build_headers(custom_headers.as_ref());

    let mut request = client.get(url).headers(headers);

    if let Some(cookie_str) = cookies {
        request = request.header(reqwest::header::COOKIE, cookie_str);
    }

    let response = request
        .send()
        .map_err(|e| format!("Request failed for {}: {}", url, e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {} for {}", status, url));
    }

    let html_text = response
        .text()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let document = Html::parse_document(&html_text);
    Ok((document, html_text))
}

// ============================================================================
// 1. NAVIGATE
// ============================================================================

async fn navigate(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, html_text) = fetch_html(params).await?;

    let title_sel = Selector::parse("title").unwrap();
    let title = document
        .select(&title_sel)
        .next()
        .map(|el| element_text(&el))
        .unwrap_or_default();

    let link_sel = Selector::parse("a[href]").unwrap();
    let link_count = document.select(&link_sel).count();

    let img_sel = Selector::parse("img[src]").unwrap();
    let img_count = document.select(&img_sel).count();

    let canonical_sel = Selector::parse("link[rel='canonical']").unwrap();
    let canonical = document
        .select(&canonical_sel)
        .next()
        .map(|el| get_attr(&el, "href"))
        .unwrap_or_default();

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "title": title,
        "content_length": html_text.len(),
        "link_count": link_count,
        "image_count": img_count,
        "canonical": canonical,
        "preview": html_text.chars().take(1000).collect::<String>(),
    }))
}

// ============================================================================
// 2. EXTRACT
// ============================================================================

async fn extract(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let selector_str = params
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: selector".to_string())?;

    let attribute = params.get("attribute").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as usize;

    let sel = Selector::parse(selector_str)
        .map_err(|e| format!("Invalid CSS selector '{}': {}", selector_str, e))?;

    let mut results: Vec<Value> = Vec::new();

    for el in document.select(&sel).take(limit) {
        let result = if let Some(attr) = attribute {
            Value::String(get_attr(&el, attr))
        } else {
            Value::String(element_text(&el))
        };
        results.push(result);
    }

    Ok(json!({
        "selector": selector_str,
        "attribute": attribute.unwrap_or("text"),
        "count": results.len(),
        "results": results,
    }))
}

// ============================================================================
// 3. LINKS
// ============================================================================

async fn links(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let link_sel = Selector::parse("a[href]").unwrap();
    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as usize;

    let base_url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("");

    let mut internal: Vec<Value> = Vec::new();
    let mut external: Vec<Value> = Vec::new();

    for el in document.select(&link_sel).take(limit) {
        let href = get_attr(&el, "href");
        if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
            continue;
        }

        let text = element_text(&el);
        let entry = json!({
            "url": href,
            "text": text,
        });

        if href.starts_with("http") && !href.contains(domain) {
            external.push(entry);
        } else {
            internal.push(entry);
        }
    }

    Ok(json!({
        "internal_count": internal.len(),
        "external_count": external.len(),
        "internal": internal,
        "external": external,
    }))
}

// ============================================================================
// 4. TITLE
// ============================================================================

async fn title(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let title_sel = Selector::parse("title").unwrap();
    let page_title = document
        .select(&title_sel)
        .next()
        .map(|el| element_text(&el))
        .unwrap_or_default();

    let h1_sel = Selector::parse("h1").unwrap();
    let h1_tags: Vec<String> = document
        .select(&h1_sel)
        .map(|el| element_text(&el))
        .filter(|t| !t.is_empty())
        .collect();

    Ok(json!({
        "title": page_title,
        "h1_tags": h1_tags,
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

// ============================================================================
// 5. TABLE
// ============================================================================

async fn table(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let table_sel = Selector::parse("table").unwrap();
    let tr_sel = Selector::parse("tr").unwrap();
    let th_sel = Selector::parse("th").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    let table_index = params.get("table").and_then(|v| v.as_i64()).unwrap_or(0) as usize;

    let mut tables: Vec<Value> = Vec::new();

    for (idx, table_el) in document.select(&table_sel).enumerate() {
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut headers: Vec<String> = Vec::new();

        for row in table_el.select(&tr_sel) {
            let cells: Vec<String> = row.select(&td_sel).map(|el| element_text(&el)).collect();

            let th_cells: Vec<String> = row.select(&th_sel).map(|el| element_text(&el)).collect();

            if !th_cells.is_empty() && headers.is_empty() {
                headers = th_cells;
            }

            if !cells.is_empty() {
                rows.push(cells);
            }
        }

        let table_obj = if !headers.is_empty() {
            let objects: Vec<Value> = rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (i, header) in headers.iter().enumerate() {
                        let val = row.get(i).cloned().unwrap_or_default();
                        obj.insert(header.clone(), Value::String(val));
                    }
                    Value::Object(obj)
                })
                .collect();
            json!({
                "index": idx,
                "headers": headers,
                "row_count": rows.len(),
                "data": objects,
            })
        } else {
            json!({
                "index": idx,
                "row_count": rows.len(),
                "data": rows,
            })
        };

        tables.push(table_obj);
    }

    if tables.is_empty() {
        return Ok(json!({
            "message": "No tables found on this page",
            "tables": [],
        }));
    }

    let result = if table_index < tables.len() {
        json!({
            "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
            "table_index": table_index,
            "total_tables": tables.len(),
            "table": tables[table_index],
        })
    } else {
        json!({
            "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
            "total_tables": tables.len(),
            "tables": tables,
        })
    };

    Ok(result)
}

// ============================================================================
// 6. SEARCH
// ============================================================================

async fn search(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _html_text) = fetch_html(params).await?;

    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: query".to_string())?;

    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as usize;

    let body_sel = Selector::parse("body").unwrap();
    let body_text = document
        .select(&body_sel)
        .next()
        .map(|el| element_text(&el))
        .unwrap_or_default();

    let query_lower = query.to_lowercase();
    let text_lower = body_text.to_lowercase();

    let mut occurrences: Vec<Value> = Vec::new();
    let mut start = 0;

    while let Some(pos) = text_lower[start..].find(&query_lower) {
        let actual_pos = start + pos;
        let context_start = actual_pos.saturating_sub(50);
        let context_end = (actual_pos + query.len() + 50).min(body_text.len());

        let context = body_text[context_start..context_end].trim().to_string();
        let line_num = body_text[..actual_pos].matches('\n').count() + 1;

        occurrences.push(json!({
            "position": actual_pos,
            "line": line_num,
            "context": format!("...{}...", context),
        }));

        if occurrences.len() >= limit {
            break;
        }

        start = actual_pos + 1;
    }

    let count = occurrences.len();
    let total_occurrences = text_lower.matches(&query_lower).count();

    Ok(json!({
        "query": query,
        "total_occurrences": total_occurrences,
        "shown": count,
        "occurrences": occurrences,
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

// ============================================================================
// 8. META
// ============================================================================

async fn meta(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let mut meta_data = serde_json::Map::new();

    let meta_sel = Selector::parse("meta").unwrap();
    for el in document.select(&meta_sel) {
        let name = get_first_attr(&el, &["name", "property", "itemprop"]);
        let content = get_attr(&el, "content");

        if !name.is_empty() && !content.is_empty() {
            meta_data.insert(name, Value::String(content));
        }
    }

    let title_sel = Selector::parse("title").unwrap();
    let title = document
        .select(&title_sel)
        .next()
        .map(|el| element_text(&el))
        .unwrap_or_default();

    let canonical_sel = Selector::parse("link[rel='canonical']").unwrap();
    let canonical = document
        .select(&canonical_sel)
        .next()
        .map(|el| get_attr(&el, "href"))
        .unwrap_or_default();

    let html_sel = Selector::parse("html").unwrap();
    let lang = document
        .select(&html_sel)
        .next()
        .map(|el| get_attr(&el, "lang"))
        .unwrap_or_default();

    meta_data.insert("title".to_string(), Value::String(title));
    if !canonical.is_empty() {
        meta_data.insert("canonical".to_string(), Value::String(canonical));
    }
    if !lang.is_empty() {
        meta_data.insert("lang".to_string(), Value::String(lang));
    }

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "meta": meta_data,
    }))
}

// ============================================================================
// 9. FORMS - Extract all forms with fields
// ============================================================================

async fn forms(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;
    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as usize;

    let form_sel = Selector::parse("form").unwrap();
    let input_sel = Selector::parse("input, select, textarea").unwrap();

    let mut forms_list: Vec<Value> = Vec::new();

    for (idx, form_el) in document.select(&form_sel).take(limit).enumerate() {
        let action = get_attr(&form_el, "action");
        let method = get_attr(&form_el, "method");
        let id = get_attr(&form_el, "id");
        let name = get_attr(&form_el, "name");

        let mut fields: Vec<Value> = Vec::new();
        for field in form_el.select(&input_sel) {
            let field_type = get_first_attr(&field, &["type", "tag"]);
            let field_name = get_attr(&field, "name");
            let placeholder = get_attr(&field, "placeholder");
            let required = get_attr(&field, "required");
            let value = get_attr(&field, "value");
            let field_id = get_attr(&field, "id");

            let tag = if field.value().name() == "select" {
                "select"
            } else if field.value().name() == "textarea" {
                "textarea"
            } else {
                "input"
            };

            let mut field_obj = serde_json::Map::new();
            field_obj.insert("tag".to_string(), Value::String(tag.to_string()));
            if !field_type.is_empty() && tag == "input" {
                field_obj.insert("type".to_string(), Value::String(field_type));
            }
            if !field_name.is_empty() {
                field_obj.insert("name".to_string(), Value::String(field_name));
            }
            if !field_id.is_empty() {
                field_obj.insert("id".to_string(), Value::String(field_id));
            }
            if !placeholder.is_empty() {
                field_obj.insert("placeholder".to_string(), Value::String(placeholder));
            }
            if !required.is_empty() {
                field_obj.insert("required".to_string(), Value::Bool(true));
            }
            if !value.is_empty() && tag != "password" {
                field_obj.insert("value".to_string(), Value::String(value));
            }

            // For select, get options
            if tag == "select" {
                let opt_sel = Selector::parse("option").unwrap();
                let options: Vec<Value> = field.select(&opt_sel)
                    .map(|opt| {
                        let opt_value = get_attr(&opt, "value");
                        let opt_text = element_text(&opt);
                        let selected = get_attr(&opt, "selected");
                        json!({
                            "value": if opt_value.is_empty() { opt_text.clone() } else { opt_value },
                            "text": opt_text,
                            "selected": !selected.is_empty(),
                        })
                    })
                    .collect();
                field_obj.insert("options".to_string(), Value::Array(options));
            }

            fields.push(Value::Object(field_obj));
        }

        // Get submit buttons
        let button_sel = Selector::parse("button, input[type='submit']").unwrap();
        let buttons: Vec<Value> = form_el.select(&button_sel)
            .map(|btn| {
                let btn_text = element_text(&btn);
                let btn_type = get_attr(&btn, "type");
                json!({
                    "text": if btn_text.is_empty() { get_attr(&btn, "value") } else { btn_text },
                    "type": if btn_type.is_empty() { "submit".to_string() } else { btn_type },
                })
            })
            .collect();

        let mut form_obj = serde_json::Map::new();
        form_obj.insert("index".to_string(), Value::Number(idx.into()));
        if !id.is_empty() {
            form_obj.insert("id".to_string(), Value::String(id));
        }
        if !name.is_empty() {
            form_obj.insert("name".to_string(), Value::String(name));
        }
        if !action.is_empty() {
            form_obj.insert("action".to_string(), Value::String(action));
        }
        form_obj.insert("method".to_string(), Value::String(if method.is_empty() { "GET".to_string() } else { method.to_uppercase() }));
        form_obj.insert("fields".to_string(), Value::Array(fields));
        if !buttons.is_empty() {
            form_obj.insert("buttons".to_string(), Value::Array(buttons));
        }

        forms_list.push(Value::Object(form_obj));
    }

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "form_count": forms_list.len(),
        "forms": forms_list,
    }))
}

// ============================================================================
// 10. IMAGES - Extract all images with metadata
// ============================================================================

async fn images(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;
    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as usize;

    let img_sel = Selector::parse("img").unwrap();
    let source_sel = Selector::parse("source").unwrap();

    let base_url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");

    let mut images_list: Vec<Value> = Vec::new();

    for el in document.select(&img_sel).take(limit) {
        let src = get_first_attr(&el, &["data-src", "src"]);
        let srcset = get_attr(&el, "srcset");
        let alt = get_attr(&el, "alt");
        let width = get_attr(&el, "width");
        let height = get_attr(&el, "height");
        let loading = get_attr(&el, "loading");

        // Get responsive sources
        let sources: Vec<Value> = el.select(&source_sel)
            .filter_map(|s| {
                let media = get_attr(&s, "media");
                let src = get_first_attr(&s, &["srcset", "src"]);
                if src.is_empty() { None } else {
                    Some(json!({
                        "media": if media.is_empty() { Value::Null } else { Value::String(media) },
                        "src": src,
                    }))
                }
            })
            .collect();

        // Resolve relative URLs
        let resolved_src = resolve_url(base_url, &src);

        let mut img_obj = serde_json::Map::new();
        img_obj.insert("src".to_string(), Value::String(resolved_src));
        if !alt.is_empty() {
            img_obj.insert("alt".to_string(), Value::String(alt));
        }
        if !width.is_empty() {
            img_obj.insert("width".to_string(), Value::String(width));
        }
        if !height.is_empty() {
            img_obj.insert("height".to_string(), Value::String(height));
        }
        if !srcset.is_empty() {
            img_obj.insert("srcset".to_string(), Value::String(srcset));
        }
        if !loading.is_empty() {
            img_obj.insert("loading".to_string(), Value::String(loading));
        }
        if !sources.is_empty() {
            img_obj.insert("sources".to_string(), Value::Array(sources));
        }

        images_list.push(Value::Object(img_obj));
    }

    // Also get picture elements
    let picture_sel = Selector::parse("picture").unwrap();
    let mut picture_count = 0;
    for _ in document.select(&picture_sel) {
        picture_count += 1;
    }

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "image_count": images_list.len(),
        "picture_elements": picture_count,
        "images": images_list,
    }))
}

/// Resolve a relative URL against a base URL
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") || relative.starts_with("//") || relative.starts_with("data:") {
        return relative.to_string();
    }
    if relative.is_empty() {
        return base.to_string();
    }
    // Simple resolution
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(full) = base_url.join(relative) {
            return full.to_string();
        }
    }
    if relative.starts_with('/') {
        if let Ok(base_url) = url::Url::parse(base) {
            if let Some(host) = base_url.host_str() {
                return format!("{}://{}{}", base_url.scheme(), host, relative);
            }
        }
    }
    // Fall back: just combine
    let base_clean = base.trim_end_matches('/');
    if relative.starts_with('/') {
        format!("{}{}", base_clean, relative)
    } else {
        format!("{}/{}", base_clean, relative)
    }
}

// ============================================================================
// 11. STRUCTURED_DATA - Extract JSON-LD and microdata
// ============================================================================

async fn structured_data(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    let mut json_ld: Vec<Value> = Vec::new();
    let mut microdata: Vec<Value> = Vec::new();

    // Extract JSON-LD
    let script_sel = Selector::parse("script[type='application/ld+json']").unwrap();
    for el in document.select(&script_sel) {
        for node in el.descendants() {
            if let scraper::Node::Text(t) = node.value() {
                let text = t.text.trim();
                if !text.is_empty() {
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        json_ld.push(parsed);
                    }
                }
            }
        }
    }

    // Extract microdata
    let itemscope_sel = Selector::parse("[itemscope]").unwrap();
    for el in document.select(&itemscope_sel) {
        let itemtype = get_attr(&el, "itemtype");
        let itemid = get_attr(&el, "itemid");

        let mut properties: serde_json::Map<String, Value> = serde_json::Map::new();
        let itemprop_sel = Selector::parse("[itemprop]").unwrap();
        for prop_el in el.select(&itemprop_sel) {
            let prop_name = get_attr(&prop_el, "itemprop");
            let prop_content = get_first_attr(&prop_el, &["content", "href", "src"]);
            let prop_text = if prop_content.is_empty() {
                element_text(&prop_el)
            } else {
                prop_content
            };
            if !prop_name.is_empty() {
                properties.insert(prop_name, Value::String(prop_text));
            }
        }

        let mut item = serde_json::Map::new();
        if !itemtype.is_empty() {
            item.insert("type".to_string(), Value::String(itemtype));
        }
        if !itemid.is_empty() {
            item.insert("id".to_string(), Value::String(itemid));
        }
        if !properties.is_empty() {
            item.insert("properties".to_string(), Value::Object(properties));
        }
        microdata.push(Value::Object(item));
    }

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "json_ld_count": json_ld.len(),
        "microdata_count": microdata.len(),
        "json_ld": json_ld,
        "microdata": microdata,
    }))
}

// ============================================================================
// 12. HEADINGS - Extract document outline (h1-h6)
// ============================================================================

async fn headings(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, _) = fetch_html(params).await?;

    // Build outline by tag level
    let heading_tags = ["h1", "h2", "h3", "h4", "h5", "h6"];
    for tag in heading_tags {
        let sel = Selector::parse(tag).map_err(|e| format!("Invalid selector: {}", e))?;
        let found: Vec<(String, String)> = document.select(&sel)
            .map(|el| (tag.to_string(), element_text(&el).trim().to_string()))
            .filter(|(_, t)| !t.is_empty())
            .collect();
        for (t, text) in found {
            outline.push(json!({
                "level": t.chars().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(0),
                "tag": t,
                "text": text,
            }));
        }
    }

    let h1_count = outline.iter().filter(|h| h["level"] == 1).count();
    let total = outline.len();

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "heading_count": total,
        "h1_count": h1_count,
        "outline": outline,
    }))
}

// ============================================================================
// 13. READABILITY - Extract main content (like Reader mode)
// ============================================================================

async fn readability(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (document, html_text) = fetch_html(params).await?;

    let body_sel = Selector::parse("body").unwrap();
    let body = document.select(&body_sel).next()
        .ok_or_else(|| "No <body> element found".to_string())?;

    // Find content-rich container (article, main, or body)
    let content = if let Ok(article_sel) = Selector::parse("article") {
        document.select(&article_sel).next().map(|el| element_text(&el))
    } else { None };

    let content = content.or_else(|| {
        if let Ok(main_sel) = Selector::parse("main") {
            document.select(&main_sel).next().map(|el| element_text(&el))
        } else { None }
    });

    let content = content.unwrap_or_else(|| element_text(&body));

    let title_sel = Selector::parse("title").unwrap();
    let title = document.select(&title_sel).next()
        .map(|el| element_text(&el))
        .unwrap_or_default();

    // Get author if available
    let author_sel = Selector::parse("meta[name='author']").unwrap();
    let author = document.select(&author_sel).next()
        .map(|el| get_attr(&el, "content"))
        .unwrap_or_default();

    let word_count = content.split_whitespace().count();
    let reading_time_secs = (word_count as f64 / 200.0 * 60.0).ceil() as usize; // ~200 wpm

    Ok(json!({
        "url": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "title": title,
        "author": if author.is_empty() { Value::Null } else { Value::String(author) },
        "content_preview": content.chars().take(2000).collect::<String>(),
        "content_length": content.len(),
        "word_count": word_count,
        "reading_time_seconds": reading_time_secs,
        "html_size": html_text.len(),
    }))
}

// ============================================================================
// 14. DIFF - Compare two pages (headings, links, content)
// ============================================================================

async fn diff_pages(params: &HashMap<String, Value>) -> Result<Value, String> {
    // Fetch first page
    let (doc1, html1) = fetch_html(params).await?;

    // Fetch second page
    let (doc2, html2) = if let Some(html2) = params.get("html2").and_then(|v| v.as_str()) {
        (Html::parse_document(html2), html2.to_string())
    } else if let Some(url2) = params.get("url2").and_then(|v| v.as_str()) {
        let custom_headers: Option<HashMap<String, String>> = params
            .get("headers").and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok());
        let cookies = params.get("cookies").and_then(|v| v.as_str());
        let proxy = params.get("proxy").and_then(|v| v.as_str());
        let client = build_client(cookies, proxy)?;
        let headers = build_headers(custom_headers.as_ref());
        let mut request = client.get(url2).headers(headers);
        if let Some(cookie_str) = cookies {
            request = request.header(reqwest::header::COOKIE, cookie_str);
        }
        let response = request.send().map_err(|e| format!("Request failed for {}: {}", url2, e))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {} for {}", response.status(), url2));
        }
        let text = response.text().map_err(|e| format!("Failed to read response body: {}", e))?;
        (Html::parse_document(&text), text)
    } else {
        return Err("diff action requires url2 or html2 parameter".to_string());
    };

    let title1_sel = Selector::parse("title").unwrap();
    let title2_sel = Selector::parse("title").unwrap();

    let title1 = doc1.select(&title1_sel).next().map(|el| element_text(&el)).unwrap_or_default();
    let title2 = doc2.select(&title2_sel).next().map(|el| element_text(&el)).unwrap_or_default();

    let link_sel = Selector::parse("a[href]").unwrap();
    let links1: Vec<String> = doc1.select(&link_sel).map(|el| get_attr(&el, "href")).collect();
    let links2: Vec<String> = doc2.select(&link_sel).map(|el| get_attr(&el, "href")).collect();

    let links1_set: std::collections::HashSet<_> = links1.iter().cloned().collect();
    let links2_set: std::collections::HashSet<_> = links2.iter().cloned().collect();

    let only_in_1: Vec<&String> = links1_set.difference(&links2_set).collect();
    let only_in_2: Vec<&String> = links2_set.difference(&links1_set).collect();
    let common: Vec<&String> = links1_set.intersection(&links2_set).collect();

    let body_sel = Selector::parse("body").unwrap();
    let body_text1 = doc1.select(&body_sel).next().map(|el| element_text(&el)).unwrap_or_default();
    let body_text2 = doc2.select(&body_sel).next().map(|el| element_text(&el)).unwrap_or_default();

    Ok(json!({
        "url1": params.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "url2": params.get("url2").and_then(|v| v.as_str()).unwrap_or(""),
        "title1": title1,
        "title2": title2,
        "html_size1": html1.len(),
        "html_size2": html2.len(),
        "size_diff": (html1.len() as i64 - html2.len() as i64).abs(),
        "links": {
            "only_in_page1_count": only_in_1.len(),
            "only_in_page2_count": only_in_2.len(),
            "common_count": common.len(),
            "only_in_page1": only_in_1.into_iter().take(20).cloned().collect::<Vec<_>>(),
            "only_in_page2": only_in_2.into_iter().take(20).cloned().collect::<Vec<_>>(),
        },
    }))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut crate::tools::ToolRegistry) {
    registry.register(Box::new(BrowserTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[tokio::test]
    async fn test_extract_from_raw_html() {
        let html = "\
        <html>\
            <head><title>Test Page</title></head>\
            <body>\
                <h1>Hello World</h1>\
                <div class=\"price\">$99.99</div>\
                <div class=\"price\">$149.00</div>\
                <a href=\"/page1\">Link 1</a>\
                <a href=\"https://external.com\">External</a>\
            </body>\
        </html>";

        let params = HashMap::from([
            ("action".to_string(), json!("extract")),
            ("html".to_string(), json!(html)),
            ("selector".to_string(), json!("div.price")),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["count"], 2);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0], "$99.99");
        assert_eq!(results[1], "$149.00");
    }

    #[tokio::test]
    async fn test_extract_attribute() {
        let html = "\
        <html><body>\
            <a href=\"/about\">About</a>\
            <a href=\"/contact\">Contact</a>\
        </body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("extract")),
            ("html".to_string(), json!(html)),
            ("selector".to_string(), json!("a")),
            ("attribute".to_string(), json!("href")),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["count"], 2);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0], "/about");
        assert_eq!(results[1], "/contact");
    }

    #[tokio::test]
    async fn test_title_from_raw_html() {
        let html =
            "<html><head><title>My Test Page</title></head><body><h1>Welcome</h1></body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("title")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["title"], "My Test Page");
        let h1s = result["h1_tags"].as_array().unwrap();
        assert_eq!(h1s[0], "Welcome");
    }

    #[tokio::test]
    async fn test_links_from_raw_html() {
        let html = "\
        <html><body>\
            <a href=\"/internal1\">Internal 1</a>\
            <a href=\"https://example.com/page\">External</a>\
            <a href=\"#anchor\">Anchor</a>\
            <a href=\"javascript:alert(1)\">JS Link</a>\
        </body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("links")),
            ("html".to_string(), json!(html)),
            ("url".to_string(), json!("https://mysite.com")),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        // Should skip #anchor and javascript: links
        assert!(result["internal_count"].as_i64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_table_from_raw_html() {
        let html = "\
        <html><body>\
            <table>\
                <tr><th>Name</th><th>Price</th><th>Stock</th></tr>\
                <tr><td>Widget A</td><td>$10</td><td>100</td></tr>\
                <tr><td>Widget B</td><td>$20</td><td>50</td></tr>\
            </table>\
        </body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("table")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["total_tables"], 1);
        let table = &result["tables"][0];
        assert_eq!(table["headers"][0], "Name");
        assert_eq!(table["row_count"], 2);
        let data = table["data"].as_array().unwrap();
        assert_eq!(data[0]["Name"], "Widget A");
        assert_eq!(data[1]["Price"], "$20");
    }

    #[tokio::test]
    async fn test_search_in_raw_html() {
        let html = "<html><body>This is a test page. The word test appears multiple times. Test TEST</body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("search")),
            ("html".to_string(), json!(html)),
            ("query".to_string(), json!("test")),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        // Case-insensitive search: "test", "Test", "TEST"
        assert_eq!(result["total_occurrences"], 3);
    }

    #[tokio::test]
    async fn test_meta_from_raw_html() {
        let html = "\
        <html lang=\"en\">\
            <head>\
                <title>SEO Page</title>\
                <meta name=\"description\" content=\"A test page\">\
                <meta property=\"og:title\" content=\"OG Title\">\
                <link rel=\"canonical\" href=\"https://example.com/page\">\
            </head>\
            <body></body>\
        </html>";

        let params = HashMap::from([
            ("action".to_string(), json!("meta")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        let meta = &result["meta"];
        assert_eq!(meta["description"], "A test page");
        assert_eq!(meta["og:title"], "OG Title");
        assert_eq!(meta["title"], "SEO Page");
        assert_eq!(meta["lang"], "en");
    }

    #[tokio::test]
    async fn test_navigate_raw_html() {
        let html = "<html><head><title>Test</title></head><body><a href=\"/1\">a</a><a href=\"/2\">b</a><img src=\"/img.png\"></body></html>";

        let params = HashMap::from([
            ("action".to_string(), json!("navigate")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["title"], "Test");
        assert_eq!(result["link_count"], 2);
        assert_eq!(result["image_count"], 1);
    }

    #[tokio::test]
    async fn test_invalid_selector() {
        let html = "<html><body></body></html>";
        let params = HashMap::from([
            ("action".to_string(), json!("extract")),
            ("html".to_string(), json!(html)),
            ("selector".to_string(), json!("[invalid")),
        ]);
        let result = BrowserTool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_params() {
        let params = HashMap::new();
        let result = BrowserTool.execute(&params).await;
        assert!(result.is_err());
    }

    // ---- New Action Tests ----

    #[tokio::test]
    async fn test_forms_from_raw_html() {
        let html = r#"<html><body>
            <form id="login" action="/login" method="POST">
                <input type="text" name="username" placeholder="Username" required>
                <input type="password" name="password" placeholder="Password" required>
                <select name="role"><option value="user">User</option><option value="admin">Admin</option></select>
                <button type="submit">Login</button>
            </form>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("forms")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["form_count"], 1);
        let form = &result["forms"][0];
        assert_eq!(form["id"], "login");
        assert_eq!(form["method"], "POST");
        assert_eq!(form["fields"].as_array().unwrap().len(), 3);
        assert_eq!(form["buttons"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_images_from_raw_html() {
        let html = r#"<html><body>
            <img src="/logo.png" alt="Logo" width="100" height="50" loading="lazy">
            <img data-src="/hero.jpg" alt="Hero" width="800" height="400">
            <picture>
                <source media="(min-width: 800px)" srcset="large.jpg">
                <source media="(min-width: 400px)" srcset="medium.jpg">
                <img src="small.jpg" alt="Responsive">
            </picture>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("images")),
            ("html".to_string(), json!(html)),
            ("url".to_string(), json!("https://example.com/page")),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["image_count"], 3);
        assert_eq!(result["picture_elements"], 1);
        let imgs = result["images"].as_array().unwrap();
        assert_eq!(imgs[0]["loading"], "lazy");
        assert!(imgs[1]["src"].as_str().unwrap().contains("hero.jpg"));
        assert_eq!(imgs[0]["alt"], "Logo");
    }

    #[tokio::test]
    async fn test_structured_data_from_raw_html() {
        let html = r#"<html><body>
            <script type="application/ld+json">
            {"@context": "https://schema.org", "@type": "Product", "name": "Widget", "price": "9.99"}
            </script>
            <div itemscope itemtype="https://schema.org/Person">
                <span itemprop="name">John Doe</span>
                <span itemprop="email">john@example.com</span>
            </div>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("structured_data")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["json_ld_count"], 1);
        assert_eq!(result["microdata_count"], 1);
        let json_ld = &result["json_ld"][0];
        assert_eq!(json_ld["@type"], "Product");
        let microdata = &result["microdata"][0];
        assert_eq!(microdata["type"], "https://schema.org/Person");
        assert_eq!(microdata["properties"]["name"], "John Doe");
    }

    #[tokio::test]
    async fn test_headings_from_raw_html() {
        let html = r#"<html><body>
            <h1>Main Title</h1>
            <h2>Section 1</h2>
            <h3>Subsection 1.1</h3>
            <h2>Section 2</h2>
            <h3>Subsection 2.1</h3>
            <h4>Detail</h4>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("headings")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["heading_count"], 6);
        assert_eq!(result["h1_count"], 1);
        let outline = result["outline"].as_array().unwrap();
        assert_eq!(outline[0]["text"], "Main Title");
        assert_eq!(outline[0]["level"], 1);
    }

    #[tokio::test]
    async fn test_readability_from_raw_html() {
        let html = r#"<html><head><title>Article Title</title><meta name="author" content="Jane Smith"></head><body>
            <nav>Skip nav</nav>
            <article>
                <h1>Article Title</h1>
                <p>This is the main content of the article. It has enough words to be meaningful.
                Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor
                incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
                exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.</p>
            </article>
            <footer>Footer content</footer>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("readability")),
            ("html".to_string(), json!(html)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["title"], "Article Title");
        assert_eq!(result["author"], "Jane Smith");
        assert!(result["word_count"].as_i64().unwrap() > 30);
        assert!(result["reading_time_seconds"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_diff_from_raw_html() {
        let html1 = r#"<html><head><title>Page 1</title></head><body>
            <a href="/home">Home</a><a href="/about">About</a><a href="/old">Old Page</a>
        </body></html>"#;
        let html2 = r#"<html><head><title>Page 2</title></head><body>
            <a href="/home">Home</a><a href="/contact">Contact</a><a href="/new">New Page</a>
        </body></html>"#;

        let params = HashMap::from([
            ("action".to_string(), json!("diff")),
            ("html".to_string(), json!(html1)),
            ("html2".to_string(), json!(html2)),
        ]);
        let result = BrowserTool.execute(&params).await.unwrap();
        assert_eq!(result["title1"], "Page 1");
        assert_eq!(result["title2"], "Page 2");
        assert_eq!(result["links"]["common_count"], 1);
        assert_eq!(result["links"]["only_in_page1_count"], 2);
        assert_eq!(result["links"]["only_in_page2_count"], 2);
    }

    #[tokio::test]
    async fn test_diff_missing_param() {
        let html1 = "<html><body></body></html>";
        let params = HashMap::from([
            ("action".to_string(), json!("diff")),
            ("html".to_string(), json!(html1)),
        ]);
        let result = BrowserTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("url2 or html2"));
    }
}
