use crate::models::*;
use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

pub async fn add_to_cart(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Json(req): Json<AddToCartRequest>,
) -> Result<StatusCode, StatusCode> {
    let product = sqlx::query!(
        "SELECT base_price, min_quantity FROM products WHERE id = ?",
        req.product_id
    )
    .fetch_optional(&state.db)
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
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

pub async fn get_cart(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
) -> Result<Json<CartResponse>, StatusCode> {
    let items = sqlx::query_as!(
        CartItem,
        r#"SELECT c.id, c.user_id, c.product_id, p.name_zh as product_name_zh, p.name_en as product_name_en,
                  p.image_url as product_image, c.material, c.size_width, c.size_height,
                  c.quantity, c.unit_price, c.created_at
           FROM cart_items c JOIN products p ON c.product_id = p.id
           WHERE c.user_id = ?"#,
        claims.0.sub
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let subtotal: f64 = items.iter().map(|i| i.total()).sum();
    let item_count: i32 = items.iter().map(|i| i.quantity).sum();

    Ok(Json(CartResponse {
        items,
        subtotal,
        item_count,
    }))
}

pub async fn update_cart(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    axum::extract::Path(item_id): axum::extract::Path<i64>,
    Json(req): Json<UpdateCartRequest>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "UPDATE cart_items SET quantity = ? WHERE id = ? AND user_id = ?",
        req.quantity,
        item_id,
        claims.0.sub
    )
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

pub async fn remove_from_cart(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    axum::extract::Path(item_id): axum::extract::Path<i64>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "DELETE FROM cart_items WHERE id = ? AND user_id = ?",
        item_id,
        claims.0.sub
    )
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
