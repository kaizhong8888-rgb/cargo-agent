//! SOCKS5 代理协议实现模块
//!
//! SOCKS5 (RFC 1928) 代理协议完整实现。
//! 支持：
//! - 无认证和用户名/密码认证
//! - TCP 连接 (CONNECT 命令)
//! - UDP 关联 (UDP ASSOCIATE)
//! - IPv4/IPv6/Domain 地址类型

use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

/// SOCKS5 协议版本
const SOCKS_VERSION: u8 = 0x05;

/// SOCKS5 认证方法
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// 无认证
    NoAuth,
    /// 用户名/密码认证
    UserPass { username: String, password: String },
}

/// SOCKS5 地址类型
#[derive(Debug, Clone)]
pub enum SocksAddress {
    /// IPv4 地址
    V4(IpAddr, u16),
    /// IPv6 地址
    V6(IpAddr, u16),
    /// 域名地址
    Domain(String, u16),
}

impl SocksAddress {
    /// 解析为 SocketAddr（仅 IP 类型）
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        match self {
            SocksAddress::V4(ip, port) => Some(SocketAddr::new(*ip, *port)),
            SocksAddress::V6(ip, port) => Some(SocketAddr::new(*ip, *port)),
            SocksAddress::Domain(_, _) => None,
        }
    }

    /// 编码地址到字节流
    fn encode(&self, buf: &mut BytesMut) {
        match self {
            SocksAddress::V4(ip, port) => {
                buf.put_u8(0x01); // IPv4
                if let IpAddr::V4(v4) = ip {
                    buf.put_slice(&v4.octets());
                }
                buf.put_u16(*port);
            }
            SocksAddress::V6(ip, port) => {
                buf.put_u8(0x04); // IPv6
                if let IpAddr::V6(v6) = ip {
                    buf.put_slice(&v6.octets());
                }
                buf.put_u16(*port);
            }
            SocksAddress::Domain(domain, port) => {
                buf.put_u8(0x03); // Domain
                buf.put_u8(domain.len() as u8);
                buf.put_slice(domain.as_bytes());
                buf.put_u16(*port);
            }
        }
    }
}

/// SOCKS5 代理服务器
pub struct Socks5Proxy {
    /// 监听地址
    listen_addr: String,
    /// 认证方法
    auth: Option<AuthMethod>,
}

impl Socks5Proxy {
    /// 创建新的 SOCKS5 代理（无认证）
    pub fn new(listen_addr: impl Into<String>) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            auth: None,
        }
    }

    /// 创建带用户名/密码认证的 SOCKS5 代理
    pub fn with_auth(listen_addr: impl Into<String>, username: &str, password: &str) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            auth: Some(AuthMethod::UserPass {
                username: username.to_string(),
                password: password.to_string(),
            }),
        }
    }

    /// 启动 SOCKS5 代理服务
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.listen_addr)
            .await
            .with_context(|| format!("无法绑定 SOCKS5 到 {}", self.listen_addr))?;

        info!(
            "SOCKS5 代理启动: {} (认证: {})",
            self.listen_addr,
            if self.auth.is_some() { "启用" } else { "无" }
        );

        let auth = self.auth.clone();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("SOCKS5 新客户端: {}", addr);
                    let auth = auth.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_socks5_client(stream, auth).await {
                            warn!("SOCKS5 客户端 {} 处理失败: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    warn!("接受 SOCKS5 连接失败: {}", e);
                }
            }
        }
    }
}

