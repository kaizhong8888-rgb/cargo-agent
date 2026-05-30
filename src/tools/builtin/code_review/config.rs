//! Configuration types for code review.

/// User-configurable thresholds for style checks.
#[derive(Clone)]
pub struct Thresholds {
    pub max_fn_length: usize,
    pub max_nesting: usize,
    pub max_line_length: usize,
}

/// Which checks to run.
#[derive(Clone, Debug)]
pub struct ActiveChecks {
    pub unsafe_check: bool,
    pub error_handling: bool,
    pub performance: bool,
    pub style: bool,
    pub safety: bool,
    pub correctness: bool,
    pub concurrency: bool,
    pub documentation: bool,
    pub naming: bool,
    pub async_check: bool,
    pub security: bool,
    pub complexity: bool,
    pub testing: bool,
    pub debug: bool,
}

impl ActiveChecks {
    pub fn all() -> Self {
        Self {
            unsafe_check: true, error_handling: true, performance: true, style: true,
            safety: true, correctness: true, concurrency: true, documentation: true,
            naming: true, async_check: true, security: true, complexity: true,
            testing: true, debug: true,
        }
    }
}

pub fn parse_checks(checks_str: &str) -> Result<ActiveChecks, String> {
    let trimmed = checks_str.trim().to_lowercase();
    if trimmed == "all" { return Ok(ActiveChecks::all()); }
    let mut checks = ActiveChecks {
        unsafe_check: false, error_handling: false, performance: false, style: false,
        safety: false, correctness: false, concurrency: false, documentation: false,
        naming: false, async_check: false, security: false, complexity: false,
        testing: false, debug: false,
    };
    for check in trimmed.split(',') {
        let c = check.trim();
        match c {
            "unsafe" => checks.unsafe_check = true,
            "error_handling" | "errorhandling" => checks.error_handling = true,
            "performance" | "perf" => checks.performance = true,
            "style" => checks.style = true,
            "safety" => checks.safety = true,
            "correctness" => checks.correctness = true,
            "concurrency" | "conc" => checks.concurrency = true,
            "documentation" | "docs" => checks.documentation = true,
            "naming" | "name" => checks.naming = true,
            "async" => checks.async_check = true,
            "security" | "sec" => checks.security = true,
            "complexity" | "compl" => checks.complexity = true,
            "testing" | "test" => checks.testing = true,
            "debug" => checks.debug = true,
            "" => {}
            _ => return Err(format!("Unknown check: '{c}'.")),
        }
    }
    Ok(checks)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error = 3,
    Warning = 2,
    Info = 1,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        }
    }
}

#[inline]
pub fn severity_threshold(level: &str) -> Severity {
    match level {
        "error" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        _ => Severity::Info,
    }
}

#[derive(Debug, Clone)]
pub struct ReviewIssue {
    pub severity: Severity,
    pub check: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub recommendation: Option<String>,
}

#[inline]
pub fn to_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for ch in name.chars() {
        if ch == '_' { capitalize = true; }
        else if capitalize { result.push(ch.to_ascii_uppercase()); capitalize = false; }
        else { result.push(ch); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_checks_all() {
        let checks = parse_checks("all").unwrap();
        assert!(checks.unsafe_check);
        assert!(checks.naming);
        assert!(checks.async_check);
    }

    #[test]
    fn test_parse_checks_subset() {
        let checks = parse_checks("unsafe,style").unwrap();
        assert!(checks.unsafe_check);
        assert!(checks.style);
        assert!(!checks.error_handling);
        assert!(!checks.naming);
        assert!(!checks.async_check);
    }

    #[test]
    fn test_parse_checks_invalid() {
        assert!(parse_checks("nonexistent").is_err());
    }

    #[test]
    fn test_severity_threshold() {
        assert_eq!(severity_threshold("error") as u8, Severity::Error as u8);
        assert_eq!(severity_threshold("warning") as u8, Severity::Warning as u8);
        assert_eq!(severity_threshold("info") as u8, Severity::Info as u8);
        assert_eq!(severity_threshold("invalid") as u8, Severity::Info as u8);
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("my_struct"), "MyStruct");
        assert_eq!(to_camel_case("foo_bar_baz"), "FooBarBaz");
        assert_eq!(to_camel_case("simple"), "Simple");
    }

    #[test]
    fn test_parse_checks_security() {
        let checks = parse_checks("security").unwrap();
        assert!(checks.security);
        assert!(!checks.unsafe_check);
        assert!(!checks.naming);
    }

    #[test]
    fn test_parse_checks_complexity() {
        let checks = parse_checks("complexity,testing,debug").unwrap();
        assert!(checks.complexity);
        assert!(checks.testing);
        assert!(checks.debug);
        assert!(!checks.unsafe_check);
        assert!(!checks.naming);
    }
}
