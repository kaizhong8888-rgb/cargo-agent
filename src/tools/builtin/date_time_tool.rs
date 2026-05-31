//! Date/Time Tool: timezone conversions, date arithmetic, calendar operations, duration formatting.
//!
//! # Actions
//!
//! - **now**: Current date/time in various formats and timezones
//! - **convert**: Convert between timezones
//! - **format**: Format a date string with custom format
//! - **parse**: Parse a date string into structured data
//! - **add**: Add/subtract time units from a date
//! - **diff**: Calculate difference between two dates
//! - **duration**: Format a duration (seconds → human-readable)
//! - **calendar**: Get calendar info (day of week, week number, is leap year, etc.)
//! - **timestamp**: Convert between Unix timestamp and date string
//! - **relative**: Generate relative time descriptions ("2 hours ago", "in 3 days")

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct DateTimeTool;

#[async_trait::async_trait]
impl Tool for DateTimeTool {
    fn name(&self) -> &str {
        "datetime"
    }

    fn description(&self) -> &str {
        "Date/Time utilities: timezone conversions, date arithmetic, calendar operations, duration formatting, relative time. Actions: now, convert, format, parse, add, diff, duration, calendar, timestamp, relative."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: now, convert, format, parse, add, diff, duration, calendar, timestamp, relative".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "date".to_string(),
                description: "Input date/time string (ISO 8601 format recommended)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "format".to_string(),
                description: "Output format string (strftime format, e.g. '%Y-%m-%d %H:%M:%S')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "timezone".to_string(),
                description: "Target timezone (e.g. 'Asia/Shanghai', 'US/Pacific', 'UTC')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "from_tz".to_string(),
                description: "Source timezone (for convert action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "to_tz".to_string(),
                description: "Target timezone (for convert action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "date2".to_string(),
                description: "Second date (for diff action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "unit".to_string(),
                description: "Time unit for add action: years, months, weeks, days, hours, minutes, seconds".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "amount".to_string(),
                description: "Amount to add (negative for subtraction)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "seconds".to_string(),
                description: "Number of seconds (for duration action)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "timestamp".to_string(),
                description: "Unix timestamp in seconds (for timestamp action)".to_string(),
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

        match action {
            "now" => action_now(params),
            "convert" => action_convert(params),
            "format" => action_format(params),
            "parse" => action_parse(params),
            "add" => action_add(params),
            "diff" => action_diff(params),
            "duration" => action_duration(params),
            "calendar" => action_calendar(params),
            "timestamp" => action_timestamp(params),
            "relative" => action_relative(params),
            _ => Ok(json!({
                "status": "error",
                "message": format!("Unknown action: {action}. Available: now, convert, format, parse, add, diff, duration, calendar, timestamp, relative"),
            })),
        }
    }
}

// ============================================================================
// Actions
// ============================================================================

fn action_now(params: &HashMap<String, Value>) -> Result<Value, String> {
    let utc_now = Utc::now();

    let tz = params.get("timezone").and_then(|v| v.as_str()).unwrap_or("UTC");
    let formatted_tz = format_tz(&utc_now, tz)?;

    let mut zones = serde_json::Map::new();
    for zone_name in ["UTC", "Asia/Shanghai", "America/New_York", "Europe/London", "Asia/Tokyo"] {
        if let Ok(formatted) = format_tz(&utc_now, zone_name) {
            zones.insert(zone_name.to_string(), json!({
                "time": formatted,
                "offset": get_offset_str(zone_name),
            }));
        }
    }

    Ok(json!({
        "status": "ok",
        "action": "now",
        "utc": utc_now.to_rfc3339(),
        "unix_timestamp": utc_now.timestamp(),
        "requested_timezone": formatted_tz,
        "common_zones": zones,
        "iso_8601": utc_now.to_rfc3339(),
        "date": utc_now.format("%Y-%m-%d").to_string(),
        "time": utc_now.format("%H:%M:%S").to_string(),
        "day_of_week": utc_now.format("%A").to_string(),
        "week_number": utc_now.iso_week().week(),
    }))
}

