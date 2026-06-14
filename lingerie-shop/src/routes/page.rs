use crate::AppState;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use sqlx::query_as;

#[derive(Template)]
#[template(path = "base.html")]
pub struct BaseTemplate<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub page: &'a str,
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub featured_products: &'a [crate::models::ProductListItem],
    pub categories: &'a [crate::models::Category],
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "products.html")]
pub struct ProductsTemplate<'a> {
    pub products: &'a [crate::models::ProductListItem],
    pub categories: &'a [crate::models::Category],
    pub current_category: Option<&'a str>,
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
    pub page_title: &'a str,
    pub total: i64,
    pub current_page: i64,
    pub total_pages: i64,
}

#[derive(Template)]
#[template(path = "product_detail.html")]
pub struct ProductDetailTemplate<'a> {
    pub product: &'a crate::models::Product,
    pub related_products: &'a [crate::models::ProductListItem],
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate<'a> {
    pub items: &'a [crate::models::CartItem],
    pub total: f64,
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutTemplate<'a> {
    pub items: &'a [crate::models::CartItem],
    pub total: f64,
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "orders.html")]
pub struct OrdersTemplate<'a> {
    pub orders: &'a [crate::models::Order],
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfileTemplate<'a> {
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
    pub name: &'a str,
}

#[derive(Template)]
#[template(path = "order_detail.html")]
pub struct OrderDetailTemplate<'a> {
    pub order: &'a crate::models::Order,
    pub items: &'a [crate::models::OrderItem],
    pub user_email: Option<&'a str>,
    pub is_admin: bool,
}

// Home page
pub async fn home(
    State(state): State<AppState>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let user_email = get_user_email(&state, &cookies);
    let is_admin = is_user_admin(&state, &cookies);

    let featured_products: Vec<crate::models::ProductListItem> = query_as(
        r#"SELECT p.id, p.name, p.slug, p.description, p.price, p.sale_price, p.images,
                  p.is_active, p.is_featured, c.name as category_name, c.slug as category_slug
           FROM products p
           LEFT JOIN categories c ON p.category_id = c.id
           WHERE p.is_active = 1
           ORDER BY p.is_featured DESC, p.created_at DESC
           LIMIT 8"#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let categories: Vec<crate::models::Category> = query_as(
        "SELECT * FROM categories WHERE is_active = 1 ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let template = IndexTemplate {
        featured_products: &featured_products,
        categories: &categories,
        user_email,
        is_admin,
    };

    Html(template.render().unwrap_or_default())
}

// Products listing
#[derive(Deserialize)]
pub struct ProductsQuery {
    pub category: Option<String>,
    pub q: Option<String>,
    pub page: Option<i64>,
}

