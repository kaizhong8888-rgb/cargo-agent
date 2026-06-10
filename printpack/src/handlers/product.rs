use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use askama::Template;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub lang: String,
    pub categories: Vec<Category>,
    pub featured_products: Vec<ProductWithCategory>,
}

#[derive(Template)]
#[template(path = "products.html")]
pub struct ProductsTemplate {
    pub lang: String,
    pub categories: Vec<Category>,
    pub products: Vec<ProductWithCategory>,
    pub current_category: Option<String>,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Template)]
#[template(path = "product_detail.html")]
pub struct ProductDetailTemplate {
    pub lang: String,
    pub product: Product,
    pub category: Category,
}

#[derive(Template)]
#[template(path = "quote.html")]
pub struct QuoteTemplate {
    pub lang: String,
    pub product: Option<Product>,
}

#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate {
    pub lang: String,
    pub items: Vec<CartItem>,
    pub subtotal: f64,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub lang: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub lang: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "orders.html")]
pub struct OrdersTemplate {
    pub lang: String,
    pub orders: Vec<Order>,
}

#[derive(Template)]
#[template(path = "order_detail.html")]
pub struct OrderDetailTemplate {
    pub lang: String,
    pub order: Order,
    pub items: Vec<OrderItem>,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboardTemplate {
    pub lang: String,
    pub product_count: i64,
    pub order_count: i64,
    pub user_count: i64,
    pub recent_orders: Vec<Order>,
}

// Home page handler
pub async fn home(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let categories = sqlx::query_as!(
        Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let featured_products = sqlx::query_as!(
        ProductWithCategory,
        r#"SELECT p.id, p.uuid, p.category_id, c.name_zh as category_name_zh, c.name_en as category_name_en,
                  p.name_zh, p.name_en, p.description_zh, p.description_en, p.image_url,
                  p.base_price, p.min_quantity, p.unit, p.materials, p.specs, p.is_active
           FROM products p JOIN categories c ON p.category_id = c.id
           WHERE p.is_active = 1 ORDER BY p.created_at DESC LIMIT 8"#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    HomeTemplate {
        lang: "zh".to_string(),
        categories,
        featured_products,
    }
}

// Products listing
pub async fn products_list(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let categories = sqlx::query_as!(
        Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let category_slug = params.get("category").cloned();
    let page: i64 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;

    let products: Vec<ProductWithCategory> = if let Some(slug) = &category_slug {
        sqlx::query_as!(
            ProductWithCategory,
            r#"SELECT p.id, p.uuid, p.category_id, c.name_zh as category_name_zh, c.name_en as category_name_en,
                      p.name_zh, p.name_en, p.description_zh, p.description_en, p.image_url,
                      p.base_price, p.min_quantity, p.unit, p.materials, p.specs, p.is_active
               FROM products p JOIN categories c ON p.category_id = c.id
               WHERE p.is_active = 1 AND c.slug = ? ORDER BY p.created_at DESC LIMIT ? OFFSET ?"#,
            slug, per_page, offset
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            ProductWithCategory,
            r#"SELECT p.id, p.uuid, p.category_id, c.name_zh as category_name_zh, c.name_en as category_name_en,
                      p.name_zh, p.name_en, p.description_zh, p.description_en, p.image_url,
                      p.base_price, p.min_quantity, p.unit, p.materials, p.specs, p.is_active
               FROM products p JOIN categories c ON p.category_id = c.id
               WHERE p.is_active = 1 ORDER BY p.created_at DESC LIMIT ? OFFSET ?"#,
            per_page, offset
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    ProductsTemplate {
        lang: "zh".to_string(),
        categories,
        products,
        current_category: category_slug,
        current_page: page,
        total_pages: 5,
    }
}

// Product detail
pub async fn product_detail(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let product = sqlx::query_as!(
        Product,
        "SELECT id, uuid, category_id, name_zh, name_en, description_zh, description_en, image_url, base_price, min_quantity, unit, materials, specs, is_active, seo_title_zh, seo_title_en, seo_description_zh, seo_description_en, created_at, updated_at FROM products WHERE uuid = ?",
        uuid
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let category = sqlx::query_as!(
        Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories WHERE id = ?",
        product.category_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(ProductDetailTemplate {
        lang: "zh".to_string(),
        product,
        category,
    })
}

// Get quote calculation
pub async fn get_quote_calc(
    State(state): State<AppState>,
    Json(req): Json<QuoteRequest>,
) -> Result<Json<QuoteResponse>, StatusCode> {
    let product = sqlx::query_as!(
        Product,
        "SELECT id, uuid, category_id, name_zh, name_en, description_zh, description_en, image_url, base_price, min_quantity, unit, materials, specs, is_active, seo_title_zh, seo_title_en, seo_description_zh, seo_description_en, created_at, updated_at FROM products WHERE id = ?",
        req.product_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Calculate price based on material, size, quantity
    let size_factor = ((req.width * req.height * req.depth.unwrap_or(1.0)) / 100.0).clamp(0.5, 5.0);
    let quantity_factor = if req.quantity >= 1000 { 0.7 } else if req.quantity >= 500 { 0.8 } else { 1.0 };
    let unit_price = (product.base_price * size_factor * quantity_factor).round() * 100.0 / 100.0;
    let total_price = (unit_price * req.quantity as f64).round() * 100.0 / 100.0;

    let size = format!("{}×{}×{} cm", req.width, req.height, req.depth.unwrap_or(0.0));

    Ok(Json(QuoteResponse {
        product_name: product.name_zh,
        material: req.material,
        size,
        quantity: req.quantity,
        unit_price,
        total_price,
        finishing: req.finishing,
    }))
}

// Quote page handler
pub async fn quote_page(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let product = if let Some(uuid) = params.get("product") {
        sqlx::query_as!(
            Product,
            "SELECT id, uuid, category_id, name_zh, name_en, description_zh, description_en, image_url, base_price, min_quantity, unit, materials, specs, is_active, seo_title_zh, seo_title_en, seo_description_zh, seo_description_en, created_at, updated_at FROM products WHERE uuid = ?",
            uuid
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    QuoteTemplate {
        lang: "zh".to_string(),
        product,
    }
}
