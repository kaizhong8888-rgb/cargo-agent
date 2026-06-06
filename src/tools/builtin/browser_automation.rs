//! Browser Automation tool - Real Chrome browser control via CDP (Chrome DevTools Protocol)
//! Uses `headless_chrome` to connect to Chrome/Chromium for:
//! - Navigate to URLs (with JS rendering, SPA support)
//! - Screenshot (PNG, full-page or viewport)
//! - Execute JavaScript in page context
//! - Extract rendered HTML (after JS execution)
//! - Click elements, fill forms, interact with pages
//! - Get page metadata (title, URL, cookies)
//!
//! Requires Chrome to be installed. Connects via:
//! 1. Launching a new headless Chrome instance
//! 2. Connecting to existing Chrome via --remote-debugging-port

#[cfg(feature = "browser")]
mod cdp_impl {
    use crate::tools::ToolParameter;
    use async_trait::async_trait;
    use headless_chrome::protocol::cdp::types::Method;
    use headless_chrome::{Browser, LaunchOptions, Tab};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    // Global browser instance (singleton)
    static BROWSER: std::sync::OnceLock<Arc<Mutex<Option<Browser>>>> = std::sync::OnceLock::new();

    fn get_or_launch_browser(
        headless: bool,
        proxy: Option<&str>,
    ) -> Result<Arc<Mutex<Browser>>, String> {
        let cell = BROWSER.get_or_init(|| Arc::new(Mutex::new(None)));
        let guard = cell.lock().map_err(|e| format!("Lock error: {e}"))?;

        if guard.is_some() {
            drop(guard);
            return Ok(cell.clone());
        }
        drop(guard);

        let mut launch_opts = LaunchOptions::default_builder()
            .headless(headless)
            .sandbox(false)
            .enable_gpu(false)
            .enable_logging(false)
            .path_option(None)
            .build()
            .map_err(|e| format!("Failed to build launch options: {e}"))?;

        if let Some(proxy_url) = proxy {
            launch_opts.proxy_server = Some(proxy_url.to_string());
        }

        let browser = Browser::new(launch_opts)
            .map_err(|e| format!("Failed to launch Chrome: {e}. Make sure Chrome/Chromium is installed."))?;

        let browser = Arc::new(Mutex::new(browser));
        {
            let mut guard = cell.lock().map_err(|e| format!("Lock error: {e}"))?;
            *guard = Some(browser.lock().map_err(|e| format!("Lock error: {e}"))?.clone());
        }

        Ok(cell.clone())
    }

    fn with_tab<F, R>(headless: bool, proxy: Option<&str>, f: F) -> Result<R, String>
    where
        F: FnOnce(&Tab) -> Result<R, String>,
    {
        let browser = get_or_launch_browser(headless, proxy)?;
        let guard = browser.lock().map_err(|e| format!("Lock error: {e}"))?;
        let browser_ref = guard.as_ref().ok_or("Browser not initialized")?;

        let tab = browser_ref
            .wait_for_initial_tab()
            .map_err(|e| format!("Failed to get tab: {e}"))?;

        f(&tab)
    }

    pub struct BrowserAutomationTool;

    #[async_trait]
    impl crate::tools::Tool for BrowserAutomationTool {
        fn name(&self) -> &str {
            "browser_automation"
        }

        fn description(&self) -> &str {
            "Real browser automation via Chrome DevTools Protocol (CDP). \
             Supports: navigate (with JS rendering), screenshot (PNG), eval (execute JavaScript), \
             html (get rendered DOM), click (click elements), type_text (fill forms), \
             metadata (page info + cookies), wait (wait for selector). \
             Requires Chrome/Chromium installed. Use headless=true for headless mode."
        }

