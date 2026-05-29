//! Scheduled task tool: set up periodic background tasks.
//!
//! Allows the agent to schedule recurring checks like dependency updates,
//! test runs, or health checks. Tasks are stored in a JSON file and
//! can be listed, cancelled, or triggered on demand.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const SCHEDULES_FILE_NAME: &str = "schedules.json";

// ============================================================================
// Data types
// ============================================================================

#[derive(Serialize, Deserialize, Clone)]
struct Schedule {
    id: String,
    description: String,
    command: String,
    interval_secs: u64,
    working_dir: String,
    created_at: String,
    last_run: Option<String>,
    enabled: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct ScheduleStore {
    schedules: Vec<Schedule>,
}

impl ScheduleStore {
    fn load() -> Self {
        let path = schedules_file_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let path = schedules_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            &path,
            serde_json::to_string_pretty(self).unwrap_or_default(),
        );
    }

    fn next_id(&self) -> String {
        let max = self
            .schedules
            .iter()
            .filter_map(|s| {
                s.id.strip_prefix("sched-")
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0);
        format!("sched-{}", max + 1)
    }
}

fn schedules_file_path() -> PathBuf {
    let dir = crate::constants::agent_dir_path();
    dir.join(SCHEDULES_FILE_NAME)
}

// ============================================================================
// SchedulerTool
// ============================================================================

pub struct SchedulerTool;

#[async_trait::async_trait]
impl Tool for SchedulerTool {
    fn name(&self) -> &str {
        "scheduler"
    }

