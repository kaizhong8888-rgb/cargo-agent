//! Configuration persistence tool: save and retrieve user preferences.
//!
//! Stores settings like default editor, project paths, common templates,
//! and other user preferences in a JSON file.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const CONFIG_FILE_NAME: &str = "preferences.json";

/// Shared config store that can be passed between tool instances.
#[derive(Clone, Default)]
pub struct ConfigStore {
    data: Arc<Mutex<serde_json::Map<String, Value>>>,
    file_path: Option<Arc<Mutex<PathBuf>>>,
}

impl ConfigStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from the file on disk.
    pub fn load() -> Self {
        let path = config_file_path();
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        ConfigStore {
            data: Arc::new(Mutex::new(data)),
            file_path: Some(Arc::new(Mutex::new(path))),
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().ok().and_then(|d| d.get(key).cloned())
    }

    pub fn set(&self, key: &str, value: Value) {
        if let Ok(mut data) = self.data.lock() {
            data.insert(key.to_string(), value);
        }
        self.save();
    }

    pub fn delete(&self, key: &str) {
        if let Ok(mut data) = self.data.lock() {
            data.remove(key);
        }
        self.save();
    }

    pub fn list(&self) -> Vec<(String, Value)> {
        self.data
            .lock()
            .map(|d| d.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let (Ok(data), Some(path_guard)) = (self.data.lock(), &self.file_path) {
            if let Ok(path) = path_guard.lock() {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(
                    &*path,
                    serde_json::to_string_pretty(&Value::Object(data.clone())).unwrap_or_default(),
                );
            }
        }
    }
}

fn config_file_path() -> PathBuf {
    let dir = crate::constants::agent_dir_path();
    dir.join(CONFIG_FILE_NAME)
}

// ============================================================================
// ConfigTool
// ============================================================================

pub struct ConfigTool {
    store: ConfigStore,
}

impl ConfigTool {
    pub fn new(store: ConfigStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "config"
    }

    fn description(&self) -> &str {
        "Persist and retrieve user preferences across sessions. Actions: set (save a preference), get (retrieve a value), list (show all preferences), delete (remove a preference). Stores data in ~/.cargo-agent/preferences.json."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: set, get, list, delete".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "key".to_string(),
                description: "Preference key (e.g. 'default_editor', 'project_path', 'preferred_template')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "value".to_string(),
                description: "Value to store (any JSON value: string, number, boolean, array, object). Used with set action.".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        match action {
            "set" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: key (for set action)")?;

                let value_str = params
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: value (for set action)")?;

                let value: Value = serde_json::from_str(value_str)
                    .map_err(|e| format!("Invalid JSON value: {e}"))?;

                self.store.set(key, value.clone());

                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "set",
                    "key": key,
                    "value": value,
                }))
            }
            "get" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: key (for get action)")?;

                match self.store.get(key) {
                    Some(value) => Ok(serde_json::json!({
                        "status": "ok",
                        "action": "get",
                        "key": key,
                        "value": value,
                    })),
                    None => Ok(serde_json::json!({
                        "status": "not_found",
                        "action": "get",
                        "key": key,
                    })),
                }
            }
            "list" => {
                let prefs = self.store.list();
                let count = prefs.len();
                let map: serde_json::Map<String, Value> = prefs.into_iter().collect();
                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "list",
                    "preferences": Value::Object(map),
                    "count": count,
                }))
            }
            "delete" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: key (for delete action)")?;

                self.store.delete(key);
                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "delete",
                    "key": key,
                }))
            }
            other => Err(format!(
                "Unknown action: {other}. Supported: set, get, list, delete"
            )),
        }
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    let store = ConfigStore::load();
    registry.register(Box::new(ConfigTool::new(store)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_store_set_and_get() {
        let store = ConfigStore::new();
        store.set("test_key", Value::String("hello".to_string()));
        let value = store.get("test_key");
        assert_eq!(value, Some(Value::String("hello".to_string())));
    }

    #[test]
    fn config_store_delete() {
        let store = ConfigStore::new();
        store.set("temp", Value::Bool(true));
        assert!(store.get("temp").is_some());
        store.delete("temp");
        assert!(store.get("temp").is_none());
    }

    #[test]
    fn config_store_list() {
        let store = ConfigStore::new();
        store.set("a", Value::Number(1.into()));
        store.set("b", Value::Bool(false));
        let prefs = store.list();
        assert_eq!(prefs.len(), 2);
    }

    #[test]
    fn config_tool_metadata() {
        let store = ConfigStore::new();
        let tool = ConfigTool::new(store);
        assert_eq!(tool.name(), "config");
        assert!(tool.description().contains("preferences"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
    }
}
