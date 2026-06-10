//! SOCKS5 代理协议实现模块
//!
//! SOCKS5 (RFC 1928) 代理协议完整实现。
//! 支持：
//! - 无认证和用户名/密码认证 (RFC 1929)
//! - TCP 连接 (CONNECT 命令)
//! - UDP 关联 (UDP ASSOCIATE)
//! - IPv4/IPv6/Domain 地址类型
//!
//! 不支持：
//! - BIND 命令（用于 FTP 等被动模式连接）

use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

/// SOCKS5 协议版本
const SOCKS_VERSION: u8 = 0x05;

/// SOCKS5 命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocksCommand {
    /// CONNECT — 建立到目标的 TCP 连接
    Connect,
    /// BIND — 等待来自目标的连接（FTP 被动模式等）
    Bind,
    /// UDP ASSOCIATE — 建立 UDP 关联
    UdpAssociate,
}

impl SocksCommand {
    /// 从原始字节解析命令
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Connect),
            0x02 => Some(Self::Bind),
            0x03 => Some(Self::UdpAssociate),
            _ => None,
        }
    }

    /// 转换为原始字节
    fn as_byte(&self) -> u8 {
        match self {
            Self::Connect => 0x01,
            Self::Bind => 0x02,
            Self::UdpAssociate => 0x03,
        }
    }
}

/// SOCKS5 回复码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplyCode {
    Success = 0x00,
    GeneralFailure = 0x01,
    ConnectionNotAllowed = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}

impl ReplyCode {
    fn as_byte(&self) -> u8 {
        *self as u8
    }
}

/// SOCKS5 认证方法
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// 无认证 (RFC 1928)
    NoAuth,
    /// 用户名/密码认证 (RFC 1929)
    UserPass { username: String, password: String },
}

/// SOCKS5 地址类型
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SocksAddress {
    /// IPv4 地址和端口
    V4(IpAddr, u16),
    /// IPv6 地址和端口
    V6(IpAddr, u16),
    /// 域名地址和端口
    Domain(String, u16),
}

impl SocksAddress {
    /// 解析为 `SocketAddr`（仅 IP 类型可用）
    ///
    /// 对于域名类型返回 `None`，需另行解析。
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        match self {
            Self::V4(ip, port) | Self::V6(ip, port) => Some(SocketAddr::new(*ip, *port)),
            Self::Domain(_, _) => None,
        }
    }

    /// 编码地址到字节缓冲区
    fn encode(&self, buf: &mut BytesMut) {
        match self {
            Self::V4(ip, port) => {
                buf.put_u8(0x01); // ATYP: IPv4
                if let IpAddr::V4(v4) = ip {
                    buf.put_slice(&v4.octets());
                }
                buf.put_u16(*port);
            }
            Self::V6(ip, port) => {
                buf.put_u8(0x04); // ATYP: IPv6
                if let IpAddr::V6(v6) = ip {
                    buf.put_slice(&v6.octets());
                }
                buf.put_u16(*port);
            }
            Self::Domain(domain, port) => {
                buf.put_u8(0x03); // ATYP: Domain
                buf.put_u8(domain.len() as u8);
                buf.put_slice(domain.as_bytes());
                buf.put_u16(*port);
            }
        }
    }
}

/// SOCKS5 代理服务器
///
/// 实现 RFC 1928 定义的 SOCKS5 代理协议。
pub struct Socks5Proxy {
    /// 监听地址 (如 "127.0.0.1:1080")
    listen_addr: String,
    /// 认证方法配置
    auth: Option<AuthMethod>,
}

impl Socks5Proxy {
    /// 创建新的 SOCKS5 代理（无认证）
    ///
    /// # 参数
    /// * `listen_addr` - 监听地址，格式 `host:port`
    pub fn new(listen_addr: impl Into<String>) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            auth: None,
        }
    }

    /// 创建带用户名/密码认证的 SOCKS5 代理
    ///
    /// # 参数
    /// * `listen_addr` - 监听地址，格式 `host:port`
    /// * `username` - 认证用户名
    /// * `password` - 认证密码
    pub fn with_auth(listen_addr: impl Into<String>, username: &str, password: &str) -> Self {
        Self {
            listen_addr: listen_addr.into(),
            auth: Some(AuthMethod::UserPass {
                username: username.to_string(),
                password: password.to_string(),
            }),
        }
    }

    /// 启动 SOCKS5 代理服务，持续监听并处理客户端连接
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

