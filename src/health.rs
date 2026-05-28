//! HTTP status endpoint: exposes agent health, uptime, and basic metrics.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static START_TIME: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_ERRORS: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub memory_bytes: u64,
}

pub fn record_request() {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_error() {
    TOTAL_ERRORS.fetch_add(1, Ordering::Relaxed);
}

/// Build the current health status payload.
pub fn current_health() -> HealthResponse {
    HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: START_TIME.elapsed().as_secs(),
        total_requests: TOTAL_REQUESTS.load(Ordering::Relaxed),
        total_errors: TOTAL_ERRORS.load(Ordering::Relaxed),
        memory_bytes: get_memory_usage(),
    }
}

/// Start the HTTP status server on the given port.
pub async fn start_status_server(port: u16) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            // Drain the HTTP request
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Ok(_) if line.trim().is_empty() => break,
                    Err(_) => break,
                    _ => {}
                }
            }

            let health = current_health();
            let body = serde_json::to_string(&health).unwrap_or_default();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    tracing::info!("Health endpoint listening on http://127.0.0.1:{port}/health");
    Ok(())
}

#[allow(deprecated)]
fn get_memory_usage() -> u64 {
    #[cfg(target_os = "macos")]
    {
        let mut info = unsafe { std::mem::zeroed::<libc::mach_task_basic_info_data_t>() };
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        let ret = unsafe {
            libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as libc::task_info_t,
                &mut count,
            )
        };
        if ret == libc::KERN_SUCCESS {
            return info.resident_size as u64;
        }
    }
    0
}
