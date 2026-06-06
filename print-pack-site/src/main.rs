mod models;
mod routes;
mod templates;

use axum::{Router, routing::get};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "print_pack_site=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build our application with some routes
    let app = Router::new()
        .route("/", get(routes::home))
        .route("/products", get(routes::products))
        .route("/products/:id", get(routes::product_detail))
        .route("/about", get(routes::about))
        .route("/contact", get(routes::contact))
        .route("/contact", post(routes::submit_inquiry))
        .route("/quote", get(routes::quote))
        .route("/quote", post(routes::submit_quote))
        .nest_service("/static", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("🚀 印刷包装独立站启动于 http://localhost:3000");
    
    axum::serve(listener, app).await?;
    Ok(())
}