fn action_convert(params: &HashMap<String, Value>) -> Result<Value, String> {
    let date_str = params
        .get("date")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date")?;

    let from_tz = params.get("from_tz").and_then(|v| v.as_str()).unwrap_or("UTC");
    let to_tz_name = params.get("to_tz").and_then(|v| v.as_str()).unwrap_or("UTC");

    let dt = parse_any_datetime(date_str)?;
    let from_tz_parsed = parse_tz(from_tz)?;
    let to_tz_parsed = parse_tz(to_tz_name)?;

    let source_dt = from_tz_parsed.from_utc_datetime(&dt.naive_utc());
    let target_dt = to_tz_parsed.from_utc_datetime(&dt.naive_utc());

    let fmt = params.get("format").and_then(|v| v.as_str()).unwrap_or("%Y-%m-%d %H:%M:%S %Z");
    let from_str = source_dt.format(fmt).to_string();
    let to_str = target_dt.format(fmt).to_string();

    Ok(json!({
        "status": "ok",
        "action": "convert",
        "input": date_str,
        "from": { "timezone": from_tz, "time": from_str },
        "to": { "timezone": to_tz_name, "time": to_str },
        "utc": dt.to_rfc3339(),
    }))
}

fn action_format(params: &HashMap<String, Value>) -> Result<Value, String> {
    let date_str = params
        .get("date")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date")?;

    let fmt = params.get("format").and_then(|v| v.as_str())
        .ok_or("Missing required parameter: format")?;

    let dt = parse_any_datetime(date_str)?;

    let mut results = serde_json::Map::new();
    results.insert("custom".to_string(), json!(dt.format(fmt).to_string()));
    results.insert("iso_8601".to_string(), json!(dt.to_rfc3339()));
    results.insert("rfc_2822".to_string(), json!(dt.format("%a, %d %b %Y %H:%M:%S %z").to_string()));
    results.insert("unix_timestamp".to_string(), json!(dt.timestamp()));
    results.insert("date_only".to_string(), json!(dt.format("%Y-%m-%d").to_string()));
    results.insert("time_only".to_string(), json!(dt.format("%H:%M:%S").to_string()));
    results.insert("human_readable".to_string(), json!(dt.format("%B %d, %Y at %I:%M %p").to_string()));
    results.insert("compact".to_string(), json!(dt.format("%Y%m%d%H%M%S").to_string()));

    Ok(json!({
        "status": "ok",
        "action": "format",
        "input": date_str,
        "results": results,
    }))
}

fn action_parse(params: &HashMap<String, Value>) -> Result<Value, String> {
    let date_str = params
        .get("date")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date")?;

    let dt = parse_any_datetime(date_str)?;

    Ok(json!({
        "status": "ok",
        "action": "parse",
        "input": date_str,
        "parsed": {
            "datetime": dt.to_rfc3339(),
            "date": dt.format("%Y-%m-%d").to_string(),
            "time": dt.format("%H:%M:%S").to_string(),
            "year": dt.year(),
            "month": dt.month(),
            "day": dt.day(),
            "hour": dt.hour(),
            "minute": dt.minute(),
            "second": dt.second(),
            "day_of_week": dt.format("%A").to_string(),
            "day_of_year": dt.ordinal(),
            "week_number": dt.iso_week().week(),
            "is_leap_year": is_leap_year(dt.year()),
            "unix_timestamp": dt.timestamp(),
            "timezone": dt.offset().to_string(),
        },
    }))
}

