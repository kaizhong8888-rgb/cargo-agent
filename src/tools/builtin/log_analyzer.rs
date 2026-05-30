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

// Match timestamps for extraction
static RE_TIMESTAMP: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}"#).expect("valid regex")
});

// Match log levels
static RE_LEVEL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|CRITICAL)\b"#).expect("valid regex")
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
        let context_lines = params.get("context_lines").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

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
                let results: Vec<Value> = filtered.iter().take(limit).map(|e| entry_to_json(*e)).collect();

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
                let tail: Vec<Value> = filtered.iter().rev().take(limit).rev().map(|e| entry_to_json(*e)).collect();
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
        let timestamp = cap.name("iso").map(|m| m.as_str().to_string())
            .or_else(|| cap.name("bracket_ts").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("syslog_ts").map(|m| m.as_str().to_string()));

        let level = cap.name("level1").map(|m| normalize_level(m.as_str()))
            .or_else(|| cap.name("level2").map(|m| normalize_level(m.as_str())))
            .or_else(|| cap.name("level3").map(|m| normalize_level(m.as_str())));

        let module = cap.name("module1").map(|m| m.as_str().to_string())
            .or_else(|| cap.name("service").map(|m| m.as_str().to_string()));

        let message = cap.name("msg1").map(|m| m.as_str().to_string())
            .or_else(|| cap.name("msg2").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("msg3").map(|m| m.as_str().to_string()))
            .or_else(|| cap.name("msg4").map(|m| m.as_str().to_string()))
            .unwrap_or_default();

        // If level wasn't captured by the main regex, try to find it in the message
        let level = level.or_else(|| {
            RE_LEVEL.captures(line)
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
        let level = RE_LEVEL.captures(line)
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

    let error_count = entries.iter().filter(|e| matches!(e.level.as_deref(), Some("ERROR"))).count();
    let warn_count = entries.iter().filter(|e| matches!(e.level.as_deref(), Some("WARN"))).count();
    let fatal_count = entries.iter().filter(|e| matches!(e.level.as_deref(), Some("FATAL"))).count();

    let error_rate = if total > 0 {
        format!("{:.1}%", (error_count + fatal_count) as f64 / total as f64 * 100.0)
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

    sorted.into_iter().take(limit).map(|(msg, count)| {
        json!({
            "message": msg,
            "count": count,
        })
    }).collect()
}

fn compute_hourly_distribution(entries: &[LogEntry]) -> Value {
    let mut hourly: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if let Some(ref ts) = entry.timestamp {
            // Extract hour from timestamp
            if let Some(hour_match) = regex::Regex::new(r#"\d{2}:\d{2}:\d{2}"#).ok().and_then(|re| re.find(ts)) {
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
    let timestamped_errors: Vec<_> = entries.iter()
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
        let matches_keyword = entry.message.to_lowercase().contains(&keyword.to_lowercase())
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
                *buckets.entry(time_bucket).or_default().entry(level).or_insert(0) += 1;
            }
        }
    }

    let mut sorted: Vec<(String, HashMap<String, usize>)> = buckets.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    sorted.into_iter().map(|(bucket, counts)| {
        json!({
            "time": bucket,
            "counts": counts,
            "total": counts.values().sum::<usize>(),
        })
    }).collect()
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
