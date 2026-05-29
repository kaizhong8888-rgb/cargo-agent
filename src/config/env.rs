use regex::Regex;

/// Expand environment variables in a string.
///
/// Supports both `${VAR}` and `$VAR` syntax. Unset variables are left as-is
/// (e.g. `${UNDEFINED}` remains unchanged).
///
/// # Example
///
/// ```
/// use cargo_agent::config::env::expand_env_vars;
///
/// // With a set environment variable
/// std::env::set_var("MY_APP_HOME", "/usr/local/myapp");
/// let result = expand_env_vars("Home is ${MY_APP_HOME}");
/// assert_eq!(result, "Home is /usr/local/myapp");
///
/// // Unset variables are preserved unchanged
/// let result = expand_env_vars("Path: ${UNDEFINED_VAR}");
/// assert_eq!(result, "Path: ${UNDEFINED_VAR}");
/// ```
pub fn expand_env_vars(s: &str) -> String {
    let re = Regex::new(r"\$\{(\w+)\}|\$(\w+)").expect("invalid regex: env var pattern");
    re.replace_all(s, |caps: &regex::Captures| {
        let key = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str()).unwrap_or("");
        std::env::var(key).unwrap_or_else(|_| format!("${{{}}}", key))
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_var(prefix: &str) -> String {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{}_{}", prefix, n)
    }

    #[test]
    fn test_expand_simple_var() {
        let var = unique_var("TEST_VAR");
        std::env::set_var(&var, "hello");
        assert_eq!(expand_env_vars(&format!("${}", var)), "hello");
    }

    #[test]
    fn test_expand_brace_var() {
        let var = unique_var("TEST_VAR");
        std::env::set_var(&var, "world");
        assert_eq!(expand_env_vars(&format!("${{{}}}", var)), "world");
    }

    #[test]
    fn test_expand_with_around_text() {
        let var = unique_var("NAME");
        std::env::set_var(&var, "Rust");
        assert_eq!(
            expand_env_vars(&format!("Hello, ${{{}}}! Love ${}.", var, var)),
            "Hello, Rust! Love Rust."
        );
    }

    #[test]
    fn test_undefined_var_kept() {
        assert_eq!(expand_env_vars("${MISSING}"), "${MISSING}");
    }

    #[test]
    fn test_no_vars() {
        assert_eq!(expand_env_vars("plain text"), "plain text");
    }
}