fn action_add(params: &HashMap<String, Value>) -> Result<Value, String> {
    let default_date = Utc::now().to_rfc3339();
    let date_str = params
        .get("date")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_date);

    let unit = params.get("unit").and_then(|v| v.as_str())
        .ok_or("Missing required parameter: unit (years, months, weeks, days, hours, minutes, seconds)")?;

    let amount = params.get("amount").and_then(|v| v.as_i64())
        .ok_or("Missing required parameter: amount")?;

    let mut dt = parse_any_datetime(date_str)?;

    match unit {
        "years" => {
            let new_year = dt.year() + amount as i32;
            dt = dt.with_year(new_year).ok_or("Invalid date after adding years")?;
        }
        "months" => {
            let month_diff = (dt.year() * 12 + dt.month() as i32 - 1) + amount as i32;
            let new_year = month_diff / 12;
            let new_month = (month_diff % 12) + 1;
            dt = dt.with_year(new_year).ok_or("Invalid year")?;
            dt = dt.with_month(new_month as u32).ok_or("Invalid month")?;
        }
        "weeks" => dt += Duration::weeks(amount),
        "days" => dt += Duration::days(amount),
        "hours" => dt += Duration::hours(amount),
        "minutes" => dt += Duration::minutes(amount),
        "seconds" => dt += Duration::seconds(amount),
        _ => return Err(format!("Unknown unit: {unit}. Use: years, months, weeks, days, hours, minutes, seconds")),
    }

    Ok(json!({
        "status": "ok",
        "action": "add",
        "input": date_str,
        "operation": format!("{} {} {}", if amount >= 0 { "add" } else { "subtract" }, amount.abs(), unit),
        "result": {
            "datetime": dt.to_rfc3339(),
            "date": dt.format("%Y-%m-%d").to_string(),
            "time": dt.format("%H:%M:%S").to_string(),
            "day_of_week": dt.format("%A").to_string(),
            "unix_timestamp": dt.timestamp(),
        },
    }))
}

fn action_diff(params: &HashMap<String, Value>) -> Result<Value, String> {
    let date1_str = params.get("date").and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date")?;
    let date2_str = params.get("date2").and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date2")?;

    let dt1 = parse_any_datetime(date1_str)?;
    let dt2 = parse_any_datetime(date2_str)?;

    let duration = if dt2 > dt1 { dt2 - dt1 } else { dt1 - dt2 };

    let total_seconds = duration.num_seconds().abs();

    Ok(json!({
        "status": "ok",
        "action": "diff",
        "date1": date1_str,
        "date2": date2_str,
        "direction": if dt2 < dt1 { "date2 is before date1" } else { "date2 is after date1" },
        "difference": {
            "total_seconds": total_seconds,
            "total_minutes": duration.num_minutes().abs(),
            "total_hours": duration.num_hours().abs(),
            "total_days": duration.num_days().abs(),
            "total_weeks": duration.num_weeks().abs(),
            "human_readable": format_duration_hms(total_seconds),
            "negative": dt2 < dt1,
        },
    }))
}

fn action_duration(params: &HashMap<String, Value>) -> Result<Value, String> {
    let seconds = params.get("seconds").and_then(|v| v.as_f64())
        .ok_or("Missing required parameter: seconds")?;

    let abs_seconds = seconds.abs();
    let human = format_duration_hms(abs_seconds as i64);

    let days = (abs_seconds / 86400.0).floor() as i64;
    let hours = ((abs_seconds % 86400.0) / 3600.0).floor() as i64;
    let minutes = ((abs_seconds % 3600.0) / 60.0).floor() as i64;
    let secs = (abs_seconds % 60.0).round() as i64;

    Ok(json!({
        "status": "ok",
        "action": "duration",
        "input_seconds": seconds,
        "breakdown": {
            "days": days,
            "hours": hours,
            "minutes": minutes,
            "seconds": secs,
        },
        "human_readable": human,
        "formatted": format!("{:02}:{:02}:{:02}:{:02}", days, hours, minutes, secs),
    }))
}