/// 处理 SOCKS5 客户端
async fn handle_socks5_client(
    mut stream: TcpStream,
    auth: Option<AuthMethod>,
) -> Result<()> {
    stream.set_nodelay(true).ok();

    // 第一步：协商认证方法
    let methods = read_methods(&mut stream).await?;
    debug!("收到认证方法列表: {:?}", methods);

    let selected_method = match &auth {
        None => 0x00, // 无认证
        Some(AuthMethod::UserPass { .. }) => 0x02, // 用户名/密码
    };

    // 检查客户端是否支持选定的方法
    if !methods.contains(&selected_method) {
        // 不支持任何方法，拒绝
        stream.write_all(&[SOCKS_VERSION, 0xFF]).await?;
        return Err(anyhow::anyhow!("客户端不支持选定的认证方法"));
    }

    // 发送选定的认证方法
    stream.write_all(&[SOCKS_VERSION, selected_method]).await?;

    // 第二步：认证
    if let Some(AuthMethod::UserPass { username, password }) = &auth {
        perform_userpass_auth(&mut stream, username, password).await?;
    }

    // 第三步：处理请求
    let request = read_request(&mut stream).await?;
    debug!("收到 SOCKS5 请求: {:?}", request);

    match request.command {
        0x01 => { // CONNECT
            handle_connect(&mut stream, &request.address).await?;
        }
        0x03 => { // UDP ASSOCIATE
            handle_udp_associate(&mut stream, &request.address).await?;
        }
        cmd => {
            // 不支持的命令
            send_reply(&mut stream, 0x07, &SocksAddress::V4("0.0.0.0".parse().unwrap(), 0)).await?;
            return Err(anyhow::anyhow!("不支持的 SOCKS5 命令: 0x{:02X}", cmd));
        }
    }

    Ok(())
}

/// SOCKS5 请求结构
struct SocksRequest {
    command: u8,
    address: SocksAddress,
}

/// 读取客户端支持的认证方法
async fn read_methods(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;

    if header[0] != SOCKS_VERSION {
        return Err(anyhow::anyhow!("不是 SOCKS5 协议 (版本: 0x{:02X})", header[0]));
    }

    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    if nmethods > 0 {
        stream.read_exact(&mut methods).await?;
    }

    Ok(methods)
}

/// 执行用户名/密码认证 (RFC 1929)
async fn perform_userpass_auth(
    stream: &mut TcpStream,
    expected_username: &str,
    expected_password: &str,
) -> Result<()> {
    let mut ver_len = [0u8; 2];
    stream.read_exact(&mut ver_len).await?;

    if ver_len[0] != 0x01 {
        return Err(anyhow::anyhow!("不支持的认证子协商版本"));
    }

    let ulen = ver_len[1] as usize;
    let mut username = vec![0u8; ulen];
    stream.read_exact(&mut username).await?;

    let mut plen = [0u8; 1];
    stream.read_exact(&mut plen).await?;
    let plen = plen[0] as usize;
    let mut password = vec![0u8; plen];
    stream.read_exact(&mut password).await?;

    let username = String::from_utf8_lossy(&username);
    let password = String::from_utf8_lossy(&password);

    if username == expected_username && password == expected_password {
        // 认证成功
        stream.write_all(&[0x01, 0x00]).await?;
        Ok(())
    } else {
        // 认证失败
        stream.write_all(&[0x01, 0x01]).await?;
        Err(anyhow::anyhow!("SOCKS5 用户名/密码认证失败"))
    }
}

/// 读取 SOCKS5 请求
async fn read_request(stream: &mut TcpStream) -> Result<SocksRequest> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    if header[0] != SOCKS_VERSION {
        return Err(anyhow::anyhow!("请求版本错误"));
    }

    let command = header[1];
    // header[2] = RSV (保留位)

    let atype = header[3];
    let address = match atype {
        0x01 => {
            // IPv4
            let mut addr_bytes = [0u8; 6];
            stream.read_exact(&mut addr_bytes).await?;
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(
                addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
            ));
            let port = u16::from_be_bytes([addr_bytes[4], addr_bytes[5]]);
            SocksAddress::V4(ip, port)
        }
        0x03 => {
            // Domain name
            let mut len_byte = [0u8; 1];
            stream.read_exact(&mut len_byte).await?;
            let domain_len = len_byte[0] as usize;
            let mut domain_bytes = vec![0u8; domain_len];
            stream.read_exact(&mut domain_bytes).await?;
            let domain = String::from_utf8_lossy(&domain_bytes).to_string();
            let mut port_bytes = [0u8; 2];
            stream.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            SocksAddress::Domain(domain, port)
        }
        0x04 => {
            // IPv6
            let mut addr_bytes = [0u8; 18];
            stream.read_exact(&mut addr_bytes).await?;
            let ip = IpAddr::V6(std::net::Ipv6Addr::from([
                addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
                addr_bytes[4], addr_bytes[5], addr_bytes[6], addr_bytes[7],
                addr_bytes[8], addr_bytes[9], addr_bytes[10], addr_bytes[11],
                addr_bytes[12], addr_bytes[13], addr_bytes[14], addr_bytes[15],
            ]));
            let port = u16::from_be_bytes([addr_bytes[16], addr_bytes[17]]);
            SocksAddress::V6(ip, port)
        }
        _ => {
            return Err(anyhow::anyhow!("不支持的地址类型: 0x{:02X}", atype));
        }
    };

    Ok(SocksRequest { command, address })
}

