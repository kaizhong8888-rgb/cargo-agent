pub mod db;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;

use std::net::SocketAddr;

use axum::{
    middleware as axum_mw,
    routing::get,
    Router,
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tracing::info;

/// Build the complete application router.
pub fn build_router(pool: SqlitePool) -> Router {
    Router::new()
        // ── Health ───────────────────────────────────────────────
        .route("/health", get(handlers::health_check))
        // ── Tasks CRUD ───────────────────────────────────────────
        .route("/api/tasks", get(handlers::list_tasks).post(handlers::create_task))
        .route(
            "/api/tasks/:id",
            get(handlers::get_task)
                .patch(handlers::update_task)
                .delete(handlers::delete_task),
        )
        // ── Shared state ─────────────────────────────────────────
        .with_state(pool)
        // ── Global middleware ────────────────────────────────────
        .layer(axum_mw::from_fn(middleware::request_logger))
        .layer(CorsLayer::permissive())
}

/// Start the HTTP server and listen until a shutdown signal is received.
pub async fn serve(pool: SqlitePool, addr: SocketAddr) -> Result<(), anyhow::Error> {
    let app = build_router(pool);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(address = %addr, "Server started");

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {e}"))?;

    Ok(())
}