fn action_calendar(params: &HashMap<String, Value>) -> Result<Value, String> {
    let default_date = Utc::now().format("%Y-%m-%d").to_string();
    let date_str = params.get("date").and_then(|v| v.as_str())
        .unwrap_or(&default_date);

    let dt = parse_any_datetime(date_str)?;
    let year = dt.year();
    let is_leap = is_leap_year(year);
    let days_in_month = days_in_month(dt.month(), is_leap);
    let days_in_year = if is_leap { 366 } else { 365 };
    let quarter = (dt.month() - 1) / 3 + 1;
    let is_weekend = dt.weekday().number_from_monday() >= 6;

    Ok(json!({
        "status": "ok",
        "action": "calendar",
        "input": date_str,
        "calendar": {
            "date": dt.format("%Y-%m-%d").to_string(),
            "year": year,
            "month": dt.month(),
            "month_name": dt.format("%B").to_string(),
            "day": dt.day(),
            "day_of_week": dt.format("%A").to_string(),
            "day_of_week_number": dt.weekday().number_from_monday(),
            "day_of_year": dt.ordinal(),
            "week_number": dt.iso_week().week(),
            "quarter": quarter,
            "is_leap_year": is_leap,
            "is_weekend": is_weekend,
            "days_in_month": days_in_month,
            "days_in_year": days_in_year,
            "days_remaining_in_year": days_in_year - dt.ordinal() as i32,
            "iso_8601": dt.to_rfc3339(),
        },
    }))
}

fn action_timestamp(params: &HashMap<String, Value>) -> Result<Value, String> {
    let ts = params.get("timestamp").and_then(|v| v.as_i64())
        .ok_or("Missing required parameter: timestamp")?;

    let dt = DateTime::from_timestamp(ts, 0)
        .ok_or_else(|| format!("Invalid Unix timestamp: {ts}"))?;

    Ok(json!({
        "status": "ok",
        "action": "timestamp",
        "input": ts,
        "datetime": {
            "utc": dt.to_rfc3339(),
            "iso_8601": dt.to_rfc3339(),
            "date": dt.format("%Y-%m-%d").to_string(),
            "time": dt.format("%H:%M:%S").to_string(),
            "day_of_week": dt.format("%A").to_string(),
            "human_readable": dt.format("%B %d, %Y at %I:%M:%S %p UTC").to_string(),
        },
        "milliseconds": ts * 1000,
        "microseconds": ts * 1_000_000,
    }))
}

fn action_relative(params: &HashMap<String, Value>) -> Result<Value, String> {
    let date_str = params.get("date").and_then(|v| v.as_str())
        .ok_or("Missing required parameter: date")?;

    let dt = parse_any_datetime(date_str)?;
    let now = Utc::now();
    let diff = now - dt;
    let abs_diff = diff.num_seconds().abs();
    let is_past = dt <= now;

    let relative = format_relative(abs_diff, is_past);

    Ok(json!({
        "status": "ok",
        "action": "relative",
        "input": date_str,
        "relative": relative,
        "is_past": is_past,
        "seconds_ago": if is_past { abs_diff } else { -abs_diff },
        "datetime": dt.to_rfc3339(),
    }))
}

// ============================================================================
// Helpers
// ============================================================================

fn format_tz(dt: &DateTime<Utc>, tz_name: &str) -> Result<String, String> {
    let tz = parse_tz(tz_name)?;
    let local = tz.from_utc_datetime(&dt.naive_utc());
    Ok(local.format("%Y-%m-%d %H:%M:%S %Z").to_string())
}

fn parse_tz(tz_name: &str) -> Result<Tz, String> {
    if tz_name.is_empty() || tz_name == "UTC" || tz_name == "utc" {
        return Ok(Tz::UTC);
    }
    tz_name.parse()
        .map_err(|_| format!("Unknown timezone: {tz_name}. Use IANA format like 'Asia/Shanghai'"))
}

fn get_offset_str(tz_name: &str) -> String {
    match tz_name {
        "UTC" => "UTC+0",
        "Asia/Shanghai" | "Asia/Chongqing" => "UTC+8",
        "Asia/Tokyo" => "UTC+9",
        "America/New_York" => "UTC-5/UTC-4",
        "Europe/London" => "UTC+0/UTC+1",
        _ => tz_name,
    }.to_string()
}

