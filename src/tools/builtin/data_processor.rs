//! Data Processor Tool — comprehensive data processing for CSV, JSON, and structured data.
//!
//! Supports: parse, filter, select, sort, aggregate, stats, merge, convert,
//! head/tail, unique, rename, add_column, and descriptive statistics.
//!
//! # Examples
//!
//! ```ignore
//! // Parse a CSV file and compute stats on a column:
//! // data_processor(
//! //   action: "parse",
//! //   file: "data.csv",
//! //   format: "csv"
//! // )
//! ```

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DataProcessorTool));
}

struct DataProcessorTool;

#[async_trait::async_trait]
impl Tool for DataProcessorTool {
    fn name(&self) -> &str {
        "data_processor"
    }

    fn description(&self) -> &str {
        "Comprehensive data processing tool for CSV/JSON/structured data. \
         Actions: parse (load CSV/JSON), filter (rows by condition), select (columns), \
         sort (by column asc/desc), aggregate (group + sum/avg/count/min/max), \
         stats (mean/median/std_dev on column), merge (join two datasets), \
         convert (CSV↔JSON), head/tail (first/last N rows), unique (distinct values), \
         rename (columns), add_column (computed column). \
         Use 'data' parameter to pass inline data, or 'file' to load from file."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Operation: parse, filter, select, sort, aggregate, stats, merge, \
                              convert, head, tail, unique, rename, add_column, info, describe"
                    .to_string(),
                required: true,
            },
            ToolParameter {
                name: "file".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to input data file (.csv or .json)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "data".to_string(),
                parameter_type: "string".to_string(),
                description: "Inline data (JSON array string or CSV text)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "format".to_string(),
                parameter_type: "string".to_string(),
                description: "Data format: 'csv' or 'json' (default: auto-detect from extension or content)"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "column".to_string(),
                parameter_type: "string".to_string(),
                description: "Column name for stats/unique/sort/aggregate operations".to_string(),
                required: false,
            },
            ToolParameter {
                name: "columns".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated column names for select/rename operations".to_string(),
                required: false,
            },
            ToolParameter {
                name: "condition".to_string(),
                parameter_type: "string".to_string(),
                description: "Filter condition (e.g. 'age > 25', 'name == Alice', 'city contains York'). \
                              Operators: ==, !=, >, >=, <, <=, contains, startswith, endswith"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "order".to_string(),
                parameter_type: "string".to_string(),
                description: "Sort order: 'asc' or 'desc' (default: 'asc')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "limit".to_string(),
                parameter_type: "number".to_string(),
                description: "Number of rows for head/tail/limit (default: 10)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "group_by".to_string(),
                parameter_type: "string".to_string(),
                description: "Column to group by for aggregate operation".to_string(),
                required: false,
            },
            ToolParameter {
                name: "aggregate_func".to_string(),
                parameter_type: "string".to_string(),
                description: "Aggregate function: sum, avg, count, min, max (default: count)"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "aggregate_column".to_string(),
                parameter_type: "string".to_string(),
                description: "Column to aggregate (for sum/avg/min/max)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "file2".to_string(),
                parameter_type: "string".to_string(),
                description: "Second file path (for merge operation)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "on".to_string(),
                parameter_type: "string".to_string(),
                description: "Column name to join on (for merge operation)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "how".to_string(),
                parameter_type: "string".to_string(),
                description: "Join type: inner, left, outer (default: inner, for merge)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path (for convert/parse with save)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "new_column".to_string(),
                parameter_type: "string".to_string(),
                description: "New column name (for add_column/rename)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "expression".to_string(),
                parameter_type: "string".to_string(),
                description: "Expression for computed column (e.g. 'price * qty' for add_column). \
                              Supports: +, -, *, /, () on numeric columns".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("action is required (parse, filter, select, sort, aggregate, stats, merge, \
                    convert, head, tail, unique, rename, add_column, info, describe)")?;

        match action {
            "parse" => cmd_parse(params),
            "filter" => cmd_filter(params),
            "select" => cmd_select(params),
            "sort" => cmd_sort(params),
            "aggregate" => cmd_aggregate(params),
            "stats" => cmd_stats(params),
            "merge" => cmd_merge(params),
            "convert" => cmd_convert(params),
            "head" => cmd_head_tail(params, true),
            "tail" => cmd_head_tail(params, false),
            "unique" => cmd_unique(params),
            "rename" => cmd_rename(params),
            "add_column" => cmd_add_column(params),
            "info" => cmd_info(params),
            "describe" => cmd_describe(params),
            _ => Err(format!("Unknown action: {action}. Available: parse, filter, select, sort, \
                              aggregate, stats, merge, convert, head, tail, unique, rename, \
                              add_column, info, describe")),
        }
    }
}

// ---------------------------------------------------------------------------
// Data loading helpers
// ---------------------------------------------------------------------------

/// Load data from params — either from a `file` or inline `data` parameter.
fn load_data(params: &HashMap<String, Value>) -> Result<(Vec<HashMap<String, Value>>, String), String> {
    if let Some(file_path) = params.get("file").and_then(|v| v.as_str()) {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file '{}': {e}", file_path))?;
        let fmt = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let format = if fmt.is_empty() {
            detect_format_from_path(file_path, &content)
        } else {
            fmt.to_string()
        };
        parse_content(&content, &format)
    } else if let Some(data_str) = params.get("data").and_then(|v| v.as_str()) {
        let fmt = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let format = if fmt.is_empty() {
            detect_format("inline", data_str)
        } else {
            fmt.to_string()
        };
        parse_content(data_str, &format)
    } else {
        Err("Either 'file' or 'data' parameter is required".to_string())
    }
}

/// Detect data format from file extension or content.
fn detect_format_from_path(path: &str, _content: &str) -> String {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv") {
        "csv".to_string()
    } else if lower.ends_with(".json") {
        "json".to_string()
    } else {
        detect_format("file", _content)
    }
}