pub async fn products_list(
    State(state): State<AppState>,
    Query(params): Query<ProductsQuery>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let user_email = get_user_email(&state, &cookies);
    let is_admin = is_user_admin(&state, &cookies);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12;
    let offset = (page - 1) * per_page;

    let categories: Vec<crate::models::Category> = query_as(
        "SELECT * FROM categories WHERE is_active = 1 ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut where_clause = "WHERE p.is_active = 1".to_string();
    let mut count_params: Vec<sqlx::sqlite::SqliteArgumentValue> = vec![];
    let mut query_params: Vec<sqlx::sqlite::SqliteArgumentValue> = vec![];

    if let Some(ref cat) = params.category {
        where_clause.push_str(" AND c.slug = ?");
        count_params.push(sqlx::sqlite::SqliteArgumentValue::Text(cat.clone().into()));
        query_params.push(sqlx::sqlite::SqliteArgumentValue::Text(cat.clone().into()));
    }

    if let Some(ref q) = params.q {
        where_clause.push_str(" AND (p.name LIKE ? OR p.description LIKE ?)");
        let like = format!("%{}%", q);
        count_params.push(sqlx::sqlite::SqliteArgumentValue::Text(like.clone().into()));
        count_params.push(sqlx::sqlite::SqliteArgumentValue::Text(like.into()));
        query_params.push(sqlx::sqlite::SqliteArgumentValue::Text(like.clone().into()));
        query_params.push(sqlx::sqlite::SqliteArgumentValue::Text(like.into()));
    }

    let count_query = format!(
        "SELECT COUNT(*) FROM products p LEFT JOIN categories c ON p.category_id = c.id {}",
        where_clause
    );

    let count_stmt = sqlx::query_with(&count_query, count_params.clone());

    let total: (i64,) = count_stmt.fetch_one(&state.db).await.unwrap_or((0,));
    let total_pages = (total.0 + per_page - 1) / per_page;

    let list_query = format!(
        r#"SELECT p.id, p.name, p.slug, p.description, p.price, p.sale_price, p.images,
                  p.is_active, p.is_featured, c.name as category_name, c.slug as category_slug
           FROM products p
           LEFT JOIN categories c ON p.category_id = c.id
           {}
           ORDER BY p.created_at DESC
           LIMIT {} OFFSET {}"#,
        where_clause, per_page, offset
    );

    let list_stmt = sqlx::query_as_with(&list_query, query_params);
    let products: Vec<crate::models::ProductListItem> =
        list_stmt.fetch_all(&state.db).await.unwrap_or_default();

    let page_title = if let Some(ref cat) = params.category {
        categories
            .iter()
            .find(|c| c.slug == *cat)
            .map(|c| c.name.as_str())
            .unwrap_or(cat)
    } else if params.q.is_some() {
        "搜索结果"
    } else {
        "全部商品"
    };

    let template = ProductsTemplate {
        products: &products,
        categories: &categories,
        current_category: params.category.as_deref(),
        user_email,
        is_admin,
        page_title,
        total: total.0,
        current_page: page,
        total_pages: total_pages.max(1),
    };

    Html(template.render().unwrap_or_default())
}

// Product detail
pub async fn product_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    cookies: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let user_email = get_user_email(&state, &cookies);
    let is_admin = is_user_admin(&state, &cookies);

    let product: Option<crate::models::Product> = query_as(
        r#"SELECT p.*, c.name as category_name, c.slug as category_slug
           FROM products p
           LEFT JOIN categories c ON p.category_id = c.id
           WHERE p.slug = ? AND p.is_active = 1"#
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    match product {
        Some(product) => {
            let related: Vec<crate::models::ProductListItem> = query_as(
                r#"SELECT p.id, p.name, p.slug, p.description, p.price, p.sale_price, p.images,
                          p.is_active, p.is_featured, c.name as category_name, c.slug as category_slug
                   FROM products p
                   LEFT JOIN categories c ON p.category_id = c.id
                   WHERE p.category_id = ? AND p.id != ? AND p.is_active = 1
                   ORDER BY RANDOM()
                   LIMIT 4"#
            )
            .bind(&product.category_id)
            .bind(&product.id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            let template = ProductDetailTemplate {
                product: &product,
                related_products: &related,
                user_email,
                is_admin,
            };

            Html(template.render().unwrap_or_default())
        }
        None => (StatusCode::NOT_FOUND, Html("商品未找到".to_string())).into_response(),
    }
}

fn get_user_email<'a>(state: &'a AppState, cookies: &'a axum_extra::extract::CookieJar) -> Option<&'a str> {
    if let Some(token) = state.auth.extract_token(cookies) {
        if let Ok(claims) = state.auth.verify_token(&token) {
            return Some(Box::leak(claims.email.into_boxed_str()));
        }
    }
    None
}

fn is_user_admin(state: &AppState, cookies: &axum_extra::extract::CookieJar) -> bool {
    if let Some(token) = state.auth.extract_token(cookies) {
        if let Ok(claims) = state.auth.verify_token(&token) {
            return claims.role == "admin";
        }
    }
    false
}
