# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**cargo-agent** is a self-evolving AI agent CLI written in Rust. It connects to LLM APIs (OpenAI-compatible) and provides a tool-use loop where the agent can call 25+ built-in tools to read/write files, execute code, run git operations, search docs, manage dependencies, and even modify its own source code.

## Essential Commands

```bash
cargo run                    # Interactive REPL mode
cargo run -- run <prompt>    # One-shot mode (bypasses UI, outputs response directly)
cargo build                  # Compile
cargo check                  # Fast compile-check (use during development)
cargo clippy -- -D warnings  # Lint (treat warnings as errors)
cargo fmt                    # Format code
cargo test                   # Run all tests
cargo test test_name         # Run tests matching pattern
cargo test --lib             # Unit tests only
```

## Architecture

```
src/
├── main.rs              # CLI entry point: REPL loop + one-shot mode
├── lib.rs               # Public module exports
├── gateway/mod.rs       # Orchestrator: wires config → client → tool registry → skills → agent
├── agent/core.rs        # AIAgent: chat loop with tool execution, memory injection, context truncation
├── model/client.rs      # LLM API client (OpenAI-compatible protocol)
├── tools/
│   ├── mod.rs           # Tool trait, ToolContext, ToolRegistry re-exports
│   ├── registry.rs      # Tool trait + ToolRegistry (name → impl mapping)
│   └── builtin/         # 25+ tool implementations:
│       ├── file_tools.rs       # read/write/list/grep
│       ├── fs_tools.rs         # filesystem operations
│       ├── net_tools.rs        # URL fetch, full HTTP client
│       ├── git_tools.rs        # status/diff/log/clone/commit/push
│       ├── code_executor.rs    # sandboxed cargo run/build/test/check/clippy
│       ├── code_analyzer.rs    # code structure analysis
│       ├── code_transform.rs   # safe refactoring (derive, rename, visibility)
│       ├── code_review.rs      # code review tool
│       ├── dep_manager.rs      # dependency add/remove/update/tree/audit
│       ├── scaffold.rs         # project scaffolding (cli/lib/web/game)
│       ├── memory_tool.rs      # SQLite-backed memory store/retrieve
│       ├── task_planner.rs     # task decomposition & tracking
│       ├── task_pool.rs        # concurrent shell commands
│       ├── llm_tool.rs         # call external LLMs for code gen/review
│       ├── database_tool.rs    # SQL queries, table management
│       ├── crypto_tool.rs      # encrypt/decrypt/hash/sign/JWT
│       ├── quantitative_trading_tool.rs  # backtesting, strategies, indicators
│       ├── config_store.rs     # persistent user preferences
│       ├── scheduler.rs        # recurring task management
│       ├── doc_search.rs       # docs.rs/crates.io lookup
│       ├── diagram.rs          # Mermaid diagram generation
│       ├── evolution_tools.rs  # self-evolution tracking
│       └── hello_tool.rs       # demo tool
├── memory/
│   └── sqlite_store.rs    # SQLite persistence for memories
├── skills/mod.rs          # YAML-based skill system (keyword-triggered context injection)
├── config/                # Configuration loading (~/.cargo-agent/config.yaml)
├── ui/                    # Terminal UI (spinner, colors, banners)
├── trading/               # Quantitative trading module
│   ├── mod.rs
│   ├── backtest.rs        # Backtesting engine
│   ├── data.rs            # Market data handling
│   ├── indicators.rs      # Technical indicators
│   ├── strategy.rs        # Trading strategies
│   └── report.rs          # Report generation
└── constants.rs           # Path constants (~/.cargo-agent/)
```

### Core Flow

1. **`main.rs`** starts the REPL loop, reads user input
2. **`Gateway`** initializes `ModelClient`, `ToolRegistry`, `SkillRegistry`, and `AIAgent`
3. **`AIAgent.chat()`** injects skill context + memory, pushes user message, runs `run_turns()`
4. **`run_turns()`** loops: sends messages to LLM → if tool_calls, executes tools → appends results → repeats
5. Tools implement the `Tool` trait (`name`, `description`, `parameters`, `execute`) and register with `ToolRegistry`

### Key Constants

- `MAX_MESSAGES = 200` — conversation window size before truncation
- `TRUNCATE_KEEP = 5` — messages kept after truncation
- `MAX_TURNS = 200` — max tool-call turns per chat request

### Memory System

Memories are stored in SQLite at `~/.cargo-agent/memories/memories.db`. The `inject_memory_context()` method searches for relevant memories by keywords from the user message and injects top-10 by importance as system context.

### Skills System

Skills are YAML files in `~/.cargo-agent/skills/` that provide domain-specific instructions. They activate via keyword matching or `always_active` flag, injecting their `system_instructions` into the conversation.

## Adding New Tools

1. Create a file in `src/tools/builtin/` implementing the `Tool` trait
2. Add `pub mod your_tool;` to `src/tools/builtin/mod.rs`
3. Add `crate::tools::builtin::your_tool::register_all(&mut tool_registry);` in `Gateway::new()` (gateway/mod.rs)
4. Add tool to the tools list in `cli_commands.rs:tools_text()` for `/tools` command
5. Update the system prompt in `Gateway::new()` to mention the new tool

## Configuration

Config lives at `~/.cargo-agent/config.yaml`. Supports environment variable expansion (`$VAR`). API key resolution order: config file → `CARGO_API_KEY` → `OPENAI_API_KEY` → `ANTHROPIC_API_KEY`.

## Testing

Tests use standard `#[cfg(test)]` modules with `#[test]` attributes. The codebase has unit tests in `tools/registry.rs` and `skills/mod.rs`. Run `cargo test` for the full suite.
