//! Chart Generator — create visualizations (Mermaid charts, markdown tables) from data.
//!
//! Complements data_processor by turning processed data into visual representations.
//! Supports: pie (Mermaid pie chart), bar (horizontal bar chart), line (line chart),
//! table (markdown table), histogram (from binned data).
//!
//! # Examples
//!
//! ```ignore
//! // Create a pie chart from data
//! // chart_generator(
//! //   action: "pie",
//! //   labels: "Engineering,Sales,Marketing",
//! //   values: "50,30,20",
//! //   title: "Department Distribution"
//! // )
//! ```

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ChartGeneratorTool));
}

struct ChartGeneratorTool;

#[async_trait::async_trait]
impl Tool for ChartGeneratorTool {
    fn name(&self) -> &str {
        "chart_generator"
    }

    fn description(&self) -> &str {
        "Generate visualizations from data. Actions: pie (Mermaid pie chart), \
         bar (horizontal bar chart with Mermaid), line (line chart), \
         table (markdown table), histogram (from binned data). \
         Use 'labels' and 'values' for simple data, or 'data' for JSON array input."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Chart type: pie, bar, line, table, histogram".to_string(),
                required: true,
            },
            ToolParameter {
                name: "labels".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated labels (e.g. 'A,B,C')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "values".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated numeric values (e.g. '10,20,30')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "data".to_string(),
                parameter_type: "string".to_string(),
                description: "JSON array of objects (e.g. '[{\"label\":\"A\",\"value\":10}]')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "title".to_string(),
                parameter_type: "string".to_string(),
                description: "Chart title".to_string(),
                required: false,
            },
            ToolParameter {
                name: "label_column".to_string(),
                parameter_type: "string".to_string(),
                description: "Column name for labels in data array (default: 'label')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "value_column".to_string(),
                parameter_type: "string".to_string(),
                description: "Column name for values in data array (default: 'value')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "width".to_string(),
                parameter_type: "number".to_string(),
                description: "Max bar width in characters for bar chart (default: 20)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "headers".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated column headers for table (default: auto from data)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "rows".to_string(),
                parameter_type: "string".to_string(),
                description: "For table: JSON array of row arrays (e.g. '[[\"a\",1],[\"b\",2]]')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "bins".to_string(),
                parameter_type: "string".to_string(),
                description: "For histogram: JSON array of {bin, count} objects".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("action is required: pie, bar, line, table, histogram")?;

        match action {
            "pie" => cmd_pie(params),
            "bar" => cmd_bar(params),
            "line" => cmd_line(params),
            "table" => cmd_table(params),
            "histogram" => cmd_histogram(params),
            _ => Err(format!(
                "Unknown action: {action}. Available: pie, bar, line, table, histogram"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Data extraction helpers
// ---------------------------------------------------------------------------

/// Extract label-value pairs from either labels/values params or data JSON.
fn extract_data(params: &HashMap<String, Value>) -> Result<Vec<(String, f64)>, String> {
    // Try JSON data first
    if let Some(data_str) = params.get("data").and_then(|v| v.as_str()) {
        return extract_data_from_json(data_str, params);
    }

    // Fall back to labels + values
    let labels_str = params
        .get("labels")
        .and_then(|v| v.as_str())
        .ok_or("Either 'data' or 'labels'+'values' is required")?;

    let values_str = params
        .get("values")
        .and_then(|v| v.as_str())
        .ok_or("'values' is required when using 'labels'")?;

    let labels: Vec<&str> = labels_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let values: Result<Vec<f64>, _> = values_str
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect();

    let values = values.map_err(|e| format!("Failed to parse values as numbers: {e}"))?;

    if labels.len() != values.len() {
        return Err(format!(
            "Number of labels ({}) must match number of values ({})",
            labels.len(),
            values.len()
        ));
    }

    Ok(labels
        .into_iter()
        .map(|s| s.to_string())
        .zip(values)
        .collect())
}

/// Extract data from a JSON array string.
fn extract_data_from_json(
    data_str: &str,
    params: &HashMap<String, Value>,
) -> Result<Vec<(String, f64)>, String> {
    let parsed: Value =
        serde_json::from_str(data_str).map_err(|e| format!("Failed to parse data JSON: {e}"))?;

    let arr = match &parsed {
        Value::Array(arr) => arr,
        _ => return Err("data must be a JSON array".to_string()),
    };

    let label_col = params
        .get("label_column")
        .and_then(|v| v.as_str())
        .unwrap_or("label");

    let value_col = params
        .get("value_column")
        .and_then(|v| v.as_str())
        .unwrap_or("value");

    let mut result = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        let label = match item.get(label_col) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(other) => format!("{other}"),
            None => format!("row_{i}"),
        };
        let value = match item.get(value_col) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
            Some(Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        };
        result.push((label, value));
    }

    Ok(result)
}

/// Get title from params.
fn get_title(params: &HashMap<String, Value>) -> String {
    params
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Chart")
        .to_string()
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

/// Generate a Mermaid pie chart.
fn cmd_pie(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data = extract_data(params)?;
    let title = get_title(params);

    if data.is_empty() {
        return Err("No data provided for pie chart".to_string());
    }

    let mut mermaid = format!("```mermaid\npie title {title}\n");

    // Validate data: pie charts need non-negative values
    let total: f64 = data.iter().map(|(_, v)| v).sum();
    if total <= 0.0 {
        return Err("Pie chart requires positive values".to_string());
    }

    for (label, value) in &data {
        if *value < 0.0 {
            return Err(format!("Negative value '{value}' not allowed in pie chart for '{label}'"));
        }
        // Escape quotes in labels
        let safe_label = label.replace('"', "'");
        mermaid.push_str(&format!("    \"{safe_label}\" : {value}\n"));
    }

    mermaid.push_str("```\n");

    Ok(serde_json::json!({
        "action": "pie",
        "title": title,
        "data_points": data.len(),
        "total": total,
        "mermaid": mermaid,
    }))
}

/// Generate an ASCII horizontal bar chart (with Mermaid xychart).
fn cmd_bar(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data = extract_data(params)?;
    let title = get_title(params);

    if data.is_empty() {
        return Err("No data provided for bar chart".to_string());
    }

    let max_val = data
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);

    if max_val <= 0.0 {
        return Err("Bar chart requires positive values".to_string());
    }

    let max_width = params
        .get("width")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;

    // Generate ASCII bar chart (works everywhere)
    let mut ascii_chart = format!("📊 {title}\n\n");

    // Find longest label for alignment
    let max_label_len = data.iter().map(|(l, _)| l.len()).max().unwrap_or(10);
    let label_width = max_label_len.min(30);

    // Also generate Mermaid xychart
    let mut mermaid = format!("```mermaid\nxychart-beta\n    title \"{title}\"\n    x-axis ")
        .to_string();

    // X-axis labels
    let x_labels: Vec<&str> = data.iter().map(|(l, _)| l.as_str()).collect();
    mermaid.push('[');
    mermaid.push_str(
        &x_labels
            .iter()
            .map(|l| format!("\"{}\"", l.replace('"', "'")))
            .collect::<Vec<_>>()
            .join(", "),
    );
    mermaid.push_str("]\n");

    // Y-axis (bar)
    let values_str: Vec<String> = data
        .iter()
        .map(|(_, v)| {
            if *v == v.floor() {
                format!("{}", *v as i64)
            } else {
                format!("{:.2}", v)
            }
        })
        .collect();

    mermaid.push_str(&format!("    y-axis \"Value\"\n    bar [{}]\n", values_str.join(", ")));
    mermaid.push_str("```\n\n");

    // ASCII representation for text fallback
    for (label, value) in &data {
        let bar_len = if max_val > 0.0 {
            ((value / max_val) * max_width as f64).round() as usize
        } else {
            0
        };
        let bar_len = bar_len.max(1).min(max_width);
        let bar = "█".repeat(bar_len);
        let padded_label = if label.len() < label_width {
            format!("{}{}", label, " ".repeat(label_width - label.len()))
        } else {
            label[..label_width.min(label.len())].to_string()
        };
        let display_val = if *value == value.floor() {
            format!("{}", *value as i64)
        } else {
            format!("{:.2}", value)
        };
        ascii_chart.push_str(&format!("  {} | {} {}\n", padded_label, bar, display_val));
    }

    Ok(serde_json::json!({
        "action": "bar",
        "title": title,
        "data_points": data.len(),
        "max_value": max_val,
        "mermaid": mermaid,
        "ascii": ascii_chart,
    }))
}

/// Generate a line chart using Mermaid xychart.
fn cmd_line(params: &HashMap<String, Value>) -> Result<Value, String> {
    let data = extract_data(params)?;
    let title = get_title(params);

    if data.is_empty() {
        return Err("No data provided for line chart".to_string());
    }

    let max_val = data
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_val = data
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::INFINITY, f64::min);

    // Mermaid xychart for line
    let x_labels: Vec<&str> = data.iter().map(|(l, _)| l.as_str()).collect();
    let values_str: Vec<String> = data
        .iter()
        .map(|(_, v)| {
            if *v == v.floor() {
                format!("{}", *v as i64)
            } else {
                format!("{:.2}", v)
            }
        })
        .collect();

    let mermaid = format!(
        "```mermaid\nxychart-beta\n    title \"{title}\"\n    x-axis [{labels}]\n    y-axis \"Value\"\n    line [{values}]\n```\n",
        labels = x_labels
            .iter()
            .map(|l| format!("\"{}\"", l.replace('"', "'")))
            .collect::<Vec<_>>()
            .join(", "),
        values = values_str.join(", "),
    );

    // ASCII sparkline approximation
    let width = 40usize;
    let ascii_line = generate_ascii_sparkline(&data, width);

    Ok(serde_json::json!({
        "action": "line",
        "title": title,
        "data_points": data.len(),
        "min_value": min_val,
        "max_value": max_val,
        "mermaid": mermaid,
        "ascii": ascii_line,
    }))
}

/// Generate an ASCII sparkline representation.
fn generate_ascii_sparkline(data: &[(String, f64)], width: usize) -> String {
    if data.is_empty() {
        return String::new();
    }

    let max_val = data.iter().map(|(_, v)| *v).fold(f64::NEG_INFINITY, f64::max);
    let min_val = data.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
    let range = max_val - min_val;
    if range == 0.0 {
        return "▁".repeat(width.min(data.len()));
    }

    let spark_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let steps = spark_chars.len() - 1;

    let labels: Vec<&str> = data.iter().map(|(l, _)| l.as_str()).collect();

    // Downsample if too many points
    let points: Vec<f64> = if data.len() > width {
        let step = data.len() / width;
        data.iter()
            .step_by(step)
            .take(width)
            .map(|(_, v)| *v)
            .collect()
    } else {
        data.iter().map(|(_, v)| *v).collect()
    };

    let result: String = points
        .iter()
        .map(|v| {
            let normalized = ((v - min_val) / range * steps as f64).round() as usize;
            let idx = normalized.min(steps);
            spark_chars[idx]
        })
        .collect();

    let mut output = format!("📈 {}/{} (min→max)\n", min_val, max_val);
    output.push_str(&format!("  {}\n", result));

    // Show first and last few labels
    if labels.len() > 1 {
        let first = labels.first().unwrap_or(&"");
        let last = labels.last().unwrap_or(&"");
        output.push_str(&format!("  {} ... {}\n", first, last));
    }

    output
}

/// Generate a markdown table.
fn cmd_table(params: &HashMap<String, Value>) -> Result<Value, String> {
    let title = get_title(params);

    // Try data array first (array of objects)
    if let Some(data_str) = params.get("data").and_then(|v| v.as_str()) {
        return generate_table_from_json(data_str, &title, params);
    }

    // Try rows + headers
    let headers_str = params
        .get("headers")
        .and_then(|v| v.as_str())
        .ok_or("'headers' or 'data' is required for table")?;

    let rows_str = params
        .get("rows")
        .and_then(|v| v.as_str())
        .ok_or("'rows' or 'data' is required for table")?;

    let headers: Vec<&str> = headers_str.split(',').map(|s| s.trim()).collect();
    let rows: Vec<Vec<String>> = serde_json::from_str::<Vec<Vec<serde_json::Value>>>(rows_str)
        .map_err(|e| format!("Failed to parse rows JSON: {e}"))?
        .into_iter()
        .map(|row| row.into_iter().map(|v| format_value(&v)).collect())
        .collect();

    let mut md = format!("### {}\n\n", title);
    // Header row
    md.push('|');
    for h in &headers {
        md.push_str(&format!(" {} |", h));
    }
    md.push('\n');

    // Separator
    md.push('|');
    for _ in &headers {
        md.push_str(" --- |");
    }
    md.push('\n');

    // Data rows
    for row in &rows {
        md.push('|');
        for (i, val) in row.iter().enumerate() {
            if i < headers.len() {
                md.push_str(&format!(" {} |", val));
            }
        }
        md.push('\n');
    }

    Ok(serde_json::json!({
        "action": "table",
        "title": title,
        "headers": headers,
        "rows": rows.len(),
        "columns": headers.len(),
        "markdown": md,
    }))
}

/// Generate a markdown table from JSON array input.
fn generate_table_from_json(
    data_str: &str,
    title: &str,
    _params: &HashMap<String, Value>,
) -> Result<Value, String> {
    let parsed: Value =
        serde_json::from_str(data_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let arr = match &parsed {
        Value::Array(arr) => arr,
        _ => return Err("data must be a JSON array".to_string()),
    };

    if arr.is_empty() {
        return Ok(serde_json::json!({
            "action": "table",
            "title": title,
            "headers": [],
            "rows": 0,
            "columns": 0,
            "markdown": format!("### {}\n\n*No data*", title),
        }));
    }

    // Collect all keys in order
    let mut headers: Vec<String> = Vec::new();
    for item in arr {
        if let Value::Object(map) = item {
            for key in map.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    if headers.is_empty() {
        // Array of scalars
        headers.push("value".to_string());
    }

    let mut md = format!("### {}\n\n", title);

    // Header row
    md.push('|');
    for h in &headers {
        md.push_str(&format!(" {} |", h));
    }
    md.push('\n');

    // Separator
    md.push('|');
    for _ in &headers {
        md.push_str(" --- |");
    }
    md.push('\n');

    // Data rows
    for item in arr {
        md.push('|');
        match item {
            Value::Object(map) => {
                for h in &headers {
                    let val = map.get(h).map(format_value).unwrap_or_default();
                    md.push_str(&format!(" {} |", val));
                }
            }
            other => {
                md.push_str(&format!(" {} |", format_value(other)));
                for _ in 1..headers.len() {
                    md.push_str(" |");
                }
            }
        }
        md.push('\n');
    }

    Ok(serde_json::json!({
        "action": "table",
        "title": title,
        "headers": headers,
        "rows": arr.len(),
        "columns": headers.len(),
        "markdown": md,
    }))
}

/// Generate a histogram visualization from binned data.
fn cmd_histogram(params: &HashMap<String, Value>) -> Result<Value, String> {
    let title = get_title(params);

    let bins_str = params
        .get("bins")
        .and_then(|v| v.as_str())
        .ok_or("'bins' is required for histogram (JSON array of {bin, count})")?;

    let bins: Vec<Value> = serde_json::from_str(bins_str)
        .map_err(|e| format!("Failed to parse bins JSON: {e}"))?;

    if bins.is_empty() {
        return Err("Histogram bins array is empty".to_string());
    }

    // Extract bin labels and counts
    let mut labels = Vec::new();
    let mut counts = Vec::new();
    let max_count = bins
        .iter()
        .filter_map(|b| b.get("count").and_then(|c| c.as_u64()))
        .max()
        .unwrap_or(1) as f64;

    for b in &bins {
        let label = b
            .get("bin")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                _ => format!("{v}"),
            })
            .unwrap_or_default();
        let count = b
            .get("count")
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize;
        labels.push(label);
        counts.push(count);
    }

    // Generate ASCII histogram
    let bar_width = 25usize;
    let mut ascii = format!("📊 Histogram: {title}\n\n");

    let max_label_len = labels.iter().map(|l| l.len()).max().unwrap_or(10);
    let label_width = max_label_len.min(25);

    for (i, count) in counts.iter().enumerate() {
        let bar_len = if max_count > 0.0 {
            ((*count as f64 / max_count) * bar_width as f64).round() as usize
        } else {
            0
        };
        let bar_len = bar_len.max(1).min(bar_width);
        let bar = "█".repeat(bar_len);
        let padded_label = if labels[i].len() < label_width {
            format!("{}{}", labels[i], " ".repeat(label_width - labels[i].len()))
        } else {
            labels[i][..label_width.min(labels[i].len())].to_string()
        };
        ascii.push_str(&format!("  {} | {} {}\n", padded_label, bar, count));
    }

    // Mermaid bar chart representation
    let labels_str: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let counts_str: Vec<String> = counts.iter().map(|c| c.to_string()).collect();

    let mermaid = format!(
        "```mermaid\nxychart-beta\n    title \"{title}\"\n    x-axis [{labels}]\n    y-axis \"Count\"\n    bar [{values}]\n```\n",
        labels = labels_str
            .iter()
            .map(|l| format!("\"{}\"", l.replace('"', "'")))
            .collect::<Vec<_>>()
            .join(", "),
        values = counts_str.join(", "),
    );

    Ok(serde_json::json!({
        "action": "histogram",
        "title": title,
        "bins": bins.len(),
        "total_count": counts.iter().sum::<usize>(),
        "mermaid": mermaid,
        "ascii": ascii,
    }))
}

/// Format a JSON value for display in table cells.
fn format_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => format!("{v}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! map {
        ($($key:expr => $value:expr),* $(,)?) => {{
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key.to_string(), $value);
            )*
            m
        }};
    }

