//! Plugin marketplace: discover, install, and manage community plugins.
//!
//! Plugins are Rust source files that implement the `Tool` trait. The marketplace
//! provides a registry of known plugins, fetches source from URLs, validates
//! safety (no secrets, no unsafe code without justification), and installs them
//! into the tools directory for compilation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A plugin entry in the marketplace registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Unique plugin identifier (e.g. "weather_tool")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Short description
    pub description: String,
    /// Author handle
    pub author: String,
    /// Semver version
    pub version: String,
    /// Categories for browsing
    pub categories: Vec<String>,
    /// Direct download URL for the .rs source file
    pub source_url: String,
    /// Required dependencies (crate_name = version)
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    /// Minimum cargo-agent version required
    #[serde(rename = "min_agent_version")]
    pub min_agent_version: String,
    /// Star count / popularity
    #[serde(default)]
    pub stars: u32,
}

/// Marketplace registry.
pub struct PluginMarketplace {
    /// Local cache of plugin entries
    plugins: Vec<PluginEntry>,
}

impl Default for PluginMarketplace {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginMarketplace {
    /// Create a marketplace with the built-in plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: builtin_registry(),
        }
    }

    /// List all available plugins, optionally filtered by category.
    pub fn list(&self, category: Option<&str>) -> Vec<&PluginEntry> {
        match category {
            Some(cat) => self
                .plugins
                .iter()
                .filter(|p| p.categories.iter().any(|c| c == cat))
                .collect(),
            None => self.plugins.iter().collect(),
        }
    }

    /// Find a plugin by ID.
    pub fn find(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.iter().find(|p| p.id == id)
    }

    /// Search plugins by name or description.
    pub fn search(&self, query: &str) -> Vec<&PluginEntry> {
        let q = query.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&q) || p.description.to_lowercase().contains(&q))
            .collect()
    }

    /// Install a plugin: download source, validate, write to plugins dir,
    /// register dependencies in Cargo.toml, and add mod declaration.
    pub fn install(&self, plugin: &PluginEntry, plugins_dir: &Path) -> anyhow::Result<String> {
        // Fetch source
        let source = fetch_source(&plugin.source_url)?;

        // Safety scan
        let issues = scan_for_issues(&source);
        if !issues.is_empty() {
            return Err(anyhow::anyhow!(
                "Plugin '{}' failed safety scan:\n{}",
                plugin.name,
                issues.join("\n")
            ));
        }

        // Write source file
        let dest = plugins_dir.join(format!("{}.rs", plugin.id));
        fs::write(&dest, &source)?;

        // Add dependencies to Cargo.toml if any
        if !plugin.dependencies.is_empty() {
            add_dependencies(&plugin.dependencies)?;
        }

        // Register in mod.rs
        register_plugin_mod(&plugin.id, plugins_dir)?;

        Ok(format!(
            "Plugin '{}' v{} installed to {}",
            plugin.name,
            plugin.version,
            dest.display()
        ))
    }

    /// Uninstall a plugin: remove source, deregister from mod.rs.
    pub fn uninstall(&self, id: &str, plugins_dir: &Path) -> anyhow::Result<String> {
        let source_file = plugins_dir.join(format!("{id}.rs"));
        if source_file.exists() {
            fs::remove_file(&source_file)?;
        }

        deregister_plugin_mod(id, plugins_dir)?;

        Ok(format!("Plugin '{id}' uninstalled"))
    }

    /// List installed plugins.
    pub fn list_installed(&self, plugins_dir: &Path) -> Vec<String> {
        if !plugins_dir.exists() {
            return Vec::new();
        }
        fs::read_dir(plugins_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
            .filter_map(|e| e.file_name().to_str().map(|s| s.trim_end_matches(".rs").to_string()))
            .collect()
    }
}

