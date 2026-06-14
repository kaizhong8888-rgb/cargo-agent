use crate::AppState;
use askama::Template;
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AddToCartForm {
    pub product_id: String,
    pub quantity: i64,
}

#[derive(Deserialize)]
pub struct UpdateCartForm {
    pub item_id: i64,
    pub quantity: i64,
}

#[derive(Deserialize)]
pub struct RemoveCartForm {
    pub item_id: i64,
}

// View cart
pub async fn view_cart(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    let items: Vec<crate::models::CartItem> = sqlx::query_as(
        r#"SELECT ci.*, p.name as product_name, p.slug as product_slug,
                  p.price as product_price, p.sale_price as product_sale_price,
                  p.images as product_images
           FROM cart_items ci
           JOIN products p ON ci.product_id = p.id
           WHERE ci.user_id = ?
           ORDER BY ci.created_at DESC"#
    )
    .bind(&claims.sub)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total: f64 = items.iter().map(|item| item.subtotal()).sum();

    let user_email = Some(claims.email.as_str());
    let is_admin = claims.role == "admin";

    Html(
        crate::routes::page::CartTemplate {
            items: &items,
            total,
            user_email,
            is_admin,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

// Add to cart
pub async fn add_to_cart(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<AddToCartForm>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    // Check product exists and has stock
    let product: Option<(i64,)> = sqlx::query_as("SELECT stock FROM products WHERE id = ? AND is_active = 1")
        .bind(&form.product_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match product {
        Some((stock,)) if stock >= form.quantity => {
            // Upsert cart item
            sqlx::query(
                r#"INSERT INTO cart_items (user_id, product_id, quantity)
                   VALUES (?, ?, ?)
                   ON CONFLICT(user_id, product_id)
                   DO UPDATE SET quantity = cart_items.quantity + ?"#
            )
            .bind(&claims.sub)
            .bind(&form.product_id)
            .bind(form.quantity)
            .bind(form.quantity)
            .execute(&state.db)
            .await
            .ok();

            Redirect::to("/cart").into_response()
        }
        _ => {
            // Product not found or insufficient stock
            Redirect::to("/products").into_response()
        }
    }
}

// Update cart item
pub async fn update_cart_item(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<UpdateCartForm>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    if form.quantity <= 0 {
        // Remove item
        sqlx::query("DELETE FROM cart_items WHERE id = ? AND user_id = ?")
            .bind(form.item_id)
            .bind(&claims.sub)
            .execute(&state.db)
            .await
            .ok();
    } else {
        sqlx::query("UPDATE cart_items SET quantity = ? WHERE id = ? AND user_id = ?")
            .bind(form.quantity)
            .bind(form.item_id)
            .bind(&claims.sub)
            .execute(&state.db)
            .await
            .ok();
    }

    Redirect::to("/cart").into_response()
}

// Remove from cart
pub async fn remove_from_cart(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<RemoveCartForm>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    sqlx::query("DELETE FROM cart_items WHERE id = ? AND user_id = ?")
        .bind(form.item_id)
        .bind(&claims.sub)
        .execute(&state.db)
        .await
        .ok();

    Redirect::to("/cart").into_response()
}

fn require_auth(state: &AppState, cookies: &CookieJar) -> Option<crate::auth::Claims> {
    if let Some(token) = state.auth.extract_token(cookies) {
        return state.auth.verify_token(&token).ok();
    }
    None
}
