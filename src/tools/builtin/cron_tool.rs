//! Cron Tool: parse, validate, and compute next execution times for cron expressions.
//!
//! Actions: parse (parse and validate a cron expression), next (compute next N run times),
//! validate (check if expression is valid), describe (human-readable description).

use chrono::{Datelike, Timelike};
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CronTool));
}

struct CronTool;

#[async_trait::async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str { "cron" }

    fn description(&self) -> &str {
        "Parse, validate, and compute next execution times for cron expressions. \
         Actions: parse (parse and validate), next (compute next N run times), \
         validate (check validity), describe (human-readable description)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "action".to_string(), parameter_type: "string".to_string(), description: "Action: parse, next, validate, describe".to_string(), required: true },
            ToolParameter { name: "expression".to_string(), parameter_type: "string".to_string(), description: "Cron expression (e.g. '*/5 * * * *')".to_string(), required: true },
            ToolParameter { name: "count".to_string(), parameter_type: "number".to_string(), description: "Number of next run times to compute (default: 5, max: 20)".to_string(), required: false },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        match action {
            "parse" | "validate" => validate_cron(params),
            "next" => next_run_times(params),
            "describe" => describe_cron(params),
            _ => Err(format!("Unknown action: {action}. Valid: parse, next, validate, describe")),
        }
    }
}

#[derive(Debug, Clone)]
struct CronField {
    values: Vec<u32>,
    original: String,
}

fn validate_cron(params: &HashMap<String, Value>) -> Result<Value, String> {
    let expression = params.get("expression").and_then(|v| v.as_str()).ok_or("'expression' is required")?;
    match parse_cron_expression(expression) {
        Ok(fields) => Ok(serde_json::json!({
            "expression": expression, "valid": true,
            "fields": { "minute": fields[0].original, "hour": fields[1].original, "day_of_month": fields[2].original, "month": fields[3].original, "day_of_week": fields[4].original },
            "field_count": 5,
        })),
        Err(e) => Ok(serde_json::json!({ "expression": expression, "valid": false, "error": e })),
    }
}

fn next_run_times(params: &HashMap<String, Value>) -> Result<Value, String> {
    let expression = params.get("expression").and_then(|v| v.as_str()).ok_or("'expression' is required")?;
    let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(5).min(20) as usize;
    let fields = parse_cron_expression(expression)?;
    let now = chrono::Local::now();
    let mut times = Vec::with_capacity(count);
    let mut current = now + chrono::Duration::minutes(1);
    current = current.with_second(0).unwrap();
    while times.len() < count {
        if matches_cron(&fields, &current) {
            times.push(current.format("%Y-%m-%d %H:%M:%S %Z").to_string());
        }
        current += chrono::Duration::minutes(1);
        if times.is_empty() && (current - now) > chrono::Duration::days(366) {
            return Err("Could not find next run time within 1 year".to_string());
        }
        if times.len() >= count { break; }
    }
    Ok(serde_json::json!({
        "expression": expression, "count": times.len(), "next_runs": times,
        "computed_from": now.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
    }))
}

fn describe_cron(params: &HashMap<String, Value>) -> Result<Value, String> {
    let expression = params.get("expression").and_then(|v| v.as_str()).ok_or("'expression' is required")?;
    let fields = parse_cron_expression(expression)?;
    let minute_desc = describe_field(&fields[0], "minute");
    let hour_desc = describe_field(&fields[1], "hour");
    let dom_desc = describe_field(&fields[2], "day of month");
    let month_desc = describe_field(&fields[3], "month");
    let dow_desc = describe_field(&fields[4], "day of week");
    let description = format!("At {} past hour {}, on {} of {}, {}", minute_desc, hour_desc, dom_desc, month_desc, dow_desc);

    let friendly = if expression == "* * * * *" {
        "Every minute".to_string()
    } else if expression.starts_with("*/") && expression.ends_with(" * * * *") {
        let step_part = expression.split(' ').next().unwrap_or(expression);
        // step_part is like "*/5" - extract the number after */
        let step_num = step_part.trim_start_matches("*/");
        format!("Every {} minutes", step_num)
    } else if expression == "0 * * * *" {
        "Every hour at minute 0".to_string()
    } else if expression == "0 0 * * *" {
        "Every day at midnight".to_string()
    } else if expression == "0 0 * * 0" || expression == "0 0 * * 7" {
        "Every Sunday at midnight".to_string()
    } else if expression == "0 0 * * 1-5" {
        "Every weekday (Mon-Fri) at midnight".to_string()
    } else if expression.starts_with("0 0 1 ") && expression.ends_with(" *") {
        "On the first day of every month at midnight".to_string()
    } else if expression.starts_with("0 ") && !expression.contains(',') && !expression.contains('/') {
        format!("At minute 0 past hour {}, {}", &fields[1].original, description)
    } else {
        description
    };

    Ok(serde_json::json!({
        "expression": expression, "description": friendly,
        "fields": {
            "minute": { "pattern": fields[0].original.clone(), "values": fields[0].values },
            "hour": { "pattern": fields[1].original.clone(), "values": fields[1].values },
            "day_of_month": { "pattern": fields[2].original.clone(), "values": fields[2].values },
            "month": { "pattern": fields[3].original.clone(), "values": fields[3].values },
            "day_of_week": { "pattern": fields[4].original.clone(), "values": fields[4].values },
        },
    }))
}

