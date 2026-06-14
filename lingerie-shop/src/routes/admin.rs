use crate::AppState;
use askama::Template;
use axum::{
    extract::{Form, Path, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboardTemplate<'a> {
    pub total_products: i64,
    pub total_orders: i64,
    pub total_users: i64,
    pub pending_orders: i64,
    pub recent_orders: &'a [crate::models::Order],
    pub low_stock_products: &'a [LowStockProduct],
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct LowStockProduct {
    pub id: String,
    pub name: String,
    pub stock: i64,
}

#[derive(Template)]
#[template(path = "admin/products.html")]
pub struct AdminProductsTemplate<'a> {
    pub products: &'a [crate::models::Product],
    pub categories: &'a [crate::models::Category],
}

#[derive(Template)]
#[template(path = "admin/orders.html")]
pub struct AdminOrdersTemplate<'a> {
    pub orders: &'a [crate::models::Order],
    pub filter_status: Option<&'a str>,
}

#[derive(Deserialize)]
pub struct AdminProductForm {
    pub name: String,
    pub category_id: Option<i64>,
    pub description: String,
    pub price: f64,
    pub sale_price: Option<f64>,
    pub stock: i64,
    pub is_featured: bool,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct AdminOrderStatusForm {
    pub status: String,
}

// Require admin
fn require_admin(state: &AppState, cookies: &CookieJar) -> Option<crate::auth::Claims> {
    if let Some(token) = state.auth.extract_token(cookies) {
        if let Ok(claims) = state.auth.verify_token(&token) {
            if claims.role == "admin" {
                return Some(claims);
            }
        }
    }
    None
}

pub async fn admin_dashboard(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let total_products: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    let total_orders: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orders")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    let total_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    let pending_orders: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orders WHERE status = 'pending'")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    let recent_orders: Vec<crate::models::Order> = sqlx::query_as(
        "SELECT * FROM orders ORDER BY created_at DESC LIMIT 10"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let low_stock_products: Vec<LowStockProduct> = sqlx::query_as(
        "SELECT id, name, stock FROM products WHERE stock < 20 AND is_active = 1 ORDER BY stock ASC LIMIT 10"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let template = AdminDashboardTemplate {
        total_products: total_products.0,
        total_orders: total_orders.0,
        total_users: total_users.0,
        pending_orders: pending_orders.0,
        recent_orders: &recent_orders,
        low_stock_products: &low_stock_products,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

pub async fn admin_products_list(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let products: Vec<crate::models::Product> = sqlx::query_as(
        r#"SELECT p.*, c.name as category_name, c.slug as category_slug
           FROM products p LEFT JOIN categories c ON p.category_id = c.id
           ORDER BY p.created_at DESC"#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let categories: Vec<crate::models::Category> = sqlx::query_as(
        "SELECT * FROM categories ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let template = AdminProductsTemplate {
        products: &products,
        categories: &categories,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

pub async fn admin_create_product(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<AdminProductForm>,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let slug = form.name.to_lowercase().replace(' ', "-");
    let id = uuid::Uuid::new_v4().to_string();

    let _ = sqlx::query(
        "INSERT INTO products (id, name, slug, description, price, sale_price, stock, category_id, is_featured, is_active) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&form.name)
    .bind(&slug)
    .bind(&form.description)
    .bind(form.price)
    .bind(form.sale_price)
    .bind(form.stock)
    .bind(form.category_id)
    .bind(form.is_featured)
    .bind(form.is_active)
    .execute(&state.db)
    .await;

    Redirect::to("/admin/products").into_response()
}

pub async fn admin_delete_product(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let _ = sqlx::query("DELETE FROM products WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/products").into_response()
}

pub async fn admin_toggle_product(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let _ = sqlx::query("UPDATE products SET is_active = NOT is_active WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Redirect::to("/admin/products").into_response()
}

pub async fn admin_orders_list(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let orders: Vec<crate::models::Order> = sqlx::query_as(
        "SELECT * FROM orders ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let template = AdminOrdersTemplate {
        orders: &orders,
        filter_status: None,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

pub async fn admin_update_order_status(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(order_id): Path<String>,
    Form(form): Form<AdminOrderStatusForm>,
) -> impl IntoResponse {
    let _ = require_admin(&state, &cookies)?;

    let valid_statuses = ["pending", "confirmed", "shipped", "delivered", "cancelled", "refunded"];
    if valid_statuses.contains(&form.status.as_str()) {
        let _ = sqlx::query("UPDATE orders SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(&form.status)
            .bind(&order_id)
            .execute(&state.db)
            .await;
    }

    Redirect::to("/admin/orders").into_response()
}
