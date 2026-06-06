---
name: cargo-agent-config-and-conventions
description: Configuration loading, path conventions, UI patterns, and coding conventions used throughout the cargo-agent codebase.
metadata:
  type: project
---

## 配置系统

**文件**：`src/config/mod.rs`, `src/config/env.rs`, `src/config/schema.rs`

**配置加载优先级**：
1. `~/.cargo-agent/config.yaml`（支持 `$VAR` 环境变量展开）
2. `~/.hermes/config.yaml`（Hermes 配置映射，转换为 OpenAI 兼容格式）
3. 环境变量：`CARGO_API_KEY` → `OPENAI_API_KEY` → `ANTHROPIC_API_KEY` → `ANTHROPIC_AUTH_TOKEN`
4. 默认值

**API Key 解析顺序**：
- 配置文件中的 `api_key`
- `CARGO_API_KEY`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `ANTHROPIC_AUTH_TOKEN`

**Base URL 解析**：
- 配置文件中的 `model.base_url`
- `ANTHROPIC_BASE_URL` 环境变量
- 默认：`https://api.openai.com`

**Hermes 配置映射**：
- `map_to_openai_compatible_url()` — 将 Hermes 的 base_url 映射到 OpenAI 兼容端点
- 已知提供商映射：DashScope、OpenAI

## 路径约定

**文件**：`src/constants.rs`

- `AGENT_DIR` → `~/.cargo-agent/`
- 可通过 `CARGO_AGENT_HOME` 环境变量覆盖
- memories.db → `~/.cargo-agent/memories.db`
- skills → `~/.cargo-agent/skills/`
- plugins → `~/.cargo-agent/plugins/`

## UI 约定

**文件**：`src/ui/mod.rs`

- `colored` 用于终端颜色
- `crossterm` 用于光标/终端控制
- Spinner 用于加载指示
- 状态栏显示 token 用量、消息计数、模型名称、耗时
- `/dashboard` 命令启动 TUI 仪表板

**Slash 命令**：
- 两种语法：`/cmd args` 和 `/cmd:args`
- 三层处理：
  1. 静态命令（help、clear、quit、version）— `cli_commands.rs`
  2. 动态命令（tools、mem、git、tasks、skills、export、stats）— `Gateway::handle_slash()`
  3. 未知命令 — 传递给 LLM

## 编码约定

- 模块导出模式：`mod.rs` 导出子模块
- 异步模式：`tokio` + `async_trait`
- 错误处理：`anyhow::Result` + `String` 错误（工具层）
- 测试：标准 `#[cfg(test)]` 模块，使用临时文件和临时数据库
- 文档测试：大量 `/// # Example` doctest
- 工具注册：每个工具文件有 `register_all(&mut registry)` 函数

**为什么**：了解配置加载、路径和编码约定是有效扩展和修改代码的基础。
