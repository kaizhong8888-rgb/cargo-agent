//! net-tools 集成测试

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

#[tokio::test]
async fn test_tcp_proxy_end_to_end() -> Result<()> {
    // 启动一个简单的 echo 服务作为目标
    let echo_addr = start_echo_server().await?;
    let proxy_addr = get_free_addr().await?;

    // 启动转发代理
    let proxy = net_tools::proxy::TcpProxy::new(&proxy_addr, &echo_addr)
        .with_max_connections(10);
    tokio::spawn(async move {
        let _ = proxy.start().await;
    });

    // 等待代理启动
    time::sleep(Duration::from_millis(100)).await;

    // 通过代理发送数据
    let mut stream = TcpStream::connect(&proxy_addr).await?;
    stream.write_all(b"hello proxy").await?;

    // 读取响应
    let mut buf = vec![0u8; 11];
    stream.read_exact(&mut buf).await?;
    assert_eq!(&buf, b"hello proxy");

    Ok(())
}

#[tokio::test]
async fn test_socks5_connect() -> Result<()> {
    // 启动 echo 服务
    let echo_addr = start_echo_server().await?;

    // 启动 SOCKS5 代理
    let socks_addr = get_free_addr().await?;
    let proxy = net_tools::socks5::Socks5Proxy::new(&socks_addr);
    tokio::spawn(async move {
        let _ = proxy.start().await;
    });

    time::sleep(Duration::from_millis(100)).await;

    // 通过 SOCKS5 代理连接 echo 服务
    // SOCKS5 握手：无认证
    let mut stream = TcpStream::connect(&socks_addr).await?;

    // 协商
    stream.write_all(&[0x05, 0x01, 0x00]).await?; // SOCKS5, 1 method, no auth
    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).await?;
    assert_eq!(resp, [0x05, 0x00]);

    // 连接请求
    let host = "127.0.0.1";
    let port = echo_addr.split(':').last().unwrap().parse::<u16>().unwrap();
    let ip_parts: Vec<u8> = host.split('.').map(|p| p.parse().unwrap()).collect();

    let mut connect_req = vec![0x05, 0x01, 0x00, 0x01]; // SOCKS5, CONNECT, RSV, IPv4
    connect_req.extend_from_slice(&ip_parts);
    connect_req.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&connect_req).await?;

    // 读取响应
    let mut resp_header = [0u8; 4];
    stream.read_exact(&mut resp_header).await?;
    assert_eq!(resp_header[0], 0x05); // SOCKS5
    assert_eq!(resp_header[1], 0x00); // Success

    // 剩余地址部分
    let mut addr_rest = vec![0u8; 6]; // IPv4 + port
    stream.read_exact(&mut addr_rest).await?;

    // 通过代理发送和接收数据
    stream.write_all(b"via socks5").await?;
    let mut buf = vec![0u8; 10];
    stream.read_exact(&mut buf).await?;
    assert_eq!(&buf, b"via socks5");

    Ok(())
}

#[tokio::test]
async fn test_connection_pool() -> Result<()> {
    use net_tools::pool::{ConnectionFactory, ConnectionPool};

    struct TestFactory;

    #[async_trait::async_trait]
    impl ConnectionFactory for TestFactory {
        type Connection = String;

        async fn create(&self) -> Result<String> {
            Ok(format!("conn_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()))
        }
    }

    let pool = ConnectionPool::new(TestFactory, 5);

    // 获取连接
    let conn1 = pool.get().await?;
    assert!(conn1.starts_with("conn_"));
    assert_eq!(pool.total_count().await, 1);

    // 归还后复用
    drop(conn1);
    time::sleep(Duration::from_millis(50)).await;

    let conn2 = pool.get().await?;
    assert!(conn2.starts_with("conn_"));
    assert_eq!(pool.total_count().await, 1); // 复用，不创建新连接

    Ok(())
}

/// 启动 echo 服务器
async fn start_echo_server() -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let addr_str = addr.to_string();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    loop {
                        match stream.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                if stream.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        }
    });

    Ok(addr_str)
}

/// 获取一个可用的自由端口
async fn get_free_addr() -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    drop(listener);
    // 短暂延迟以确保端口释放
    time::sleep(Duration::from_millis(50)).await;
    Ok(addr.to_string())
}
