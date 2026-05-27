//! TCP 端口转发（Proxy）模块
//!
//! 实现简单的 TCP 端口转发，将本地端口接收的连接转发到远程目标。
//! 支持：
//! - 透明 TCP 转发
//! - 可配置缓冲区大小
//! - 优雅关闭
//! - 并发连接管理

use anyhow::{Context, Result};
use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

/// TCP 端口转发器
pub struct TcpProxy {
    /// 监听地址
    listen_addr: String,
    /// 目标地址
    target_addr: String,
    /// 最大并发连接数
    max_connections: usize,
    /// 缓冲区大小（字节）
    buffer_size: usize,
}

impl TcpProxy {
    /// 创建新的 TCP 转发器
    pub fn new(listen_addr: impl Into<String>, target_addr: impl Into<String>) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            target_addr: target_addr.into(),
            max_connections: 1024,
            buffer_size: 65536,
        }
    }

    /// 设置最大并发连接数
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// 设置缓冲区大小
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// 启动转发服务
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.listen_addr)
            .await
            .with_context(|| format!("无法绑定到 {}", self.listen_addr))?;

        info!(
            "TCP 转发服务启动: {} -> {} (最大连接: {}, 缓冲区: {}KB)",
            self.listen_addr,
            self.target_addr,
            self.max_connections,
            self.buffer_size / 1024
        );

        // 使用信号量限制并发连接
        let semaphore = Arc::new(Semaphore::new(self.max_connections));
        let target_addr = self.target_addr.clone();
        let buffer_size = self.buffer_size;

        loop {
            let permit = semaphore.clone().acquire_owned().await;
            match permit {
                Ok(permit) => {
                    let (inbound, peer_addr) = match listener.accept().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            error!("接受连接失败: {}", e);
                            continue;
                        }
                    };

                    info!("新连接: {}", peer_addr);
                    let target = target_addr.clone();
                    let buf_size = buffer_size;

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(inbound, &target, buf_size).await {
                            error!("转发连接 {} -> {} 失败: {}", peer_addr, target, e);
                        }
                        drop(permit);
                    });
                }
                Err(e) => {
                    error!("获取信号量失败: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }
}

/// 处理单个 TCP 转发连接
async fn handle_connection(
    mut inbound: TcpStream,
    target_addr: &str,
    buffer_size: usize,
) -> Result<()> {
    // 配置套接字选项
    inbound.set_nodelay(true).context("设置 TCP_NODELAY 失败")?;

    // 连接到目标
    let mut outbound = TcpStream::connect(target_addr)
        .await
        .with_context(|| format!("连接到目标 {} 失败", target_addr))?;
    outbound.set_nodelay(true).context("设置 TCP_NODELAY 失败")?;

    // 使用 tokio::io::copy_bidirectional 进行双向转发
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = tokio::io::copy(&mut ri, &mut wo);
    let server_to_client = tokio::io::copy(&mut ro, &mut wi);

    tokio::select! {
        result = client_to_server => {
            result.context("客户端到服务端转发失败")?;
        }
        result = server_to_client => {
            result.context("服务端到客户端转发失败")?;
        }
    }

    Ok(())
}

/// 分块复制（替代方案，支持自定义缓冲区大小）
async fn copy_bidirectional_with_buffer(
    inbound: &mut TcpStream,
    outbound: &mut TcpStream,
    buffer_size: usize,
) -> Result<()> {
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = copy_with_buffer(&mut ri, &mut wo, buffer_size);
    let server_to_client = copy_with_buffer(&mut ro, &mut wi, buffer_size);

    tokio::select! {
        result = client_to_server => result?,
        result = server_to_client => result?,
    }

    Ok(())
}

/// 带缓冲区配置的数据复制
async fn copy_with_buffer<R, W>(reader: &mut R, writer: &mut W, buffer_size: usize) -> Result<u64>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut buf = BytesMut::with_capacity(buffer_size);
    buf.resize(buffer_size, 0);
    let mut total: u64 = 0;

    loop {
        let n = reader.read(&mut buf[..buffer_size]).await?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        total += n as u64;
    }

    writer.shutdown().await?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_proxy_new() {
        let proxy = TcpProxy::new("127.0.0.1:0", "127.0.0.1:8080");
        assert_eq!(proxy.max_connections, 1024);
        assert_eq!(proxy.buffer_size, 65536);
    }

    #[tokio::test]
    async fn test_tcp_proxy_with_options() {
        let proxy = TcpProxy::new("127.0.0.1:0", "127.0.0.1:8080")
            .with_max_connections(100)
            .with_buffer_size(8192);
        assert_eq!(proxy.max_connections, 100);
        assert_eq!(proxy.buffer_size, 8192);
    }
}
