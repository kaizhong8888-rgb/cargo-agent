use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
    http::StatusCode,
};
use mime_guess::from_path;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod templates;

#[tokio::main]
async fn main() {
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    let app = Router::new()
        .route("/", get(templates::home))
        .route("/shop", get(templates::shop))
        .route("/product/:slug", get(templates::product_detail))
        .route("/about", get(templates::about))
        .route("/faq", get(templates::faq))
        .route("/contact", get(templates::contact))
        .route("/robots.txt", get(templates::robots_txt))
        .fallback_service(ServeDir::new("static"));

    info!("☁️ CloudSilk server starting on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