fn parse_cron_expression(expr: &str) -> Result<Vec<CronField>, String> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(format!("Expected 5 fields, got {}. Format: 'minute hour day_of_month month day_of_week'", parts.len()));
    }
    let ranges = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)];
    let mut fields = Vec::with_capacity(5);
    for (i, part) in parts.iter().enumerate() {
        let (min, max) = ranges[i];
        let values = parse_cron_field(part, min, max)?;
        fields.push(CronField { values, original: part.to_string() });
    }
    Ok(fields)
}

fn parse_cron_field(field: &str, min: u32, max: u32) -> Result<Vec<u32>, String> {
    let mut values = Vec::new();
    for part in field.split(',') {
        let part = part.trim();
        if part == "*" {
            for v in min..=max { values.push(v); }
        } else if let Some(star_pos) = part.find("*/") {
            let step: u32 = part[star_pos + 2..].parse().map_err(|_| format!("Invalid step in '{part}'"))?;
            if step == 0 || step > max { return Err(format!("Step {step} out of range [{min}-{max}]")); }
            let mut v = min;
            while v <= max { values.push(v); v += step; }
        } else if let Some(slash_pos) = part.find('/') {
            let step: u32 = part[slash_pos + 1..].parse().map_err(|_| format!("Invalid step in '{part}'"))?;
            if step == 0 { return Err("Step cannot be 0".to_string()); }
            let range_part = &part[..slash_pos];
            if range_part.contains('-') {
                let range_parts: Vec<&str> = range_part.splitn(2, '-').collect();
                let start: u32 = range_parts[0].parse().map_err(|_| format!("Invalid start in '{part}'"))?;
                let end: u32 = range_parts[1].parse().map_err(|_| format!("Invalid end in '{part}'"))?;
                if start > end { return Err(format!("Start {start} > end {end}")); }
                if start < min || end > max { return Err(format!("Value out of range [{min}-{max}]")); }
                let mut v = start;
                while v <= end { values.push(v); v += step; }
            } else {
                let start: u32 = range_part.parse().map_err(|_| format!("Invalid start in '{part}'"))?;
                let mut v = start;
                while v <= max { values.push(v); v += step; }
            }
        } else if part.contains('-') {
            let parts: Vec<&str> = part.splitn(2, '-').collect();
            let start: u32 = parts[0].parse().map_err(|_| format!("Invalid start in '{part}'"))?;
            let end: u32 = parts[1].parse().map_err(|_| format!("Invalid end in '{part}'"))?;
            if start > end { return Err(format!("Start {start} > end {end}")); }
            if start < min || end > max { return Err(format!("Value out of range [{min}-{max}]")); }
            for v in start..=end { values.push(v); }
        } else {
            let v: u32 = part.parse().map_err(|_| format!("Invalid value '{part}'"))?;
            if v < min || v > max { return Err(format!("Value {v} out of range [{min}-{max}]")); }
            values.push(v);
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn matches_cron(fields: &[CronField], dt: &chrono::DateTime<chrono::Local>) -> bool {
    let minute = dt.minute();
    let hour = dt.hour();
    let dom = dt.day();
    let month = dt.month();
    let dow = dt.weekday().num_days_from_sunday();
    fields[0].values.contains(&minute)
        && fields[1].values.contains(&hour)
        && fields[2].values.contains(&dom)
        && fields[3].values.contains(&month)
        && (fields[4].values.contains(&dow) || fields[4].values.contains(&(dow + 7)))
}

fn describe_field(field: &CronField, unit: &str) -> String {
    if field.values.len() == 60 || field.values.len() == 24 || field.values.len() == 31 || field.values.len() == 12 || field.values.len() == 8 {
        return format!("every {unit}");
    }
    if field.values.is_empty() { return unit.to_string(); }
    if field.values.len() <= 5 {
        field.values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
    } else {
        format!("{} values", field.values.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wildcard() {
        let vals = parse_cron_field("*", 0, 59).unwrap();
        assert_eq!(vals.len(), 60);
        assert_eq!(vals[0], 0);
        assert_eq!(vals[59], 59);
    }

    #[test]
    fn test_parse_step() {
        let vals = parse_cron_field("*/15", 0, 59).unwrap();
        assert_eq!(vals, vec![0, 15, 30, 45]);
    }

    #[test]
    fn test_parse_range() {
        let vals = parse_cron_field("1-5", 0, 59).unwrap();
        assert_eq!(vals, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_comma() {
        let vals = parse_cron_field("1,3,5", 0, 59).unwrap();
        assert_eq!(vals, vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_single() {
        let vals = parse_cron_field("30", 0, 59).unwrap();
        assert_eq!(vals, vec![30]);
    }

    #[test]
    fn test_parse_range_with_step() {
        let vals = parse_cron_field("0-30/10", 0, 59).unwrap();
        assert_eq!(vals, vec![0, 10, 20, 30]);
    }

    #[test]
    fn test_parse_invalid_range() {
        assert!(parse_cron_field("5-3", 0, 59).is_err());
    }

    #[test]
    fn test_parse_out_of_range() {
        assert!(parse_cron_field("60", 0, 59).is_err());
        assert!(parse_cron_field("100", 0, 59).is_err());
    }

    #[test]
    fn test_parse_invalid_step() {
        assert!(parse_cron_field("*/0", 0, 59).is_err());
    }

    #[test]
    fn test_parse_expression_5_fields() {
        let fields = parse_cron_expression("*/5 * * * *").unwrap();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0].values.len(), 12); // 0,5,10,...,55
        assert_eq!(fields[1].values.len(), 24); // all hours
    }

    #[test]
    fn test_parse_expression_wrong_field_count() {
        assert!(parse_cron_expression("* * *").is_err());
        assert!(parse_cron_expression("* * * * * *").is_err());
    }

    #[test]
    fn test_validate_valid_expression() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("0 0 * * *".to_string()));
        let r = validate_cron(&p).unwrap();
        assert_eq!(r["valid"], true);
    }

    #[test]
    fn test_validate_invalid_expression() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("invalid".to_string()));
        let r = validate_cron(&p).unwrap();
        assert_eq!(r["valid"], false);
        assert!(r["error"].as_str().is_some());
    }

    #[test]
    fn test_describe_every_minute() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("* * * * *".to_string()));
        let r = describe_cron(&p).unwrap();
        assert_eq!(r["description"], "Every minute");
    }

    #[test]
    fn test_describe_every_5_minutes() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("*/5 * * * *".to_string()));
        let r = describe_cron(&p).unwrap();
        assert_eq!(r["description"], "Every 5 minutes");
    }

    #[test]
    fn test_describe_midnight() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("0 0 * * *".to_string()));
        let r = describe_cron(&p).unwrap();
        assert_eq!(r["description"], "Every day at midnight");
    }

    #[test]
    fn test_describe_weekdays() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("0 0 * * 1-5".to_string()));
        let r = describe_cron(&p).unwrap();
        assert_eq!(r["description"], "Every weekday (Mon-Fri) at midnight");
    }

    #[test]
    fn test_describe_first_of_month() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("0 0 1 * *".to_string()));
        let r = describe_cron(&p).unwrap();
        assert_eq!(r["description"], "On the first day of every month at midnight");
    }

    #[test]
    fn test_next_run_times() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("* * * * *".to_string()));
        p.insert("count".to_string(), Value::Number(3.into()));
        let r = next_run_times(&p).unwrap();
        assert_eq!(r["count"].as_u64().unwrap(), 3);
        assert_eq!(r["next_runs"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_next_run_times_custom_count() {
        let mut p = HashMap::new();
        p.insert("expression".to_string(), Value::String("0 * * * *".to_string()));
        p.insert("count".to_string(), Value::Number(10.into()));
        let r = next_run_times(&p).unwrap();
        assert_eq!(r["count"].as_u64().unwrap(), 10);
    }

    #[test]
    fn test_cron_tool_metadata() {
        let t = CronTool;
        assert_eq!(t.name(), "cron");
        assert!(t.description().contains("cron"));
        let params = t.parameters();
        assert_eq!(params.len(), 3);
        assert!(params.iter().any(|p| p.name == "expression" && p.required));
    }

    #[test]
    fn test_describe_field_all_values() {
        let f = CronField { values: (0..60).collect(), original: "*".to_string() };
        assert_eq!(describe_field(&f, "minute"), "every minute");
    }

    #[test]
    fn test_describe_field_few_values() {
        let f = CronField { values: vec![1, 2, 3], original: "1-3".to_string() };
        assert_eq!(describe_field(&f, "minute"), "1, 2, 3");
    }

    #[test]
    fn test_describe_field_many_values() {
        let f = CronField { values: (0..24).collect(), original: "*".to_string() };
        assert_eq!(describe_field(&f, "hour"), "every hour");
    }

    #[test]
    fn test_validate_missing_expression() {
        assert!(validate_cron(&HashMap::new()).is_err());
    }

    #[test]
    fn test_matches_cron_specific_minute() {
        let fields = parse_cron_expression("30 * * * *").unwrap();
        let now = chrono::Local::now()
            .with_minute(30).unwrap()
            .with_second(0).unwrap();
        assert!(matches_cron(&fields, &now));

        let now2 = chrono::Local::now()
            .with_minute(15).unwrap()
            .with_second(0).unwrap();
        assert!(!matches_cron(&fields, &now2));
    }
}