        fn parameters(&self) -> Vec<ToolParameter> {
            vec![
                ToolParameter {
                    name: "action".into(),
                    description: "Action: navigate, screenshot, eval, html, click, type_text, metadata, wait".into(),
                    required: true,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "url".into(),
                    description: "URL to navigate to (for navigate/screenshot/html/metadata)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "selector".into(),
                    description: "CSS selector (for click/type_text/wait/screenshot)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "javascript".into(),
                    description: "JavaScript code to execute (for eval action)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "text".into(),
                    description: "Text to type (for type_text action)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "output_path".into(),
                    description: "File path to save screenshot (for screenshot action)".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "full_page".into(),
                    description: "Capture full page screenshot (true/false, default: false)".into(),
                    required: false,
                    parameter_type: "boolean".into(),
                },
                ToolParameter {
                    name: "headless".into(),
                    description: "Run in headless mode (true/false, default: true)".into(),
                    required: false,
                    parameter_type: "boolean".into(),
                },
                ToolParameter {
                    name: "proxy".into(),
                    description: "Proxy URL (e.g. 'http://127.0.0.1:7890')".into(),
                    required: false,
                    parameter_type: "string".into(),
                },
                ToolParameter {
                    name: "timeout_ms".into(),
                    description: "Timeout in milliseconds for page load (default: 30000)".into(),
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
                "navigate" => cdp_navigate(params),
                "screenshot" => cdp_screenshot(params),
                "eval" => cdp_eval(params),
                "html" => cdp_html(params),
                "click" => cdp_click(params),
                "type_text" => cdp_type_text(params),
                "metadata" => cdp_metadata(params),
                "wait" => cdp_wait(params),
                _ => Err(format!(
                    "Unknown action: {action}. Supported: navigate, screenshot, eval, html, click, type_text, metadata, wait"
                )),
            }
        }
    }

    fn get_param(params: &HashMap<String, Value>, key: &str) -> Option<String> {
        params.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    fn get_bool_param(params: &HashMap<String, Value>, key: &str, default: bool) -> bool {
        params.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    // ============================================================================
    // 1. NAVIGATE - Navigate to URL with full JS rendering
    // ============================================================================

    fn cdp_navigate(params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30000);

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.set_default_timeout(std::time::Duration::from_millis(timeout_ms));

            let navigation = tab
                .navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?;

            navigation
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            // Wait for page to be mostly loaded
            tab.wait_until_navigated()
                .map_err(|e| format!("Page load wait failed: {e}"))?;

            let title = tab
                .get_title()
                .unwrap_or_else(|_| "Unknown".to_string());

            let current_url = tab
                .get_url()
                .map_err(|e| format!("Failed to get URL: {e}"))?;

            Ok(json!({
                "status": "ok",
                "url": current_url,
                "title": title,
                "final_url": current_url,
            }))
        })
    }

    // ============================================================================
    // 2. SCREENSHOT - Capture page screenshot
    // ============================================================================

