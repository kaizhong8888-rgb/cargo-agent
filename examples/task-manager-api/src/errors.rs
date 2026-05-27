use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Unified application error type.
///
/// Every DB or domain error is mapped into an HTTP response so that the
/// caller always gets a consistent JSON error body.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ── 4xx ──────────────────────────────────────────────────────────
    #[error("Task not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unprocessable entity: {0}")]
    Unprocessable(String),

    // ── 5xx ──────────────────────────────────────────────────────────
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
//  axum integration – every AppError becomes a JSON response
// ---------------------------------------------------------------------------

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::Unprocessable(msg) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "unprocessable", msg.clone())
            }
            AppError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "A database error occurred".into(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred".into(),
                )
            }
        };

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}