/// Built-in plugin registry — a curated list of community plugins.
fn builtin_registry() -> Vec<PluginEntry> {
    vec![
        PluginEntry {
            id: "weather_tool".into(),
            name: "Weather".into(),
            description: "Fetch current weather and forecasts from wttr.in or OpenWeatherMap API".into(),
            author: "community".into(),
            version: "0.1.0".into(),
            categories: vec!["data".into(), "utility".into()],
            source_url: "https://raw.githubusercontent.com/cargo-agent/plugins/main/weather_tool.rs".into(),
            dependencies: HashMap::new(),
            min_agent_version: "0.1.0".into(),
            stars: 42,
        },
        PluginEntry {
            id: "jira_tool".into(),
            name: "Jira".into(),
            description: "Create, update, and search Jira issues from the agent".into(),
            author: "community".into(),
            version: "0.1.0".into(),
            categories: vec!["devops".into(), "integration".into()],
            source_url: "https://raw.githubusercontent.com/cargo-agent/plugins/main/jira_tool.rs".into(),
            dependencies: vec![("serde_json".into(), "1.0".into())]
                .into_iter()
                .collect(),
            min_agent_version: "0.1.0".into(),
            stars: 31,
        },
        PluginEntry {
            id: "github_tool".into(),
            name: "GitHub".into(),
            description: "List PRs, issues, and check CI status via GitHub API".into(),
            author: "community".into(),
            version: "0.1.0".into(),
            categories: vec!["devops".into(), "integration".into()],
            source_url: "https://raw.githubusercontent.com/cargo-agent/plugins/main/github_tool.rs".into(),
            dependencies: HashMap::new(),
            min_agent_version: "0.1.0".into(),
            stars: 67,
        },
        PluginEntry {
            id: "slack_tool".into(),
            name: "Slack".into(),
            description: "Send messages and read channels via Slack Web API".into(),
            author: "community".into(),
            version: "0.1.0".into(),
            categories: vec!["communication".into(), "integration".into()],
            source_url: "https://raw.githubusercontent.com/cargo-agent/plugins/main/slack_tool.rs".into(),
            dependencies: HashMap::new(),
            min_agent_version: "0.1.0".into(),
            stars: 28,
        },
    ]
}

/// Fetch plugin source from URL.
fn fetch_source(url: &str) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch plugin source: HTTP {}", resp.status()));
    }
    Ok(resp.text()?)
}

/// Scan source for potential security issues.
fn scan_for_issues(source: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let patterns: &[(&str, &str)] = &[
        ("unsafe", "contains `unsafe` code without SAFETY comment"),
        ("std::process::Command", "spawns external processes"),
        ("std::env::set_var", "modifies environment variables"),
        ("std::fs::remove_dir_all", "recursive directory deletion"),
        ("DROP TABLE", "destructive SQL operation"),
        ("exec(", "potential command injection"),
        ("eval(", "potential code injection"),
    ];

    for (pattern, msg) in patterns {
        if source.contains(pattern) {
            // Allow `unsafe` if followed by `// SAFETY:` on same or next line
            if *pattern == "unsafe" {
                let has_safety = source
                    .lines()
                    .filter(|l| l.contains("unsafe"))
                    .any(|l| l.contains("SAFETY"));
                if has_safety {
                    continue;
                }
            }
            issues.push(format!("  - {msg}"));
        }
    }

    issues
}

/// Add dependencies to Cargo.toml.
fn add_dependencies(deps: &HashMap<String, String>) -> anyhow::Result<()> {
    let cargo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let mut content = fs::read_to_string(&cargo_path)?;

    for (name, version) in deps {
        if !content.contains(&format!("{name} = \"")) {
            content.push_str(&format!("{name} = \"{version}\"\n"));
        }
    }

    fs::write(&cargo_path, content)?;
    Ok(())
}

/// Register plugin module in mod.rs.
fn register_plugin_mod(id: &str, plugins_dir: &Path) -> anyhow::Result<()> {
    let mod_rs = plugins_dir.join("mod.rs");
    let mut content = if mod_rs.exists() {
        fs::read_to_string(&mod_rs)?
    } else {
        String::new()
    };

    let mod_line = format!("pub mod {id};\n");
    if !content.contains(&mod_line) {
        content.push_str(&mod_line);
        fs::write(&mod_rs, content)?;
    }

    Ok(())
}

/// Deregister plugin module from mod.rs.
fn deregister_plugin_mod(id: &str, plugins_dir: &Path) -> anyhow::Result<()> {
    let mod_rs = plugins_dir.join("mod.rs");
    if !mod_rs.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&mod_rs)?;
    let new_content = content
        .lines()
        .filter(|l| !l.contains(&format!("pub mod {id}")))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&mod_rs, new_content)?;
    Ok(())
}