    fn description(&self) -> &str {
        "Manage scheduled/recurring tasks. Actions: create (new recurring task), list (show all schedules), cancel (disable a schedule), enable (re-enable), delete (remove), run_now (trigger immediately). Useful for periodic dependency checks, test runs, or health monitoring."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: create, list, cancel, enable, delete, run_now".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "schedule_id".to_string(),
                description: "Schedule ID (used with cancel, enable, delete, run_now actions)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Human-readable description of the task (used with create)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "command".to_string(),
                description: "Shell command to execute (used with create, e.g. 'cargo test' or 'cargo audit')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "interval".to_string(),
                description: "Interval in seconds between runs (used with create, e.g. 3600 for hourly)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "working_dir".to_string(),
                description: "Working directory for the command (default: current directory)".to_string(),
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

        let mut store = ScheduleStore::load();

        match action {
            "create" => {
                let description = params
                    .get("description")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: description (for create action)")?;

                let command = params
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: command (for create action)")?;

                let interval_secs = params
                    .get("interval")
                    .and_then(|v| v.as_u64())
                    .ok_or("Missing required parameter: interval (for create action)")?;

                let working_dir = params
                    .get("working_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();

                let id = store.next_id();
                let now = chrono::Utc::now().to_rfc3339();

                let schedule = Schedule {
                    id: id.clone(),
                    description: description.to_string(),
                    command: command.to_string(),
                    interval_secs,
                    working_dir,
                    created_at: now.clone(),
                    last_run: None,
                    enabled: true,
                };

                store.schedules.push(schedule);
                store.save();

                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "create",
                    "schedule_id": id,
                    "description": description,
                    "command": command,
                    "interval_secs": interval_secs,
                    "created_at": now,
                    "note": "Schedule created and saved. Note: actual execution requires an external scheduler daemon to process the schedules file.",
                }))
            }
            "list" => {
                let schedules: Vec<Value> = store
                    .schedules
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "description": s.description,
                            "command": s.command,
                            "interval_secs": s.interval_secs,
                            "working_dir": s.working_dir,
                            "enabled": s.enabled,
                            "created_at": s.created_at,
                            "last_run": s.last_run,
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "status": "ok",
                    "action": "list",
                    "schedules": schedules,
                    "count": schedules.len(),
                    "enabled_count": store.schedules.iter().filter(|s| s.enabled).count(),
                }))
            }
            "cancel" => {
                let id = params
                    .get("schedule_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: schedule_id (for cancel action)")?;

                if let Some(schedule) = store.schedules.iter_mut().find(|s| s.id == id) {
                    schedule.enabled = false;
                    store.save();
                    Ok(serde_json::json!({
                        "status": "ok",
                        "action": "cancel",
                        "schedule_id": id,
                        "message": "Schedule disabled",
                    }))
                } else {
                    Ok(serde_json::json!({
                        "status": "not_found",
                        "action": "cancel",
                        "schedule_id": id,
                    }))
                }
            }
            "enable" => {
                let id = params
                    .get("schedule_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: schedule_id (for enable action)")?;

                if let Some(schedule) = store.schedules.iter_mut().find(|s| s.id == id) {
                    schedule.enabled = true;
                    store.save();
                    Ok(serde_json::json!({
                        "status": "ok",
                        "action": "enable",
                        "schedule_id": id,
                        "message": "Schedule re-enabled",
                    }))
                } else {
                    Ok(serde_json::json!({
                        "status": "not_found",
                        "action": "enable",
                        "schedule_id": id,
                    }))
                }
            }
            "delete" => {
                let id = params
                    .get("schedule_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: schedule_id (for delete action)")?;

                let before = store.schedules.len();
                store.schedules.retain(|s| s.id != id);
                if store.schedules.len() < before {
                    store.save();
                }

                Ok(serde_json::json!({
                    "status": if store.schedules.len() < before { "ok" } else { "not_found" },
                    "action": "delete",
                    "schedule_id": id,
                    "deleted": store.schedules.len() < before,
                }))
            }
            "run_now" => {
                let id = params
                    .get("schedule_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: schedule_id (for run_now action)")?;

                if let Some(schedule) = store.schedules.iter_mut().find(|s| s.id == id) {
                    // Execute the command
                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&schedule.command)
                        .current_dir(&schedule.working_dir)
                        .output();

                    let now = chrono::Utc::now().to_rfc3339();
                    schedule.last_run = Some(now.clone());
                    store.save();

                    match output {
                        Ok(out) => {
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            Ok(serde_json::json!({
                                "status": if out.status.success() { "ok" } else { "error" },
                                "action": "run_now",
                                "schedule_id": id,
                                "exit_code": out.status.code(),
                                "stdout": stdout.chars().take(5000).collect::<String>(),
                                "stderr": stderr.chars().take(5000).collect::<String>(),
                                "executed_at": now,
                            }))
                        }
                        Err(e) => Ok(serde_json::json!({
                            "status": "error",
                            "action": "run_now",
                            "schedule_id": id,
                            "error": format!("Failed to execute command: {e}"),
                            "executed_at": now,
                        })),
                    }
                } else {
                    Ok(serde_json::json!({
                        "status": "not_found",
                        "action": "run_now",
                        "schedule_id": id,
                    }))
                }
            }
            other => Err(format!(
                "Unknown action: {other}. Supported: create, list, cancel, enable, delete, run_now"
            )),
        }
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(SchedulerTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_tool_metadata() {
        let tool = SchedulerTool;
        assert_eq!(tool.name(), "scheduler");
        assert!(tool.description().contains("scheduled"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action" && p.required));
        assert!(params.iter().any(|p| p.name == "schedule_id"));
    }

    #[test]
    fn schedule_store_next_id() {
        let store = ScheduleStore::default();
        assert_eq!(store.next_id(), "sched-1");
    }

    #[test]
    fn schedule_store_next_id_after_existing() {
        let mut store = ScheduleStore::default();
        store.schedules.push(Schedule {
            id: "sched-5".to_string(),
            description: "".to_string(),
            command: "".to_string(),
            interval_secs: 60,
            working_dir: ".".to_string(),
            created_at: "".to_string(),
            last_run: None,
            enabled: true,
        });
        assert_eq!(store.next_id(), "sched-6");
    }
}
