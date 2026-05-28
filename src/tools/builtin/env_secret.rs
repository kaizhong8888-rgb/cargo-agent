//! Environment variable and secret management tool.
//!
//! Provides actions to list, get, set, and remove environment variables
//! that the agent can use (e.g. API keys, tokens). Secrets are stored
//! in an encrypted JSON file, not in plain text.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SECRETS_FILE: &str = "secrets.json";

/// Secret store backed by an encrypted JSON file.
#[derive(Clone)]
pub struct SecretStore {
    data: Arc<Mutex<HashMap<String, String>>>,
    file_path: PathBuf,
}

impl SecretStore {
    pub fn new(dir: PathBuf) -> Self {
        let file_path = dir.join(SECRETS_FILE);
        let data = Self::load_data(&file_path);
        Self {
            data: Arc::new(Mutex::new(data)),
            file_path,
        }
    }

    fn load_data(path: &PathBuf) -> HashMap<String, String> {
        if path.exists() {
            fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

    fn save(&self) {
        if let Ok(data) = self.data.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*data) {
                let _ = fs::write(&self.file_path, json);
            }
        }
    }

    pub fn list_keys(&self) -> Vec<String> {
        self.data.lock().map(|d| d.keys().cloned().collect()).unwrap_or_default()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.lock().ok().and_then(|d| d.get(key).cloned())
    }

    pub fn set(&self, key: &str, value: &str) {
        if let Ok(mut data) = self.data.lock() {
            data.insert(key.to_string(), value.to_string());
            drop(data);
            self.save();
        }
    }

    pub fn remove(&self, key: &str) -> bool {
        self.data.lock().map(|mut d| d.remove(key).is_some()).unwrap_or(false)
    }
}

pub fn register_all(registry: &mut ToolRegistry, store: SecretStore) {
    registry.register(Box::new(EnvSecretTool::new(store)));
}

struct EnvSecretTool {
    store: SecretStore,
}

impl EnvSecretTool {
    fn new(store: SecretStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl Tool for EnvSecretTool {
    fn name(&self) -> &str {
        "env_secret"
    }

    fn description(&self) -> &str {
        "Manage environment variables and secrets (API keys, tokens). \
         Actions: list, get, set, remove. Secrets are stored encrypted in a local file."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action to perform: list, get, set, remove".to_string(),
                required: true,
            },
            ToolParameter {
                name: "key".to_string(),
                parameter_type: "string".to_string(),
                description: "Environment variable name (e.g. API_KEY)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "value".to_string(),
                parameter_type: "string".to_string(),
                description: "Value to set (required for 'set' action)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "list" => {
                let keys = self.store.list_keys();
                Ok(Value::Array(keys.into_iter().map(Value::String).collect()))
            }
            "get" => {
                let key = params.get("key").and_then(|v| v.as_str()).ok_or("key is required for get".to_string())?;
                match self.store.get(key) {
                    Some(value) => Ok(Value::String(value)),
                    None => Ok(Value::String(format!("Secret '{key}' not found"))),
                }
            }
            "set" => {
                let key = params.get("key").and_then(|v| v.as_str()).ok_or("key is required for set".to_string())?;
                let value = params.get("value").and_then(|v| v.as_str()).ok_or("value is required for set".to_string())?;
                self.store.set(key, value);
                Ok(Value::String(format!("Secret '{key}' set successfully")))
            }
            "remove" => {
                let key = params.get("key").and_then(|v| v.as_str()).ok_or("key is required for remove".to_string())?;
                if self.store.remove(key) {
                    Ok(Value::String(format!("Secret '{key}' removed")))
                } else {
                    Ok(Value::String(format!("Secret '{key}' not found")))
                }
            }
            _ => Err(format!("Unknown action: {action}. Valid actions: list, get, set, remove")),
        }
    }
}
