//! Log Analyzer tool: parse, filter, analyze, and visualize log files.
//!
//! # Actions
//!
//! - **parse**: Parse log lines into structured data (timestamp, level, message)
//! - **filter**: Filter logs by level, time range, or keyword
//! - **stats**: Generate statistics (level counts, time distribution, top errors)
//! - **patterns**: Detect anomaly patterns (error spikes, repeated errors)
//! - **tail**: Show last N lines with optional level filter
//! - **search**: Full-text search with context lines
//! - **timeline**: Generate a timeline visualization of log events

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

// ============================================================================
// Pre-compiled regex patterns
// ============================================================================

// Match common log formats:
// 2024-01-15T10:30:00Z [INFO] message
// 2024-01-15 10:30:00.123 INFO  [module] message
// Jan 15 10:30:00 hostname service[pid]: message
// [2024-01-15 10:30:00] [ERROR] message
static RE_LOG_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        ^\s*
        (?:
            # ISO format: 2024-01-15T10:30:00Z or 2024-01-15 10:30:00.123
            (?P<iso>(?:\d{4}-\d{2}-\d{2})[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?)
            \s*
            (?:\[(?P<level1>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|CRITICAL)\])?
            \s*
            (?:\[(?P<module1>[^\]]*)\])?
            \s*
            (?P<msg1>.*)
        |
            # Bracket format: [2024-01-15 10:30:00] [ERROR]
            \[(?P<bracket_ts>(?:\d{4}-\d{2}-\d{2})\s+\d{2}:\d{2}:\d{2}(?:\.\d+)?)\]
            \s*
            \[(?P<level2>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|CRITICAL)\]
            \s*
            (?P<msg2>.*)
        |
            # Syslog format: Jan 15 10:30:00 hostname service[pid]:
            (?P<syslog_ts>(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})
            \s+
            (?P<hostname>\S+)
            \s+
            (?P<service>\S+?)(?:\[(?P<pid>\d+)\])?:
            \s*
            (?P<msg3>.*)
        |
            # Simple format: INFO: message or [INFO] message
            (?:\[)?(?P<level3>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|CRITICAL)(?:\])?:?\s*
            (?P<msg4>.*)
        )
        "#,
    )
    .expect("valid regex")
});

static RE_HOUR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\d{2}:\d{2}:\d{2}"#).expect("valid regex"));

// Match timestamps for extraction
static RE_TIMESTAMP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}"#).expect("valid regex"));

// Match log levels
static RE_LEVEL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|CRITICAL)\b"#)
        .expect("valid regex")
});

// ============================================================================
// LogAnalyzerTool
// ============================================================================

pub struct LogAnalyzerTool;

#[derive(Debug, Clone)]
struct LogEntry {
    line_number: usize,
    raw: String,
    timestamp: Option<String>,
    level: Option<String>,
    module: Option<String>,
    message: String,
}

#[async_trait::async_trait]
impl Tool for LogAnalyzerTool {
    fn name(&self) -> &str {
        "log_analyzer"
    }

