# cargo-agent

> Self-evolving AI Agent CLI written in Rust. 60+ built-in tools for coding, analysis, refactoring, trading, and more.

## What is cargo-agent?

cargo-agent is an autonomous AI programming assistant that runs as a CLI tool. It connects to LLM APIs (OpenAI, Anthropic, Ollama) and provides a tool-use loop where the agent can read/write files, execute code, run git operations, analyze code structure, refactor, test, and even **modify its own source code**.

## Key Features

### 🤖 AI Agent Core
- **Tool-use loop**: 60+ built-in tools the agent can autonomously call
- **Multi-model support**: OpenAI, Anthropic, Ollama (auto-adaptation)
- **Model router**: Automatic model selection based on task complexity
- **Streaming responses**: Real-time output with token usage display

### 🔧 60+ Built-in Tools

| Category | Tools |
|----------|-------|
| **File/FS** | Read, write, list, grep, diff, archive |
| **Code Analysis** | AST analyzer, code quality, code review, Clippy lint |
| **Code Execution** | Sandboxed cargo run/build/test/check/clippy |
| **Refactoring** | Smart refactor: derive, rename, unwrap, visibility |
| **Testing** | Test generator, fuzz driver, benchmark |
| **Git** | Status, diff, log, clone, commit, push, workflow |
| **Dependencies** | Add, remove, update, tree, audit, outdated |
| **Memory** | SQLite-backed persistent memory with TF-IDF search |
| **Tasks** | Task planner, task pool, TODO manager |
| **LLM** | Call external LLMs for code gen/review |
| **Database** | SQL queries, table management, migrations |
| **Crypto** | AES encrypt/decrypt, hash, sign, JWT |
| **Trading** | Quantitative backtesting, strategies, indicators |
| **Security** | Security scanner, license audit |
| **CI/CD** | CI/CD pipeline tools |
| **Data** | CSV/JSON processing, chart generation |
| **Network** | HTTP client, GitHub API, OpenAPI spec generation |
| **Communication** | Email, webhooks, templates, PDF generation |
| **System** | System monitor, Docker generation, cross-compile |

### 🧬 Self-Evolution System
- **Source code modification**: Safely read, write, create, delete files
- **Tool creation**: Dynamically generate new tools and integrate them
- **Verification**: Auto-run cargo check after modifications
- **Memory persistence**: Durable knowledge with tags and importance levels
- **Evolution tracking**: Record tool creation, code changes, lessons learned
- **Self-reflection**: Analyze past behavior, identify improvement patterns

### 📊 Quantitative Trading Module
- **Backtesting engine**: Position sizing, stop-loss, walk-forward validation
- **6+ strategies**: SMA crossover, RSI mean-reversion, MACD, Bollinger Bands, Triple EMA, VWAP-RSI
- **26 submodules**: ML models, factor analysis, feature engineering, portfolio optimization
- **Market data**: Fetch candle data, Chinese stock market integration
- **Risk management**: Market regime detection, portfolio optimizer

### 💬 MCP Server
- **Model Context Protocol**: Expose all 60+ tools to MCP-compatible clients
- **JSON-RPC 2.0**: Standard protocol over stdio
- **External integration**: Connect with any MCP-compatible IDE or agent

## Quick Start

### Prerequisites
- Rust 1.75+ (edition 2021)
- An LLM API key (OpenAI, Anthropic, or Ollama for local)

### Install

```bash
# From crates.io (coming soon)
cargo install cargo-agent

# From source
git clone https://github.com/kaizhong8888-rgb/cargo-agent.git
cd cargo-agent
cargo build --release
```

### Configure

Config lives at `~/.cargo-agent/config.yaml`:

```yaml
model:
  api_key: "your-api-key"
  base_url: "https://api.openai.com/v1"  # OpenAI-compatible
  model: "gpt-4o"
```

API key resolution order: config file → `CARGO_API_KEY` → `OPENAI_API_KEY` → `ANTHROPIC_API_KEY`

### Usage

```bash
# Interactive REPL mode
cargo run

# One-shot mode (bypasses UI, outputs response directly)
cargo run -- run "Analyze the code quality of src/"

# Run with specific prompt
cargo run -- run "Create a new CLI project with serde and tokio"
```

### Example Prompts

```
→ 分析 src/ 目录的代码质量
→ 为 lib.rs 生成单元测试
→ 创建一个新的 CLI 项目
→ 运行 clippy 并修复常见问题
→ 回测 SMA 交叉策略在 A 股的表现
→ 搜索 crates.io 上最好的 Rust web 框架
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  User Interface                  │
│         REPL Loop / One-shot / MCP / TUI         │
├─────────────────────────────────────────────────┤
│                   Gateway                        │
│     ModelClient → ToolRegistry → SkillRegistry  │
├─────────────────────────────────────────────────┤
│                   AIAgent                        │
│  Chat loop → Tool execution → Memory injection  │
├─────────────────────────────────────────────────┤
│                  Tool Layer                      │
│    60+ Tools: File, Code, Git, DB, Trading...   │
├─────────────────────────────────────────────────┤
│                Infrastructure                    │
│    LLM APIs | SQLite | Git | FS | Network       │
└─────────────────────────────────────────────────┘
```

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Public module exports
├── gateway/             # Orchestrator
├── agent/               # AIAgent chat loop
├── model/               # LLM API client + router
├── tools/               # 60+ builtin tools
│   └── builtin/
├── trading/             # Quantitative trading (26 modules)
├── memory/              # SQLite memory store
├── skills/              # YAML skill system
├── config/              # YAML configuration
├── ui/                  # Terminal UI
├── tui/                 # Full-screen dashboard
└── mcp/                 # MCP server
```

## License

Dual-licensed:
- **MIT License** — Free for personal and open-source use. See [LICENSE](LICENSE) for details.
- **Commercial License** — For enterprise use, contact us for pricing and support.

## Commercial Use

For commercial licensing, enterprise support, or custom AI Agent development services, see [PRICING.md](PRICING.md).

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for our test coverage standards.

## Links

- **Documentation**: [docs.rs/cargo-agent](https://docs.rs/cargo-agent) (coming soon)
- **Crates.io**: [crates.io/crates/cargo-agent](https://crates.io/crates/cargo-agent) (coming soon)
- **Issues**: [GitHub Issues](https://github.com/kaizhong8888-rgb/cargo-agent/issues)
