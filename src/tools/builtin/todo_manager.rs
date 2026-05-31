//! Todo Manager: A personal todo list manager with priorities, categories,
//! tags, due dates, search, and statistics. Uses its own SQLite database.
//!
//! Database location: ~/.cargo-agent/todos.db

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

struct TodoManager {
    db: Mutex<rusqlite::Connection>,
}

impl TodoManager {
    fn new() -> Result<Self, String> {
        let db_path = Self::db_path()?;
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open todo database '{}': {}", db_path.display(), e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id              TEXT PRIMARY KEY,
                title           TEXT NOT NULL,
                description     TEXT DEFAULT '',
                status          TEXT DEFAULT 'pending',
                priority        TEXT DEFAULT 'medium',
                category        TEXT DEFAULT '',
                tags            TEXT DEFAULT '',
                due_date        TEXT,
                completed_at    TEXT,
                completion_note TEXT DEFAULT '',
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);
            CREATE INDEX IF NOT EXISTS idx_todos_priority ON todos(priority);
            CREATE INDEX IF NOT EXISTS idx_todos_category ON todos(category);
            CREATE INDEX IF NOT EXISTS idx_todos_due_date ON todos(due_date);",
        )
        .map_err(|e| format!("Failed to create todo table: {}", e))?;

        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    fn db_path() -> Result<PathBuf, String> {
        let agent_dir = crate::constants::AGENT_DIR.as_str();
        let path = PathBuf::from(agent_dir).join("todos.db");
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
        }
        Ok(path)
    }

    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[async_trait::async_trait]
impl Tool for TodoManager {
    fn name(&self) -> &str {
        "todo_manager"
    }

    fn description(&self) -> &str {
        "Personal todo list manager with priorities, categories, tags, due dates, \
         search/filter, and statistics. Actions: add, list, get, update, delete, \
         complete, pending, stats, archive, purge."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "Action: add, list, get, update, delete, complete, \
                              pending, stats, archive, purge".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "id".into(),
                description: "Todo ID (required for get, update, delete, complete, pending)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "title".into(),
                description: "Todo title (required for add)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "description".into(),
                description: "Detailed description".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "priority".into(),
                description: "Priority level: low, medium, high, urgent (default: medium)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "category".into(),
                description: "Category name for grouping".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "tags".into(),
                description: "Comma-separated tags for filtering".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "due_date".into(),
                description: "Due date (ISO 8601 format, e.g. '2025-12-31' or '2025-12-31T23:59:59Z')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "status".into(),
                description: "Filter by status for list action: pending, in_progress, completed, archived".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "search".into(),
                description: "Search text in title and description (for list action)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "completion_note".into(),
                description: "Note to record when completing a todo".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "sort_by".into(),
                description: "Sort field: created_at, updated_at, due_date, priority, title (default: created_at)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "sort_order".into(),
                description: "Sort order: asc, desc (default: desc)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "limit".into(),
                description: "Maximum number of todos to return (default: 50, max: 500)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: 'action'")?;

        let db = self.db.lock().map_err(|e| format!("Lock error: {}", e))?;

        match action {
            "add" => action_add(&db, params),
            "list" => action_list(&db, params),
            "get" => action_get(&db, params),
            "update" => action_update(&db, params),
            "delete" => action_delete(&db, params),
            "complete" => action_complete(&db, params),
            "pending" => action_pending(&db, params),
            "stats" => action_stats(&db),
            "archive" => action_archive(&db),
            "purge" => action_purge(&db),
            _ => Err(format!(
                "Unknown action: '{}'. Available: add, list, get, update, delete, \
                 complete, pending, stats, archive, purge",
                action
            )),
        }
    }
}

// ─── Action Implementations ────────────────────────────

