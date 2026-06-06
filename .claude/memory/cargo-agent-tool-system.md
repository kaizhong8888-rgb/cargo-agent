---
name: cargo-agent-tool-system
description: How tools work in cargo-agent: Tool trait, ToolRegistry, 64 built-in tools across 12 categories, and how to add new ones.
metadata:
  type: project
---

## 工具系统架构

**核心 trait**（`src/tools/registry.rs`）：
```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String>;
}
```

**注册方式**：`ToolRegistry::register(Box::new(MyTool))`

## 64 个内置工具（按类别）

### 代码相关
- `file_tools` — read/write/list/grep
- `code_analyzer_tool` — Rust 代码结构分析
- `code_quality_tool` — 质量评分、重复检测
- `code_executor` — 沙箱 cargo run/build/test/check/clippy
- `code_transform` — 安全重构（derive、重命名、可见性）
- `code_review/` — 代码审查（analyzer, checks, patterns, reports, config）
- `test_generator` — 测试生成
- `ast_analyzer` — AST 级分析（基于 syn）
- `smart_refactor` — 智能重构
- `benchmark_tool` — 性能微基准测试
- `async_profiler` — 异步阻塞检测
- `clippy_lint_tool` — Clippy lint 运行和分类
- `fuzz_driver` — cargo-fuzz 目标生成

### Git 相关
- `git_tools` — status/diff/log/clone/commit/push
- `git_workflow_tool` — 分支管理、changelog、发布自动化

### 项目和依赖
- `scaffold` — 项目脚手架（cli/lib/web/game）
- `dep_manager` — 依赖 add/remove/update/tree/audit
- `ci_cd_tool` — CI/CD 配置生成和运行
- `container_tool` — Dockerfile/docker-compose 生成
- `cross_compile` — 交叉编译配置
- `license_audit` — 依赖许可证审计

### 数据和处理
- `data_processor` — CSV/JSON 解析、过滤、排序、聚合
- `chart_generator` — 数据可视化（饼图、柱状图、折线图）
- `text_processor` — 大小写转换、编码、UUID、正则
- `database_tool` — SQL 查询、表管理
- `db_migration` — 数据库迁移生成（SQLx/Diesel/SeaORM）
- `csv` 工具（内建于 data_processor）

### 网络和通信
- `net_tools` — URL fetch、HTTP 客户端
- `doc_search` — docs.rs/crates.io 查找
- `github_tool` — PR/issue/CI 状态
- `mail_tool` — 邮件发送
- `notify` — Webhook 通知（Slack、钉钉）
- `image_tool` — 图片分析和操作

### 安全
- `security_scanner` — 安全模式检测、依赖审计
- `crypto_tool` — AES 加密、哈希、签名、JWT
- `env_secret` — 密钥管理（SecretStore）
- `env_file_tool` — .env 文件解析和验证
- `hash_tool` — 文件/字符串校验和

### 记忆和知识
- `memory_tool` — SQLite 记忆存储
- `evolution_tools` — 自我进化追踪
- `task_planner` — 任务分解和追踪
- `task_pool` — 并发 shell 命令
- `todo_manager` — 个人 TODO 列表
- `scheduler` — 定时任务管理

### 文档和模板
- `doc_generator` — API 文档、README 生成
- `diagram` — Mermaid 架构图生成
- `markdown_tool` — Markdown 转 HTML/TOC/lint
- `openapi_tool` — OpenAPI spec 生成
- `template_tool` — 模板引擎（minijinja）
- `pdf_tool` — PDF 生成

### 实用工具
- `hello_tool` — 演示工具
- `fortune_tool` — Rust 编程智慧
- `date_time_tool` — 时区、日期运算
- `regex_tool` — 高级正则表达式
- `diff_tool` — 文本/代码比较
- `log_analyzer` — 日志分析
- `sysmonitor_tool` — 系统监控
- `process_tool` — 进程管理
- `archive_tool` — 压缩/解压
- `config_store` — 持久化用户偏好
- `llm_tool` — 调用外部 LLM
- `plugin_tool` — 插件市场浏览/安装
- `json_schema_tool` — JSON Schema 验证
- `cron_tool` — cron 任务管理

## 添加工具的流程

1. 在 `src/tools/builtin/` 创建文件实现 `Tool` trait
2. 在 `src/tools/builtin/mod.rs` 添加 `pub mod your_tool;`
3. 在 `Gateway::new()` 中添加 `register_all(&mut tool_registry)` 调用
4. 在 `cli_commands.rs:tools_text()` 中添加到 `/tools` 命令列表
5. 更新 `Gateway::new()` 中的系统提示以提及新工具

**为什么**：工具系统是 cargo-agent 的核心扩展机制，了解所有现有工具和如何添加新工具至关重要。