/// Detect format from content inspection.
fn detect_format(_name: &str, content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        "json".to_string()
    } else if trimmed.contains(',') && trimmed.lines().count() > 1 {
        "csv".to_string()
    } else {
        "json".to_string()
    }
}

/// Parse content string into rows of key-value maps.
fn parse_content(content: &str, format: &str) -> Result<(Vec<HashMap<String, Value>>, String), String> {
    match format {
        "csv" => parse_csv(content),
        "json" => parse_json(content),
        other => Err(format!("Unsupported format: {other}. Use 'csv' or 'json'")),
    }
}

/// Parse CSV text into rows.
fn parse_csv(content: &str) -> Result<(Vec<HashMap<String, Value>>, String), String> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("Failed to parse CSV headers: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        return Err("CSV has no headers".to_string());
    }

    let mut rows = Vec::new();
    for (i, result) in reader.records().enumerate() {
        let record = result.map_err(|e| format!("CSV row {}: {e}", i + 2))?;
        let mut row = HashMap::new();
        for (j, header) in headers.iter().enumerate() {
            let val = record.get(j).unwrap_or("");
            row.insert(header.clone(), parse_value(val));
        }
        rows.push(row);
    }

    Ok((rows, "csv".to_string()))
}

/// Parse JSON text into rows.
fn parse_json(content: &str) -> Result<(Vec<HashMap<String, Value>>, String), String> {
    let trimmed = content.trim();
    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    match parsed {
        Value::Array(arr) => {
            let rows: Vec<HashMap<String, Value>> = arr
                .into_iter()
                .map(|item| match item {
                    Value::Object(map) => Ok(map
                        .into_iter()
                        .map(|(k, v)| (k, v))
                        .collect::<HashMap<String, Value>>()),
                    other => Ok({
                        let mut m = HashMap::new();
                        m.insert("value".to_string(), other);
                        m
                    }),
                })
                .collect::<Result<Vec<_>, String>>()?;

            if rows.is_empty() {
                return Ok((rows, "json".to_string()));
            }
            Ok((rows, "json".to_string()))
        }
        Value::Object(map) => {
            // Single object -> wrap as single-row array
            let mut rows = Vec::new();
            let row: HashMap<String, Value> = map.into_iter().map(|(k, v)| (k, v)).collect();
            rows.push(row);
            Ok((rows, "json".to_string()))
        }
        other => {
            // Scalar value -> wrap
            let mut row = HashMap::new();
            row.insert("value".to_string(), other);
            Ok((vec![row], "json".to_string()))
        }
    }
}

/// Try to parse a string value as a number, otherwise keep as string.
fn parse_value(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    // Try integer first
    if let Ok(i) = s.parse::<i64>() {
        return json!(i);
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return json!(f);
    }
    // Try boolean
    match s.trim().to_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" | "nil" | "none" | "na" => return Value::Null,
        _ => {}
    }
    json!(s)
}

/// Convert rows back to JSON array value.
fn rows_to_json(rows: &[HashMap<String, Value>]) -> Value {
    Value::Array(
        rows.iter()
            .map(|row| {
                let map: serde_json::Map<String, Value> = row
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                Value::Object(map)
            })
            .collect(),
    )
}

/// Get a column value as a numeric f64 for computation.
fn get_numeric(row: &HashMap<String, Value>, column: &str) -> Option<f64> {
    row.get(column).and_then(|v| match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    })
}

/// Get a column value as a string for comparison.
fn get_string(row: &HashMap<String, Value>, column: &str) -> String {
    row.get(column)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => format!("{v}"),
        })
        .unwrap_or_default()
}

