use crate::AppState;
use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CheckoutForm {
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub shipping_city: String,
    pub shipping_state: String,
    pub shipping_zip: String,
    pub shipping_country: String,
    pub payment_method: String,
    pub notes: String,
}

// Show checkout page
pub async fn show_checkout(
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

    if items.is_empty() {
        return Redirect::to("/cart").into_response();
    }

    let total: f64 = items.iter().map(|item| item.subtotal()).sum();

    let user_email = Some(claims.email.as_str());
    let is_admin = claims.role == "admin";

    Html(
        crate::routes::page::CheckoutTemplate {
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

// Handle checkout
pub async fn handle_checkout(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<CheckoutForm>,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    // Get cart items
    let cart_items: Vec<(String, String, i64, f64, String, String)> = sqlx::query_as(
        r#"SELECT ci.product_id, p.name, ci.quantity,
                  COALESCE(p.sale_price, p.price), p.images, ci.id as cart_item_id
           FROM cart_items ci
           JOIN products p ON ci.product_id = p.id
           WHERE ci.user_id = ?"#
    )
    .bind(&claims.sub)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if cart_items.is_empty() {
        return Redirect::to("/cart").into_response();
    }

    let total: f64 = cart_items
        .iter()
        .map(|(_, _, qty, price, _, _)| (*price as f64) * (*qty as f64))
        .sum();

    let order_id = uuid::Uuid::new_v4().to_string();

    // Create order
    match sqlx::query(
        r#"INSERT INTO orders (id, user_id, total, status, shipping_name, shipping_phone,
           shipping_address, shipping_city, shipping_state, shipping_zip, shipping_country,
           payment_method, payment_status, notes)
           VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, 'unpaid', ?)"#
    )
    .bind(&order_id)
    .bind(&claims.sub)
    .bind(total)
    .bind(&form.shipping_name)
    .bind(&form.shipping_phone)
    .bind(&form.shipping_address)
    .bind(&form.shipping_city)
    .bind(&form.shipping_state)
    .bind(&form.shipping_zip)
    .bind(&form.shipping_country)
    .bind(&form.payment_method)
    .bind(&form.notes)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            // Insert order items & update stock
            for (product_id, name, qty, price, images, _) in &cart_items {
                let first_image: Option<String> = serde_json::from_str(images)
                    .ok()
                    .and_then(|imgs: Vec<String>| imgs.into_iter().next());

                sqlx::query(
                    "INSERT INTO order_items (order_id, product_id, product_name, product_image, quantity, price) VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(&order_id)
                .bind(product_id)
                .bind(name)
                .bind(first_image.as_deref().unwrap_or(""))
                .bind(qty)
                .bind(price)
                .execute(&state.db)
                .await
                .ok();

                // Update stock
                sqlx::query("UPDATE products SET stock = stock - ? WHERE id = ?")
                    .bind(qty)
                    .bind(product_id)
                    .execute(&state.db)
                    .await
                    .ok();
            }

            // Clear cart
            sqlx::query("DELETE FROM cart_items WHERE user_id = ?")
                .bind(&claims.sub)
                .execute(&state.db)
                .await
                .ok();

            Redirect::to(&format!("/orders/{}", order_id)).into_response()
        }
        Err(e) => {
            tracing::error!("Checkout failed: {}", e);
            Redirect::to("/checkout").into_response()
        }
    }
}

// View order history
pub async fn view_orders(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    let orders: Vec<crate::models::Order> = sqlx::query_as(
        "SELECT * FROM orders WHERE user_id = ? ORDER BY created_at DESC"
    )
    .bind(&claims.sub)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let user_email = Some(claims.email.as_str());
    let is_admin = claims.role == "admin";

    Html(
        crate::routes::page::OrdersTemplate {
            orders: &orders,
            user_email,
            is_admin,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

// View order detail
pub async fn view_order_detail(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    let order: Option<crate::models::Order> = sqlx::query_as(
        "SELECT * FROM orders WHERE id = ? AND user_id = ?"
    )
    .bind(&order_id)
    .bind(&claims.sub)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    match order {
        Some(order) => {
            let items: Vec<crate::models::OrderItem> = sqlx::query_as(
                "SELECT * FROM order_items WHERE order_id = ? ORDER BY id ASC"
            )
            .bind(&order_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            let user_email = Some(claims.email.as_str());
            let is_admin = claims.role == "admin";

            Html(
                crate::routes::page::OrderDetailTemplate {
                    order: &order,
                    items: &items,
                    user_email,
                    is_admin,
                }
                .render()
                .unwrap(),
            )
            .into_response()
        }
        None => Redirect::to("/orders").into_response(),
    }
}

// View profile
pub async fn view_profile(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let claims = match require_auth(&state, &cookies) {
        Some(c) => c,
        None => return Redirect::to("/login").into_response(),
    };

    let name: (String,) = sqlx::query_as("SELECT name FROM users WHERE id = ?")
        .bind(&claims.sub)
        .fetch_one(&state.db)
        .await
        .unwrap_or(("".to_string(),));

    let user_email = Some(claims.email.as_str());
    let is_admin = claims.role == "admin";

    Html(
        crate::routes::page::ProfileTemplate {
            user_email,
            is_admin,
            name: &name.0,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

fn require_auth(state: &AppState, cookies: &CookieJar) -> Option<crate::auth::Claims> {
    if let Some(token) = state.auth.extract_token(cookies) {
        return state.auth.verify_token(&token).ok();
    }
    None
}