fn action_add(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'title'")?;

    if title.trim().is_empty() {
        return Err("Title cannot be empty".to_string());
    }

    let id = TodoManager::generate_id();
    let now = TodoManager::now();
    let description = get_str(params, "description", "");
    let priority = validate_priority(get_str(params, "priority", "medium"))?;
    let category = get_str(params, "category", "");
    let tags = get_str(params, "tags", "");
    let due_date: Option<String> = params
        .get("due_date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    db.execute(
        "INSERT INTO todos (id, title, description, status, priority, category, tags, due_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![id, title, description, priority, category, tags, due_date, now, now],
    )
    .map_err(|e| format!("Failed to insert todo: {}", e))?;

    let todo = fetch_by_id(db, &id)?;
    Ok(serde_json::json!({
        "status": "ok",
        "action": "add",
        "todo": todo,
    }))
}

fn action_list(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let status_filter = params.get("status").and_then(|v| v.as_str());
    let category_filter = params.get("category").and_then(|v| v.as_str());
    let priority_filter = params.get("priority").and_then(|v| v.as_str());
    let search_text = params.get("search").and_then(|v| v.as_str());
    let sort_by = get_str(params, "sort_by", "created_at");
    let sort_order = get_str(params, "sort_order", "desc");
    let limit = get_int(params, "limit", 50).clamp(1, 500);

    // Build SQL query
    let (query, params_owned) = build_sql_query(
        db,
        status_filter,
        category_filter,
        priority_filter,
        search_text,
        sort_by,
        sort_order,
        limit,
    )?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_owned.iter().map(|b| b.as_ref()).collect();

    // Execute and map to JSON
    let todos = execute_and_map_todos(db, &query, &param_refs)?;
    let count = todos.len();

    // Compute stats
    let active_count = count_todos_by_status(db, "pending")
        + count_todos_by_status(db, "in_progress");
    let completed_count = count_todos_by_status(db, "completed");

    Ok(serde_json::json!({
        "status": "ok",
        "action": "list",
        "count": count,
        "active": active_count,
        "completed": completed_count,
        "todos": todos,
    }))
}

#[allow(clippy::too_many_arguments)]
/// Builds the final SQL query string and its parameter vector.
fn build_sql_query(
    _db: &rusqlite::Connection,
    status_filter: Option<&str>,
    category_filter: Option<&str>,
    priority_filter: Option<&str>,
    search_text: Option<&str>,
    sort_by: &str,
    sort_order: &str,
    limit: i64,
) -> Result<(String, Vec<Box<dyn rusqlite::types::ToSql>>), String> {
    // Validate sort_by
    let valid_sort_fields = [
        "created_at", "updated_at", "due_date", "priority", "title", "status",
    ];
    let sort_col = if valid_sort_fields.contains(&sort_by) {
        sort_by
    } else {
        "created_at"
    };

    // Validate sort_order
    let order = if sort_order == "asc" { "ASC" } else { "DESC" };

    // Priority ordering: urgent=0, high=1, medium=2, low=3
    let priority_order = "CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END";

    let sort_column = if sort_col == "priority" {
        priority_order
    } else {
        sort_col
    };

    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = status_filter {
        conditions.push(format!("status = ?{}", param_values.len() + 1));
        param_values.push(Box::new(s.to_string()));
    }
    if let Some(c) = category_filter {
        conditions.push(format!("category = ?{}", param_values.len() + 1));
        param_values.push(Box::new(c.to_string()));
    }
    if let Some(p) = priority_filter {
        if let Ok(vp) = validate_priority(p) {
            conditions.push(format!("priority = ?{}", param_values.len() + 1));
            param_values.push(Box::new(vp));
        }
    }
    if let Some(q) = search_text {
        let pattern = format!("%{}%", q);
        conditions.push(format!(
            "(title LIKE ?{} OR description LIKE ?{})",
            param_values.len() + 1,
            param_values.len() + 2
        ));
        param_values.push(Box::new(pattern.clone()));
        param_values.push(Box::new(pattern));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let query = format!(
        "SELECT id, title, description, status, priority, category, tags, due_date, \
         completed_at, completion_note, created_at, updated_at \
         FROM todos {} ORDER BY {} {} LIMIT ?{}",
        where_clause,
        sort_column,
        order,
        param_values.len() + 1
    );
    param_values.push(Box::new(limit));

    Ok((query, param_values))
}

/// Executes the prepared query and maps rows to JSON objects.
fn execute_and_map_todos(
    db: &rusqlite::Connection,
    query: &str,
    param_refs: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<Value>, String> {
    let mut stmt = db
        .prepare(query)
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let rows = stmt
        .query_map(param_refs, |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "status": row.get::<_, String>(3)?,
                "priority": row.get::<_, String>(4)?,
                "category": row.get::<_, String>(5)?,
                "tags": row.get::<_, String>(6)?,
                "due_date": row.get::<_, Option<String>>(7)?,
                "completed_at": row.get::<_, Option<String>>(8)?,
                "completion_note": row.get::<_, String>(9)?,
                "created_at": row.get::<_, String>(10)?,
                "updated_at": row.get::<_, String>(11)?,
            }))
        })
        .map_err(|e| format!("Failed to query todos: {}", e))?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn action_get(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'id'")?;

    let todo = fetch_by_id(db, id)?;
    Ok(serde_json::json!({
        "status": "ok",
        "action": "get",
        "todo": todo,
    }))
}