/// Collect all column names across all rows (preserving order).
fn collect_columns(rows: &[HashMap<String, Value>]) -> Vec<String> {
    let mut seen = Vec::new();
    for row in rows {
        for key in row.keys() {
            if !seen.contains(key) {
                seen.push(key.clone());
            }
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

/// Load data, optionally save to output, return summary.
fn cmd_parse(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, format) = load_data(params)?;

    // Save to output if requested
    if let Some(output) = params.get("output").and_then(|v| v.as_str()) {
        let out_fmt = if output.ends_with(".csv") {
            "csv"
        } else if output.ends_with(".json") {
            "json"
        } else {
            &format
        };
        save_data(&rows, output, out_fmt)?;
    }

    let cols = collect_columns(&rows);
    Ok(json!({
        "action": "parse",
        "rows": rows.len(),
        "columns": cols,
        "format": format,
        "data": rows_to_json(&rows),
        "preview": build_preview(&rows, 5),
    }))
}

/// Filter rows by a condition expression.
fn cmd_filter(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let condition = params
        .get("condition")
        .and_then(|v| v.as_str())
        .ok_or("condition is required (e.g. 'age > 25', 'city contains York')")?;

    let filtered = filter_rows(&rows, condition)?;

    Ok(json!({
        "action": "filter",
        "condition": condition,
        "input_rows": rows.len(),
        "output_rows": filtered.len(),
        "data": rows_to_json(&filtered),
        "preview": build_preview(&filtered, 5),
    }))
}

/// Select specific columns.
fn cmd_select(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let cols_str = params
        .get("columns")
        .and_then(|v| v.as_str())
        .ok_or("columns is required (comma-separated)")?;

    let selected_cols: Vec<&str> = cols_str.split(',').map(|s| s.trim()).collect();
    let mut result = Vec::new();

    for row in &rows {
        let mut new_row = HashMap::new();
        for col in &selected_cols {
            if let Some(val) = row.get(*col) {
                new_row.insert((*col).to_string(), val.clone());
            }
        }
        result.push(new_row);
    }

    Ok(json!({
        "action": "select",
        "columns": selected_cols,
        "input_rows": rows.len(),
        "output_rows": result.len(),
        "data": rows_to_json(&result),
        "preview": build_preview(&result, 5),
    }))
}

/// Sort rows by a column.
fn cmd_sort(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (mut rows, _) = load_data(params)?;
    let column = params
        .get("column")
        .and_then(|v| v.as_str())
        .ok_or("column is required for sort")?;

    let order = params
        .get("order")
        .and_then(|v| v.as_str())
        .unwrap_or("asc");

    let descending = order == "desc";

    // Try numeric sort first, fall back to string sort
    let all_numeric = rows
        .iter()
        .filter_map(|r| get_numeric(r, column))
        .count()
        == rows.iter().filter(|r| r.contains_key(column)).count();

    if all_numeric && !rows.is_empty() {
        rows.sort_by(|a, b| {
            let va = get_numeric(a, column).unwrap_or(f64::NEG_INFINITY);
            let vb = get_numeric(b, column).unwrap_or(f64::NEG_INFINITY);
            if descending {
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    } else {
        rows.sort_by(|a, b| {
            let va = get_string(a, column);
            let vb = get_string(b, column);
            if descending {
                vb.cmp(&va)
            } else {
                va.cmp(&vb)
            }
        });
    }

    Ok(json!({
        "action": "sort",
        "column": column,
        "order": order,
        "rows": rows.len(),
        "data": rows_to_json(&rows),
        "preview": build_preview(&rows, 5),
    }))
}

/// Group by a column and compute aggregate.
fn cmd_aggregate(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let group_by = params
        .get("group_by")
        .and_then(|v| v.as_str())
        .ok_or("group_by is required for aggregate")?;

    let agg_func = params
        .get("aggregate_func")
        .and_then(|v| v.as_str())
        .unwrap_or("count");

    let agg_col = params
        .get("aggregate_column")
        .and_then(|v| v.as_str());

    let mut groups: HashMap<String, Vec<&HashMap<String, Value>>> = HashMap::new();
    for row in &rows {
        let key = get_string(row, group_by);
        groups.entry(key).or_default().push(row);
    }

    let mut results = Vec::new();
    for (key, group) in &groups {
        let mut result = serde_json::Map::new();
        result.insert(group_by.to_string(), json!(key));
        result.insert("count".to_string(), json!(group.len()));

        if let Some(ac) = agg_col {
            let vals: Vec<f64> = group
                .iter()
                .filter_map(|r| get_numeric(r, ac))
                .collect();
            let agg_value: Value = match agg_func {
                "sum" => json!(vals.iter().sum::<f64>()),
                "avg" | "mean" => {
                    if vals.is_empty() {
                        Value::Null
                    } else {
                        json!(vals.iter().sum::<f64>() / vals.len() as f64)
                    }
                }
                "min" => json!(vals.iter().cloned().fold(f64::MAX, f64::min)),
                "max" => json!(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
                _ => json!(group.len()), // count
            };
            result.insert(agg_func.to_string(), agg_value);
        }

        results.push(Value::Object(result));
    }

    // Sort results by key for consistent output
    results.sort_by(|a, b| {
        let ka = a.get(group_by).and_then(|v| v.as_str()).unwrap_or("");
        let kb = b.get(group_by).and_then(|v| v.as_str()).unwrap_or("");
        ka.cmp(kb)
    });

    Ok(json!({
        "action": "aggregate",
        "group_by": group_by,
        "function": agg_func,
        "groups": groups.len(),
        "data": Value::Array(results),
    }))
}

/// Compute descriptive statistics on a numeric column.
fn cmd_stats(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let column = params
        .get("column")
        .and_then(|v| v.as_str())
        .ok_or("column is required for stats")?;

    let values: Vec<f64> = rows.iter().filter_map(|r| get_numeric(r, column)).collect();
    let non_null = values.len();
    let null_count = rows.len() - non_null;

    if values.is_empty() {
        return Ok(json!({
            "action": "stats",
            "column": column,
            "total_rows": rows.len(),
            "non_null": 0,
            "null_count": null_count,
            "message": "No numeric values found in column",
        }));
    }

    let sum: f64 = values.iter().sum();
    let mean = sum / values.len() as f64;

    // Median
    let mut sorted = values.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len() % 2 == 0 {
        let mid = sorted.len() / 2;
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    // Variance & std dev
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();

    // Min / Max
    let min = sorted.first().copied().unwrap_or(0.0);
    let max = sorted.last().copied().unwrap_or(0.0);

    // Quartiles
    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);

    Ok(json!({
        "action": "stats",
        "column": column,
        "total_rows": rows.len(),
        "non_null": non_null,
        "null_count": null_count,
        "count": values.len(),
        "sum": sum,
        "mean": mean,
        "median": median,
        "std_dev": std_dev,
        "variance": variance,
        "min": min,
        "max": max,
        "q1": q1,
        "q3": q3,
        "range": max - min,
        "histogram": compute_histogram(&values, 10),
    }))
}

/// Compute percentile from sorted array.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let k = (p / 100.0) * (sorted.len() - 1) as f64;
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        sorted[f]
    } else {
        let d0 = sorted[f] * (c as f64 - k);
        let d1 = sorted[c] * (k - f as f64);
        d0 + d1
    }
}

/// Compute a simple histogram with equal-width bins.
fn compute_histogram(values: &[f64], num_bins: usize) -> Value {
    if values.is_empty() || num_bins == 0 {
        return Value::Array(vec![]);
    }

    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max - min).abs() < f64::EPSILON {
        // All values are the same
        return json!([{"bin": format!("{:.2}", min), "count": values.len()}]);
    }

    let bin_width = (max - min) / num_bins as f64;
    let mut bins = vec![0usize; num_bins];

    for v in values {
        let mut idx = ((v - min) / bin_width) as usize;
        if idx >= num_bins {
            idx = num_bins - 1;
        }
        bins[idx] += 1;
    }

    let result: Vec<Value> = bins
        .into_iter()
        .enumerate()
        .map(|(i, count)| {
            let bin_start = min + i as f64 * bin_width;
            let bin_end = bin_start + bin_width;
            json!({
                "bin": format!("{:.2}-{:.2}", bin_start, bin_end),
                "count": count,
            })
        })
        .collect();

    Value::Array(result)
}

/// Merge/join two datasets.
fn cmd_merge(params: &HashMap<String, Value>) -> Result<Value, String> {
    // Load first dataset
    let (rows1, _) = load_data(params)?;

    // Load second dataset — need file2 or data2
    let file2 = params.get("file2").and_then(|v| v.as_str());
    let data2_raw = params.get("data2").and_then(|v| v.as_str());

    let rows2 = if let Some(f2) = file2 {
        let content =
            std::fs::read_to_string(f2).map_err(|e| format!("Failed to read file2 '{f2}': {e}"))?;
        let fmt = detect_format_from_path(f2, &content);
        parse_content(&content, &fmt).map(|(r, _)| r)?
    } else if let Some(d2) = data2_raw {
        let fmt = detect_format("data2", d2);
        parse_content(d2, &fmt).map(|(r, _)| r)?
    } else {
        return Err("merge requires 'file2' or 'data2' parameter for the second dataset".to_string());
    };

    let on = params
        .get("on")
        .and_then(|v| v.as_str())
        .ok_or("on (join column) is required for merge")?;

    let how = params
        .get("how")
        .and_then(|v| v.as_str())
        .unwrap_or("inner");

    // Build lookup from second dataset
    let mut lookup: HashMap<String, Vec<&HashMap<String, Value>>> = HashMap::new();
    for row in &rows2 {
        let key = get_string(row, on);
        lookup.entry(key).or_default().push(row);
    }

    let _left_keys: std::collections::HashSet<String> =
        rows2.iter().map(|r| get_string(r, on)).collect();

    let mut result = Vec::new();
    let mut matched_right = std::collections::HashSet::new();

    for row1 in &rows1 {
        let key = get_string(row1, on);
        if let Some(matches) = lookup.get(&key) {
            for row2 in matches {
                let mut merged = row1.clone();
                for (k, v) in row2.iter() {
                    if k != on {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                matched_right.insert(key.clone());
                result.push(merged);
            }
        } else if how == "left" || how == "outer" {
            // Left row with nulls for right side
            let mut merged = row1.clone();
            // Add all columns from rows2 with null
            if let Some(first_row2) = rows2.first() {
                for k in first_row2.keys() {
                    if k != on {
                        merged.entry(k.clone()).or_insert(Value::Null);
                    }
                }
            }
            result.push(merged);
        }
    }

    // Outer: add unmatched right rows
    if how == "outer" {
        for row2 in &rows2 {
            let key = get_string(row2, on);
            if !matched_right.contains(&key) {
                let mut merged = HashMap::new();
                // Add columns from rows1 with null
                if let Some(first_row1) = rows1.first() {
                    for k in first_row1.keys() {
                        merged.insert(k.clone(), Value::Null);
                    }
                }
                for (k, v) in row2.iter() {
                    merged.insert(k.clone(), v.clone());
                }
                result.push(merged);
            }
        }
    }

    Ok(json!({
        "action": "merge",
        "on": on,
        "how": how,
        "left_rows": rows1.len(),
        "right_rows": rows2.len(),
        "output_rows": result.len(),
        "data": rows_to_json(&result),
        "preview": build_preview(&result, 5),
    }))
}

/// Convert between CSV and JSON.
fn cmd_convert(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, source_format) = load_data(params)?;
    let output = params
        .get("output")
        .and_then(|v| v.as_str())
        .ok_or("output file path is required for convert")?;

    let target_format = if output.ends_with(".csv") {
        "csv"
    } else if output.ends_with(".json") {
        "json"
    } else {
        // Default: opposite of source
        if source_format == "csv" {
            "json"
        } else {
            "csv"
        }
    };

    save_data(&rows, output, target_format)?;

    Ok(json!({
        "action": "convert",
        "from": source_format,
        "to": target_format,
        "rows": rows.len(),
        "output": output,
    }))
}

/// Head or tail of dataset.
fn cmd_head_tail(params: &HashMap<String, Value>, is_head: bool) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    let selected: Vec<HashMap<String, Value>> = if is_head {
        rows.iter().take(limit).cloned().collect()
    } else {
        rows.iter().rev().take(limit).rev().cloned().collect()
    };

    Ok(json!({
        "action": if is_head { "head" } else { "tail" },
        "limit": limit,
        "total_rows": rows.len(),
        "data": rows_to_json(&selected),
    }))
}

/// Get unique values in a column.
fn cmd_unique(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let column = params
        .get("column")
        .and_then(|v| v.as_str())
        .ok_or("column is required for unique")?;

    let mut values: Vec<Value> = rows
        .iter()
        .filter_map(|r| r.get(column).cloned())
        .collect::<Vec<_>>();

    // Deduplicate
    values.sort_by(|a, b| {
        let sa = format!("{a}");
        let sb = format!("{b}");
        sa.cmp(&sb)
    });
    values.dedup_by(|a, b| format!("{a}") == format!("{b}"));

    // Count occurrences
    let mut counts: HashMap<String, usize> = HashMap::new();
    for row in &rows {
        let val = get_string(row, column);
        *counts.entry(val).or_insert(0) += 1;
    }

    let counts_arr: Vec<Value> = counts
        .into_iter()
        .map(|(val, count)| json!({"value": val, "count": count}))
        .collect();

    Ok(json!({
        "action": "unique",
        "column": column,
        "unique_values": values.len(),
        "total_rows": rows.len(),
        "values": Value::Array(values),
        "counts": Value::Array(counts_arr),
    }))
}

/// Rename columns.
fn cmd_rename(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let columns_str = params
        .get("columns")
        .and_then(|v| v.as_str())
        .ok_or("columns is required: comma-separated old column names")?;

    let new_columns_str = params
        .get("new_column")
        .and_then(|v| v.as_str())
        .ok_or("new_column is required: comma-separated new column names")?;

    let old_cols: Vec<&str> = columns_str.split(',').map(|s| s.trim()).collect();
    let new_cols: Vec<&str> = new_columns_str.split(',').map(|s| s.trim()).collect();

    if old_cols.len() != new_cols.len() {
        return Err(format!(
            "Number of old columns ({}) must match number of new columns ({})",
            old_cols.len(),
            new_cols.len()
        ));
    }

    let mut rename_map = HashMap::new();
    for (old, new) in old_cols.iter().zip(new_cols.iter()) {
        rename_map.insert(old.to_string(), new.to_string());
    }

    let result: Vec<HashMap<String, Value>> = rows
        .into_iter()
        .map(|row| {
            let mut new_row = HashMap::new();
            for (k, v) in row {
                let new_key = rename_map.get(&k).cloned().unwrap_or(k);
                new_row.insert(new_key, v);
            }
            new_row
        })
        .collect();

    Ok(json!({
        "action": "rename",
        "renamed": rename_map,
        "rows": result.len(),
        "data": rows_to_json(&result),
        "preview": build_preview(&result, 5),
    }))
}

/// Add a computed column using a simple expression.
fn cmd_add_column(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, _) = load_data(params)?;
    let new_column = params
        .get("new_column")
        .and_then(|v| v.as_str())
        .ok_or("new_column name is required for add_column")?;

    let expression = params
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or("expression is required (e.g. 'price * qty')")?;

    let mut result = Vec::new();
    for row in &rows {
        let mut new_row = row.clone();
        let computed = eval_expression(row, expression);
        new_row.insert(new_column.to_string(), computed);
        result.push(new_row);
    }

    Ok(json!({
        "action": "add_column",
        "new_column": new_column,
        "expression": expression,
        "rows": result.len(),
        "data": rows_to_json(&result),
        "preview": build_preview(&result, 5),
    }))
}

/// Simple arithmetic expression evaluator. Supports: +, -, *, /, () on numeric column references.
fn eval_expression(row: &HashMap<String, Value>, expr: &str) -> Value {
    let trimmed = expr.trim();

    // Try to resolve as a direct column value
    if let Some(val) = row.get(trimmed) {
        return val.clone();
    }

    // Tokenize and evaluate simple arithmetic
    let tokens = tokenize(trimmed);
    let evaluated = try_eval_tokens(&tokens, row);

    match evaluated {
        Some(n) => json!(n),
        None => json!(format!("<expr: {expr}>")),
    }
}

/// Simple tokenizer for arithmetic expressions.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch.is_ascii_digit() || ch == '.' {
            let mut num = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '.' {
                    num.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::Number(num.parse::<f64>().unwrap_or(0.0)));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let mut ident = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    ident.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::Ident(ident));
        } else {
            match ch {
                '+' => tokens.push(Token::Plus),
                '-' => tokens.push(Token::Minus),
                '*' => tokens.push(Token::Star),
                '/' => tokens.push(Token::Slash),
                '(' => tokens.push(Token::LParen),
                ')' => tokens.push(Token::RParen),
                _ => {} // skip unknown
            }
            chars.next();
        }
    }

    tokens
}

