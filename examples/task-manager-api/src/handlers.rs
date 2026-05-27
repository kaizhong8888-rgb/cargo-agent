use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;

use crate::db;
use crate::errors::AppError;
use crate::models::{NewTask, TaskJson, TaskQuery, UpdateTask};

/// POST /api/tasks — create a new task
pub async fn create_task(
    State(pool): State<SqlitePool>,
    Json(payload): Json<NewTask>,
) -> Result<(StatusCode, Json<TaskJson>), AppError> {
    let task = db::insert_task(&pool, payload).await?;
    Ok((StatusCode::CREATED, Json(task)))
}

/// GET /api/tasks — list tasks with optional filtering & pagination
pub async fn list_tasks(
    State(pool): State<SqlitePool>,
    Query(q): Query<TaskQuery>,
) -> Result<Json<db::ListResponse>, AppError> {
    let response = db::list_tasks(&pool, &q).await?;
    Ok(Json(response))
}

/// GET /api/tasks/:id — get a single task
pub async fn get_task(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<TaskJson>, AppError> {
    let task = db::find_task(&pool, &id).await?;
    Ok(Json(task))
}

/// PATCH /api/tasks/:id — partially update a task
pub async fn update_task(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTask>,
) -> Result<Json<TaskJson>, AppError> {
    let task = db::update_task(&pool, &id, payload).await?;
    Ok(Json(task))
}

/// DELETE /api/tasks/:id — delete a task
pub async fn delete_task(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    db::delete_task(&pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
//  Health check
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// GET /health — simple liveness probe
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