fn action_update(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'id'")?;

    // Verify todo exists
    let _existing = fetch_by_id(db, id)?;

    let now = TodoManager::now();
    let mut sets: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(v) = params.get("title").and_then(|v| v.as_str()) {
        if v.trim().is_empty() {
            return Err("Title cannot be empty".to_string());
        }
        sets.push(format!("title = ?{}", param_values.len() + 1));
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = params.get("description").and_then(|v| v.as_str()) {
        sets.push(format!("description = ?{}", param_values.len() + 1));
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = params.get("priority").and_then(|v| v.as_str()) {
        let p = validate_priority(v)?;
        sets.push(format!("priority = ?{}", param_values.len() + 1));
        param_values.push(Box::new(p));
    }
    if let Some(v) = params.get("category").and_then(|v| v.as_str()) {
        sets.push(format!("category = ?{}", param_values.len() + 1));
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = params.get("tags").and_then(|v| v.as_str()) {
        sets.push(format!("tags = ?{}", param_values.len() + 1));
        param_values.push(Box::new(v.to_string()));
    }
    if let Some(v) = params.get("due_date").and_then(|v| v.as_str()) {
        if v.is_empty() {
            sets.push("due_date = NULL".to_string());
        } else {
            sets.push(format!("due_date = ?{}", param_values.len() + 1));
            param_values.push(Box::new(v.to_string()));
        }
    }
    if let Some(v) = params.get("status").and_then(|v| v.as_str()) {
        let valid_statuses = ["pending", "in_progress", "completed", "archived"];
        if !valid_statuses.contains(&v) {
            return Err(format!(
                "Invalid status: '{}'. Valid: pending, in_progress, completed, archived",
                v
            ));
        }
        sets.push(format!("status = ?{}", param_values.len() + 1));
        param_values.push(Box::new(v.to_string()));

        // If marking as completed, set completed_at
        if v == "completed" {
            sets.push(format!("completed_at = ?{}", param_values.len() + 1));
            param_values.push(Box::new(now.clone()));
        }
        // If un-completing, clear completed_at
        if v != "completed" {
            sets.push("completed_at = NULL".to_string());
            sets.push("completion_note = ''".to_string());
        }
    }

    if sets.is_empty() {
        return Err("No fields to update. Provide at least one field: title, description, priority, category, tags, due_date, status".to_string());
    }

    sets.push(format!("updated_at = ?{}", param_values.len() + 1));
    param_values.push(Box::new(now));

    let query = format!(
        "UPDATE todos SET {} WHERE id = ?{}",
        sets.join(", "),
        param_values.len() + 1
    );
    param_values.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    db.execute(&query, param_refs.as_slice())
        .map_err(|e| format!("Failed to update todo: {}", e))?;

    let todo = fetch_by_id(db, id)?;
    Ok(serde_json::json!({
        "status": "ok",
        "action": "update",
        "todo": todo,
    }))
}

fn action_delete(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'id'")?;

    let affected = db
        .execute("DELETE FROM todos WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| format!("Failed to delete todo: {}", e))?;

    if affected == 0 {
        return Err(format!("Todo not found: {}", id));
    }

    Ok(serde_json::json!({
        "status": "ok",
        "action": "delete",
        "deleted_id": id,
    }))
}

fn action_complete(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'id'")?;

    let _existing = fetch_by_id(db, id)?;
    let now = TodoManager::now();
    let note = get_str(params, "completion_note", "");

    db.execute(
        "UPDATE todos SET status = 'completed', completed_at = ?1, completion_note = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![now, note, now, id],
    )
    .map_err(|e| format!("Failed to complete todo: {}", e))?;

    let todo = fetch_by_id(db, id)?;
    Ok(serde_json::json!({
        "status": "ok",
        "action": "complete",
        "todo": todo,
    }))
}

fn action_pending(db: &rusqlite::Connection, params: &HashMap<String, Value>) -> Result<Value, String> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: 'id'")?;

    let _existing = fetch_by_id(db, id)?;
    let now = TodoManager::now();

    db.execute(
        "UPDATE todos SET status = 'pending', completed_at = NULL, completion_note = '', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| format!("Failed to set todo as pending: {}", e))?;

    let todo = fetch_by_id(db, id)?;
    Ok(serde_json::json!({
        "status": "ok",
        "action": "pending",
        "todo": todo,
    }))
}

fn action_stats(db: &rusqlite::Connection) -> Result<Value, String> {
    let total: i64 = db
        .query_row("SELECT COUNT(*) FROM todos", [], |r| r.get(0))
        .unwrap_or(0);

    let by_status = count_by_group(db, "status");
    let by_priority = count_by_group(db, "priority");
    let by_category = count_by_group(db, "category");

    // Overdue count (due date is in the past and not completed)
    let now = chrono::Utc::now().to_rfc3339();
    let overdue: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM todos WHERE due_date IS NOT NULL AND due_date < ?1 AND status != 'completed' AND status != 'archived'",
            rusqlite::params![now],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Due today
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let due_today: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM todos WHERE due_date LIKE ?1 AND status != 'completed' AND status != 'archived'",
            rusqlite::params![format!("{}%", today)],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Completed this week
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let completed_week: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM todos WHERE status = 'completed' AND completed_at >= ?1",
            rusqlite::params![week_ago],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let urgent_count = by_priority.iter().find(|(k, _)| k == "urgent").map(|(_, c)| *c).unwrap_or(0);

    Ok(serde_json::json!({
        "status": "ok",
        "action": "stats",
        "total": total,
        "overdue": overdue,
        "due_today": due_today,
        "completed_this_week": completed_week,
        "by_status": serde_json::json!(by_status),
        "by_priority": serde_json::json!(by_priority),
        "by_category": serde_json::json!(by_category),
        "urgent_count": urgent_count,
    }))
}

fn action_archive(db: &rusqlite::Connection) -> Result<Value, String> {
    let now = TodoManager::now();
    let affected = db
        .execute(
            "UPDATE todos SET status = 'archived', updated_at = ?1 WHERE status = 'completed'",
            rusqlite::params![now],
        )
        .map_err(|e| format!("Failed to archive todos: {}", e))?;

    Ok(serde_json::json!({
        "status": "ok",
        "action": "archive",
        "archived_count": affected,
    }))
}

fn action_purge(db: &rusqlite::Connection) -> Result<Value, String> {
    let affected = db
        .execute("DELETE FROM todos WHERE status = 'archived'", [])
        .map_err(|e| format!("Failed to purge archived todos: {}", e))?;

    Ok(serde_json::json!({
        "status": "ok",
        "action": "purge",
        "deleted_count": affected,
    }))
}

// ─── Helpers ───────────────────────────────────────────

fn get_str<'a>(params: &'a HashMap<String, Value>, key: &str, default: &'a str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or(default)
}

fn get_int(params: &HashMap<String, Value>, key: &str, default: i64) -> i64 {
    params.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

fn validate_priority(p: &str) -> Result<String, String> {
    match p {
        "low" | "medium" | "high" | "urgent" => Ok(p.to_string()),
        _ => Err(format!(
            "Invalid priority: '{}'. Valid: low, medium, high, urgent",
            p
        )),
    }
}

fn fetch_by_id(db: &rusqlite::Connection, id: &str) -> Result<Value, String> {
    db.query_row(
        "SELECT id, title, description, status, priority, category, tags, due_date, \
         completed_at, completion_note, created_at, updated_at \
         FROM todos WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "status": row.get::<_, String>(3)?,
                "priority": row.get::<_, String>(4)?,
                "category": row.get::<_, String>(5)?,
                "tags": row.get::<_, String>(6)?,
                "due_date": row.get::<_, Option<String>>(7)?,
                "completed_at": row.get::<_, Option<String>>(8)?,
                "completion_note": row.get::<_, String>(9)?,
                "created_at": row.get::<_, String>(10)?,
                "updated_at": row.get::<_, String>(11)?,
            }))
        },
    )
    .map_err(|e| format!("Todo not found (id={}): {}", id, e))
}

