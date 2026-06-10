//! net-tools CLI 入口
//!
//! 提供端口转发、SOCKS5 代理、TLS 隧道等网络工具的命令行接口。

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use net_tools::{cert, init_logging, proxy, socks5, tunnel};
use std::sync::Arc;

/// Rust 网络工具集：端口转发、SOCKS5 代理、加密隧道、连接池
#[derive(Parser)]
#[command(name = "net-tools", version, author, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// TCP 端口转发
    #[command(name = "proxy")]
    TcpProxy {
        /// 本地监听地址
        #[arg(short = 'l', long = "listen", default_value = "127.0.0.1:8080")]
        listen: String,

        /// 目标转发地址
        #[arg(short = 't', long = "target")]
        target: String,

        /// 最大并发连接数
        #[arg(short = 'c', long = "max-connections", default_value = "1024")]
        max_connections: usize,
    },

    /// SOCKS5 代理服务器
    #[command(name = "socks5")]
    Socks5 {
        /// 监听地址
        #[arg(short = 'l', long = "listen", default_value = "127.0.0.1:1080")]
        listen: String,

        /// 用户名（可选，用于认证）
        #[arg(short = 'u', long = "username")]
        username: Option<String>,

        /// 密码（可选，用于认证）
        #[arg(short = 'p', long = "password")]
        password: Option<String>,
    },

    /// TLS 加密隧道（服务端）
    #[command(name = "tunnel-server")]
    TunnelServer {
        /// TLS 监听地址
        #[arg(short = 'l', long = "listen", default_value = "127.0.0.1:4433")]
        listen: String,

        /// 目标转发地址（解密后）
        #[arg(short = 't', long = "target")]
        target: String,

        /// 证书文件路径（PEM 格式，可选，默认使用自签名证书）
        #[arg(short = 'c', long = "cert")]
        cert_path: Option<String>,

        /// 私钥文件路径（PEM 格式，可选）
        #[arg(short = 'k', long = "key")]
        key_path: Option<String>,
    },

    /// TLS 加密隧道（客户端）
    #[command(name = "tunnel-client")]
    TunnelClient {
        /// 隧道服务器地址
        #[arg(short = 's', long = "server")]
        server: String,

        /// 本地转发端口
        #[arg(short = 'l', long = "local-listen", default_value = "127.0.0.1:8888")]
        local_listen: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let cli = Cli::parse();

    match cli.command {
        Commands::TcpProxy {
            listen,
            target,
            max_connections,
        } => cmd_tcp_proxy(&listen, &target, max_connections).await,

        Commands::Socks5 {
            listen,
            username,
            password,
        } => cmd_socks5(&listen, username.as_deref(), password.as_deref()).await,

        Commands::TunnelServer {
            listen,
            target,
            cert_path,
            key_path,
        } => {
            cmd_tunnel_server(&listen, &target, cert_path.as_deref(), key_path.as_deref()).await
        }

        Commands::TunnelClient {
            server,
            local_listen,
        } => cmd_tunnel_client(&server, &local_listen).await,
    }
}

/// 启动 TCP 端口转发
async fn cmd_tcp_proxy(listen: &str, target: &str, max_connections: usize) -> Result<()> {
    println!("启动 TCP 端口转发: {} -> {}", listen, target);
    let proxy = proxy::TcpProxy::new(listen, target).with_max_connections(max_connections);
    proxy.start().await
}

/// 启动 SOCKS5 代理
async fn cmd_socks5(listen: &str, username: Option<&str>, password: Option<&str>) -> Result<()> {
    let proxy = match (username, password) {
        (Some(user), Some(pass)) => {
            println!("启动 SOCKS5 代理: {} (用户名认证)", listen);
            socks5::Socks5Proxy::with_auth(listen, user, pass)
        }
        _ => {
            println!("启动 SOCKS5 代理: {} (无认证)", listen);
            socks5::Socks5Proxy::new(listen)
        }
    };
    proxy.start().await
}

/// 启动 TLS 隧道服务器
async fn cmd_tunnel_server(
    listen: &str,
    target: &str,
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> Result<()> {
    let config = match (cert_path, key_path) {
        (Some(cert), Some(key)) => {
            let certs = load_certs(cert)?;
            let priv_key = load_private_key(key)?;
            Arc::new(
                rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(certs, priv_key)
                    .context("TLS 服务端配置失败")?,
            )
        }
        _ => {
            println!("使用自签名证书");
            cert::generate_self_signed_server_config()?
        }
    };

    println!("启动 TLS 隧道服务器: {} -> {}", listen, target);
    let server = tunnel::TlsTunnelServer::new(listen, target, config);
    server.start().await
}

/// 启动 TLS 隧道客户端
async fn cmd_tunnel_client(server: &str, local_listen: &str) -> Result<()> {
    let config = cert::generate_client_config(None)?;
    println!(
        "启动 TLS 隧道客户端: {} -> {} (加密隧道)",
        local_listen, server
    );

    let listener = tokio::net::TcpListener::bind(local_listen).await?;
    let client = tunnel::TlsTunnelClient::new(server, config);

    loop {
        let (local_stream, peer_addr) = listener.accept().await?;
        println!("隧道客户端收到本地连接: {}", peer_addr);

        let tls_stream = client.connect().await?;
        tokio::spawn(async move {
            if let Err(e) = relay_traffic(local_stream, tls_stream).await {
                eprintln!("转发错误 ({}): {}", peer_addr, e);
            }
        });
    }
}

/// 双向流量转发
async fn relay_traffic(
    mut local: tokio::net::TcpStream,
    mut remote: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
) -> Result<()> {
    let (mut local_read, mut local_write) = local.split();
    let (mut remote_read, mut remote_write) = tokio::io::split(remote);

    tokio::select! {
        r = tokio::io::copy(&mut local_read, &mut remote_write) => r?,
        r = tokio::io::copy(&mut remote_read, &mut local_write) => r?,
    }
    Ok(())
}

/// 从 PEM 文件加载证书
fn load_certs(path: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    if certs.is_empty() {
        return Err(anyhow::anyhow!("未找到证书"));
    }
    Ok(certs)
}

/// 从 PEM 文件加载私钥
fn load_private_key(path: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let key = rustls_pemfile::private_key(&mut reader)?.ok_or_else(|| anyhow::anyhow!("未找到私钥"))?;
    Ok(key)
}
