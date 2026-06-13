use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use askama::Template;
use serde_json::json;

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
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub lang: String,
}

#[derive(Template)]
#[template(path = "contact.html")]
pub struct ContactTemplate {
    pub lang: String,
}

// Home page handler
pub async fn home(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let categories = sqlx::query_as!(
        Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories ORDER BY sort_order"
    )
    .fetch_all(&state.pool)
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
    .fetch_all(&state.pool)
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
    .fetch_all(&state.pool)
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
        .fetch_all(&state.pool)
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
        .fetch_all(&state.pool)
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
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let category = sqlx::query_as!(
        Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories WHERE id = ?",
        product.category_id
    )
    .fetch_one(&state.pool)
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
    .fetch_optional(&state.pool)
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
        .fetch_optional(&state.pool)
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

// About page handler
pub async fn about_page() -> impl axum::response::IntoResponse {
    AboutTemplate {
        lang: "zh".to_string(),
    }
}

// Contact page handler
pub async fn contact_page() -> impl axum::response::IntoResponse {
    ContactTemplate {
        lang: "zh".to_string(),
    }
}

// Contact form submission
pub async fn submit_contact(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let email = req.get("email").and_then(|v| v.as_str()).unwrap_or("");
    let subject = req.get("subject").and_then(|v| v.as_str()).unwrap_or("");
    let message = req.get("message").and_then(|v| v.as_str()).unwrap_or("");

    if name.is_empty() || email.is_empty() || message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Store contact message in database
    sqlx::query!(
        "INSERT INTO contact_messages (name, email, subject, message) VALUES (?, ?, ?, ?)",
        name, email, subject, message
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "success": true, "message": "消息已发送，我们会尽快回复您" })))
}

// Language switcher
pub async fn switch_language(
    Path(lang): Path<String>,
) -> impl axum::response::IntoResponse {
    let redirect_url = match lang.as_str() {
        "zh" | "en" => "/",
        _ => "/",
    };
    axum::response::Redirect::to(redirect_url)
}