    fn description(&self) -> &str {
        "Analyze log files: parse structured entries, filter by level/keyword/time, generate statistics, detect anomaly patterns (error spikes, repeated errors), tail, search with context, and timeline visualization."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: parse (parse log lines), filter (filter by level/keyword), stats (statistics), patterns (detect anomalies), tail (last N lines), search (full-text search), timeline (timeline visualization)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the log file".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "level".to_string(),
                description: "Filter by log level: TRACE, DEBUG, INFO, WARN, ERROR, FATAL (comma-separated for multiple)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "keyword".to_string(),
                description: "Filter/search keyword (case-insensitive)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "limit".to_string(),
                description: "Maximum results to return (default: 100)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "context_lines".to_string(),
                description: "Number of context lines for search (default: 2)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "regex".to_string(),
                description: "Regex pattern for advanced filtering/search".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read log file '{path}': {e}"))?;

        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
        let context_lines = params
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        let entries = parse_log_content(&content);

        match action {
            "parse" => {
                let parsed: Vec<Value> = entries.iter().take(limit).map(entry_to_json).collect();
                Ok(json!({
                    "status": "ok",
                    "action": "parse",
                    "total_lines": content.lines().count(),
                    "parsed_entries": entries.len(),
                    "entries": parsed,
                }))
            }
            "filter" => {
                let level_filter = params.get("level").and_then(|v| v.as_str());
                let keyword = params.get("keyword").and_then(|v| v.as_str());
                let regex_pat = params.get("regex").and_then(|v| v.as_str());

                let filtered = filter_entries(&entries, level_filter, keyword, regex_pat)?;
                let results: Vec<Value> = filtered
                    .iter()
                    .take(limit)
                    .map(|e| entry_to_json(e))
                    .collect();

                Ok(json!({
                    "status": "ok",
                    "action": "filter",
                    "total_entries": entries.len(),
                    "matched": results.len(),
                    "entries": results,
                }))
            }
            "stats" => {
                let stats = compute_stats(&entries);
                let level_distribution = compute_level_distribution(&entries);
                let top_errors = find_top_errors(&entries, 10);
                let hourly_distribution = compute_hourly_distribution(&entries);

                Ok(json!({
                    "status": "ok",
                    "action": "stats",
                    "total_lines": content.lines().count(),
                    "total_entries": entries.len(),
                    "stats": stats,
                    "level_distribution": level_distribution,
                    "top_errors": top_errors,
                    "hourly_distribution": hourly_distribution,
                }))
            }
            "patterns" => {
                let patterns = detect_patterns(&entries);
                Ok(json!({
                    "status": "ok",
                    "action": "patterns",
                    "total_entries": entries.len(),
                    "patterns_detected": patterns.len(),
                    "patterns": patterns,
                }))
            }
            "tail" => {
                let level_filter = params.get("level").and_then(|v| v.as_str());
                let filtered = filter_entries(&entries, level_filter, None, None)?;
                let tail: Vec<Value> = filtered
                    .iter()
                    .rev()
                    .take(limit)
                    .rev()
                    .map(|e| entry_to_json(e))
                    .collect();
                Ok(json!({
                    "status": "ok",
                    "action": "tail",
                    "showing": tail.len(),
                    "entries": tail,
                }))
            }
            "search" => {
                let keyword = params
                    .get("keyword")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: keyword (for search action)")?;

                let regex_pat = params.get("regex").and_then(|v| v.as_str());
                let results = search_entries(&entries, keyword, regex_pat, context_lines);

                Ok(json!({
                    "status": "ok",
                    "action": "search",
                    "keyword": keyword,
                    "matches": results.len(),
                    "results": results,
                }))
            }
            "timeline" => {
                let timeline = generate_timeline(&entries);
                Ok(json!({
                    "status": "ok",
                    "action": "timeline",
                    "total_entries": entries.len(),
                    "timeline": timeline,
                }))
            }
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: parse, filter, stats, patterns, tail, search, timeline"),
            })),
        }
    }
}

// ============================================================================
// Log Parsing
// ============================================================================

fn parse_log_content(content: &str) -> Vec<LogEntry> {
    let mut entries = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let entry = parse_log_line(line, idx + 1);
        entries.push(entry);
    }

    entries
}

