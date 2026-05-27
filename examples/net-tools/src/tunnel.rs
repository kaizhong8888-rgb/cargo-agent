//! TLS 加密隧道模块
//!
//! 基于 rustls 实现加密通信隧道。
//! 支持：
//! - 自签名证书生成
//! - 服务端/客户端模式
//! - 双向认证 (mTLS)
//! - 加密隧道数据转发

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tracing::{info, warn};

/// TLS 隧道服务器
pub struct TlsTunnelServer {
    /// 监听地址
    listen_addr: String,
    /// 目标地址（解密后转发到）
    target_addr: String,
    /// TLS 配置
    config: Arc<rustls::ServerConfig>,
}

impl TlsTunnelServer {
    /// 创建新的 TLS 隧道服务器
    pub fn new(
        listen_addr: impl Into<String>,
        target_addr: impl Into<String>,
        config: Arc<rustls::ServerConfig>,
    ) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            target_addr: target_addr.into(),
            config,
        }
    }

    /// 启动 TLS 隧道服务器
    pub async fn start(&self) -> Result<()> {
        let acceptor = TlsAcceptor::from(self.config.clone());
        let listener = TcpListener::bind(&self.listen_addr)
            .await
            .with_context(|| format!("无法绑定 TLS 隧道到 {}", self.listen_addr))?;

        info!(
            "TLS 隧道服务器启动: {} -> {} (加密)",
            self.listen_addr, self.target_addr
        );

        let target_addr = self.target_addr.clone();

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!("TLS 隧道新连接: {}", peer_addr);
                    let acceptor = acceptor.clone();
                    let target = target_addr.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_tls_connection(stream, acceptor, &target).await {
                            warn!("TLS 隧道连接 {} 处理失败: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    warn!("接受 TLS 隧道连接失败: {}", e);
                }
            }
        }
    }
}

/// TLS 隧道客户端
pub struct TlsTunnelClient {
    /// 隧道服务器地址
    server_addr: String,
    /// TLS 配置
    config: Arc<rustls::ClientConfig>,
}

impl TlsTunnelClient {
    /// 创建新的 TLS 隧道客户端
    pub fn new(
        server_addr: impl Into<String>,
        config: Arc<rustls::ClientConfig>,
    ) -> Self {
        Self {
            server_addr: server_addr.into(),
            config,
        }
    }

    /// 连接到 TLS 隧道服务器，返回加密流
    pub async fn connect(&self) -> Result<tokio_rustls::TlsStream<TcpStream>> {
        let connector = TlsConnector::from(self.config.clone());
        let stream = TcpStream::connect(&self.server_addr)
            .await
            .with_context(|| format!("连接到 TLS 隧道服务器 {} 失败", self.server_addr))?;

        // DNS 名称用于 TLS SNI
        let server_name = self
            .server_addr
            .split(':')
            .next()
            .unwrap_or("localhost")
            .try_into()
            .map_err(|_| anyhow::anyhow!("无效的服务器名称"))?;

        let tls_stream = connector
            .connect(server_name, stream)
            .await
            .context("TLS 握手失败")?;

        Ok(tls_stream)
    }
}

/// 处理 TLS 加密连接（服务端）
async fn handle_tls_connection(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    target_addr: &str,
) -> Result<()> {
    // TLS 握手
    let mut tls_stream = acceptor
        .accept(stream)
        .await
        .context("TLS 握手失败")?;

    // 连接到目标
    let mut target = TcpStream::connect(target_addr)
        .await
        .with_context(|| format!("TLS 隧道连接到目标 {} 失败", target_addr))?;

    // 双向转发（解密流量转发到目标）
    let (mut ri, mut wi) = tls_stream.split();
    let (mut ro, mut wo) = target.split();

    let client_to_target = tokio::io::copy(&mut ri, &mut wo);
    let target_to_client = tokio::io::copy(&mut ro, &mut wi);

    tokio::select! {
        result = client_to_target => { result?; }
        result = target_to_client => { result?; }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert;

    #[tokio::test]
    async fn test_tls_tunnel_server_new() {
        let config = cert::generate_self_signed_server_config().unwrap();
        let server = TlsTunnelServer::new("127.0.0.1:0", "127.0.0.1:80", config);
        assert_eq!(server.listen_addr, "127.0.0.1:0");
        assert_eq!(server.target_addr, "127.0.0.1:80");
    }
}
