//! Browser/Scraper tool - HTML parsing and web scraping
//! Uses reqwest + scraper for headless browsing capabilities.
//! Supports: navigate, extract, links, title, table, search, meta

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
                description: "Action: navigate, extract, links, title, table, search, meta".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "url".into(),
                description: "URL to fetch (for navigate/links/title/table/meta)".into(),
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
            _ => Err(format!(
                "Unknown action: {}. Supported: navigate, extract, links, title, table, search, meta",
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
// 7. META
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
}