/// Simple recursive-descent expression evaluator.
fn try_eval_tokens(tokens: &[Token], row: &HashMap<String, Value>) -> Option<f64> {
    let mut pos = 0;
    parse_expr(tokens, &mut pos, row)
}

fn parse_expr(tokens: &[Token], pos: &mut usize, row: &HashMap<String, Value>) -> Option<f64> {
    let mut left = parse_term(tokens, pos, row)?;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                let right = parse_term(tokens, pos, row)?;
                left += right;
            }
            Token::Minus => {
                *pos += 1;
                let right = parse_term(tokens, pos, row)?;
                left -= right;
            }
            _ => break,
        }
    }

    Some(left)
}

fn parse_term(tokens: &[Token], pos: &mut usize, row: &HashMap<String, Value>) -> Option<f64> {
    let mut left = parse_factor(tokens, pos, row)?;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Star => {
                *pos += 1;
                let right = parse_factor(tokens, pos, row)?;
                left *= right;
            }
            Token::Slash => {
                *pos += 1;
                let right = parse_factor(tokens, pos, row)?;
                if right == 0.0 {
                    return None; // division by zero
                }
                left /= right;
            }
            _ => break,
        }
    }

    Some(left)
}

fn parse_factor(tokens: &[Token], pos: &mut usize, row: &HashMap<String, Value>) -> Option<f64> {
    if *pos >= tokens.len() {
        return None;
    }

    match &tokens[*pos] {
        Token::Number(n) => {
            *pos += 1;
            Some(*n)
        }
        Token::Ident(name) => {
            *pos += 1;
            get_numeric(row, name)
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_expr(tokens, pos, row)?;
            if *pos < tokens.len() && tokens[*pos] == Token::RParen {
                *pos += 1;
            }
            Some(val)
        }
        Token::Minus => {
            *pos += 1;
            let val = parse_factor(tokens, pos, row)?;
            Some(-val)
        }
        _ => None,
    }
}

