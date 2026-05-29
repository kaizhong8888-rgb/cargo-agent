//! PDF Tool: generate PDF documents using genpdf.
//!
//! Requires LiberationSans fonts installed. Supports markdown-like text
//! and structured JSON reports with sections and tables.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use genpdf::{elements, Document, SimplePageDecorator};
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(PdfTool));
}

struct PdfTool;

#[async_trait::async_trait]
impl Tool for PdfTool {
    fn name(&self) -> &str { "pdf" }

    fn description(&self) -> &str {
        "Generate PDF documents. Actions: text (from markdown text), \
         report (from JSON with sections/tables). Requires LiberationSans fonts."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            tp("action", "Action: text, report", true),
            tp("output", "Output PDF file path (default: output.pdf)", false),
            tp("title", "Document title (for report)", false),
            tp("content", "Text content or JSON with sections/tables", true),
            tp("author", "Document author", false),
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        match params.get("action").and_then(|v| v.as_str()).unwrap_or("") {
            "text" => text_pdf(params),
            "report" => report_pdf(params),
            a => Err(format!("Unknown action: {a}. Valid: text, report")),
        }
    }
}

fn tp(name: &str, desc: &str, required: bool) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: desc.to_string(),
        required,
        parameter_type: "string".to_string(),
    }
}

fn text_pdf(params: &HashMap<String, Value>) -> Result<Value, String> {
    let output = params.get("output").and_then(|v| v.as_str()).unwrap_or("output.pdf");
    let content = params.get("content").and_then(|v| v.as_str()).ok_or("content is required")?;

    let family = load_fonts()?;
    let mut doc = Document::new(family);
    doc.set_title("Document");

    let mut para = elements::Paragraph::new("");
    let mut has_content = false;

    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() {
            if has_content {
                let mut p = elements::Paragraph::new("");
                std::mem::swap(&mut para, &mut p);
                doc.push(p);
                has_content = false;
            }
            continue;
        }

        if t.starts_with("### ") {
            if has_content {
                let mut p = elements::Paragraph::new("");
                std::mem::swap(&mut para, &mut p);
                doc.push(p);
                has_content = false;
            }
            doc.push(elements::Break::new(0.15));
            let mut h = elements::Paragraph::new(&t[4..]);
            h.set_alignment(genpdf::Alignment::Left);
            doc.push(h);
            continue;
        }
        if t.starts_with("## ") {
            if has_content {
                let mut p = elements::Paragraph::new("");
                std::mem::swap(&mut para, &mut p);
                doc.push(p);
                has_content = false;
            }
            doc.push(elements::Break::new(0.15));
            let mut h = elements::Paragraph::new(&t[3..]);
            h.set_alignment(genpdf::Alignment::Left);
            doc.push(h);
            continue;
        }
        if t.starts_with("# ") {
            if has_content {
                let mut p = elements::Paragraph::new("");
                std::mem::swap(&mut para, &mut p);
                doc.push(p);
                has_content = false;
            }
            doc.push(elements::Break::new(0.2));
            let mut h = elements::Paragraph::new(&t[2..]);
            h.set_alignment(genpdf::Alignment::Left);
            doc.push(h);
            continue;
        }
        if t.starts_with("---") || t.starts_with("***") {
            if has_content {
                let mut p = elements::Paragraph::new("");
                std::mem::swap(&mut para, &mut p);
                doc.push(p);
                has_content = false;
            }
            doc.push(elements::Break::new(0.2));
            continue;
        }

        if has_content { para.push(" "); }
        para.push(line);
        has_content = true;
    }

    if has_content {
        doc.push(para);
    }

    render_pdf(doc, output)
}

fn report_pdf(params: &HashMap<String, Value>) -> Result<Value, String> {
    let output = params.get("output").and_then(|v| v.as_str()).unwrap_or("report.pdf");
    let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("Report");
    let raw = params.get("content").and_then(|v| v.as_str()).unwrap_or("{}");

    let json: Value = serde_json::from_str(raw).map_err(|e| format!("Invalid JSON: {e}"))?;

    let family = load_fonts()?;
    let mut doc = Document::new(family);
    doc.set_title(title);

    // Title
    let mut tp = elements::Paragraph::new(title);
    tp.set_alignment(genpdf::Alignment::Center);
    doc.push(tp);
    doc.push(elements::Break::new(0.5));

    // Sections
    if let Some(sections) = json.get("sections").and_then(|v| v.as_array()) {
        for section in sections {
            let st = section.get("title").and_then(|v| v.as_str()).unwrap_or("Section");
            let txt = section.get("text").and_then(|v| v.as_str()).unwrap_or("");

            let mut h = elements::Paragraph::new(st);
            h.set_alignment(genpdf::Alignment::Left);
            doc.push(h);
            doc.push(elements::Break::new(0.15));

            if !txt.is_empty() {
                doc.push(elements::Paragraph::new(txt));
                doc.push(elements::Break::new(0.2));
            }

            // Table rendering
            if let Some(table) = section.get("table") {
                let headers: Vec<String> = table.get("headers")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let rows: Vec<Vec<String>> = table.get("rows")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().map(|r| {
                        r.as_array().map(|c| c.iter().filter_map(|v| match v {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        }).collect()).unwrap_or_default()
                    }).collect())
                    .unwrap_or_default();

                if !headers.is_empty() {
                    doc.push(elements::Paragraph::new(format!("[{}]", headers.join(" | "))));
                    for row in &rows {
                        doc.push(elements::Paragraph::new(format!("  {}", row.join(" | "))));
                    }
                    doc.push(elements::Break::new(0.3));
                }
            }
        }
    }

    render_pdf(doc, output)
}

fn render_pdf(mut doc: Document, output: &str) -> Result<Value, String> {
    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);
    doc.set_font_size(11);
    doc.render_to_file(output).map_err(|e| format!("Render error: {e}"))?;

    let meta = std::fs::metadata(output).map_err(|e| format!("File error: {e}"))?;
    Ok(serde_json::json!({
        "success": true, "message": "PDF created",
        "output": output, "size_bytes": meta.len(),
        "size_display": fmt_size(meta.len()),
    }))
}

fn load_fonts() -> Result<genpdf::fonts::FontFamily<genpdf::fonts::FontData>, String> {
    for dir in &[
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/liberation",
        "/usr/local/share/fonts/liberation",
        "./fonts",
    ] {
        if std::path::Path::new(dir).join("LiberationSans-Regular.ttf").exists() {
            return genpdf::fonts::from_files(dir, "LiberationSans", None)
                .map_err(|e| format!("Font error: {e}"));
        }
    }
    Err("LiberationSans not found. Install: sudo apt install fonts-liberation \
         or brew install --cask font-liberation or place TTF in ./fonts/".into())
}

fn fmt_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut s = bytes as f64;
    let mut i = 0;
    while s >= 1024.0 && i < 3 { s /= 1024.0; i += 1; }
    if i == 0 { format!("{} {}", bytes, units[i]) }
    else { format!("{:.2} {}", s, units[i]) }
}
