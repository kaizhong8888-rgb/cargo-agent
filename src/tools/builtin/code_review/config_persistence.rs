//! Config persistence — save/load/list/delete review parameter profiles.

use crate::tools::builtin::config_store::ConfigStore;
use serde_json::Value;
use std::collections::HashMap;

/// Namespace prefix for saved code_review config profiles in the config store.
const CONFIG_NAMESPACE: &str = "code_review_config:";

/// Parameters that can be saved/loaded as config profiles.
fn configurable_param_names() -> &'static [&'static str] {
    &[
        "recursive", "checks", "format", "min_severity",
        "max_fn_length", "max_nesting", "max_line_length", "parallel",
    ]
}

/// Merge CLI params with loaded config profiles.
/// Returns a complete response for config operations (list/delete),
/// or merged parameters (as a JSON object) for normal analysis.
pub(super) fn merge_config_params(params: &HashMap<String, Value>) -> Result<Value, String> {
    let save_config = params.get("save_config").and_then(|v| v.as_str());
    let load_config = params.get("load_config").and_then(|v| v.as_str());
    let list_configs = params.get("list_configs").and_then(|v| v.as_bool()).unwrap_or(false);
    let delete_config = params.get("delete_config").and_then(|v| v.as_str());

    if list_configs { return list_saved_configs(); }
    if let Some(name) = delete_config { return delete_saved_config(name); }

    if let Some(config_name) = load_config {
        let store = ConfigStore::load();
        let key = format!("{CONFIG_NAMESPACE}{config_name}");
        match store.get(&key) {
            Some(config_obj) => {
                if let Some(obj) = config_obj.as_object() {
                    let mut merged = serde_json::Map::new();
                    for (k, v) in obj { merged.insert(k.clone(), v.clone()); }
                    for (k, v) in params { merged.insert(k.clone(), v.clone()); }
                    return Ok(Value::Object(merged));
                } else {
                    return Ok(serde_json::json!({
                        "status": "error",
                        "message": format!("Config profile '{config_name}' is corrupted (not an object)."),
                    }));
                }
            }
            None => {
                return Ok(serde_json::json!({
                    "status": "error",
                    "message": format!("Config profile '{config_name}' not found. Use --save_config '{config_name}' to create it."),
                }));
            }
        }
    }

    if let Some(config_name) = save_config {
        let store = ConfigStore::load();
        let key = format!("{CONFIG_NAMESPACE}{config_name}");
        let mut config = serde_json::Map::new();
        for param_name in configurable_param_names() {
            if let Some(val) = params.get(*param_name) {
                config.insert(param_name.to_string(), val.clone());
            }
        }
        store.set(&key, Value::Object(config));
    }

    // Return merged params as a JSON object (no "status" field, so execute continues)
    let obj: serde_json::Map<String, Value> = params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Ok(Value::Object(obj))
}

/// List all saved config profiles.
pub(super) fn list_saved_configs() -> Result<Value, String> {
    let store = ConfigStore::load();
    let all_prefs = store.list();
    let mut profiles: Vec<Value> = Vec::new();
    for (key, value) in &all_prefs {
        if let Some(name) = key.strip_prefix(CONFIG_NAMESPACE) {
            if let Some(obj) = value.as_object() {
                let settings: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                profiles.push(serde_json::json!({
                    "name": name,
                    "settings": settings,
                }));
            }
        }
    }
    Ok(serde_json::json!({
        "status": "ok",
        "configs": profiles,
        "count": profiles.len(),
    }))
}

/// Delete a saved config profile by name.
pub(super) fn delete_saved_config(name: &str) -> Result<Value, String> {
    let store = ConfigStore::load();
    let key = format!("{CONFIG_NAMESPACE}{name}");
    let existing = store.get(&key);
    if existing.is_none() {
        return Ok(serde_json::json!({
            "status": "error",
            "message": format!("Config profile '{name}' not found. Use --list_configs to see available profiles."),
        }));
    }
    store.delete(&key);
    Ok(serde_json::json!({
        "status": "ok",
        "action": "deleted",
        "config": name,
    }))
}