    #[test]
    fn test_pie_chart_basic() {
        let params = map![
            "action" => json!("pie"),
            "labels" => json!("A,B,C"),
            "values" => json!("10,20,30"),
            "title" => json!("Test Pie"),
        ];
        let result = cmd_pie(&params).unwrap();
        assert_eq!(result["action"], "pie");
        assert_eq!(result["data_points"], 3);
        assert_eq!(result["total"], 60.0);
        let mermaid = result["mermaid"].as_str().unwrap();
        assert!(mermaid.contains("pie title Test Pie"));
        assert!(mermaid.contains("\"A\" : 10"));
        assert!(mermaid.contains("\"B\" : 20"));
        assert!(mermaid.contains("\"C\" : 30"));
    }

    #[test]
    fn test_pie_chart_from_json_data() {
        let params = map![
            "action" => json!("pie"),
            "data" => json!("[{\"label\":\"X\",\"value\":50},{\"label\":\"Y\",\"value\":30}]"),
            "title" => json!("JSON Pie"),
        ];
        let result = cmd_pie(&params).unwrap();
        assert_eq!(result["data_points"], 2);
        assert!(result["mermaid"].as_str().unwrap().contains("\"X\" : 50"));
    }

    #[test]
    fn test_pie_chart_negative_value() {
        let params = map![
            "action" => json!("pie"),
            "labels" => json!("A,B"),
            "values" => json!("10,-5"),
        ];
        let result = cmd_pie(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("negative"));
    }

