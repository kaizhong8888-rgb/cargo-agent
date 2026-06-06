use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 正在启动 MCP Client...");

    // 1. 启动外部 MCP Server (这里以官方的 Filesystem Server 为例)
    // 注意：你的电脑需要安装 Node.js 才能运行 npx
    let mut child = Command::new("npx")
        .arg("-y")
        .arg("@modelcontextprotocol/server-filesystem")
        .arg(".") // 允许访问当前目录
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut writer = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // 2. 发送 Initialize 请求
    println!("🔗 正在初始化连接...");
    let init_req = build_request(1, "initialize", json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "rust-mcp-client",
            "version": "0.1.0"
        }
    }));
    send_request(&mut writer, &init_req).await?;
    let _init_resp = read_response(&mut reader).await?;

    // 发送 initialized 通知 (MCP 协议规范要求)
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    });
    send_request(&mut writer, &notif).await?;

    // 3. 列出可用工具
    println!("📦 正在获取工具列表...");
    let list_req = build_request(2, "tools/list", Value::Null);
    send_request(&mut writer, &list_req).await?;
    let list_resp = read_response(&mut reader).await?;

    if let Some(tools) = list_resp.get("result")?.get("tools") {
        println!("✅ 发现以下工具:");
        for tool in tools.as_array().unwrap_or(&vec![]) {
            let name = tool["name"].as_str().unwrap_or("unknown");
            let desc = tool["description"].as_str().unwrap_or("No description");
            println!("   - {} : {}", name, desc);
        }
    } else {
        println!("❌ 未获取到工具列表");
    }

    // 4. 调用工具 (读取当前目录下的 Cargo.toml)
    println!("\n📄 正在调用 read_file 工具...");
    let call_req = build_request(3, "tools/call", json!({
        "name": "read_file",
        "arguments": {
            "path": "./Cargo.toml"
        }
    }));

    send_request(&mut writer, &call_req).await?;
    let call_resp = read_response(&mut reader).await?;

    // 解析返回内容
    if let Some(content) = call_resp.get("result")?.get("content") {
        for item in content.as_array().unwrap_or(&vec![]) {
            if let Some(text) = item.get("text") {
                println!("📄 文件内容:\n{}", text.as_str().unwrap());
            }
        }
    } else if let Some(err) = call_resp.get("error") {
        eprintln!("❌ 工具调用失败: {}", err);
    }

    // 5. 清理资源
    drop(writer);
    child.wait().await?;

    println!("\n✨ MCP 交互演示完成!");
    Ok(())
}

// 辅助函数：构建 JSON-RPC 请求
fn build_request(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })
}

// 辅助函数：发送请求
async fn send_request<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let line = serde_json::to_string(msg)?;
    writer.write_all(format!("{}\n", line).as_bytes()).await?;
    Ok(())
}

// 辅助函数：读取响应
async fn read_response(reader: &mut BufReader<tokio::process::ChildStdout>) -> Result<Value, Box<dyn std::error::Error>> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.trim().is_empty() {
        return Err("Empty response".into());
    }
    let resp: Value = serde_json::from_str(line.trim())?;
    Ok(resp)
}