/// Get dataset info (column names, types, row count).
fn cmd_info(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, format) = load_data(params)?;
    let cols = collect_columns(&rows);

    let mut col_info: Vec<Value> = Vec::new();
    for col in &cols {
        let non_null = rows.iter().filter(|r| r.contains_key(col)).count();
        let numeric_count = rows.iter().filter(|r| get_numeric(r, col).is_some()).count();
        let string_count = rows.iter().filter(|r| {
            r.get(col)
                .map(|v| matches!(v, Value::String(_)))
                .unwrap_or(false)
        })
        .count();
        let bool_count = rows.iter().filter(|r| {
            r.get(col)
                .map(|v| matches!(v, Value::Bool(_)))
                .unwrap_or(false)
        })
        .count();
        let null_count = rows.len() - non_null;

        let inferred_type = if numeric_count == non_null && non_null > 0 {
            "numeric"
        } else if bool_count == non_null && non_null > 0 {
            "boolean"
        } else {
            "string"
        };

        col_info.push(json!({
            "name": col,
            "non_null": non_null,
            "null_count": null_count,
            "numeric_count": numeric_count,
            "string_count": string_count,
            "bool_count": bool_count,
            "inferred_type": inferred_type,
        }));
    }

    Ok(json!({
        "action": "info",
        "rows": rows.len(),
        "columns": cols.len(),
        "format": format,
        "column_info": col_info,
    }))
}

