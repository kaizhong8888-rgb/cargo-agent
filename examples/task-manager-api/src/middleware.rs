use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn};

/// Log every incoming request together with its HTTP method, path,
/// status code and duration.
pub async fn request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let elapsed = start.elapsed();
    let status = response.status();

    if status.is_server_error() {
        warn!(
            method = %method,
            path = %uri,
            status = status.as_u16(),
            duration_ms = elapsed.as_secs_f64() * 1000.0,
            "Request completed with server error"
        );
    } else {
        info!(
            method = %method,
            path = %uri,
            status = status.as_u16(),
            duration_ms = elapsed.as_secs_f64() * 1000.0,
            "Request completed"
        );
    }

    response
}