fn parse_log_line(line: &str, line_number: usize) -> LogEntry {
    if let Some(cap) = RE_LOG_LINE.captures(line) {
        // Try ISO format
        let timestamp = cap
            .name("iso")
            .map(|m| m.as_str().to_string())
            .or_else(|| cap.name("bracket_ts").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("syslog_ts").map(|m| m.as_str().to_string()));

        let level = cap
            .name("level1")
            .map(|m| normalize_level(m.as_str()))
            .or_else(|| cap.name("level2").map(|m| normalize_level(m.as_str())))
            .or_else(|| cap.name("level3").map(|m| normalize_level(m.as_str())));

        let module = cap
            .name("module1")
            .map(|m| m.as_str().to_string())
            .or_else(|| cap.name("service").map(|m| m.as_str().to_string()));

        let message = cap
            .name("msg1")
            .map(|m| m.as_str().to_string())
            .or_else(|| cap.name("msg2").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("msg3").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("msg4").map(|m| m.as_str().to_string()))
            .unwrap_or_default();

        // If level wasn't captured by the main regex, try to find it in the message
        let level = level.or_else(|| {
            RE_LEVEL
                .captures(line)
                .and_then(|c| c.get(1))
                .map(|m| normalize_level(m.as_str()))
        });

        LogEntry {
            line_number,
            raw: line.to_string(),
            timestamp,
            level,
            module,
            message,
        }
    } else {
        // Fallback: try to extract level and message
        let level = RE_LEVEL
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| normalize_level(m.as_str()));

        let message = line.to_string();

        LogEntry {
            line_number,
            raw: line.to_string(),
            timestamp: RE_TIMESTAMP.find(line).map(|m| m.as_str().to_string()),
            level,
            module: None,
            message,
        }
    }
}

fn normalize_level(level: &str) -> String {
    match level.to_uppercase().as_str() {
        "TRACE" => "TRACE".to_string(),
        "DEBUG" => "DEBUG".to_string(),
        "INFO" => "INFO".to_string(),
        "WARN" | "WARNING" => "WARN".to_string(),
        "ERROR" => "ERROR".to_string(),
        "FATAL" | "CRITICAL" => "FATAL".to_string(),
        _ => level.to_uppercase(),
    }
}

fn entry_to_json(entry: &LogEntry) -> Value {
    json!({
        "line": entry.line_number,
        "timestamp": entry.timestamp,
        "level": entry.level,
        "module": entry.module,
        "message": entry.message,
        "raw": entry.raw.chars().take(200).collect::<String>(),
    })
}

// ============================================================================
// Filtering
// ============================================================================

fn filter_entries<'a>(
    entries: &'a [LogEntry],
    level_filter: Option<&str>,
    keyword: Option<&str>,
    regex_pat: Option<&str>,
) -> Result<Vec<&'a LogEntry>, String> {
    let levels: Vec<String> = level_filter
        .map(|l| l.split(',').map(|s| normalize_level(s.trim())).collect())
        .unwrap_or_default();

    let regex_filter = if let Some(pat) = regex_pat {
        Some(regex::Regex::new(pat).map_err(|e| format!("Invalid regex: {e}"))?)
    } else {
        None
    };

    let mut filtered = Vec::new();

    for entry in entries {
        // Level filter
        if !levels.is_empty() {
            if let Some(ref level) = entry.level {
                if !levels.contains(level) {
                    continue;
                }
            } else {
                continue;
            }
        }

        // Keyword filter
        if let Some(kw) = keyword {
            if !entry.message.to_lowercase().contains(&kw.to_lowercase())
                && !entry.raw.to_lowercase().contains(&kw.to_lowercase())
            {
                continue;
            }
        }

        // Regex filter
        if let Some(ref re) = regex_filter {
            if !re.is_match(&entry.raw) {
                continue;
            }
        }

        filtered.push(entry);
    }

    Ok(filtered)
}

// ============================================================================
// Statistics
// ============================================================================

fn compute_stats(entries: &[LogEntry]) -> Value {
    let total = entries.len();
    let with_timestamp = entries.iter().filter(|e| e.timestamp.is_some()).count();
    let with_level = entries.iter().filter(|e| e.level.is_some()).count();
    let with_module = entries.iter().filter(|e| e.module.is_some()).count();

    let error_count = entries
        .iter()
        .filter(|e| matches!(e.level.as_deref(), Some("ERROR")))
        .count();
    let warn_count = entries
        .iter()
        .filter(|e| matches!(e.level.as_deref(), Some("WARN")))
        .count();
    let fatal_count = entries
        .iter()
        .filter(|e| matches!(e.level.as_deref(), Some("FATAL")))
        .count();

    let error_rate = if total > 0 {
        format!(
            "{:.1}%",
            (error_count + fatal_count) as f64 / total as f64 * 100.0
        )
    } else {
        "0.0%".to_string()
    };

    json!({
        "total_entries": total,
        "with_timestamp": with_timestamp,
        "with_level": with_level,
        "with_module": with_module,
        "error_count": error_count,
        "warning_count": warn_count,
        "fatal_count": fatal_count,
        "error_rate": error_rate,
    })
}

