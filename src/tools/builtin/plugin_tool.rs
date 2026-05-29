//! Plugin marketplace tool: browse, install, and uninstall community plugins.

use crate::plugin::marketplace::PluginMarketplace;
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn register_all(registry: &mut ToolRegistry, plugins_dir: PathBuf) {
    let marketplace = PluginMarketplace::new();
    registry.register(Box::new(PluginTool {
        marketplace,
        plugins_dir,
    }));
}

struct PluginTool {
    marketplace: PluginMarketplace,
    plugins_dir: PathBuf,
}

#[async_trait::async_trait]
impl Tool for PluginTool {
    fn name(&self) -> &str {
        "plugin"
    }

    fn description(&self) -> &str {
        "Manage community plugins. Actions: \
         browse (list plugins), search (find by name), \
         install (download and install), uninstall (remove), \
         info (plugin details), installed (list installed plugins)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: browse, search, install, uninstall, info, installed".to_string(),
                required: true,
            },
            ToolParameter {
                name: "plugin_id".to_string(),
                parameter_type: "string".to_string(),
                description: "Plugin ID (for install/uninstall/info)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "query".to_string(),
                parameter_type: "string".to_string(),
                description: "Search query (for search action)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "category".to_string(),
                parameter_type: "string".to_string(),
                description: "Filter by category (for browse action)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "browse" => {
                let category = params.get("category").and_then(|v| v.as_str());
                let plugins = self.marketplace.list(category);
                if plugins.is_empty() {
                    return Ok(Value::String("No plugins found.".into()));
                }
                let mut out = String::from("Available plugins:\n\n");
                for p in &plugins {
                    out.push_str(&format!(
                        "  {:<20} v{}  by {}  [{}]  ★{}\n    {}\n\n",
                        p.id,
                        p.version,
                        p.author,
                        p.categories.join(", "),
                        p.stars,
                        p.description,
                    ));
                }
                Ok(Value::String(out))
            }
            "search" => {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or("query is required for search".to_string())?;
                let results = self.marketplace.search(query);
                if results.is_empty() {
                    return Ok(Value::String(format!("No plugins matching '{query}'")));
                }
                let mut out = format!("Search results for '{query}':\n\n");
                for p in &results {
                    out.push_str(&format!(
                        "  {:<20} v{}  ★{}\n    {}\n\n",
                        p.id, p.version, p.stars, p.description
                    ));
                }
                Ok(Value::String(out))
            }
            "install" => {
                let plugin_id = params
                    .get("plugin_id")
                    .and_then(|v| v.as_str())
                    .ok_or("plugin_id is required for install".to_string())?;
                let plugin = self
                    .marketplace
                    .find(plugin_id)
                    .ok_or(format!("Plugin '{plugin_id}' not found"))?;
                match self.marketplace.install(plugin, &self.plugins_dir) {
                    Ok(msg) => Ok(Value::String(msg)),
                    Err(e) => Ok(Value::String(format!("Install failed: {e}"))),
                }
            }
            "uninstall" => {
                let plugin_id = params
                    .get("plugin_id")
                    .and_then(|v| v.as_str())
                    .ok_or("plugin_id is required for uninstall".to_string())?;
                match self.marketplace.uninstall(plugin_id, &self.plugins_dir) {
                    Ok(msg) => Ok(Value::String(msg)),
                    Err(e) => Ok(Value::String(format!("Uninstall failed: {e}"))),
                }
            }
            "info" => {
                let plugin_id = params
                    .get("plugin_id")
                    .and_then(|v| v.as_str())
                    .ok_or("plugin_id is required for info".to_string())?;
                let plugin = self
                    .marketplace
                    .find(plugin_id)
                    .ok_or(format!("Plugin '{plugin_id}' not found"))?;
                let mut out = format!(
                    "Plugin: {}\n  ID:       {}\n  Version:  {}\n  Author:   {}\n  Stars:    {}\n  Category: {}\n  Min agent: {}\n  URL:      {}\n\n  {}\n",
                    plugin.name, plugin.id, plugin.version, plugin.author,
                    plugin.stars, plugin.categories.join(", "),
                    plugin.min_agent_version, plugin.source_url, plugin.description,
                );
                if !plugin.dependencies.is_empty() {
                    out.push_str("\n  Dependencies:\n");
                    for (name, version) in &plugin.dependencies {
                        out.push_str(&format!("    {name} = \"{version}\"\n"));
                    }
                }
                Ok(Value::String(out))
            }
            "installed" => {
                let installed = self.marketplace.list_installed(&self.plugins_dir);
                if installed.is_empty() {
                    return Ok(Value::String("No plugins installed.".into()));
                }
                let out = format!(
                    "Installed plugins ({}):\n\n  {}\n",
                    installed.len(),
                    installed.join("\n  ")
                );
                Ok(Value::String(out))
            }
            _ => Err(format!(
                "Unknown action: {action}. Valid: browse, search, install, uninstall, info, installed"
            )),
        }
    }
}
