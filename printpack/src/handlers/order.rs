use crate::models::*;
use crate::AppState;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "orders/list.html")]
pub struct OrdersPageTemplate {
    pub lang: String,
    pub orders: Vec<OrderSummary>,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Template)]
#[template(path = "orders/detail.html")]
pub struct OrderDetailPageTemplate {
    pub lang: String,
    pub order: Order,
    pub items: Vec<OrderItem>,
}

#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutPageTemplate {
    pub lang: String,
    pub cart_items: Vec<CartItemSummary>,
    pub subtotal: f64,
    pub shipping: f64,
    pub total: f64,
}

// ---------------------------------------------------------------------------
// Page handlers
// ---------------------------------------------------------------------------

pub async fn list_orders(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let page: i64 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let per_page = 10;
    let offset = (page - 1) * per_page;

    // Without auth, show empty orders
    let orders: Vec<OrderSummary> = vec![];

    OrdersPageTemplate {
        lang: "zh".to_string(),
        orders,
        current_page: page,
        total_pages: 1,
    }
}

pub async fn order_detail(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let order = sqlx::query_as!(
        Order,
        "SELECT id, uuid, user_id, order_number, status, subtotal, shipping_fee as shipping_fee, discount, total, payment_method, payment_status, shipping_name, shipping_phone, shipping_address, notes, tracking_number, created_at, updated_at FROM orders WHERE uuid = ?",
        uuid
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let items = sqlx::query_as!(
        OrderItem,
        "SELECT id, order_id, product_id, product_name_zh, product_name_en, material, size_info, quantity, unit_price, total_price, created_at FROM order_items WHERE order_id = ?",
        order.id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(OrderDetailPageTemplate {
        lang: "zh".to_string(),
        order,
        items,
    })
}

pub async fn checkout_page(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    // Without auth, show empty checkout
    CheckoutPageTemplate {
        lang: "zh".to_string(),
        cart_items: vec![],
        subtotal: 0.0,
        shipping: 0.0,
        total: 0.0,
    }
}

// ---------------------------------------------------------------------------
// API handlers
// ---------------------------------------------------------------------------

pub async fn checkout(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Json(req): Json<CheckoutRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let order_number = generate_order_number();
    let uuid = uuid::Uuid::new_v4().to_string();

    let subtotal: f64 = req.items.iter().map(|i| i.unit_price * i.quantity as f64).sum();
    let shipping_fee = if subtotal > 500.0 { 0.0 } else { 25.0 };
    let total = subtotal + shipping_fee;

    let result = sqlx::query!(
        "INSERT INTO orders (uuid, user_id, order_number, status, subtotal, shipping_fee, discount, total, payment_method, payment_status, shipping_name, shipping_phone, shipping_address, notes)
         VALUES (?, ?, ?, 'pending', ?, ?, 0.0, ?, ?, 'unpaid', ?, ?, ?, ?)",
        uuid, claims.0.sub, order_number, subtotal, shipping_fee, total, req.payment_method,
        req.shipping_name, req.shipping_phone, req.shipping_address, req.notes
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let order_id = result.last_insert_rowid();

    for item in &req.items {
        let total_price = item.unit_price * item.quantity as f64;
        sqlx::query!(
            "INSERT INTO order_items (order_id, product_id, product_name_zh, product_name_en, material, size_info, quantity, unit_price, total_price)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            order_id, item.product_id, item.product_name_zh, item.product_name_en, item.material, item.size_info,
            item.quantity, item.unit_price, total_price
        )
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Clear cart
    sqlx::query!("DELETE FROM cart_items WHERE user_id = ?", claims.0.sub)
        .execute(&state.pool)
        .await
        .ok();

    Ok(Json(json!({
        "success": true,
        "order_uuid": uuid,
        "order_number": order_number,
        "total": total
    })))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "UPDATE orders SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP WHERE uuid = ? AND status = 'pending'",
        uuid
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

pub async fn confirm_delivery(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "UPDATE orders SET status = 'completed', updated_at = CURRENT_TIMESTAMP WHERE uuid = ? AND status = 'delivered'",
        uuid
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn generate_order_number() -> String {
    let now = chrono::Utc::now();
    format!("PP{}{:04}", now.format("%Y%m%d"), rand::random::<u32>() % 10000)
}

// ---------------------------------------------------------------------------
// Request/Response types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct CheckoutRequest {
    pub items: Vec<CheckoutItem>,
    pub payment_method: String,
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CheckoutItem {
    pub product_id: i64,
    pub product_name_zh: String,
    pub product_name_en: String,
    pub material: String,
    pub size_info: String,
    pub quantity: i64,
    pub unit_price: f64,
}

// ---------------------------------------------------------------------------
// Summary types for templates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OrderSummary {
    pub uuid: String,
    pub order_number: String,
    pub status: String,
    pub total: f64,
    pub created_at: chrono::NaiveDateTime,
    pub item_count: i64,
}

#[derive(Debug, Clone)]
pub struct CartItemSummary {
    pub id: i64,
    pub product_name_zh: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub subtotal: f64,
}