fn compute_level_distribution(entries: &[LogEntry]) -> Value {
    let mut dist: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        if let Some(ref level) = entry.level {
            *dist.entry(level.clone()).or_insert(0) += 1;
        }
    }

    // Sort by count
    let mut sorted: Vec<(String, usize)> = dist.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    json!(sorted)
}

fn find_top_errors(entries: &[LogEntry], limit: usize) -> Vec<Value> {
    let mut error_counts: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if matches!(entry.level.as_deref(), Some("ERROR") | Some("FATAL")) {
            // Truncate message for grouping
            let key: String = entry.message.chars().take(80).collect();
            *error_counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = error_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    sorted
        .into_iter()
        .take(limit)
        .map(|(msg, count)| {
            json!({
                "message": msg,
                "count": count,
            })
        })
        .collect()
}

fn compute_hourly_distribution(entries: &[LogEntry]) -> Value {
    let mut hourly: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if let Some(ref ts) = entry.timestamp {
            // Extract hour from timestamp
            if let Some(hour_match) = RE_HOUR.find(ts) {
                let time_str = hour_match.as_str();
                if let Some(hour) = time_str.split(':').next() {
                    *hourly.entry(format!("{}:00", hour)).or_insert(0) += 1;
                }
            }
        }
    }

    let mut sorted: Vec<(String, usize)> = hourly.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    json!(sorted)
}

// ============================================================================
// Pattern Detection
// ============================================================================

fn detect_patterns(entries: &[LogEntry]) -> Vec<Value> {
    let mut patterns = Vec::new();

    // Detect error spikes (multiple errors in consecutive lines)
    let mut error_streak_start: Option<usize> = None;
    let mut error_streak_count = 0;
    let mut error_spikes = Vec::new();

    for entry in entries {
        if matches!(entry.level.as_deref(), Some("ERROR") | Some("FATAL")) {
            if error_streak_start.is_none() {
                error_streak_start = Some(entry.line_number);
                error_streak_count = 1;
            } else {
                error_streak_count += 1;
            }
        } else {
            if error_streak_count >= 3 {
                error_spikes.push(json!({
                    "type": "error_spike",
                    "start_line": error_streak_start.unwrap(),
                    "count": error_streak_count,
                    "severity": if error_streak_count >= 10 { "critical" } else if error_streak_count >= 5 { "high" } else { "medium" },
                }));
            }
            error_streak_start = None;
            error_streak_count = 0;
        }
    }
    // Check final streak
    if error_streak_count >= 3 {
        error_spikes.push(json!({
            "type": "error_spike",
            "start_line": error_streak_start.unwrap(),
            "count": error_streak_count,
            "severity": if error_streak_count >= 10 { "critical" } else if error_streak_count >= 5 { "high" } else { "medium" },
        }));
    }

    patterns.extend(error_spikes);

    // Detect repeated errors (same message appearing multiple times)
    let mut error_freq: HashMap<String, Vec<usize>> = HashMap::new();
    for entry in entries {
        if matches!(entry.level.as_deref(), Some("ERROR") | Some("FATAL")) {
            let key: String = entry.message.chars().take(60).collect();
            error_freq.entry(key).or_default().push(entry.line_number);
        }
    }

    for (msg, lines) in error_freq {
        if lines.len() >= 5 {
            patterns.push(json!({
                "type": "repeated_error",
                "message": msg,
                "count": lines.len(),
                "first_occurrence": lines.first().unwrap(),
                "last_occurrence": lines.last().unwrap(),
                "severity": if lines.len() >= 20 { "critical" } else if lines.len() >= 10 { "high" } else { "medium" },
            }));
        }
    }

    // Detect rapid errors (many errors within a short time span)
    let timestamped_errors: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.level.as_deref(), Some("ERROR") | Some("FATAL")))
        .filter(|e| e.timestamp.is_some())
        .collect();

    if timestamped_errors.len() >= 5 {
        // Simple heuristic: check if there are many errors in the dataset
        let total = entries.len();
        let error_count = timestamped_errors.len();
        if total > 0 && (error_count as f64 / total as f64) > 0.3 {
            patterns.push(json!({
                "type": "high_error_rate",
                "error_count": error_count,
                "total_entries": total,
                "error_rate": format!("{:.1}%", error_count as f64 / total as f64 * 100.0),
                "severity": "high",
            }));
        }
    }

    // Sort patterns by severity
    patterns.sort_by(|a, b| {
        let sev_a = a["severity"].as_str().unwrap_or("");
        let sev_b = b["severity"].as_str().unwrap_or("");
        let order = |s: &str| match s {
            "critical" => 0,
            "high" => 1,
            "medium" => 2,
            _ => 3,
        };
        order(sev_a).cmp(&order(sev_b))
    });

    patterns
}

