# net-tools

基于 Tokio + Rustls 的高性能网络工具集，提供端口转发、SOCKS5 代理和 TLS 加密隧道功能。

## 功能特性

- **TCP 端口转发** — 高性能双向转发，支持并发连接数限制
- **SOCKS5 代理** — 完整支持 CONNECT 命令，可选用户名/密码认证
- **TLS 加密隧道** — 基于 rustls 的安全隧道，支持自签名证书和文件证书
- **连接池** — 通用连接池实现，支持异步工厂模式和健康检查
- **优雅关闭** — 支持 Ctrl+C 信号处理，主动关闭所有连接

## 快速开始

```bash
# TCP 端口转发：将本地 8080 转发到 remote:9000
cargo run -- proxy -l 127.0.0.1:8080 -t 192.168.1.100:9000

# SOCKS5 代理（无认证）
cargo run -- socks5 -l 127.0.0.1:1080

# SOCKS5 代理（用户名密码认证）
cargo run -- socks5 -l 127.0.0.1:1080 -u alice -p secret

# TLS 隧道服务端：监听 4433，转发到本地 8080
cargo run -- tunnel-server -l 127.0.0.1:4433 -t 127.0.0.1:8080

# TLS 隧道客户端：本地 8888 转发到隧道服务端
cargo run -- tunnel-client -s 127.0.0.1:4433 -l 127.0.0.1:8888
```

## CLI 选项

| 子命令 | 选项 | 说明 | 默认值 |
|--------|------|------|--------|
| `proxy` | `-l, --listen` | 本地监听地址 | `127.0.0.1:8080` |
| | `-t, --target` | 目标转发地址 | *必填* |
| | `-c, --max-connections` | 最大并发连接数 | `1024` |
| `socks5` | `-l, --listen` | 监听地址 | `127.0.0.1:1080` |
| | `-u, --username` | 认证用户名 | *可选* |
| | `-p, --password` | 认证密码 | *可选* |
| `tunnel-server` | `-l, --listen` | TLS 监听地址 | `127.0.0.1:4433` |
| | `-t, --target` | 解密后转发地址 | *必填* |
| | `-c, --cert` | PEM 证书文件 | *自签名* |
| | `-k, --key` | PEM 私钥文件 | *自签名* |
| `tunnel-client` | `-s, --server` | 隧道服务器地址 | *必填* |
| | `-l, --local-listen` | 本地转发端口 | `127.0.0.1:8888` |

## 项目结构

```
examples/net-tools/
├── Cargo.toml          # 依赖配置
├── src/
│   ├── main.rs         #  CLI 入口（clap 命令行解析）
│   ├── lib.rs          #  库入口（模块导出）
│   ├── proxy.rs        #  TCP 端口转发（带连接池和限流）
│   ├── socks5.rs       #  SOCKS5 协议实现（RFC 1928）
│   ├── tunnel.rs       #  TLS 加密隧道（服务端+客户端）
│   ├── pool.rs         #  通用连接池（异步工厂+健康检查）
│   └── cert.rs         #  自签名证书生成
├── tests/
│   └── integration_tests.rs  # 集成测试
└── README.md
```

## 技术要点

- **Tokio** — 全异步 I/O，基于 `tokio::io::copy_bidirectional` 双向转发
- **rustls** — 纯 Rust TLS 实现，无 OpenSSL 依赖
- **clap** — 声明式 CLI 参数解析，自动生成帮助信息
- **连接池** — `Semaphore` 限流 + `tokio::sync::mpsc` 连接复用
- **SOCKS5** — 完整实现 RFC 1928，支持无认证和用户名/密码认证
- **自签名证书** — 使用 `rcgen` 运行时生成，支持 SubjectAltName

## 示例用法

### 端口转发 + 压力测试

```bash
# 终端1：启动转发
cargo run -- proxy -l 127.0.0.1:8080 -t 127.0.0.1:9000

# 终端2：启动 echo 服务（使用 nc）
nc -l 127.0.0.1 9000 -k

# 终端3：测试
echo "hello" | nc 127.0.0.1 8080
```

### TLS 加密隧道

```bash
# 终端1：启动隧道服务端（转发到本地 echo 服务）
cargo run -- tunnel-server -l 0.0.0.0:4433 -t 127.0.0.1:9000

# 终端2：启动隧道客户端
cargo run -- tunnel-client -s 127.0.0.1:4433 -l 127.0.0.1:8888

# 终端3：通过加密隧道发送数据
echo "secure data" | nc 127.0.0.1 8888
```

### SOCKS5 代理

```bash
# 启动代理
cargo run -- socks5 -l 127.0.0.1:1080

# 使用 curl 测试
curl --socks5 127.0.0.1:1080 http://example.com

# 使用浏览器配置 SOCKS5 代理为 127.0.0.1:1080
```

## 运行测试

```bash
cargo test --test integration_tests -- --nocapture
```

## 依赖关系

| Crate | 版本 | 用途 |
|-------|------|------|
| tokio | 1 | 异步运行时 |
| tokio-rustls | 0.27 | TLS 异步适配 |
| rustls | 0.23 | TLS 协议实现 |
| rustls-pemfile | 2 | PEM 证书解析 |
| rustls-pki-types | 1 | PKI 类型定义 |
| webpki-roots | 0.26 | Mozilla 根证书 |
| rcgen | 0.13 | 自签名证书生成 |
| clap | 4 | CLI 参数解析 |
| anyhow | 1 | 错误处理 |
| tracing | 0.1 | 日志/诊断 |
| bytes | 1 | 缓冲管理 |
| async-trait | 0.1 | 异步 trait |
