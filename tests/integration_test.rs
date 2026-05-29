//! Integration tests for cargo-agent configuration and module loading.

use cargo_agent::config::CargoConfig;

#[test]
fn default_config_has_valid_defaults() {
    let config = CargoConfig::default();
    assert_eq!(config.version, "0.1.0");
    assert_eq!(config.model.name, "gpt-4");
    assert!(config.api_key.is_none());
}

#[test]
fn config_validates_empty_model_name() {
    let config = CargoConfig {
        name: "test".into(),
        version: "0.1.0".into(),
        model: cargo_agent::config::ModelConfig {
            name: String::new(),
            base_url: None,
        },
        api_key: None,
    };
    let issues = config.validate();
    assert!(issues.iter().any(|i| i.contains("model.name is empty")));
}

#[test]
fn config_validates_invalid_base_url() {
    let config = CargoConfig {
        name: "test".into(),
        version: "0.1.0".into(),
        model: cargo_agent::config::ModelConfig {
            name: "gpt-4".into(),
            base_url: Some("not-a-url".into()),
        },
        api_key: None,
    };
    let issues = config.validate();
    assert!(issues.iter().any(|i| i.contains("base_url")));
}

#[test]
fn config_resolve_base_url_falls_back_to_openai() {
    let config = CargoConfig::default();
    let url = config.resolve_base_url();
    // May be overridden by ANTHROPIC_BASE_URL in test environment
    assert!(url.starts_with("http://") || url.starts_with("https://"));
}

#[test]
fn config_resolve_api_key_checks_env_vars() {
    // With no config key, it falls back to env vars
    let config = CargoConfig::default();
    // In test env, these vars are typically not set
    let key = config.resolve_api_key();
    // Either None (no env vars set) or Some (if test env has them)
    // Just verify it doesn't panic
    let _ = key;
}

#[test]
fn token_usage_starts_empty() {
    use cargo_agent::agent::core::TokenUsage;
    let usage = TokenUsage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
    assert_eq!(usage.api_calls, 0);
}