    fn cdp_screenshot(params: &HashMap<String, Value>) -> Result<Value, String> {
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");
        let full_page = get_bool_param(params, "full_page", false);

        // Navigate first if URL provided
        if let Some(url) = get_param(params, "url") {
            let timeout_ms = params
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000);
            with_tab(headless, proxy.as_deref(), |tab| {
                tab.set_default_timeout(std::time::Duration::from_millis(timeout_ms));
                tab.navigate_to(&url)
                    .map_err(|e| format!("Navigation failed: {e}"))?
                    .wait_until_navigated()
                    .map_err(|e| format!("Navigation timeout: {e}"))?;
                tab.wait_until_navigated()
                    .map_err(|e| format!("Page load wait failed: {e}"))?;

                // Wait for selector if provided
                if let Some(selector) = get_param(params, "selector") {
                    tab.wait_for_element(&selector)
                        .map_err(|e| format!("Selector wait failed: {e}"))?;
                }

                let png_data = if full_page {
                    tab.capture_screenshot(
                        headless_chrome::protocol::cdp::types::Encoding::Png,
                        None,
                        true,
                    )
                    .map_err(|e| format!("Screenshot failed: {e}"))?
                } else {
                    tab.capture_screenshot(
                        headless_chrome::protocol::cdp::types::Encoding::Png,
                        None,
                        false,
                    )
                    .map_err(|e| format!("Screenshot failed: {e}"))?
                };

                // Save to file if path provided
                let saved_path = if let Some(output_path) = get_param(params, "output_path") {
                    let path = PathBuf::from(&output_path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create dir: {e}"))?;
                    }
                    std::fs::write(&path, &png_data)
                        .map_err(|e| format!("Failed to save screenshot: {e}"))?;
                    Some(output_path)
                } else {
                    None
                };

                let size_kb = png_data.len() / 1024;
                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_data);

                Ok(json!({
                    "status": "ok",
                    "size_bytes": png_data.len(),
                    "size_kb": size_kb,
                    "full_page": full_page,
                    "saved_to": saved_path,
                    "preview_base64": if b64.len() > 2000 { format!("{}...(truncated)", &b64[..2000]) } else { b64 },
                }))
            })
        } else {
            Err("Screenshot requires a url parameter".to_string())
        }
    }

    // ============================================================================
    // 3. EVAL - Execute JavaScript in page context
    // ============================================================================

    fn cdp_eval(params: &HashMap<String, Value>) -> Result<Value, String> {
        let javascript = get_param(params, "javascript")
            .ok_or_else(|| "Missing required parameter: javascript".to_string())?;

        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");

        // Navigate first if URL provided
        if let Some(url) = get_param(params, "url") {
            let timeout_ms = params
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000);
            with_tab(headless, proxy.as_deref(), |tab| {
                tab.set_default_timeout(std::time::Duration::from_millis(timeout_ms));
                tab.navigate_to(&url)
                    .map_err(|e| format!("Navigation failed: {e}"))?
                    .wait_until_navigated()
                    .map_err(|e| format!("Navigation timeout: {e}"))?;

                let result = tab
                    .evaluate(&javascript)
                    .map_err(|e| format!("JS evaluation failed: {e}"))?;

                let value = result.result.value;
                Ok(json!({
                    "status": "ok",
                    "result": value,
                }))
            })
        } else {
            Err("eval action requires a url parameter to load a page first".to_string())
        }
    }

    // ============================================================================
    // 4. HTML - Get rendered HTML (after JS execution)
    // ============================================================================

    fn cdp_html(params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");
        let timeout_ms = params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30000);

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.set_default_timeout(std::time::Duration::from_millis(timeout_ms));
            tab.navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            // Wait for selector if provided
            if let Some(selector) = get_param(params, "selector") {
                tab.wait_for_element(&selector)
                    .map_err(|e| format!("Selector wait failed: {e}"))?;
            }

            // Get outer HTML of body (or specific element)
            let html = if let Some(selector) = get_param(params, "selector") {
                let element = tab
                    .wait_for_element(&selector)
                    .map_err(|e| format!("Element not found '{selector}': {e}"))?;
                element
                    .get_outer_html()
                    .map_err(|e| format!("Failed to get HTML: {e}"))?
            } else {
                tab.get_content()
                    .map_err(|e| format!("Failed to get content: {e}"))?
            };

            let title = tab.get_title().unwrap_or_default();

            Ok(json!({
                "status": "ok",
                "url": url,
                "title": title,
                "content_length": html.len(),
                "html_preview": html.chars().take(2000).collect::<String>(),
            }))
        })
    }

    // ============================================================================
    // 5. CLICK - Click an element
    // ============================================================================

    fn cdp_click(params: &HashMap<String, Value>) -> Result<Value, String> {
        let selector = get_param(params, "selector")
            .ok_or_else(|| "Missing required parameter: selector".to_string())?;
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            let element = tab
                .wait_for_element(&selector)
                .map_err(|e| format!("Element not found '{selector}': {e}"))?;

            element.click().map_err(|e| format!("Click failed: {e}"))?;

            // Wait a bit for navigation
            std::thread::sleep(std::time::Duration::from_millis(500));

            let current_url = tab.get_url().unwrap_or_default();
            let title = tab.get_title().unwrap_or_default();

            Ok(json!({
                "status": "ok",
                "selector": selector,
                "url": current_url,
                "title": title,
            }))
        })
    }

    // ============================================================================
    // 6. TYPE_TEXT - Fill form fields
    // ============================================================================

    fn cdp_type_text(params: &HashMap<String, Value>) -> Result<Value, String> {
        let selector = get_param(params, "selector")
            .ok_or_else(|| "Missing required parameter: selector".to_string())?;
        let text = get_param(params, "text")
            .ok_or_else(|| "Missing required parameter: text".to_string())?;
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            let element = tab
                .wait_for_element(&selector)
                .map_err(|e| format!("Element not found '{selector}': {e}"))?;

            element.click().map_err(|e| format!("Click failed: {e}"))?;

            // Use JS to set the value (more reliable than send_keys)
            let js = format!(
                "document.querySelector('{}').value = '{}'; document.querySelector('{}').dispatchEvent(new Event('input', {{ bubbles: true }}));",
                selector.replace('\'', "\\'"),
                text.replace('\'', "\\'"),
                selector.replace('\'', "\\'")
            );
            tab.evaluate(&js)
                .map_err(|e| format!("Failed to type text: {e}"))?;

            Ok(json!({
                "status": "ok",
                "selector": selector,
                "text": text,
            }))
        })
    }

    // ============================================================================
    // 7. METADATA - Get page info + cookies
    // ============================================================================

    fn cdp_metadata(params: &HashMap<String, Value>) -> Result<Value, String> {
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            let title = tab.get_title().unwrap_or_default();
            let current_url = tab.get_url().unwrap_or_default();

            // Get cookies
            let cookies_result = tab.get_cookies();
            let cookies: Vec<Value> = match cookies_result {
                Ok(cookie_list) => cookie_list
                    .iter()
                    .map(|c| {
                        json!({
                            "name": c.name,
                            "value": c.value,
                            "domain": c.domain,
                            "path": c.path,
                        })
                    })
                    .collect(),
                Err(_) => vec![],
            };

            Ok(json!({
                "status": "ok",
                "url": current_url,
                "title": title,
                "cookie_count": cookies.len(),
                "cookies": cookies,
            }))
        })
    }

    // ============================================================================
    // 8. WAIT - Wait for a selector to appear
    // ============================================================================

    fn cdp_wait(params: &HashMap<String, Value>) -> Result<Value, String> {
        let selector = get_param(params, "selector")
            .ok_or_else(|| "Missing required parameter: selector".to_string())?;
        let url = get_param(params, "url")
            .ok_or_else(|| "Missing required parameter: url".to_string())?;
        let headless = get_bool_param(params, "headless", true);
        let proxy = get_param(params, "proxy");

        with_tab(headless, proxy.as_deref(), |tab| {
            tab.navigate_to(&url)
                .map_err(|e| format!("Navigation failed: {e}"))?
                .wait_until_navigated()
                .map_err(|e| format!("Navigation timeout: {e}"))?;

            let start = std::time::Instant::now();
            let element = tab
                .wait_for_element(&selector)
                .map_err(|e| format!("Wait for '{selector}' failed: {e}"))?;
            let elapsed_ms = start.elapsed().as_millis();

            let found = element.exists().unwrap_or(false);

            Ok(json!({
                "status": "ok",
                "selector": selector,
                "found": found,
                "waited_ms": elapsed_ms,
            }))
        })
    }

    pub fn register_all(registry: &mut crate::tools::ToolRegistry) {
        registry.register(Box::new(BrowserAutomationTool));
    }
}

// Stub for when feature is not enabled
#[cfg(not(feature = "browser"))]
mod cdp_impl {
    use crate::tools::ToolParameter;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    pub struct BrowserAutomationTool;

    #[async_trait]
    impl crate::tools::Tool for BrowserAutomationTool {
        fn name(&self) -> &str {
            "browser_automation"
        }

        fn description(&self) -> &str {
            "Browser automation via Chrome DevTools Protocol (CDP). Requires the 'browser' feature to be enabled. \
             Compile with: cargo build --features browser"
        }

        fn parameters(&self) -> Vec<ToolParameter> {
            vec![ToolParameter {
                name: "action".into(),
                description: "Action (feature not enabled)".into(),
                required: true,
                parameter_type: "string".into(),
            }]
        }

        async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
            Err("Browser automation requires the 'browser' feature. Rebuild with: cargo build --features browser".to_string())
        }
    }

    pub fn register_all(registry: &mut crate::tools::ToolRegistry) {
        registry.register(Box::new(BrowserAutomationTool));
    }
}

pub use cdp_impl::register_all;
