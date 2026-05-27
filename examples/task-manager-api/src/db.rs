use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::models::*;
use serde::Serialize;

/// Run embedded migrations so the app works out of the box without an
/// external migration tool.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::raw_sql(include_str!("../migrations/001_create_tasks.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

/// Create a new pool and apply migrations.
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, AppError> {
    let pool = SqlitePool::connect(database_url).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

// ---------------------------------------------------------------------------
//  CRUD operations
// ---------------------------------------------------------------------------

pub async fn insert_task(pool: &SqlitePool, new: NewTask) -> Result<TaskJson, AppError> {
    let id = generate_id();
    let now = now_iso();
    let title = new.title.trim().to_owned();
    let description = new.description.unwrap_or_default();
    let priority = new.priority.unwrap_or(3).clamp(1, 5);

    if title.is_empty() {
        return Err(AppError::BadRequest("Title must not be empty".into()));
    }

    sqlx::query(
        "INSERT INTO tasks (id, title, description, status, priority, created_at, updated_at)
         VALUES (?, ?, ?, 'todo', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&title)
    .bind(&description)
    .bind(priority)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await?;

    Ok(task.into_json())
}

pub async fn find_task(pool: &SqlitePool, id: &str) -> Result<TaskJson, AppError> {
    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task {id} not found")))?;

    Ok(task.into_json())
}

pub async fn list_tasks(pool: &SqlitePool, q: &TaskQuery) -> Result<ListResponse, AppError> {
    let page = q.page();
    let per_page = q.per_page();
    let offset = q.offset();

    // Build dynamic query
    let mut where_clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(status) = &q.status {
        where_clauses.push(format!("status = ?{}", params.len() + 1));
        params.push(status.as_str().to_owned());
    }
    if let Some(min) = &q.priority_min {
        where_clauses.push(format!("priority >= ?{}", params.len() + 1));
        params.push(min.to_string());
    }
    if let Some(max) = &q.priority_max {
        where_clauses.push(format!("priority <= ?{}", params.len() + 1));
        params.push(max.to_string());
    }
    if let Some(search) = &q.search {
        where_clauses.push(format!(
            "(title LIKE ?{} OR description LIKE ?{})",
            params.len() + 1,
            params.len() + 2
        ));
        let pattern = format!("%{search}%");
        params.push(pattern.clone());
        params.push(pattern);
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Sorting
    let sort_column = match q.sort_by.as_deref() {
        Some("priority") => "priority",
        Some("status") => "status",
        Some("created_at") => "created_at",
        Some("updated_at") => "updated_at",
        _ => "created_at",
    };
    let sort_order = match q.sort_order.as_deref() {
        Some("asc") | Some("ASC") => "ASC",
        _ => "DESC",
    };

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM tasks {where_sql}");
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    for p in &params {
        count_query = count_query.bind(p);
    }
    let total: i64 = count_query.fetch_one(pool).await?;

    // Data
    let data_sql = format!(
        "SELECT * FROM tasks {where_sql} ORDER BY {sort_column} {sort_order} LIMIT ?{limit_idx} OFFSET ?{offset_idx}",
        limit_idx = params.len() + 1,
        offset_idx = params.len() + 2,
    );
    let mut data_query = sqlx::query_as::<_, Task>(&data_sql);
    for p in &params {
        data_query = data_query.bind(p);
    }
    data_query = data_query.bind(per_page as i64);
    data_query = data_query.bind(offset as i64);
    let rows: Vec<Task> = data_query.fetch_all(pool).await?;

    let tasks: Vec<TaskJson> = rows.into_iter().map(|r| r.into_json()).collect();
    let total_pages = (total as f64 / per_page as f64).ceil() as u32;

    Ok(ListResponse {
        data: tasks,
        page,
        per_page,
        total: total as u64,
        total_pages,
    })
}

pub async fn update_task(
    pool: &SqlitePool,
    id: &str,
    update: UpdateTask,
) -> Result<TaskJson, AppError> {
    if update.is_empty() {
        return Err(AppError::BadRequest("No fields to update".into()));
    }

    // Fetch existing to make sure it exists and to get current values
    let existing = find_task(pool, id).await?;

    let title = update.title.unwrap_or(existing.title.clone());
    if title.trim().is_empty() {
        return Err(AppError::BadRequest("Title must not be empty".into()));
    }

    let description = update.description.unwrap_or(existing.description.clone());
    let status = update
        .status
        .unwrap_or(TaskStatus::from_str(&existing.status.as_str()).unwrap_or(TaskStatus::Todo));
    let priority = update.priority.unwrap_or(existing.priority).clamp(1, 5);
    let now = now_iso();

    sqlx::query(
        "UPDATE tasks SET title = ?, description = ?, status = ?, priority = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(title.trim())
    .bind(&description)
    .bind(status.as_str())
    .bind(priority)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;

    find_task(pool, id).await
}

pub async fn delete_task(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Task {id} not found")));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
//  Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub data: Vec<TaskJson>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}