fn count_todos_by_status(db: &rusqlite::Connection, status: &str) -> i64 {
    db.query_row(
        "SELECT COUNT(*) FROM todos WHERE status = ?1",
        rusqlite::params![status],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn count_by_group(db: &rusqlite::Connection, column: &str) -> Vec<(String, i64)> {
    let query = format!(
        "SELECT {}, COUNT(*) as cnt FROM todos GROUP BY {} ORDER BY cnt DESC",
        column, column
    );
    let mut stmt = match db.prepare(&query) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((key, count))
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn register_all(registry: &mut ToolRegistry) {
    match TodoManager::new() {
        Ok(tool) => registry.register(Box::new(tool)),
        Err(e) => {
            tracing::error!("Failed to initialize TodoManager: {}", e);
            // Register a fallback that returns error messages
            registry.register(Box::new(FallbackTodoManager));
        }
    }
}

/// Fallback tool that returns initialization errors
struct FallbackTodoManager;

#[async_trait::async_trait]
impl Tool for FallbackTodoManager {
    fn name(&self) -> &str {
        "todo_manager"
    }
    fn description(&self) -> &str {
        "Todo manager (currently unavailable due to initialization error)"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "action".into(),
            description: "Action to perform".into(),
            required: true,
            parameter_type: "string".into(),
        }]
    }
    async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
        Err("TodoManager failed to initialize. Check logs for details.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_db() -> rusqlite::Connection {
        let path = PathBuf::from(std::env::temp_dir())
            .join(format!("todo_test_{}.db", uuid::Uuid::new_v4()));
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT DEFAULT '',
                status TEXT DEFAULT 'pending',
                priority TEXT DEFAULT 'medium',
                category TEXT DEFAULT '',
                tags TEXT DEFAULT '',
                due_date TEXT,
                completed_at TEXT,
                completion_note TEXT DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_test_todo(conn: &rusqlite::Connection, title: &str, priority: &str, category: &str, status: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO todos (id, title, description, status, priority, category, tags, created_at, updated_at)
             VALUES (?1, ?2, '', ?3, ?4, ?5, '', ?6, ?6)",
            rusqlite::params![id, title, status, priority, category, now],
        )
        .unwrap();
        id
    }

    // ─── Add ───────────────────────────────────────────────

    #[test]
    fn test_add_basic() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("title".into(), Value::String("Test todo".into()));
        let result = action_add(&conn, &params).unwrap();
        assert_eq!(result["action"], "add");
        assert_eq!(result["todo"]["title"], "Test todo");
        assert_eq!(result["todo"]["status"], "pending");
        assert_eq!(result["todo"]["priority"], "medium");
    }

    #[test]
    fn test_add_empty_title() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("title".into(), Value::String("".into()));
        let result = action_add(&conn, &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_with_all_fields() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("title".into(), Value::String("Full todo".into()));
        params.insert("description".into(), Value::String("A detailed description".into()));
        params.insert("priority".into(), Value::String("high".into()));
        params.insert("category".into(), Value::String("work".into()));
        params.insert("tags".into(), Value::String("rust,backend,api".into()));
        params.insert("due_date".into(), Value::String("2025-12-31".into()));
        let result = action_add(&conn, &params).unwrap();
        assert_eq!(result["todo"]["priority"], "high");
        assert_eq!(result["todo"]["category"], "work");
        assert_eq!(result["todo"]["tags"], "rust,backend,api");
        assert_eq!(result["todo"]["due_date"], Value::String("2025-12-31".into()));
    }

    #[test]
    fn test_add_invalid_priority() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("title".into(), Value::String("Bad priority".into()));
        params.insert("priority".into(), Value::String("critical".into()));
        let result = action_add(&conn, &params);
        assert!(result.is_err());
    }

    // ─── List ──────────────────────────────────────────────

    #[test]
    fn test_list_empty() {
        let conn = create_test_db();
        let params = HashMap::new();
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 0);
    }

    #[test]
    fn test_list_all() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Task A", "high", "work", "pending");
        insert_test_todo(&conn, "Task B", "low", "personal", "completed");
        let params = HashMap::new();
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 2);
    }

    #[test]
    fn test_list_filter_by_status() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Task A", "medium", "work", "pending");
        insert_test_todo(&conn, "Task B", "medium", "work", "completed");
        insert_test_todo(&conn, "Task C", "medium", "work", "pending");

        let mut params = HashMap::new();
        params.insert("status".into(), Value::String("pending".into()));
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 2);
    }

    #[test]
    fn test_list_filter_by_category() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Task A", "medium", "work", "pending");
        insert_test_todo(&conn, "Task B", "medium", "personal", "pending");

        let mut params = HashMap::new();
        params.insert("category".into(), Value::String("work".into()));
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 1);
    }

    #[test]
    fn test_list_search() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Buy groceries", "medium", "personal", "pending");
        insert_test_todo(&conn, "Fix bug in login", "high", "work", "pending");

        let mut params = HashMap::new();
        params.insert("search".into(), Value::String("groceries".into()));
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 1);
    }

    #[test]
    fn test_list_limit() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Task A", "medium", "", "pending");
        insert_test_todo(&conn, "Task B", "medium", "", "pending");
        insert_test_todo(&conn, "Task C", "medium", "", "pending");

        let mut params = HashMap::new();
        params.insert("limit".into(), Value::Number(2.into()));
        let result = action_list(&conn, &params).unwrap();
        assert_eq!(result["count"], 2);
    }

    // ─── Get ───────────────────────────────────────────────

    #[test]
    fn test_get_existing() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "My Todo", "medium", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id));
        let result = action_get(&conn, &params).unwrap();
        assert_eq!(result["todo"]["title"], "My Todo");
    }

    #[test]
    fn test_get_not_found() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("id".into(), Value::String("nonexistent".into()));
        let result = action_get(&conn, &params);
        assert!(result.is_err());
    }

    // ─── Update ────────────────────────────────────────────

    #[test]
    fn test_update_title() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Old Title", "medium", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id.clone()));
        params.insert("title".into(), Value::String("New Title".into()));
        let result = action_update(&conn, &params).unwrap();
        assert_eq!(result["todo"]["title"], "New Title");
    }

    #[test]
    fn test_update_multiple_fields() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Old", "low", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id.clone()));
        params.insert("priority".into(), Value::String("urgent".into()));
        params.insert("category".into(), Value::String("critical".into()));
        params.insert("tags".into(), Value::String("urgent,important".into()));
        let result = action_update(&conn, &params).unwrap();
        assert_eq!(result["todo"]["priority"], "urgent");
        assert_eq!(result["todo"]["category"], "critical");
        assert_eq!(result["todo"]["tags"], "urgent,important");
    }

    #[test]
    fn test_update_no_fields() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Test", "medium", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id));
        let result = action_update(&conn, &params);
        assert!(result.is_err());
    }

    // ─── Delete ────────────────────────────────────────────

    #[test]
    fn test_delete_existing() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Delete me", "medium", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id.clone()));
        let result = action_delete(&conn, &params).unwrap();
        assert_eq!(result["deleted_id"], id);

        // Verify it's gone
        let mut get_params = HashMap::new();
        get_params.insert("id".into(), Value::String(id));
        assert!(action_get(&conn, &get_params).is_err());
    }

    #[test]
    fn test_delete_not_found() {
        let conn = create_test_db();
        let mut params = HashMap::new();
        params.insert("id".into(), Value::String("nonexistent".into()));
        let result = action_delete(&conn, &params);
        assert!(result.is_err());
    }

    // ─── Complete / Pending ────────────────────────────────

    #[test]
    fn test_complete_todo() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Do something", "high", "", "pending");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id.clone()));
        params.insert("completion_note".into(), Value::String("All done!".into()));
        let result = action_complete(&conn, &params).unwrap();
        assert_eq!(result["todo"]["status"], "completed");
        assert_eq!(result["todo"]["completion_note"], "All done!");
        assert!(result["todo"]["completed_at"].is_string());
    }

    #[test]
    fn test_pending_todo() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "Do something", "high", "", "completed");

        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id.clone()));
        let result = action_pending(&conn, &params).unwrap();
        assert_eq!(result["todo"]["status"], "pending");
        assert!(result["todo"]["completed_at"].is_null());
    }

    // ─── Stats ─────────────────────────────────────────────

    #[test]
    fn test_stats_empty() {
        let conn = create_test_db();
        let result = action_stats(&conn).unwrap();
        assert_eq!(result["total"], 0);
    }

    #[test]
    fn test_stats_with_data() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Task A", "high", "work", "pending");
        insert_test_todo(&conn, "Task B", "low", "personal", "completed");
        insert_test_todo(&conn, "Task C", "urgent", "work", "pending");

        let result = action_stats(&conn).unwrap();
        assert_eq!(result["total"], 3);
        assert_eq!(result["urgent_count"], 1);
    }

    // ─── Archive / Purge ───────────────────────────────────

    #[test]
    fn test_archive_completed() {
        let conn = create_test_db();
        insert_test_todo(&conn, "Done 1", "medium", "", "completed");
        insert_test_todo(&conn, "Done 2", "medium", "", "completed");
        insert_test_todo(&conn, "Still active", "medium", "", "pending");

        let result = action_archive(&conn).unwrap();
        assert_eq!(result["archived_count"], 2);
    }

    #[test]
    fn test_purge_archived() {
        let conn = create_test_db();
        let id = insert_test_todo(&conn, "To purge", "medium", "", "archived");
        insert_test_todo(&conn, "Active", "medium", "", "pending");

        let result = action_purge(&conn).unwrap();
        assert_eq!(result["deleted_count"], 1);

        // Archived todo should be gone
        let mut params = HashMap::new();
        params.insert("id".into(), Value::String(id));
        assert!(action_get(&conn, &params).is_err());
    }

    // ─── Priority validation ───────────────────────────────

    #[test]
    fn test_validate_priority_valid() {
        assert_eq!(validate_priority("low").unwrap(), "low");
        assert_eq!(validate_priority("medium").unwrap(), "medium");
        assert_eq!(validate_priority("high").unwrap(), "high");
        assert_eq!(validate_priority("urgent").unwrap(), "urgent");
    }

    #[test]
    fn test_validate_priority_invalid() {
        assert!(validate_priority("critical").is_err());
        assert!(validate_priority("").is_err());
    }

    // ─── Integration: todo_manager tool ────────────────────

    #[tokio::test]
    async fn test_tool_metadata() {
        // Test that the tool name and description are correct via the trait
        // We use the FallbackTodoManager for this test since it's always available
        let tool = FallbackTodoManager;
        assert_eq!(tool.name(), "todo_manager");
        assert!(!tool.description().is_empty());
        assert!(!tool.parameters().is_empty());

        let params = tool.parameters();
        let action_param = params.iter().find(|p| p.name == "action");
        assert!(action_param.is_some());
        assert!(action_param.unwrap().required);
    }

    #[tokio::test]
    async fn test_fallback_error() {
        let tool = FallbackTodoManager;
        let params = HashMap::new();
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to initialize"));
    }
}
