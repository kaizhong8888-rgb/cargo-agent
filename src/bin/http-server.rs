//! cargo-agent HTTP Server
//!
//! 一个简单的 HTTP 服务器，将 cargo-agent 的功能暴露为 REST API。
//! 无需 API Key，无需注册，本地即可使用。
//!
//! 启动：
//! ```bash
//! cargo run --bin http-server
//! ```
//!
//! 使用：
//! ```bash
//! curl "http://localhost:3000/run?q=分析项目代码质量"
//! curl -X POST http://localhost:3000/run -d '{"prompt": "生成单元测试"}'
//! ```

use axum::{
    extract::{Query, State, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use cargo_agent::config::CargoConfig;
use cargo_agent::gateway::Gateway;

#[derive(Clone)]
struct AppState {
    gateway: Arc<Mutex<Gateway>>,
}

#[derive(Deserialize)]
struct RunQuery {
    q: String,
}

#[derive(Deserialize)]
struct RunRequest {
    prompt: String,
}

#[derive(Serialize)]
struct RunResponse {
    success: bool,
    result: String,
    error: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    tools_count: usize,
    version: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🚀 cargo-agent HTTP Server");
    println!("=========================\n");

    // 加载配置
    let config = CargoConfig::load()?;
    println!("✅ 配置加载成功");

    // 创建 Gateway
    let gateway = Gateway::new(config);
    let tools_count = gateway.agent().tool_registry.list_tools().len();
    println!("✅ 可用工具：{} 个\n", tools_count);

    let state = AppState {
        gateway: Arc::new(Mutex::new(gateway)),
    };

    // 构建路由
    let app = Router::new()
        .route("/run", get(run_get).post(run_post))
        .route("/health", get(health))
        .with_state(state);

    let addr = "0.0.0.0:3000";
    println!("📡 服务器监听：http://{}", addr);
    println!("📝 使用示例：");
    println!("   curl \"http://localhost:3000/run?q=分析代码质量\"");
    println!("   curl -X POST http://localhost:3000/run -d '{{\"prompt\":\"生成测试\"}}'\n");
    println!("按 Ctrl+C 停止\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// GET /run?q=你的问题
async fn run_get(
    Query(query): Query<RunQuery>,
    State(state): State<AppState>,
) -> Json<RunResponse> {
    run_prompt(query.q, state).await
}

/// POST /run {"prompt": "你的问题"}
#[axum::debug_handler]
async fn run_post(
    State(state): State<AppState>,
    Json(req): Json<RunRequest>,
) -> Json<RunResponse> {
    run_prompt(req.prompt, state).await
}

/// 健康检查 GET /health
async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let gateway = state.gateway.lock().await;
    let tools_count = gateway.agent().tool_registry.list_tools().len();

    Json(HealthResponse {
        status: "ok".to_string(),
        tools_count,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// 执行 prompt
async fn run_prompt(prompt: String, state: AppState) -> Json<RunResponse> {
    println!("📥 收到请求：{}", prompt);

    let mut gateway = state.gateway.lock().await;

    match gateway.handle_message(&prompt).await {
        Ok(result) => {
            println!("✅ 请求完成");
            Json(RunResponse {
                success: true,
                result,
                error: None,
            })
        }
        Err(e) => {
            eprintln!("❌ 请求失败：{}", e);
            Json(RunResponse {
                success: false,
                result: String::new(),
                error: Some(e.to_string()),
            })
        }
    }
}
