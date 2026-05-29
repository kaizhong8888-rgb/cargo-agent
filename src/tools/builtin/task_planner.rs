//! Task planner tool: decompose complex requests into trackable tasks,
//! manage dependencies, and track progress via the SQLite memory store.

use crate::memory::SqliteMemoryStore;
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Key prefix used in the memory store for task records.
const TASK_NS: &str = "task";

pub struct TaskPlannerTool {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for TaskPlannerTool {
    fn name(&self) -> &str {
        "task_planner"
    }

    fn description(&self) -> &str {
        "Decompose complex requests into trackable tasks. Actions: create \
         (add a new task or subtask), list (show all tasks), update (change \
         task status/details), show (show a single task), delete (remove a \
         task), decompose (break a request into multiple subtasks)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: create, list, update, show, delete, decompose".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "task_id".to_string(),
                description: "Task UUID (required for update, show, delete)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "title".to_string(),
                description: "Task title / summary".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Detailed task description".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "status".to_string(),
                description: "Task status: pending, in_progress, completed, blocked, failed"
                    .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "depends_on".to_string(),
                description: "Comma-separated task IDs this task depends on".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "parent_id".to_string(),
                description: "Parent task ID for subtasks".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "request".to_string(),
                description: "User request to decompose into tasks (for decompose action)"
                    .to_string(),
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
            "create" => self.create(params),
            "list" => self.list(params),
            "update" => self.update(params),
            "show" => self.show(params),
            "delete" => self.delete(params),
            "decompose" => self.decompose(params),
            other => Err(format!("Unknown action: {other}")),
        }
    }
}

impl TaskPlannerTool {
    fn task_key(id: &str) -> String {
        format!("task:{id}")
    }