    #[test]
    fn test_bar_chart_basic() {
        let params = map![
            "action" => json!("bar"),
            "labels" => json!("Alpha,Beta,Gamma"),
            "values" => json!("100,50,25"),
            "title" => json!("Test Bar"),
        ];
        let result = cmd_bar(&params).unwrap();
        assert_eq!(result["action"], "bar");
        assert_eq!(result["data_points"], 3);
        assert_eq!(result["max_value"], 100.0);
        let mermaid = result["mermaid"].as_str().unwrap();
        assert!(mermaid.contains("xychart-beta"));
        assert!(mermaid.contains("\"Alpha\""));
        let ascii = result["ascii"].as_str().unwrap();
        assert!(ascii.contains("Alpha"));
        assert!(ascii.contains("100"));
    }

    #[test]
    fn test_bar_chart_with_custom_width() {
        let params = map![
            "action" => json!("bar"),
            "labels" => json!("A,B"),
            "values" => json!("10,20"),
            "width" => json!(10),
        ];
        let result = cmd_bar(&params).unwrap();
        assert!(result["mermaid"].as_str().unwrap().contains("bar [10, 20]"));
    }

    #[test]
    fn test_line_chart_basic() {
        let params = map![
            "action" => json!("line"),
            "labels" => json!("Jan,Feb,Mar,Apr"),
            "values" => json!("10,25,15,30"),
            "title" => json!("Trend"),
        ];
        let result = cmd_line(&params).unwrap();
        assert_eq!(result["data_points"], 4);
        assert_eq!(result["min_value"], 10.0);
        assert_eq!(result["max_value"], 30.0);
        let mermaid = result["mermaid"].as_str().unwrap();
        assert!(mermaid.contains("xychart-beta"));
        assert!(mermaid.contains("line [10, 25, 15, 30]"));
        let ascii = result["ascii"].as_str().unwrap();
        assert!(ascii.contains("10"));
        assert!(ascii.contains("30"));
    }