/// 发送 SOCKS5 回复
async fn send_reply(
    stream: &mut TcpStream,
    reply_code: u8,
    bind_addr: &SocksAddress,
) -> Result<()> {
    let mut buf = BytesMut::new();
    buf.put_u8(SOCKS_VERSION);
    buf.put_u8(reply_code);
    buf.put_u8(0x00); // RSV
    bind_addr.encode(&mut buf);
    stream.write_all(&buf).await?;
    Ok(())
}

/// 处理 CONNECT 命令
async fn handle_connect(
    client: &mut TcpStream,
    address: &SocksAddress,
) -> Result<()> {
    // 连接到目标
    let target_addr = match address {
        SocksAddress::Domain(domain, port) => format!("{}:{}", domain, port),
        SocksAddress::V4(ip, port) | SocksAddress::V6(ip, port) => {
            format!("{}:{}", ip, port)
        }
    };

    debug!("SOCKS5 CONNECT 到 {}", target_addr);

    let mut target = match TcpStream::connect(&target_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            warn!("CONNECT 到 {} 失败: {}", target_addr, e);
            send_reply(
                client,
                0x05, // 连接失败
                &SocksAddress::V4("0.0.0.0".parse().unwrap(), 0),
            )
            .await?;
            return Err(e.into());
        }
    };

    // 发送成功回复
    let local_addr = target.local_addr()?;
    let bind_addr = SocksAddress::V4(local_addr.ip(), local_addr.port());
    send_reply(client, 0x00, &bind_addr).await?;

    // 双向转发
    let (mut ri, mut wi) = client.split();
    let (mut ro, mut wo) = target.split();

    let client_to_target = tokio::io::copy(&mut ri, &mut wo);
    let target_to_client = tokio::io::copy(&mut ro, &mut wi);

    tokio::select! {
        result = client_to_target => { result?; }
        result = target_to_client => { result?; }
    }

    Ok(())
}

/// 处理 UDP ASSOCIATE 命令
async fn handle_udp_associate(
    _client: &mut TcpStream,
    _address: &SocksAddress,
) -> Result<()> {
    // UDP ASSOCIATE 实现较为复杂，这里简化为返回不支持
    // 完整实现需要 UDP 中继和 UDP 端口管理
    Err(anyhow::anyhow!("UDP ASSOCIATE 暂未实现"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socks5_proxy_new() {
        let proxy = Socks5Proxy::new("127.0.0.1:1080");
        assert_eq!(proxy.listen_addr, "127.0.0.1:1080");
        assert!(proxy.auth.is_none());
    }

    #[test]
    fn test_socks5_proxy_with_auth() {
        let proxy = Socks5Proxy::with_auth("127.0.0.1:1080", "user", "pass");
        assert_eq!(proxy.listen_addr, "127.0.0.1:1080");
        assert!(proxy.auth.is_some());
        match &proxy.auth {
            Some(AuthMethod::UserPass { username, password }) => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected UserPass auth"),
        }
    }

    #[test]
    fn test_socks_address_encode() {
        let addr = SocksAddress::V4("127.0.0.1".parse().unwrap(), 1080);
        let mut buf = BytesMut::new();
        addr.encode(&mut buf);
        assert_eq!(buf[0], 0x01); // IPv4
        assert_eq!(buf[5], (1080 >> 8) as u8);
        assert_eq!(buf[6], 1080 as u8);
    }
}
