use crate::models::*;
use crate::AppState;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "cart/cart.html")]
pub struct CartPageTemplate {
    pub lang: String,
    pub cart_items: Vec<CartItemDetail>,
    pub total_quantity: i64,
    pub cart_total: f64,
}

// ---------------------------------------------------------------------------
// Page handlers
// ---------------------------------------------------------------------------

pub async fn cart_page(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    // For now, show an empty cart view. Real cart requires auth middleware.
    CartPageTemplate {
        lang: "zh".to_string(),
        cart_items: vec![],
        total_quantity: 0,
        cart_total: 0.0,
    }
}

// ---------------------------------------------------------------------------
// API handlers
// ---------------------------------------------------------------------------

pub async fn add_to_cart(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Json(req): Json<AddToCartRequest>,
) -> Result<StatusCode, StatusCode> {
    let product = sqlx::query!(
        "SELECT base_price, min_quantity FROM products WHERE id = ?",
        req.product_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if req.quantity < product.min_quantity {
        return Err(StatusCode::BAD_REQUEST);
    }

    sqlx::query!(
        "INSERT INTO cart_items (user_id, product_id, material, size_width, size_height, quantity, unit_price)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, product_id, material, size_width, size_height)
         DO UPDATE SET quantity = quantity + excluded.quantity, updated_at = CURRENT_TIMESTAMP",
        claims.0.sub,
        req.product_id,
        req.material,
        req.width,
        req.height,
        req.quantity,
        product.base_price
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

pub async fn update_quantity(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Path(item_id): Path<i64>,
    Json(req): Json<UpdateQuantityRequest>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "UPDATE cart_items SET quantity = ? WHERE id = ? AND user_id = ?",
        req.quantity,
        item_id,
        claims.0.sub
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

pub async fn remove_item(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "DELETE FROM cart_items WHERE id = ? AND user_id = ?",
        item_id,
        claims.0.sub
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Request/Response types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct AddToCartRequest {
    pub product_id: i64,
    pub material: String,
    pub width: f64,
    pub height: f64,
    pub quantity: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateQuantityRequest {
    pub quantity: i64,
}

// ---------------------------------------------------------------------------
// CartItemDetail for template rendering
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CartItemDetail {
    pub id: i64,
    pub product: crate::models::Product,
    pub material: String,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub quantity: i64,
    pub unit_price: f64,
    pub subtotal: f64,
    pub finishing: Option<String>,
}
