// Product API routes (for future API usage, currently served via page templates)
// The main product routes are in page.rs

use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

// Product API - get single product as JSON (for AJAX/future API)
pub async fn api_get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let product: Option<crate::models::Product> = sqlx::query_as(
        r#"SELECT p.*, c.name as category_name, c.slug as category_slug
           FROM products p
           LEFT JOIN categories c ON p.category_id = c.id
           WHERE p.id = ?"#
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    match product {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (StatusCode::NOT_FOUND, Json::<serde_json::Value>(serde_json::json!({
            "error": "Product not found"
        }))).into_response(),
    }
}
