mod db;
mod handlers;
mod i18n;
mod middleware;
mod models;

use askama::Template;
use axum::{
    extract::{Query, State},
    routing::{get, post, put, delete},
    Router,
};
use handlers::AppState;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// ---------------------------------------------------------------------------
// Simple page templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    lang: String,
    categories: Vec<models::Category>,
    featured_products: Vec<models::ProductWithCategory>,
}

#[derive(Template)]
#[template(path = "about.html")]
struct AboutTemplate {
    lang: String,
}

#[derive(Template)]
#[template(path = "contact.html")]
struct ContactTemplate {
    lang: String,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    lang: String,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate {
    lang: String,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Page handlers
// ---------------------------------------------------------------------------

async fn home_page(State(state): State<AppState>) -> HomeTemplate {
    let categories = sqlx::query_as!(
        models::Category,
        "SELECT id, name_zh, name_en, slug, description_zh, description_en, sort_order, created_at FROM categories ORDER BY sort_order"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let featured_products = sqlx::query_as!(
        models::ProductWithCategory,
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

async fn about_page() -> AboutTemplate {
    AboutTemplate {
        lang: "zh".to_string(),
    }
}

async fn contact_page() -> ContactTemplate {
    ContactTemplate {
        lang: "zh".to_string(),
    }
}

async fn login_page(Query(params): Query<std::collections::HashMap<String, String>>) -> LoginTemplate {
    LoginTemplate {
        lang: "zh".to_string(),
        error: params.get("error").cloned(),
    }
}

async fn register_page(Query(params): Query<std::collections::HashMap<String, String>>) -> RegisterTemplate {
    RegisterTemplate {
        lang: "zh".to_string(),
        error: params.get("error").cloned(),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = db::init_pool("printpack.db").await?;
    db::migrate(&pool).await?;

    let app_state = AppState {
        pool,
        jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".to_string()),
    };

    let app = Router::new()
        // Public pages
        .route("/", get(home_page))
        .route("/about", get(about_page))
        .route("/contact", get(contact_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        // Products
        .route("/products", get(handlers::product::products_list))
        .route("/products/{id}", get(handlers::product::product_detail))
        .route("/quote", get(handlers::product::quote_page))
        // API
        .route("/api/quote", post(handlers::product::get_quote_calc))
        .route("/api/register", post(handlers::user::register))
        .route("/api/login", post(handlers::user::login))
        .route("/api/contact", post(handlers::product::submit_contact))
        // Cart
        .route("/cart", get(handlers::cart::cart_page))
        .route("/api/cart", post(handlers::cart::add_to_cart))
        .route("/api/cart/{id}/quantity", put(handlers::cart::update_quantity))
        .route("/api/cart/{id}", delete(handlers::cart::remove_item))
        // Orders
        .route("/orders", get(handlers::order::list_orders))
        .route("/orders/{id}", get(handlers::order::order_detail))
        .route("/checkout", get(handlers::order::checkout_page))
        .route("/api/checkout", post(handlers::order::checkout))
        .route("/api/orders/{id}/cancel", post(handlers::order::cancel_order))
        .route("/api/orders/{id}/confirm", post(handlers::order::confirm_delivery))
        // Language switcher
        .route("/lang/{lang}", get(handlers::product::switch_language))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("🚀 PrintPack server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