// ============================================================================
// Search
// ============================================================================

fn search_entries(
    entries: &[LogEntry],
    keyword: &str,
    regex_pat: Option<&str>,
    context_lines: usize,
) -> Vec<Value> {
    let regex_filter = regex_pat.and_then(|p| regex::Regex::new(p).ok());

    let mut results = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let matches_keyword = entry
            .message
            .to_lowercase()
            .contains(&keyword.to_lowercase())
            || entry.raw.to_lowercase().contains(&keyword.to_lowercase());

        let matches_regex = regex_filter
            .as_ref()
            .map(|re| re.is_match(&entry.raw))
            .unwrap_or(true);

        if matches_keyword && matches_regex {
            // Get context lines
            let start = idx.saturating_sub(context_lines);
            let end = (idx + context_lines + 1).min(entries.len());

            let context: Vec<Value> = entries[start..end]
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let is_match = (start + i) == idx;
                    json!({
                        "line": e.line_number,
                        "is_match": is_match,
                        "content": e.raw.chars().take(200).collect::<String>(),
                    })
                })
                .collect();

            results.push(json!({
                "match_line": entry.line_number,
                "match_content": entry.raw.chars().take(200).collect::<String>(),
                "context": context,
            }));
        }
    }

    results
}

// ============================================================================
// Timeline
// ============================================================================

fn generate_timeline(entries: &[LogEntry]) -> Vec<Value> {
    // Group entries by time bucket (minute-level granularity)
    let mut buckets: HashMap<String, HashMap<String, usize>> = HashMap::new();

    for entry in entries {
        if let Some(ref ts) = entry.timestamp {
            // Extract YYYY-MM-DD HH:MM
            if let Some(time_bucket) = extract_time_bucket(ts) {
                let level = entry.level.clone().unwrap_or_else(|| "UNKNOWN".to_string());
                *buckets
                    .entry(time_bucket)
                    .or_default()
                    .entry(level)
                    .or_insert(0) += 1;
            }
        }
    }

    let mut sorted: Vec<(String, HashMap<String, usize>)> = buckets.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    sorted
        .into_iter()
        .map(|(bucket, counts)| {
            json!({
                "time": bucket,
                "counts": counts,
                "total": counts.values().sum::<usize>(),
            })
        })
        .collect()
}