    fn create(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: title")?;

        let id = Uuid::new_v4().to_string();
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let status = params
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        let parent_id = params
            .get("parent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let depends_on = params
            .get("depends_on")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let deps: Vec<String> = if depends_on.is_empty() {
            Vec::new()
        } else {
            depends_on
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        let now = now_iso8601();
        let record = serde_json::json!({
            "id": id,
            "title": title,
            "description": description,
            "status": status,
            "parent_id": if parent_id.is_empty() { Value::Null } else { Value::String(parent_id.to_string()) },
            "depends_on": deps,
            "created_at": now,
            "updated_at": now,
        });

        let value_str =
            serde_json::to_string(&record).map_err(|e| format!("Failed to serialize task: {e}"))?;

        self.memory
            .store(&Self::task_key(&id), &value_str, TASK_NS, &[], 5)
            .map_err(|e| format!("Failed to store task: {e}"))?;

        Ok(serde_json::json!({
            "status": "ok",
            "action": "create",
            "task": record,
        }))
    }

    fn list(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let status_filter = params.get("status").and_then(|v| v.as_str());
        let parent_filter = params.get("parent_id").and_then(|v| v.as_str());

        let all = self
            .memory
            .search(Some(TASK_NS), None, None, None, 1000)
            .unwrap_or_default();

        let mut tasks: Vec<Value> = all
            .iter()
            .filter_map(|m| serde_json::from_str::<Value>(&m.value).ok())
            .collect();

        if let Some(s) = status_filter {
            tasks.retain(|t| t["status"].as_str() == Some(s));
        }

        if let Some(p) = parent_filter {
            tasks.retain(|t| t["parent_id"].as_str() == Some(p));
        }

        // Sort: parent tasks first, then by created_at
        tasks.sort_by(|a, b| {
            let a_is_child = a["parent_id"].is_null();
            let b_is_child = b["parent_id"].is_null();
            a_is_child
                .cmp(&b_is_child)
                .then(b["created_at"].as_str().cmp(&a["created_at"].as_str()))
        });

        let summary = build_summary(&tasks);

        Ok(serde_json::json!({
            "status": "ok",
            "action": "list",
            "count": tasks.len(),
            "tasks": tasks,
            "summary": summary,
        }))
    }

    fn update(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: task_id")?;

        let key = Self::task_key(task_id);
        let existing = self
            .memory
            .search(Some(TASK_NS), None, Some(&key), None, 1)
            .map_err(|e| format!("Failed to search task: {e}"))?;

        if existing.is_empty() {
            return Err(format!("Task not found: {task_id}"));
        }

        let mut task: Value = serde_json::from_str(&existing[0].value)
            .map_err(|e| format!("Failed to parse task: {e}"))?;

        if let Some(s) = params.get("status").and_then(|v| v.as_str()) {
            task["status"] = s.into();
        }
        if let Some(t) = params.get("title").and_then(|v| v.as_str()) {
            task["title"] = t.into();
        }
        if let Some(d) = params.get("description").and_then(|v| v.as_str()) {
            task["description"] = d.into();
        }

        task["updated_at"] = now_iso8601().into();

        let value_str =
            serde_json::to_string(&task).map_err(|e| format!("Failed to serialize task: {e}"))?;

        self.memory
            .store(&key, &value_str, TASK_NS, &[], 5)
            .map_err(|e| format!("Failed to update task: {e}"))?;

        Ok(serde_json::json!({
            "status": "ok",
            "action": "update",
            "task": task,
        }))
    }

    fn show(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: task_id")?;

        let key = Self::task_key(task_id);
        let existing = self
            .memory
            .search(Some(TASK_NS), None, Some(&key), None, 1)
            .map_err(|e| format!("Failed to search task: {e}"))?;

        if existing.is_empty() {
            return Err(format!("Task not found: {task_id}"));
        }

        let task: Value = serde_json::from_str(&existing[0].value)
            .map_err(|e| format!("Failed to parse task: {e}"))?;

        Ok(serde_json::json!({
            "status": "ok",
            "action": "show",
            "task": task,
        }))
    }

    fn delete(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: task_id")?;

        let key = Self::task_key(task_id);
        self.memory
            .delete(&key)
            .map_err(|e| format!("Failed to delete task: {e}"))?;

        Ok(serde_json::json!({
            "status": "ok",
            "action": "delete",
            "task_id": task_id,
        }))
    }

    fn decompose(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let request = params
            .get("request")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: request")?;

        // The agent itself should do the decomposition — this action just
        // provides a structured way to create the parent + subtasks.
        // We create the parent task and return its ID for the agent to
        // then create subtasks via the create action.

        let id = Uuid::new_v4().to_string();
        let now = now_iso8601();

        let record = serde_json::json!({
            "id": id,
            "title": request.chars().take(80).collect::<String>(),
            "description": request,
            "status": "pending",
            "parent_id": null,
            "depends_on": [],
            "created_at": now,
            "updated_at": now,
            "is_epic": true,
        });

        let value_str =
            serde_json::to_string(&record).map_err(|e| format!("Failed to serialize task: {e}"))?;

        self.memory
            .store(&Self::task_key(&id), &value_str, TASK_NS, &[], 5)
            .map_err(|e| format!("Failed to store task: {e}"))?;

        Ok(serde_json::json!({
            "status": "ok",
            "action": "decompose",
            "epic_task_id": id,
            "message": "Parent task created. Use 'create' action with parent_id to add subtasks.",
            "task": record,
        }))
    }
}

fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn build_summary(tasks: &[Value]) -> Value {
    let mut total = 0;
    let mut pending = 0;
    let mut in_progress = 0;
    let mut completed = 0;
    let mut blocked = 0;
    let mut failed = 0;

    for t in tasks {
        total += 1;
        match t["status"].as_str() {
            Some("pending") => pending += 1,
            Some("in_progress") => in_progress += 1,
            Some("completed") => completed += 1,
            Some("blocked") => blocked += 1,
            Some("failed") => failed += 1,
            _ => {}
        }
    }

    serde_json::json!({
        "total": total,
        "pending": pending,
        "in_progress": in_progress,
        "completed": completed,
        "blocked": blocked,
        "failed": failed,
    })
}

pub fn register(registry: &mut ToolRegistry, memory: Arc<SqliteMemoryStore>) {
    registry.register(Box::new(TaskPlannerTool { memory }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_store() -> Arc<SqliteMemoryStore> {
        let path =
            PathBuf::from(std::env::temp_dir().join(format!("task_test_{}.db", Uuid::new_v4())));
        Arc::new(SqliteMemoryStore::open(path).expect("failed to open test store"))
    }

    #[test]
    fn test_create_and_show_task() {
        let store = test_store();
        let tool = TaskPlannerTool {
            memory: store.clone(),
        };

        let mut params = HashMap::new();
        params.insert("title".to_string(), Value::String("Test task".into()));
        params.insert("description".to_string(), Value::String("A test".into()));

        let result = tool.create(&params).unwrap();
        let id = result["task"]["id"].as_str().unwrap().to_string();

        let mut show_params = HashMap::new();
        show_params.insert("task_id".to_string(), Value::String(id));
        let show_result = tool.show(&show_params).unwrap();
        assert_eq!(show_result["task"]["title"].as_str().unwrap(), "Test task");
    }

    #[test]
    fn test_list_tasks_empty() {
        let store = test_store();
        let tool = TaskPlannerTool { memory: store };

        let params = HashMap::new();
        let result = tool.list(&params).unwrap();
        assert_eq!(result["count"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_list_tasks_with_filter() {
        let store = test_store();
        let tool = TaskPlannerTool {
            memory: store.clone(),
        };

        // Create two tasks with different statuses
        let mut p1 = HashMap::new();
        p1.insert("title".to_string(), Value::String("Task A".into()));
        p1.insert("status".to_string(), Value::String("pending".into()));
        tool.create(&p1).unwrap();

        let mut p2 = HashMap::new();
        p2.insert("title".to_string(), Value::String("Task B".into()));
        p2.insert("status".to_string(), Value::String("completed".into()));
        tool.create(&p2).unwrap();

        // List all
        let all = tool.list(&HashMap::new()).unwrap();
        assert_eq!(all["count"].as_u64().unwrap(), 2);

        // Filter by pending
        let mut filter = HashMap::new();
        filter.insert("status".to_string(), Value::String("pending".into()));
        let pending = tool.list(&filter).unwrap();
        assert_eq!(pending["count"].as_u64().unwrap(), 1);
    }

    #[test]
    fn test_update_task_status() {
        let store = test_store();
        let tool = TaskPlannerTool {
            memory: store.clone(),
        };

        let mut params = HashMap::new();
        params.insert("title".to_string(), Value::String("Updatable".into()));
        let result = tool.create(&params).unwrap();
        let id = result["task"]["id"].as_str().unwrap().to_string();

        let mut update = HashMap::new();
        update.insert("task_id".to_string(), Value::String(id.clone()));
        update.insert("status".to_string(), Value::String("in_progress".into()));
        tool.update(&update).unwrap();

        let mut show = HashMap::new();
        show.insert("task_id".to_string(), Value::String(id));
        let shown = tool.show(&show).unwrap();
        assert_eq!(shown["task"]["status"].as_str().unwrap(), "in_progress");
    }

    #[test]
    fn test_delete_task() {
        let store = test_store();
        let tool = TaskPlannerTool {
            memory: store.clone(),
        };

        let mut params = HashMap::new();
        params.insert("title".to_string(), Value::String("To delete".into()));
        let result = tool.create(&params).unwrap();
        let id = result["task"]["id"].as_str().unwrap().to_string();

        let mut del = HashMap::new();
        del.insert("task_id".to_string(), Value::String(id.clone()));
        tool.delete(&del).unwrap();

        let mut show = HashMap::new();
        show.insert("task_id".to_string(), Value::String(id));
        assert!(tool.show(&show).is_err());
    }

    #[test]
    fn test_decompose_creates_epic() {
        let store = test_store();
        let tool = TaskPlannerTool { memory: store };

        let mut params = HashMap::new();
        params.insert(
            "request".to_string(),
            Value::String("Build a REST API with auth".into()),
        );
        let result = tool.decompose(&params).unwrap();
        assert!(result["epic_task_id"].as_str().is_some());
        assert!(result["task"]["is_epic"].as_bool().unwrap());
    }
}