/// Generate a summary description of the dataset.
fn cmd_describe(params: &HashMap<String, Value>) -> Result<Value, String> {
    let (rows, format) = load_data(params)?;
    let cols = collect_columns(&rows);

    let mut descriptions = Vec::new();
    for col in &cols {
        let numeric_vals: Vec<f64> = rows.iter().filter_map(|r| get_numeric(r, col)).collect();
        let string_vals: Vec<String> = rows
            .iter()
            .filter_map(|r| {
                let s = get_string(r, col);
                if s.is_empty() { None } else { Some(s) }
            })
            .collect();

        if !numeric_vals.is_empty() {
            let sum: f64 = numeric_vals.iter().sum();
            let mean = sum / numeric_vals.len() as f64;
            let min = numeric_vals.iter().cloned().fold(f64::MAX, f64::min);
            let max = numeric_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            descriptions.push(json!({
                "column": col,
                "type": "numeric",
                "count": numeric_vals.len(),
                "null_count": rows.len() - numeric_vals.len(),
                "mean": mean,
                "min": min,
                "max": max,
                "sum": sum,
            }));
        } else if !string_vals.is_empty() {
            // Count distinct
            let mut uniq: Vec<&str> = string_vals.iter().map(|s| s.as_str()).collect();
            uniq.sort();
            uniq.dedup();

            let most_common_val = uniq.first().copied().unwrap_or("");
            let most_common_count = string_vals
                .iter()
                .filter(|s| *s == most_common_val)
                .count();

            descriptions.push(json!({
                "column": col,
                "type": "string",
                "count": string_vals.len(),
                "null_count": rows.len() - string_vals.len(),
                "distinct": uniq.len(),
                "most_common": most_common_val,
                "most_common_count": most_common_count,
            }));
        } else {
            descriptions.push(json!({
                "column": col,
                "type": "unknown",
                "count": 0,
                "null_count": rows.len(),
            }));
        }
    }

    Ok(json!({
        "action": "describe",
        "rows": rows.len(),
        "columns": cols.len(),
        "format": format,
        "columns_detail": descriptions,
        "preview": build_preview(&rows, 3),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Save data to file in the specified format.
fn save_data(rows: &[HashMap<String, Value>], path: &str, format: &str) -> Result<(), String> {
    match format {
        "csv" => {
            let cols = collect_columns(rows);
            let mut wtr = csv::Writer::from_path(path)
                .map_err(|e| format!("Failed to create CSV writer: {e}"))?;

            wtr.write_record(&cols)
                .map_err(|e| format!("Failed to write CSV header: {e}"))?;

            for row in rows {
                let values: Vec<String> = cols
                    .iter()
                    .map(|c| get_string(row, c))
                    .collect();
                wtr.write_record(&values)
                    .map_err(|e| format!("Failed to write CSV row: {e}"))?;
            }

            wtr.flush()
                .map_err(|e| format!("Failed to flush CSV: {e}"))?;
        }
        "json" => {
            let json = rows_to_json(rows);
            let content = serde_json::to_string_pretty(&json)
                .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
            std::fs::write(path, content)
                .map_err(|e| format!("Failed to write JSON file: {e}"))?;
        }
        other => return Err(format!("Unsupported output format: {other}")),
    }
    Ok(())
}

/// Build a preview string showing first N rows.
fn build_preview(rows: &[HashMap<String, Value>], n: usize) -> Value {
    let preview_rows: Vec<Value> = rows.iter().take(n).map(|row| {
        let map: serde_json::Map<String, Value> = row
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Value::Object(map)
    }).collect();

    json!({
        "rows_shown": preview_rows.len().min(rows.len()),
        "total_rows": rows.len(),
        "data": Value::Array(preview_rows),
    })
}

/// Apply a filter condition to rows.
fn filter_rows(
    rows: &[HashMap<String, Value>],
    condition: &str,
) -> Result<Vec<HashMap<String, Value>>, String> {
    let condition = condition.trim();

    // Parse condition: "column operator value"
    let operators = [">=", "<=", "!=", "==", ">", "<", " contains ", " startswith ", " endswith "];

    let mut matched_op: Option<(&str, &str, &str)> = None;

    for op in &operators {
        if let Some(pos) = condition.find(op) {
            let column = condition[..pos].trim();
            let value = condition[pos + op.len()..].trim();
            let trimmed_op = op.trim();
            matched_op = Some((column, trimmed_op, value));
            break;
        }
    }

    let (column, op, value) = matched_op
        .ok_or_else(|| format!("Could not parse condition: '{condition}'. Use format: 'column op value' \
                                where op is one of: ==, !=, >, >=, <, <=, contains, startswith, endswith"))?;

    let is_numeric_compare = value.parse::<f64>().is_ok() || rows.iter().any(|r| get_numeric(r, column).is_some());
    let cmp_value = value.parse::<f64>().ok();

    let filtered: Vec<HashMap<String, Value>> = rows
        .iter()
        .filter(|row| match op {
            "==" => get_string(row, column) == value,
            "!=" => get_string(row, column) != value,
            ">" => {
                if is_numeric_compare {
                    get_numeric(row, column) > cmp_value
                } else {
                    get_string(row, column) > value.to_string()
                }
            }
            ">=" => {
                if is_numeric_compare {
                    get_numeric(row, column) >= cmp_value
                } else {
                    get_string(row, column) >= value.to_string()
                }
            }
            "<" => {
                if is_numeric_compare {
                    get_numeric(row, column) < cmp_value
                } else {
                    get_string(row, column) < value.to_string()
                }
            }
            "<=" => {
                if is_numeric_compare {
                    get_numeric(row, column) <= cmp_value
                } else {
                    get_string(row, column) <= value.to_string()
                }
            }
            "contains" => get_string(row, column).to_lowercase().contains(&value.to_lowercase()),
            "startswith" => get_string(row, column).to_lowercase().starts_with(&value.to_lowercase()),
            "endswith" => get_string(row, column).to_lowercase().ends_with(&value.to_lowercase()),
            _ => false,
        })
        .cloned()
        .collect();

    Ok(filtered)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper macro to create a HashMap from key-value pairs.
    macro_rules! map {
        ($($key:expr => $value:expr),* $(,)?) => {{
            let mut m: ::std::collections::HashMap<String, serde_json::Value> = ::std::collections::HashMap::new();
            $(
                m.insert($key.to_string(), $value);
            )*
            m
        }};
    }

    #[test]
    fn test_parse_csv_basic() {
        let csv = "name,age,city\nAlice,30,New York\nBob,25,Los Angeles\nCharlie,35,Chicago";
        let (rows, fmt) = parse_csv(csv).unwrap();
        assert_eq!(fmt, "csv");
        assert_eq!(rows.len(), 3);
        assert_eq!(get_string(&rows[0], "name"), "Alice");
        assert_eq!(get_numeric(&rows[0], "age"), Some(30.0));
        assert_eq!(get_string(&rows[0], "city"), "New York");
    }

    #[test]
    fn test_parse_csv_with_headers_only() {
        let csv = "name,age,city";
        let (rows, _) = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_parse_json_array() {
        let json = r#"[
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]"#;
        let (rows, fmt) = parse_json(json).unwrap();
        assert_eq!(fmt, "json");
        assert_eq!(rows.len(), 2);
        assert_eq!(get_string(&rows[0], "name"), "Alice");
        assert_eq!(get_numeric(&rows[0], "age"), Some(30.0));
    }

    #[test]
    fn test_parse_json_object() {
        let json = r#"{"name": "Alice", "age": 30}"#;
        let (rows, _) = parse_json(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(get_string(&rows[0], "name"), "Alice");
    }

    #[test]
    fn test_filter_numeric_gt() {
        let rows = vec![
            map!["name" => json!("Alice"), "age" => json!(30)],
            map!["name" => json!("Bob"), "age" => json!(25)],
            map!["name" => json!("Charlie"), "age" => json!(35)],
        ];
        let result = filter_rows(&rows, "age > 25").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(get_string(&result[0], "name"), "Alice");
        assert_eq!(get_string(&result[1], "name"), "Charlie");
    }

    #[test]
    fn test_filter_string_equals() {
        let rows = vec![
            map!["name" => json!("Alice"), "city" => json!("New York")],
            map!["name" => json!("Bob"), "city" => json!("Los Angeles")],
        ];
        let result = filter_rows(&rows, "city == New York").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(get_string(&result[0], "name"), "Alice");
    }

    #[test]
    fn test_filter_contains() {
        let rows = vec![
            map!["name" => json!("Alice"), "city" => json!("New York")],
            map!["name" => json!("Bob"), "city" => json!("Yorkville")],
            map!["name" => json!("Charlie"), "city" => json!("Boston")],
        ];
        let result = filter_rows(&rows, "city contains York").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_sort_numeric() {
        let mut rows = vec![
            map!["name" => json!("Bob"), "age" => json!(25)],
            map!["name" => json!("Charlie"), "age" => json!(35)],
            map!["name" => json!("Alice"), "age" => json!(30)],
        ];

        // Sort ascending
        rows.sort_by(|a, b| {
            let va = get_numeric(a, "age").unwrap_or(0.0);
            let vb = get_numeric(b, "age").unwrap_or(0.0);
            va.partial_cmp(&vb).unwrap()
        });
        assert_eq!(get_string(&rows[0], "name"), "Bob");
        assert_eq!(get_string(&rows[2], "name"), "Charlie");
    }

    #[test]
    fn test_stats_basic() {
        let rows = vec![
            map!["value" => json!(10)],
            map!["value" => json!(20)],
            map!["value" => json!(30)],
            map!["value" => json!(40)],
            map!["value" => json!(50)],
        ];

        let values: Vec<f64> = rows.iter().filter_map(|r| get_numeric(r, "value")).collect();
        assert_eq!(values.len(), 5);

        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        assert!((mean - 30.0).abs() < 1e-10);

        let median = {
            let mut s = values.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            s[2]
        };
        assert!((median - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_unique() {
        let rows = vec![
            map!["city" => json!("New York")],
            map!["city" => json!("Boston")],
            map!["city" => json!("New York")],
            map!["city" => json!("Chicago")],
        ];
        let mut values: Vec<String> = rows.iter().map(|r| get_string(r, "city")).collect();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&"New York".to_string()));
        assert!(values.contains(&"Boston".to_string()));
        assert!(values.contains(&"Chicago".to_string()));
    }

    #[test]
    fn test_aggregate_count() {
        let rows = vec![
            map!["dept" => json!("Engineering"), "name" => json!("Alice")],
            map!["dept" => json!("Engineering"), "name" => json!("Bob")],
            map!["dept" => json!("Sales"), "name" => json!("Charlie")],
        ];

        let mut groups: HashMap<String, Vec<&HashMap<String, Value>>> = HashMap::new();
        for row in &rows {
            let key = get_string(row, "dept");
            groups.entry(key).or_default().push(row);
        }
        assert_eq!(groups.get("Engineering").unwrap().len(), 2);
        assert_eq!(groups.get("Sales").unwrap().len(), 1);
    }

    #[test]
    fn test_rename_columns() {
        let rows = vec![
            map!["old_name" => json!("Alice"), "old_age" => json!(30)],
        ];

        let mut rename_map = HashMap::new();
        rename_map.insert("old_name".to_string(), "name".to_string());
        rename_map.insert("old_age".to_string(), "age".to_string());

        let result: Vec<HashMap<String, Value>> = rows
            .into_iter()
            .map(|row| {
                let mut new_row = HashMap::new();
                for (k, v) in row {
                    let new_key = rename_map.get(&k).cloned().unwrap_or(k);
                    new_row.insert(new_key, v);
                }
                new_row
            })
            .collect();

        assert!(result[0].contains_key("name"));
        assert!(result[0].contains_key("age"));
        assert!(!result[0].contains_key("old_name"));
    }

    #[test]
    fn test_eval_expression_simple() {
        let row = map![
            "price" => json!(100),
            "qty" => json!(5),
        ];

        let result = eval_expression(&row, "price * qty");
        assert_eq!(result, json!(500.0));
    }

    #[test]
    fn test_eval_expression_complex() {
        let row = map![
            "a" => json!(10),
            "b" => json!(20),
            "c" => json!(5),
        ];

        let result = eval_expression(&row, "a + b * c");
        assert_eq!(result, json!(110.0)); // 10 + (20*5) = 110
    }

    #[test]
    fn test_eval_expression_with_parens() {
        let row = map![
            "a" => json!(10),
            "b" => json!(20),
            "c" => json!(5),
        ];

        let result = eval_expression(&row, "(a + b) * c");
        assert_eq!(result, json!(150.0)); // (10+20)*5 = 150
    }

    #[test]
    fn test_parse_value_numeric() {
        assert_eq!(parse_value("42"), json!(42));
        assert_eq!(parse_value("3.14"), json!(3.14));
        assert_eq!(parse_value("true"), json!(true));
        assert_eq!(parse_value("false"), json!(false));
        assert_eq!(parse_value("null"), json!(null));
        assert_eq!(parse_value("hello"), json!("hello"));
    }

    #[test]
    fn test_collect_columns() {
        let rows = vec![
            map!["a" => json!(1), "b" => json!(2)],
            map!["b" => json!(3), "c" => json!(4)],
        ];
        let cols = collect_columns(&rows);
        assert!(cols.contains(&"a".to_string()));
        assert!(cols.contains(&"b".to_string()));
        assert!(cols.contains(&"c".to_string()));
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let p50 = percentile(&data, 50.0);
        assert!((p50 - 3.0).abs() < 1e-10);

        let p25 = percentile(&data, 25.0);
        assert!((p25 - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_histogram() {
        let values = vec![1.0, 1.5, 2.0, 8.0, 9.0, 9.5];
        let hist = compute_histogram(&values, 4);
        if let Value::Array(bins) = hist {
            assert_eq!(bins.len(), 4);
            let total: usize = bins
                .iter()
                .filter_map(|b| b.get("count").and_then(|c| c.as_u64()))
                .sum::<u64>() as usize;
            assert_eq!(total, values.len());
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_detect_format_csv() {
        let csv = "name,age\nAlice,30\nBob,25";
        let fmt = detect_format("test", csv);
        assert_eq!(fmt, "csv");
    }

    #[test]
    fn test_detect_format_json() {
        let json = r#"[{"name":"Alice"}]"#;
        let fmt = detect_format("test", json);
        assert_eq!(fmt, "json");
    }

    #[test]
    fn test_detect_format_from_path() {
        assert_eq!(detect_format_from_path("data.csv", ""), "csv");
        assert_eq!(detect_format_from_path("data.json", ""), "json");
        assert_eq!(detect_format_from_path("data.txt", "a,b\n1,2"), "csv");
    }

    #[test]
    fn test_merge_simple() {
        let rows1 = vec![
            map!["id" => json!(1), "name" => json!("Alice")],
            map!["id" => json!(2), "name" => json!("Bob")],
        ];
        let rows2 = vec![
            map!["id" => json!(1), "dept" => json!("Engineering")],
            map!["id" => json!(3), "dept" => json!("Sales")],
        ];

        // Inner join
        let mut lookup: HashMap<String, Vec<&HashMap<String, Value>>> = HashMap::new();
        for row in &rows2 {
            let key = get_string(row, "id");
            lookup.entry(key).or_default().push(row);
        }

        let mut result = Vec::new();
        for row1 in &rows1 {
            let key = get_string(row1, "id");
            if let Some(matches) = lookup.get(&key) {
                for row2 in matches {
                    let mut merged = row1.clone();
                    for (k, v) in row2.iter() {
                        if k != "id" {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                    result.push(merged);
                }
            }
        }

        assert_eq!(result.len(), 1);
        assert_eq!(get_string(&result[0], "name"), "Alice");
        assert_eq!(get_string(&result[0], "dept"), "Engineering");
    }

    #[test]
    fn test_info_basic() {
        let rows = vec![
            map!["name" => json!("Alice"), "age" => json!(30)],
            map!["name" => json!("Bob"), "age" => json!(25)],
        ];
        let cols = collect_columns(&rows);
        assert_eq!(cols.len(), 2);
        assert!(cols.contains(&"name".to_string()));
        assert!(cols.contains(&"age".to_string()));
    }
}