    #[test]
    fn test_table_basic() {
        let params = map![
            "action" => json!("table"),
            "headers" => json!("Name,Age,City"),
            "rows" => json!("[[\"Alice\",30,\"NYC\"],[\"Bob\",25,\"LA\"],[\"Charlie\",35,\"Chicago\"]]"),
            "title" => json!("People"),
        ];
        let result = cmd_table(&params).unwrap();
        assert_eq!(result["rows"], 3);
        assert_eq!(result["columns"], 3);
        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("Alice"));
        assert!(md.contains("Bob"));
        assert!(md.contains("Charlie"));
        assert!(md.contains("Name"));
        assert!(md.contains("Age"));
        assert!(md.contains("---"));
    }

    #[test]
    fn test_table_from_json_data() {
        let params = map![
            "action" => json!("table"),
            "data" => json!("[{\"name\":\"Alice\",\"age\":30},{\"name\":\"Bob\",\"age\":25}]"),
            "title" => json!("From JSON"),
        ];
        let result = cmd_table(&params).unwrap();
        assert_eq!(result["rows"], 2);
        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("Alice"));
        assert!(md.contains("Bob"));
        assert!(md.contains("name"));
        assert!(md.contains("age"));
    }

    #[test]
    fn test_histogram_basic() {
        let params = map![
            "action" => json!("histogram"),
            "bins" => json!("[{\"bin\":\"0-10\",\"count\":5},{\"bin\":\"10-20\",\"count\":12},{\"bin\":\"20-30\",\"count\":8}]"),
            "title" => json!("Age Distribution"),
        ];
        let result = cmd_histogram(&params).unwrap();
        assert_eq!(result["bins"], 3);
        assert_eq!(result["total_count"], 25);
        let ascii = result["ascii"].as_str().unwrap();
        assert!(ascii.contains("0-10"));
        assert!(ascii.contains("10-20"));
        assert!(ascii.contains("20-30"));
        assert!(ascii.contains("12"));
        let mermaid = result["mermaid"].as_str().unwrap();
        assert!(mermaid.contains("xychart-beta"));
        assert!(mermaid.contains("bar [5, 12, 8]"));
    }

    #[test]
    fn test_extract_data_labels_values() {
        let params = map![
            "labels" => json!("X,Y,Z"),
            "values" => json!("1.5,2.5,3.5"),
        ];
        let data = extract_data(&params).unwrap();
        assert_eq!(data.len(), 3);
        assert_eq!(data[0].0, "X");
        assert!((data[0].1 - 1.5).abs() < 1e-10);
        assert_eq!(data[2].0, "Z");
        assert!((data[2].1 - 3.5).abs() < 1e-10);
    }

    #[test]
    fn test_extract_data_json() {
        let params = map![
            "data" => json!("[{\"label\":\"A\",\"value\":10},{\"label\":\"B\",\"value\":20}]"),
        ];
        let data = extract_data(&params).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].0, "A");
        assert!((data[0].1 - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_extract_data_mismatched_lengths() {
        let params = map![
            "labels" => json!("A,B"),
            "values" => json!("1,2,3"),
        ];
        let result = extract_data(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(&json!("hello")), "hello");
        assert_eq!(format_value(&json!(42)), "42");
        assert_eq!(format_value(&json!(3.14)), "3.14");
        assert_eq!(format_value(&json!(true)), "true");
        assert_eq!(format_value(&Value::Null), "");
    }

    #[test]
    fn test_sparkline() {
        let data = vec![
            ("A".to_string(), 10.0),
            ("B".to_string(), 20.0),
            ("C".to_string(), 30.0),
            ("D".to_string(), 15.0),
            ("E".to_string(), 25.0),
        ];
        let spark = generate_ascii_sparkline(&data, 20);
        assert!(spark.contains("10"));
        assert!(spark.contains("30"));
        assert!(spark.contains("A"));
        assert!(spark.contains("E"));
    }

    #[test]
    fn test_chart_tool_metadata() {
        let tool = ChartGeneratorTool;
        assert_eq!(tool.name(), "chart_generator");
        assert!(tool.description().contains("pie"));
        assert!(tool.description().contains("bar"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
    }

    #[test]
    fn test_empty_data_returns_error() {
        let params = map![
            "action" => json!("pie"),
            "labels" => json!(""),
            "values" => json!(""),
        ];
        let result = cmd_pie(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_table_empty_data() {
        let params = map![
            "action" => json!("table"),
            "data" => json!("[]"),
            "title" => json!("Empty"),
        ];
        let result = cmd_table(&params).unwrap();
        assert_eq!(result["rows"], 0);
    }

    #[test]
    fn test_bar_chart_negative_values() {
        let params = map![
            "action" => json!("bar"),
            "labels" => json!("A"),
            "values" => json!("-10"),
        ];
        let result = cmd_bar(&params);
        assert!(result.is_err());
    }
}
