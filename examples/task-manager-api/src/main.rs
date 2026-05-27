use std::net::SocketAddr;
use task_manager_api::db;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // ── Logging ─────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "task_manager_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ── Database ────────────────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:task_manager.db?mode=rwc".into());

    let pool = db::create_pool(&database_url).await?;
    info!(database = %database_url, "Database connected");

    // ── Server ──────────────────────────────────────────────────────
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".into())
        .parse()
        .expect("Invalid BIND_ADDR");

    task_manager_api::serve(pool, addr).await
}
