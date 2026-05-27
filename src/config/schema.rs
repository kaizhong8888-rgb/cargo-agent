use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level configuration for cargo-agent.
///
/// All fields have sensible defaults, so you can construct a minimal config with:
///
/// ```
/// use cargo_agent::config::schema::CargoConfig;
///
/// let config = CargoConfig::default();
/// assert_eq!(config.agent.max_iterations, 90);
/// assert!(config.memory.memory_enabled);
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CargoConfig {
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub fallback_providers: Vec<String>,
    #[serde(default = "default_credential_pool_strategies")]
    pub credential_pool_strategies: HashMap<String, String>,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub toolsets: Vec<String>,
    #[serde(default)]
    pub display: DisplayConfig,
}

impl Default for CargoConfig {
    fn default() -> Self {
        CargoConfig {
            model: ModelConfig::default(),
            providers: HashMap::new(),
            fallback_providers: vec![],
            credential_pool_strategies: default_credential_pool_strategies(),
            agent: AgentConfig::default(),
            terminal: TerminalConfig::default(),
            memory: MemoryConfig::default(),
            mcp_servers: HashMap::new(),
            toolsets: vec!["cargo-cli".to_string()],
            display: DisplayConfig::default(),
        }
    }
}

impl CargoConfig {
    /// Save configuration to a YAML file.
    ///
    /// Creates parent directories if they don't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cargo_agent::config::schema::CargoConfig;
    /// use std::path::Path;
    ///
    /// let config = CargoConfig::default();
    /// config.save(Path::new("/tmp/cargo-agent-config.yaml")).unwrap();
    /// ```
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        std::fs::write(path, &yaml)
            .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;
        Ok(())
    }
}

/// Model configuration
///
/// # Example
///
/// ```
/// use cargo_agent::config::schema::ModelConfig;
///
/// let config = ModelConfig::default();
/// assert_eq!(config.default, "deepseek-v4-flash");
/// assert_eq!(config.provider, "deepseek");
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    #[serde(default = "default_model")]
    pub default: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

fn default_model() -> String {
    "deepseek-v4-flash".to_string()
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            default: default_model(),
            provider: "deepseek".to_string(),
            base_url: Some(
                "https://api.deepseek.com".to_string(),
            ),
        }
    }
}

/// Provider configuration (base URL, API key, API mode).
///
/// # Example
///
/// ```
/// use cargo_agent::config::schema::ProviderConfig;
///
/// let config = ProviderConfig {
///     base_url: Some("https://api.example.com".into()),
///     api_key: Some("sk-xxx".into()),
///     api_mode: Some("chat".into()),
/// };
/// assert!(config.api_key.is_some());
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProviderConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_mode: Option<String>,
}

fn default_credential_pool_strategies() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("alibaba".to_string(), "fill_first".to_string());
    map
}

/// Agent configuration
///
/// Controls the behavior of the AI agent, including iteration limits,
/// timeouts, retries, and reasoning effort.
///
/// # Example
///
/// ```
/// use cargo_agent::config::schema::AgentConfig;
///
/// let config = AgentConfig::default();
/// assert_eq!(config.max_iterations, 90);
/// assert_eq!(config.tool_use_enforcement, "auto");
/// assert_eq!(config.reasoning_effort, "medium");
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_gateway_timeout")]
    pub gateway_timeout: u64,
    #[serde(default = "default_restart_drain_timeout")]
    pub restart_drain_timeout: u64,
    #[serde(default = "default_api_max_retries")]
    pub api_max_retries: u32,
    #[serde(default)]
    pub service_tier: String,
    #[serde(default)]
    pub tool_use_enforcement: String,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub reasoning_effort: String,
    #[serde(default)]
    pub personalities: HashMap<String, String>,
    #[serde(default)]
    pub personalities_default: String,
}

fn default_max_iterations() -> u32 {
    90
}
fn default_gateway_timeout() -> u64 {
    1800
}
fn default_restart_drain_timeout() -> u64 {
    60
}
fn default_api_max_retries() -> u32 {
    3
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model: None, // No default model - use CargoConfig or provider detection
            provider: None,
            base_url: None,
            api_key: None,
            max_iterations: 90,
            gateway_timeout: 1800,
            restart_drain_timeout: 60,
            api_max_retries: 3,
            service_tier: String::new(),
            tool_use_enforcement: "auto".to_string(),
            verbose: false,
            reasoning_effort: "medium".to_string(),
            personalities: HashMap::new(),
            personalities_default: "helpful".to_string(),
        }
    }
}

/// Terminal configuration
///
/// # Example
///
/// ```
/// use cargo_agent::config::schema::TerminalConfig;
///
/// let config = TerminalConfig::default();
/// assert_eq!(config.backend, "");
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TerminalConfig {
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub timeout: u64,
}

/// Memory configuration
///
/// Controls whether memory persistence and user profiling are enabled.
///
/// # Example
///
/// ```
/// use cargo_agent::config::schema::MemoryConfig;
///
/// let config = MemoryConfig::default();
/// assert!(config.memory_enabled);
/// assert!(config.user_profile_enabled);
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryConfig {
    #[serde(default = "default_true")]
    pub memory_enabled: bool,
    #[serde(default = "default_true")]
    pub user_profile_enabled: bool,
    #[serde(default)]
    pub provider: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            memory_enabled: true,
            user_profile_enabled: true,
            provider: None,
        }
    }
}

/// MCP server configuration for connecting to external services.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpServerConfig {
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub url: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// Display configuration (language, personality).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DisplayConfig {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub personality: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_config_default_model() {
        let config = CargoConfig::default();
        assert_eq!(config.model.default, "deepseek-v4-flash");
    }

    #[test]
    fn test_cargo_config_default_agent() {
        let config = CargoConfig::default();
        assert_eq!(config.agent.max_iterations, 90);
        assert!(!config.agent.verbose);
    }

    #[test]
    fn test_cargo_config_default_memory() {
        let config = CargoConfig::default();
        assert!(config.memory.memory_enabled);
    }

    #[test]
    fn test_cargo_config_default_toolsets() {
        let config = CargoConfig::default();
        assert_eq!(config.toolsets, vec!["cargo-cli".to_string()]);
    }

    #[test]
    fn test_agent_config_custom_values() {
        let config = AgentConfig {
            max_iterations: 50,
            verbose: true,
            ..Default::default()
        };
        assert_eq!(config.max_iterations, 50);
        assert!(config.verbose);
        assert_eq!(config.tool_use_enforcement, "auto");
    }
}