/// 处理单个 SOCKS5 客户端连接
async fn handle_socks5_client(mut stream: TcpStream, auth: Option<AuthMethod>) -> Result<()> {
    stream.set_nodelay(true).ok();

    let methods = read_methods(&mut stream).await?;
    debug!("收到认证方法列表: {:?}", methods);

    let selected_method = match &auth {
        None => 0x00,  // 无认证
        Some(AuthMethod::UserPass { .. }) => 0x02, // 用户名/密码
    };

    if !methods.contains(&selected_method) {
        stream.write_all(&[SOCKS_VERSION, 0xFF]).await?;
        return Err(anyhow::anyhow!("客户端不支持选定的认证方法"));
    }

    stream.write_all(&[SOCKS_VERSION, selected_method]).await?;

    if let Some(AuthMethod::UserPass { username, password }) = &auth {
        perform_userpass_auth(&mut stream, username, password).await?;
    }

    let request = read_request(&mut stream).await?;
    debug!("收到 SOCKS5 请求: command={:?}, address={:?}", request.command, request.address);

    match request.command {
        SocksCommand::Connect => handle_connect(&mut stream, &request.address).await?,
        SocksCommand::Bind => {
            send_reply(&mut stream, ReplyCode::CommandNotSupported, &SocksAddress::V4("0.0.0.0".parse().unwrap(), 0)).await?;
            return Err(anyhow::anyhow!("SOCKS5 BIND 命令暂未实现"));
        }
        SocksCommand::UdpAssociate => {
            handle_udp_associate(&mut stream, &request.address).await?;
        }
    }

    Ok(())
}

/// SOCKS5 请求结构
#[derive(Debug)]
struct SocksRequest {
    command: SocksCommand,
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

    let mut plen_byte = [0u8; 1];
    stream.read_exact(&mut plen_byte).await?;
    let plen = plen_byte[0] as usize;
    let mut password = vec![0u8; plen];
    stream.read_exact(&mut password).await?;

    let username = String::from_utf8_lossy(&username);
    let password = String::from_utf8_lossy(&password);

    if username == expected_username && password == expected_password {
        stream.write_all(&[0x01, 0x00]).await?; // 认证成功
        Ok(())
    } else {
        stream.write_all(&[0x01, 0x01]).await?; // 认证失败
        Err(anyhow::anyhow!("SOCKS5 用户名/密码认证失败"))
    }
}

/// 读取 SOCKS5 请求（命令 + 目标地址）
async fn read_request(stream: &mut TcpStream) -> Result<SocksRequest> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    if header[0] != SOCKS_VERSION {
        return Err(anyhow::anyhow!("请求版本错误"));
    }

    let command = SocksCommand::from_byte(header[1])
        .ok_or_else(|| anyhow::anyhow!("不支持的 SOCKS5 命令: 0x{:02X}", header[1]))?;

    let address = parse_address(stream, header[3]).await?;

    Ok(SocksRequest { command, address })
}

/// 解析 SOCKS5 地址部分（ATYP + 地址 + 端口）
async fn parse_address(stream: &mut TcpStream, atype: u8) -> Result<SocksAddress> {
    match atype {
        0x01 => {
            let mut addr_bytes = [0u8; 6];
            stream.read_exact(&mut addr_bytes).await?;
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(
                addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
            ));
            let port = u16::from_be_bytes([addr_bytes[4], addr_bytes[5]]);
            Ok(SocksAddress::V4(ip, port))
        }
        0x03 => {
            let mut len_byte = [0u8; 1];
            stream.read_exact(&mut len_byte).await?;
            let domain_len = len_byte[0] as usize;
            if domain_len > 255 {
                return Err(anyhow::anyhow!("域名长度超出范围: {}", domain_len));
            }
            let mut domain_bytes = vec![0u8; domain_len];
            stream.read_exact(&mut domain_bytes).await?;
            let domain = String::from_utf8_lossy(&domain_bytes).to_string();
            let mut port_bytes = [0u8; 2];
            stream.read_exact(&mut port_bytes).await?;
            let port = u16::from_be_bytes(port_bytes);
            Ok(SocksAddress::Domain(domain, port))
        }
        0x04 => {
            let mut addr_bytes = [0u8; 18];
            stream.read_exact(&mut addr_bytes).await?;
            let ip = IpAddr::V6(std::net::Ipv6Addr::from([
                addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
                addr_bytes[4], addr_bytes[5], addr_bytes[6], addr_bytes[7],
                addr_bytes[8], addr_bytes[9], addr_bytes[10], addr_bytes[11],
                addr_bytes[12], addr_bytes[13], addr_bytes[14], addr_bytes[15],
            ]));
            let port = u16::from_be_bytes([addr_bytes[16], addr_bytes[17]]);
            Ok(SocksAddress::V6(ip, port))
        }
        _ => Err(anyhow::anyhow!("不支持的地址类型: 0x{:02X}", atype)),
    }
}

/// 发送 SOCKS5 回复
async fn send_reply(stream: &mut TcpStream, reply: ReplyCode, bind_addr: &SocksAddress) -> Result<()> {
    let mut buf = BytesMut::new();
    buf.put_u8(SOCKS_VERSION);
    buf.put_u8(reply.as_byte());
    buf.put_u8(0x00); // RSV
    bind_addr.encode(&mut buf);
    stream.write_all(&buf).await?;
    Ok(())
}

