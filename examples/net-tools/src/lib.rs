//! net-tools: Rust 网络工具集
//!
//! 提供以下网络功能模块：
//! - `proxy`: TCP 端口转发
//! - `socks5`: SOCKS5 代理服务器
//! - `tunnel`: TLS 加密隧道
//! - `pool`: 通用连接池

pub mod proxy;
pub mod socks5;
pub mod tunnel;
pub mod pool;
pub mod cert;

use anyhow::Result;

/// 通用网络结果类型
pub type NetResult<T> = std::result::Result<T, NetError>;

/// 网络错误类型
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),
}

/// 启动 Tracing 日志
pub fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "net_tools=info".into()),
        )
        .init();
}
