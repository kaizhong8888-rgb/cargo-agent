---
name: cargo-agent-architecture
description: Complete architecture map of cargo-agent's source modules, data flow, and core runtime loop.
metadata:
  type: project
---

## 架构概览

```
src/
├── main.rs              — CLI 入口：REPL 循环 + 单次模式
├── lib.rs               — 公共模块导出（18 个模块）
├── gateway/mod.rs       —  orchestrator：串联 config → client → tool registry → skills → agent
├── agent/core.rs        — AIAgent：聊天循环 + 工具执行 + 记忆注入 + 上下文截断
├── model/
│   ├── client.rs        — LLM API 客户端（OpenAI 兼容 + Anthropic 自动适配）
│   └── router.rs        — 基于任务复杂度的模型路由（Low/Medium/High）
├── tools/
│   ├── mod.rs           — Tool trait + ToolContext + ToolRegistry 导出
│   ├── registry.rs      — 工具注册表（name → impl 映射）
│   └── builtin/         — 64 个工具实现
├── memory/sqlite_store.rs  — SQLite 持久化记忆存储（TF-IDF 语义搜索）
├── skills/mod.rs        — YAML 技能系统（关键词触发的上下文注入）
├── config/              — 配置加载（~/.cargo-agent/config.yaml）
├── hooks.rs             — 钩子系统（before/after chat/tool）
├── metrics.rs           — 会话级别指标（原子计数器）
├── health.rs            — HTTP 健康检查端点（端口 8787）
├── mcp/                 — MCP 服务器支持
├── plugin/              — 插件市场
├── state/               — 会话状态持久化
├── trading/             — 量化交易模块（26 个子模块）
├── tui/                 — 终端仪表板 UI
└── ui/                  — 终端 UI（spinner、颜色、横幅）
```

## 核心数据流

```
用户输入 → main.rs REPL 循环
    → Gateway::new(config) 初始化所有组件
    → Gateway::handle_message(text)
        → AIAgent::chat(text)
            → inject_skill_context() — 根据关键词激活技能
            → inject_memory_context() — TF-IDF 语义搜索相关记忆
            → run_turns()
                → build_tool_schemas() — 构建工具 JSON Schema
                → client.chat(messages, tools) — 调用 LLM API
                → 如果有 tool_calls：并行执行工具（futures::join_all）
                → 结果追加到消息列表，继续循环
                → 直到无 tool_calls 或达到 MAX_TURNS
            → 返回最终文本响应
```

## 关键常量

- `MAX_MESSAGES = 200` — 截断前的最大消息数
- `TRUNCATE_KEEP = 5` — 截断后保留的消息数
- `MAX_TURNS = 200` — 每次聊天请求的最大工具调用轮数
- `MAX_RETRIES = 3` — API 重试次数
- `BASE_DELAY_MS = 1000` — 指数退避基础延迟

**为什么**：理解代码如何流动是有效工作的基础。这个架构映射覆盖了所有模块及其交互方式。