fn extract_time_bucket(timestamp: &str) -> Option<String> {
    // Try to extract YYYY-MM-DD HH:MM from various formats
    if let Some(m) = RE_TIMESTAMP.find(timestamp) {
        let ts = m.as_str();
        // Replace T with space if present
        let ts = ts.replace('T', " ");
        // Truncate to minute
        if ts.len() >= 16 {
            Some(ts[..16].to_string())
        } else {
            Some(ts)
        }
    } else {
        None
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(LogAnalyzerTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_tool() -> LogAnalyzerTool {
        LogAnalyzerTool
    }

    fn create_test_log(content: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let file_name = format!("test_log_{}_{}.log", std::process::id(), id);
        let path = dir.join(file_name);
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(content.as_bytes()).expect("write log");
        file.flush().expect("flush");
        path
    }

    #[tokio::test]
    async fn test_parse_action() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Server started\n\
             2024-01-15T10:30:01Z [ERROR] Connection failed\n\
             2024-01-15T10:30:02Z [DEBUG] Processing request\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("parse".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "parse");
        assert_eq!(result["parsed_entries"], 3);
        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["level"], "INFO");
        assert_eq!(entries[1]["level"], "ERROR");
        assert_eq!(entries[2]["level"], "DEBUG");
    }

    #[tokio::test]
    async fn test_filter_by_level() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Server started\n\
             2024-01-15T10:30:01Z [ERROR] Connection failed\n\
             2024-01-15T10:30:02Z [ERROR] Timeout occurred\n\
             2024-01-15T10:30:03Z [DEBUG] Processing request\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("filter".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );
        params.insert("level".to_string(), Value::String("ERROR".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        // Filter should find entries with ERROR level (at least 1)
        let matched = result["matched"].as_u64().unwrap_or(0);
        assert!(matched >= 1);
    }

    #[tokio::test]
    async fn test_filter_by_keyword() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Server started\n\
             2024-01-15T10:30:01Z [ERROR] Connection to database failed\n\
             2024-01-15T10:30:02Z [INFO] Request processed\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("filter".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );
        params.insert("keyword".to_string(), Value::String("database".to_string()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["matched"], 1);
    }

    #[tokio::test]
    async fn test_stats_action() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Server started\n\
             2024-01-15T10:30:01Z [ERROR] Connection failed\n\
             2024-01-15T10:30:02Z [WARN] Slow query\n\
             2024-01-15T10:30:03Z [FATAL] Out of memory\n\
             2024-01-15T10:30:04Z [INFO] Recovery complete\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("stats".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let stats = &result["stats"];
        assert_eq!(stats["total_entries"], 5);
        assert_eq!(stats["error_count"], 1);
        assert_eq!(stats["warning_count"], 1);
        assert_eq!(stats["fatal_count"], 1);
    }

    #[tokio::test]
    async fn test_tail_action() {
        let tool = make_tool();
        let log_content = (0..20)
            .map(|i| format!("2024-01-15T10:30:{:02}Z [INFO] Line {}", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        let log = create_test_log(&log_content);

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("tail".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );
        params.insert("limit".to_string(), Value::Number(5.into()));

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["showing"], 5);
        let entries = result["entries"].as_array().unwrap();
        // Should be the last 5 lines
        assert_eq!(entries[0]["line"], 16);
        assert_eq!(entries[4]["line"], 20);
    }

    #[tokio::test]
    async fn test_search_action() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Server started on port 8080\n\
             2024-01-15T10:30:01Z [ERROR] Connection to port 5432 failed\n\
             2024-01-15T10:30:02Z [INFO] Listening on port 8080\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("search".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );
        params.insert(
            "keyword".to_string(),
            Value::String("port 8080".to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let matches = result["matches"].as_u64().unwrap_or(0);
        assert!(matches >= 1);
        let results = result["results"].as_array().unwrap();
        assert!(results[0]["context"].as_array().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_timeline_action() {
        let tool = make_tool();
        let log = create_test_log(
            "2024-01-15T10:30:00Z [INFO] Event 1\n\
             2024-01-15T10:30:30Z [ERROR] Event 2\n\
             2024-01-15T10:31:00Z [INFO] Event 3\n",
        );

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("timeline".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let timeline = result["timeline"].as_array().unwrap();
        assert!(!timeline.is_empty());
    }

    #[tokio::test]
    async fn test_patterns_error_spike() {
        let tool = make_tool();
        // Use format that spans multiple lines to ensure all are on separate lines
        let log_content: String = (0..10)
            .map(|i| format!("2024-01-15T10:30:{:02}Z [ERROR] Error number {}\n", i, i))
            .collect();
        let log = create_test_log(&log_content);

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("patterns".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "ok");
        let patterns = result["patterns"].as_array().unwrap();
        // At least one pattern should be detected (error spike or high error rate)
        assert!(!patterns.is_empty() || result["patterns_detected"].as_u64().unwrap_or(0) >= 1);
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = make_tool();
        let log = create_test_log("some log line\n");

        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("invalid".to_string()));
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["status"], "error");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = make_tool();
        let log = create_test_log("some log line\n");

        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            Value::String(log.to_str().unwrap().to_string()),
        );

        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonexistent_file() {
        let tool = make_tool();
        let mut params = HashMap::new();
        params.insert("action".to_string(), Value::String("parse".to_string()));
        params.insert(
            "path".to_string(),
            Value::String("/nonexistent/logfile.log".to_string()),
        );

        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read log file"));
    }

    #[test]
    fn test_normalize_level() {
        assert_eq!(normalize_level("INFO"), "INFO");
        assert_eq!(normalize_level("WARN"), "WARN");
        assert_eq!(normalize_level("WARNING"), "WARN");
        assert_eq!(normalize_level("FATAL"), "FATAL");
        assert_eq!(normalize_level("CRITICAL"), "FATAL");
        assert_eq!(normalize_level("ERROR"), "ERROR");
        assert_eq!(normalize_level("DEBUG"), "DEBUG");
        assert_eq!(normalize_level("TRACE"), "TRACE");
    }

    #[test]
    fn test_parse_log_line_iso_format() {
        let entry = parse_log_line("2024-01-15T10:30:00Z [INFO] Server started", 1);
        assert_eq!(entry.level, Some("INFO".to_string()));
        assert_eq!(entry.message, "Server started");
        assert!(entry.timestamp.is_some());
    }

    #[test]
    fn test_parse_log_line_bracket_format() {
        let entry = parse_log_line("[2024-01-15 10:30:00] [ERROR] Connection failed", 1);
        assert_eq!(entry.level, Some("ERROR".to_string()));
        assert_eq!(entry.message, "Connection failed");
    }

    #[test]
    fn test_parse_log_line_simple_format() {
        let entry = parse_log_line("ERROR: something went wrong", 1);
        assert_eq!(entry.level, Some("ERROR".to_string()));
    }

    #[test]
    fn test_compute_stats() {
        let entries = vec![
            LogEntry {
                line_number: 1,
                raw: "2024-01-15T10:30:00Z [INFO] test".to_string(),
                timestamp: Some("2024-01-15T10:30:00Z".to_string()),
                level: Some("INFO".to_string()),
                module: None,
                message: "test".to_string(),
            },
            LogEntry {
                line_number: 2,
                raw: "2024-01-15T10:30:01Z [ERROR] failure".to_string(),
                timestamp: Some("2024-01-15T10:30:01Z".to_string()),
                level: Some("ERROR".to_string()),
                module: None,
                message: "failure".to_string(),
            },
        ];

        let stats = compute_stats(&entries);
        assert_eq!(stats["total_entries"], 2);
        assert_eq!(stats["error_count"], 1);
    }

    #[test]
    fn test_filter_entries_invalid_regex() {
        let entries: Vec<LogEntry> = vec![];
        let result = filter_entries(&entries, None, None, Some("[invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_entries_by_level() {
        let entries = vec![
            LogEntry {
                line_number: 1,
                raw: "info line".to_string(),
                timestamp: None,
                level: Some("INFO".to_string()),
                module: None,
                message: "info line".to_string(),
            },
            LogEntry {
                line_number: 2,
                raw: "error line".to_string(),
                timestamp: None,
                level: Some("ERROR".to_string()),
                module: None,
                message: "error line".to_string(),
            },
        ];

        let result = filter_entries(&entries, Some("ERROR"), None, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].level, Some("ERROR".to_string()));
    }

    #[test]
    fn test_extract_time_bucket() {
        let bucket = extract_time_bucket("2024-01-15T10:30:45Z");
        assert_eq!(bucket, Some("2024-01-15 10:30".to_string()));

        let bucket = extract_time_bucket("2024-01-15 10:30:45");
        assert_eq!(bucket, Some("2024-01-15 10:30".to_string()));

        let bucket = extract_time_bucket("no timestamp here");
        assert!(bucket.is_none());
    }
}
