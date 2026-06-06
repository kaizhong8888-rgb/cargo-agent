pub mod env;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoConfig {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub api_key: Option<String>,
    /// MCP server configurations keyed by display name.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
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

/// Configuration for connecting to an external MCP server.
///
/// # Example (config.yaml)
///
/// ```yaml
/// mcp_servers:
///   filesystem:
///     enabled: true
///     command: npx
///     args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
///     timeout: 10000
///   my-api:
///     enabled: true
///     transport: http
///     url: http://localhost:3000/mcp
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerConfig {
    /// Whether to auto-connect this server on startup.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Command to execute (for stdio transport).
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// HTTP endpoint URL (for HTTP/SSE transport).
    #[serde(default)]
    pub url: Option<String>,
    /// Transport type: "stdio" (default), "http", "sse".
    #[serde(default)]
    pub transport: Option<String>,
    /// Request timeout in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Allowed filesystem paths (for filesystem MCP servers).
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

fn default_true() -> bool {
    true
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
            mcp_servers: HashMap::new(),
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

        // Fallback 1: try loading from Hermes config
        if let Ok(hermes_config) = Self::load_hermes_config() {
            return Ok(hermes_config);
        }

        // Fallback 2: try ANTHROPIC_* env vars
        if let Some(env_config) = Self::load_from_env_vars() {
            return Ok(env_config);
        }

        Ok(Self::default())
    }

    /// Load configuration from ANTHROPIC_* environment variables.
    fn load_from_env_vars() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .ok()
            .filter(|s| !s.is_empty())?;

        let model = std::env::var("ANTHROPIC_MODEL")
            .ok()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let base_url = std::env::var("ANTHROPIC_BASE_URL").ok().unwrap_or_default();

        Some(Self {
            name: "agent-cargo".to_string(),
            version: "0.1.0".to_string(),
            model: ModelConfig {
                name: model,
                base_url: if base_url.is_empty() {
                    None
                } else {
                    Some(base_url)
                },
            },
            api_key: Some(api_key),
            mcp_servers: HashMap::new(),
        })
    }

    /// Load model configuration from Hermes config file (~/.hermes/config.yaml).
    /// Maps Hermes model settings to cargo-agent's OpenAI-compatible protocol.
    fn load_hermes_config() -> Result<Self> {
        let hermes_path = dirs_next::home_dir()
            .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
            .join(".hermes")
            .join("config.yaml");

        if !hermes_path.exists() {
            return Err(anyhow::anyhow!("Hermes config not found"));
        }

        let content = std::fs::read_to_string(&hermes_path)?;
        let hermes: HermesConfig = serde_yaml::from_str(&content)?;

        // Hermes may use Anthropic Messages API format, but cargo-agent
        // speaks OpenAI-compatible protocol. Extract the host from the
        // base_url and map to an OpenAI-compatible endpoint.
        let base_url = map_to_openai_compatible_url(&hermes.model);

        Ok(Self {
            name: "agent-cargo".to_string(),
            version: "0.1.0".to_string(),
            model: ModelConfig {
                name: hermes.model.default.clone(),
                base_url: Some(base_url),
            },
            api_key: Some(hermes.model.api_key.clone()),
            mcp_servers: HashMap::new(),
        })
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
        for var in [
            "CARGO_API_KEY",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
        ] {
            if let Ok(key) = std::env::var(var) {
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }
        None
    }

    pub fn resolve_base_url(&self) -> String {
        // Use config value if set
        if let Some(url) = &self.model.base_url {
            if !url.is_empty() {
                return url.clone();
            }
        }
        // Check for ANTHROPIC_BASE_URL env var
        if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
            if !url.is_empty() {
                return url;
            }
        }
        "https://api.openai.com".to_string()
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

/// Minimal Hermes config structure for model settings.
#[derive(Debug, Clone, Deserialize)]
struct HermesConfig {
    model: HermesModel,
}

#[derive(Debug, Clone, Deserialize)]
struct HermesModel {
    default: String,
    base_url: String,
    api_key: String,
}

/// Map a Hermes model config to an OpenAI-compatible base URL.
///
/// Hermes may use different API modes (anthropic_messages, openai, etc.).
/// cargo-agent only speaks OpenAI-compatible protocol. This function extracts
/// the host from the Hermes base_url and maps it to the correct OpenAI endpoint.
fn map_to_openai_compatible_url(model: &HermesModel) -> String {
    let url = model.base_url.as_str();

    // Extract scheme and host (strip any path segments)
    let host = if let Some(after_scheme) = url.find("://") {
        let rest = &url[after_scheme + 3..];
        rest.split('/').next().unwrap_or(rest)
    } else {
        url.split('/').next().unwrap_or(url)
    };

    let scheme = if url.starts_with("http://") {
        "http"
    } else {
        "https"
    };

    // Known provider mappings
    // Note: ModelClient appends /v1/chat/completions to the base_url,
    // so we only return scheme + host here.
    if host.contains("dashscope") || host.contains("aliyuncs.com") {
        // DashScope: use same host; client will append /v1/chat/completions
        return format!("{scheme}://{host}");
    }

    if host.contains("openai") {
        return format!("{scheme}://{host}/v1");
    }

    // Fallback: use scheme + host only (strip any path that would
    // conflict with the /v1/chat/completions suffix appended by the client)
    format!("{scheme}://{host}")
}