fn parse_any_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = d.and_time(NaiveTime::MIN);
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
    }
    Err(format!("Unable to parse date: '{s}'. Supported formats: ISO 8601, YYYY-MM-DD HH:MM:SS, YYYY-MM-DD"))
}

fn format_duration_hms(total_seconds: i64) -> String {
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let mut parts = Vec::new();
    if days > 0 { parts.push(format!("{days} day{}", if days != 1 { "s" } else { "" })); }
    if hours > 0 { parts.push(format!("{hours} hour{}", if hours != 1 { "s" } else { "" })); }
    if minutes > 0 { parts.push(format!("{minutes} minute{}", if minutes != 1 { "s" } else { "" })); }
    if seconds > 0 || parts.is_empty() { parts.push(format!("{seconds} second{}", if seconds != 1 { "s" } else { "" })); }
    parts.join(", ")
}

fn format_relative(abs_diff: i64, is_past: bool) -> String {
    let ago = |s: &str| if is_past { format!("{s} ago") } else { format!("in {s}") };
    if abs_diff < 60 {
        ago(&format!("{abs_diff} second{}", if abs_diff != 1 { "s" } else { "" }))
    } else if abs_diff < 3600 {
        let mins = abs_diff / 60;
        ago(&format!("{mins} minute{}", if mins != 1 { "s" } else { "" }))
    } else if abs_diff < 86400 {
        let hours = abs_diff / 3600;
        ago(&format!("{hours} hour{}", if hours != 1 { "s" } else { "" }))
    } else if abs_diff < 604800 {
        let days = abs_diff / 86400;
        ago(&format!("{days} day{}", if days != 1 { "s" } else { "" }))
    } else if abs_diff < 2592000 {
        let weeks = abs_diff / 604800;
        ago(&format!("{weeks} week{}", if weeks != 1 { "s" } else { "" }))
    } else {
        let months = (abs_diff as f64 / 2592000.0).floor() as i64;
        ago(&format!("{months} month{}", if months != 1 { "s" } else { "" }))
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(month: u32, leap: bool) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if leap { 29 } else { 28 },
        _ => 0,
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DateTimeTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_hms() {
        assert_eq!(format_duration_hms(0), "0 seconds");
        assert_eq!(format_duration_hms(1), "1 second");
        assert_eq!(format_duration_hms(60), "1 minute");
        assert_eq!(format_duration_hms(3661), "1 hour, 1 minute, 1 second");
        assert_eq!(format_duration_hms(90061), "1 day, 1 hour, 1 minute, 1 second");
    }

    #[test]
    fn test_parse_any_datetime_iso() {
        let dt = parse_any_datetime("2024-01-15T10:30:00Z").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_any_datetime_date_only() {
        let dt = parse_any_datetime("2024-03-20").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 20);
        assert_eq!(dt.hour(), 0);
    }

    #[test]
    fn test_parse_any_datetime_with_time() {
        let dt = parse_any_datetime("2024-06-01 14:30:00").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_any_datetime("not-a-date").is_err());
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(1, false), 31);
        assert_eq!(days_in_month(2, true), 29);
        assert_eq!(days_in_month(2, false), 28);
        assert_eq!(days_in_month(4, false), 30);
    }

    #[test]
    fn test_format_duration_hms_singular_plural() {
        assert_eq!(format_duration_hms(86400), "1 day");
        assert_eq!(format_duration_hms(3600), "1 hour");
        assert_eq!(format_duration_hms(60), "1 minute");
        assert_eq!(format_duration_hms(86400 * 2 + 3600 * 3 + 60 * 4 + 5), "2 days, 3 hours, 4 minutes, 5 seconds");
    }

    #[test]
    fn test_format_duration_hms_zero_seconds() {
        assert_eq!(format_duration_hms(0), "0 seconds");
    }

    #[test]
    fn test_format_relative_seconds() {
        assert_eq!(format_relative(30, true), "30 seconds ago");
        assert_eq!(format_relative(1, true), "1 second ago");
        assert_eq!(format_relative(30, false), "in 30 seconds");
        assert_eq!(format_relative(1, false), "in 1 second");
    }

    #[test]
    fn test_format_relative_minutes() {
        assert_eq!(format_relative(120, true), "2 minutes ago");
        assert_eq!(format_relative(60, true), "1 minute ago");
        assert_eq!(format_relative(120, false), "in 2 minutes");
    }

    #[test]
    fn test_format_relative_hours() {
        assert_eq!(format_relative(7200, true), "2 hours ago");
        assert_eq!(format_relative(3600, true), "1 hour ago");
        assert_eq!(format_relative(7200, false), "in 2 hours");
    }

    #[test]
    fn test_format_relative_days() {
        assert_eq!(format_relative(172800, true), "2 days ago");
        assert_eq!(format_relative(86400, true), "1 day ago");
        assert_eq!(format_relative(172800, false), "in 2 days");
    }

    #[test]
    fn test_format_relative_weeks() {
        assert_eq!(format_relative(1209600, true), "2 weeks ago");
        assert_eq!(format_relative(604800, true), "1 week ago");
        assert_eq!(format_relative(1209600, false), "in 2 weeks");
    }

    #[test]
    fn test_format_relative_months() {
        assert_eq!(format_relative(5184000, true), "2 months ago");
        assert_eq!(format_relative(2592000, true), "1 month ago");
        assert_eq!(format_relative(5184000, false), "in 2 months");
    }

    #[test]
    fn test_parse_any_datetime_iso_with_offset() {
        let dt = parse_any_datetime("2024-01-15T10:30:00+08:00").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_any_datetime_no_t_naive() {
        let dt = parse_any_datetime("2024-06-01T14:30:00").unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn test_days_in_month_all_months() {
        assert_eq!(days_in_month(1, false), 31);
        assert_eq!(days_in_month(2, false), 28);
        assert_eq!(days_in_month(2, true), 29);
        assert_eq!(days_in_month(3, false), 31);
        assert_eq!(days_in_month(4, false), 30);
        assert_eq!(days_in_month(5, false), 31);
        assert_eq!(days_in_month(6, false), 30);
        assert_eq!(days_in_month(7, false), 31);
        assert_eq!(days_in_month(8, false), 31);
        assert_eq!(days_in_month(9, false), 30);
        assert_eq!(days_in_month(10, false), 31);
        assert_eq!(days_in_month(11, false), 30);
        assert_eq!(days_in_month(12, false), 31);
        assert_eq!(days_in_month(13, false), 0); // invalid month
    }

    #[test]
    fn test_parse_tz_valid_and_invalid() {
        assert!(parse_tz("UTC").is_ok());
        assert!(parse_tz("Asia/Shanghai").is_ok());
        assert!(parse_tz("America/New_York").is_ok());
        assert!(parse_tz("Europe/London").is_ok());
        assert!(parse_tz("Invalid/Timezone").is_err());
        assert!(parse_tz("").is_ok()); // empty defaults to UTC
    }

    #[test]
    fn test_offset_str_common_zones() {
        assert_eq!(get_offset_str("UTC"), "UTC+0");
        assert_eq!(get_offset_str("Asia/Shanghai"), "UTC+8");
        assert_eq!(get_offset_str("Asia/Tokyo"), "UTC+9");
        assert_eq!(get_offset_str("America/New_York"), "UTC-5/UTC-4");
        assert_eq!(get_offset_str("Europe/London"), "UTC+0/UTC+1");
        assert_eq!(get_offset_str("Custom/Zone"), "Custom/Zone");
    }

    #[test]
    fn test_duration_add_negative() {
        let _dt = parse_any_datetime("2024-06-15T12:00:00Z").unwrap();
        let result = action_add(&HashMap::from([
            ("action".to_string(), Value::String("add".into())),
            ("date".to_string(), Value::String("2024-06-15T12:00:00Z".into())),
            ("unit".to_string(), Value::String("days".into())),
            ("amount".to_string(), Value::Number((-5i64).into())),
        ]));
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res["status"], "ok");
    }
}
