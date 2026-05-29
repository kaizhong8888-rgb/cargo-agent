pub mod env;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoConfig {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: "gpt-4".to_string(),
            base_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub model: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "default-agent".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: "You are a helpful AI assistant.".to_string(),
        }
    }
}

impl Default for CargoConfig {
    fn default() -> Self {
        Self {
            name: "agent-cargo".to_string(),
            version: "0.1.0".to_string(),
            model: ModelConfig::default(),
            api_key: None,
        }
    }
}

impl CargoConfig {
    pub fn load() -> Result<Self> {
        let config_path = crate::constants::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let expanded = env::expand_env_vars(&content);
            let config: CargoConfig = serde_yaml::from_str(&expanded)?;
            return Ok(config);
        }
        Ok(Self::default())
    }

    pub fn agent_config(&self) -> AgentConfig {
        AgentConfig::default()
    }

    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                return Some(key.clone());
            }
        }
        // Fall back to environment variables
        for var in ["CARGO_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"] {
            if let Ok(key) = std::env::var(var) {
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }
        None
    }

    pub fn resolve_base_url(&self) -> String {
        self.model
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com".to_string())
    }

    /// Validate critical configuration fields at startup.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.model.name.is_empty() {
            issues.push("model.name is empty — set a valid LLM model".into());
        }

        // API key will be checked at first use since env vars may resolve it
        // but warn if config has an empty explicit key
        if let Some(key) = &self.api_key {
            if key.is_empty() {
                issues.push(
                    "api_key is set but empty in config — remove it or provide a valid key".into(),
                );
            }
        }

        let base_url = self.resolve_base_url();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            issues.push(format!(
                "base_url '{base_url}' does not start with http:// or https://"
            ));
        }

        issues
    }
}

pub fn load_env_file() -> Result<()> {
    Ok(())
}

pub fn expand_env_vars(s: &str) -> String {
    env::expand_env_vars(s)
}
