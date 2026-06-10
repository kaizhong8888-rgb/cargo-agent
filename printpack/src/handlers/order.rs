use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

pub async fn create_order(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Json(req): Json<CreateOrder>,
) -> Result<Json<OrderResponse>, StatusCode> {
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
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let order_id = result.last_insert_rowid();

    for item in &req.items {
        let product = sqlx::query!(
            "SELECT name_zh, name_en FROM products WHERE id = ?",
            item.product_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

        let total_price = item.unit_price * item.quantity as f64;
        sqlx::query!(
            "INSERT INTO order_items (order_id, product_id, product_name_zh, product_name_en, material, size_info, quantity, unit_price, total_price)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            order_id, item.product_id, product.name_zh, product.name_en, item.material, item.size_info,
            item.quantity, item.unit_price, total_price
        )
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Clear cart
    sqlx::query!("DELETE FROM cart_items WHERE user_id = ?", claims.0.sub)
        .execute(&state.db)
        .await
        .ok();

    let order = get_order_by_uuid(&state.db, &uuid).await?;
    let items = get_order_items(&state.db, order_id).await?;

    Ok(Json(OrderResponse { order, items }))
}

pub async fn get_user_orders(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
) -> Result<Json<Vec<Order>>, StatusCode> {
    let orders = sqlx::query_as!(
        Order,
        "SELECT id, uuid, user_id, order_number, status, subtotal, shipping_fee, discount, total, payment_method, payment_status, shipping_name, shipping_phone, shipping_address, notes, created_at, updated_at FROM orders WHERE user_id = ? ORDER BY created_at DESC",
        claims.0.sub
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(orders))
}

pub async fn get_order(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Path(uuid): Path<String>,
) -> Result<Json<OrderResponse>, StatusCode> {
    let order = get_order_by_uuid(&state.db, &uuid).await?;
    if order.user_id.to_string() != claims.0.sub && claims.0.role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    let items = get_order_items(&state.db, order.id).await?;
    Ok(Json(OrderResponse { order, items }))
}

async fn get_order_by_uuid(db: &sqlx::SqlitePool, uuid: &str) -> Result<Order, StatusCode> {
    sqlx::query_as!(
        Order,
        "SELECT id, uuid, user_id, order_number, status, subtotal, shipping_fee, discount, total, payment_method, payment_status, shipping_name, shipping_phone, shipping_address, notes, created_at, updated_at FROM orders WHERE uuid = ?",
        uuid
    )
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)
}

async fn get_order_items(db: &sqlx::SqlitePool, order_id: i64) -> Result<Vec<OrderItem>, StatusCode> {
    sqlx::query_as!(
        OrderItem,
        "SELECT id, order_id, product_id, product_name_zh, product_name_en, material, size_info, quantity, unit_price, total_price, created_at FROM order_items WHERE order_id = ?",
        order_id
    )
    .fetch_all(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn process_payment(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    let order = sqlx::query!(
        "SELECT id, total, payment_status FROM orders WHERE uuid = ?",
        uuid
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if order.payment_status == "paid" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Simulate payment processing
    sqlx::query!(
        "UPDATE orders SET payment_status = 'paid', status = 'confirmed', updated_at = CURRENT_TIMESTAMP WHERE uuid = ?",
        uuid
    )
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

fn generate_order_number() -> String {
    let now = chrono::Utc::now();
    format!("PP{}{}", now.format("%Y%m%d"), rand::random::<u32>() % 10000)
}
