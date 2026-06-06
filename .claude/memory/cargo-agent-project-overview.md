---
name: cargo-agent-project-overview
description: cargo-agent is a self-evolving AI agent CLI written in Rust, connecting to LLM APIs and providing 50+ built-in tools for coding, git, analysis, and self-modification.
metadata:
  type: project
---

cargo-agent 是一个自进化 AI 编程助手 CLI，用 Rust 编写。

**核心概念**：连接 LLM API（OpenAI 兼容协议），提供 REPL 交互循环，AI 可以调用 50+ 内置工具来读写文件、执行代码、运行 git 操作、搜索文档、管理依赖，甚至修改自身源代码。

**项目路径**：`/Users/kai/projects/cargo-agent/`
**配置路径**：`~/.cargo-agent/config.yaml`
**数据路径**：`~/.cargo-agent/`（memories.db、skills/、plugins/）

**关键命令**：
- `cargo run` — 交互 REPL 模式
- `cargo run -- run <prompt>` — 单次模式（直接输出响应）
- `cargo clippy -- -D warnings` — lint（警告视为错误）
- `cargo test` — 运行所有测试

**依赖项**：tokio（async 运行时）、reqwest（HTTP）、rusqlite（SQLite）、serde（序列化）、syn/quote/proc-macro2（AST 分析）、crossterm（终端 UI）、colored（颜色）、tracing（日志）

**LLM 提供商支持**：OpenAI 兼容协议 + Anthropic Messages API（自动检测 base_url 中是否包含 "anthropic"）
**当前配置**：DeepSeek V4 Flash（`https://api.deepseek.com`）

**为什么**：这是项目的核心概述，每次进入项目需要快速理解这是什么。