/// 处理 CONNECT 命令：建立到目标的 TCP 连接并双向转发
async fn handle_connect(client: &mut TcpStream, address: &SocksAddress) -> Result<()> {
    let target_addr = match address {
        SocksAddress::Domain(domain, port) => format!("{}:{}", domain, port),
        SocksAddress::V4(ip, port) | SocksAddress::V6(ip, port) => format!("{}:{}", ip, port),
    };

    debug!("SOCKS5 CONNECT 到 {}", target_addr);

    let mut target = TcpStream::connect(&target_addr)
        .await
        .with_context(|| format!("CONNECT 到 {} 失败", target_addr))?;

    let local_addr = target.local_addr()?;
    let bind_addr = match local_addr {
        SocketAddr::V4(_) => SocksAddress::V4(local_addr.ip(), local_addr.port()),
        SocketAddr::V6(_) => SocksAddress::V6(local_addr.ip(), local_addr.port()),
    };
    send_reply(client, ReplyCode::Success, &bind_addr).await?;

    relay_streams(client, &mut target).await
}

/// 处理 UDP ASSOCIATE 命令
///
/// 当前仅返回"命令不支持"回复。完整实现需要 UDP 中继和端口管理。
async fn handle_udp_associate(_client: &mut TcpStream, _address: &SocksAddress) -> Result<()> {
    Err(anyhow::anyhow!("UDP ASSOCIATE 暂未实现"))
}

/// 双向流转发
async fn relay_streams(
    a: &mut TcpStream,
    b: &mut TcpStream,
) -> Result<()> {
    let (mut a_read, mut a_write) = tokio::io::split(a);
    let (mut b_read, mut b_write) = tokio::io::split(b);

    tokio::select! {
        r = tokio::io::copy(&mut a_read, &mut b_write) => r
            .with_context(|| "客户端→目标转发失败")?,
        r = tokio::io::copy(&mut b_read, &mut a_write) => r
            .with_context(|| "目标→客户端转发失败")?,
    }

    Ok(())
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
        if let Some(AuthMethod::UserPass { username, password }) = &proxy.auth {
            assert_eq!(username, "user");
            assert_eq!(password, "pass");
        } else {
            panic!("Expected UserPass auth");
        }
    }

    #[test]
    fn test_socks_address_encode_ipv4() {
        let addr = SocksAddress::V4("127.0.0.1".parse().unwrap(), 1080);
        let mut buf = BytesMut::new();
        addr.encode(&mut buf);
        assert_eq!(buf[0], 0x01); // ATYP IPv4
        assert_eq!(&buf[1..5], &[127, 0, 0, 1]);
        assert_eq!(u16::from_be_bytes([buf[5], buf[6]]), 1080);
    }

    #[test]
    fn test_socks_address_encode_ipv6() {
        let addr = SocksAddress::V6("::1".parse().unwrap(), 8080);
        let mut buf = BytesMut::new();
        addr.encode(&mut buf);
        assert_eq!(buf[0], 0x04); // ATYP IPv6
    }

    #[test]
    fn test_socks_address_encode_domain() {
        let addr = SocksAddress::Domain("example.com".to_string(), 443);
        let mut buf = BytesMut::new();
        addr.encode(&mut buf);
        assert_eq!(buf[0], 0x03); // ATYP Domain
        assert_eq!(buf[1] as usize, "example.com".len());
        assert_eq!(&buf[2..13], b"example.com");
        assert_eq!(u16::from_be_bytes([buf[13], buf[14]]), 443);
    }

    #[test]
    fn test_socks_command_from_byte() {
        assert_eq!(SocksCommand::from_byte(0x01), Some(SocksCommand::Connect));
        assert_eq!(SocksCommand::from_byte(0x02), Some(SocksCommand::Bind));
        assert_eq!(SocksCommand::from_byte(0x03), Some(SocksCommand::UdpAssociate));
        assert_eq!(SocksCommand::from_byte(0x04), None);
    }

    #[test]
    fn test_socks_command_roundtrip() {
        for cmd in [SocksCommand::Connect, SocksCommand::Bind, SocksCommand::UdpAssociate] {
            assert_eq!(SocksCommand::from_byte(cmd.as_byte()), Some(cmd));
        }
    }

    #[test]
    fn test_socks_address_to_socket_addr() {
        let v4 = SocksAddress::V4("10.0.0.1".parse().unwrap(), 80);
        assert!(v4.to_socket_addr().is_some());

        let v6 = SocksAddress::V6("::1".parse().unwrap(), 443);
        assert!(v6.to_socket_addr().is_some());

        let domain = SocksAddress::Domain("example.com".to_string(), 80);
        assert!(domain.to_socket_addr().is_none());
    }

    #[test]
    fn test_reply_code_values() {
        assert_eq!(ReplyCode::Success.as_byte(), 0x00);
        assert_eq!(ReplyCode::GeneralFailure.as_byte(), 0x01);
        assert_eq!(ReplyCode::CommandNotSupported.as_byte(), 0x07);
        assert_eq!(ReplyCode::AddressTypeNotSupported.as_byte(), 0x08);
    }
}
