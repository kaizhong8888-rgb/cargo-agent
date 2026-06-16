mod tools;

use crate::agent::core::AIAgent;
use crate::config::CargoConfig;
use crate::goal_manager::GoalManager;
use crate::loop_manager::LoopManager;
use crate::mcp::bridge::McpBridge;
use crate::model::client::ModelClient;
use crate::model::router::ModelRouter;
use crate::skills::SkillRegistry;
use crate::tools::builtin::config_store::ConfigStore;
use crate::tools::ToolRegistry;
use anyhow::Result;
use colored::Colorize;
use std::sync::Arc;

pub struct Gateway {
    agent: AIAgent,
    model_router: ModelRouter,
    mcp_bridge: Option<McpBridge>,
    config_store: ConfigStore,
    loop_manager: LoopManager,
    goal_manager: GoalManager,
}

impl Gateway {
    pub fn new(config: CargoConfig) -> Self {
        // Validate config and warn about issues before proceeding
        for issue in config.validate() {
            tracing::warn!("Config validation: {issue}");
        }

        let api_key = config.resolve_api_key().unwrap_or_default();
        let model_name = config.model.name.clone();
        let base_url = config.resolve_base_url();

        let client = ModelClient::new(api_key, model_name.clone(), base_url);

        // Create shared memory store for tools that need it (created early)
        let memory_store = AIAgent::create_memory_store();

        let mut tool_registry = ToolRegistry::new();
        tools::register_builtin_tools(&mut tool_registry, memory_store.clone());

        // Load skills from ~/.cargo-agent/skills/
        let skills_dir = crate::constants::skills_dir();
        let skill_registry = Arc::new(SkillRegistry::load_from_dir(&skills_dir).unwrap_or_else(
            |_| {
                tracing::warn!("Failed to load skills from {}", skills_dir.display());
                SkillRegistry::new()
            },
        ));

        let active_skills = skill_registry.active_skills().len();
        let total_skills = skill_registry.list().len();
        if total_skills > 0 {
            tracing::info!("Loaded {total_skills} skills ({active_skills} always-active)");
        }

        let mut agent = AIAgent::new(client, tool_registry, skill_registry);

        // Share the memoryStore with the agent
        if let Some(store) = memory_store {
            agent.set_memory_store(store);
        }

        // Configure model router for task-complexity-based model selection
        let model_router = ModelRouter::new(model_name.clone());

        agent.set_system_prompt(
            "You are cargo-agent, a self-evolving AI assistant. \
            You can modify your own source code using the self_modify tool (actions: read_file, write_file, create_file, delete_file, patch_file, create_tool, cargo_check, cargo_test). \
            The create_tool action lets you generate new tools: provide tool_name (PascalCase) and tool_spec (full Rust source implementing the Tool trait). \
            Use code_analyze to understand Rust code structure (functions, structs, enums, traits, dependencies, complexity). \
            Use test_generate to analyze Rust source code and generate unit tests, integration tests, and property tests with edge cases, error handling, and boundary conditions. \
            Use benchmark to run performance micro-benchmarks, compare implementations, generate criterion benchmark code, and detect performance hotspots. \
            Use clippy_lint to run cargo clippy with structured output: categorize lints, suggest fixes, score code quality (A+-F grade), and auto-fix common patterns. \
            Use regex_tool for advanced regex testing: test/find_all/replace patterns, validate syntax, explain components, and generate Rust regex code. \
            Use code_quality for code quality scoring (0-100), duplicate detection, and dependency visualization. \
            Use security_scan for code security patterns, dependency audits, and hardcoded secrets detection. \
            Use ci_cd for CI/CD integration (generate configs, run tests/builds, coverage, audit, pre-release checks). \
            Use git_workflow for branch management, changelog generation, release automation, merge with conflict detection, PR description, file blame, and contributor stats. \
            Use doc_gen for documentation generation: auto-generate API docs from Rust source, README.md, architecture docs with Mermaid diagrams, and module documentation. \
            Use smart_refactor for intelligent code refactoring: detect code smells, boolean simplification, modernization, performance optimization, and idiomatic patterns. \
            Use task_planner to decompose complex requests into trackable tasks (actions: decompose, create, list, update, show, delete). \
            Use git_status, git_diff, git_log, git_clone, git_commit, git_push for full Git integration. \
            Use code_execute to compile and run Rust code in an isolated sandbox (supports cargo run/build/test/check/clippy). \
            Use http_client for full HTTP requests (GET/POST/PUT/DELETE with JSON, headers, cookies, multipart). \
            Use project_scaffold to generate complete project structures (cli, lib, web, game templates). \
            Use dep_manager for dependency operations (add, remove, update, tree, audit, outdated). \
            Use code_transform for safe code refactoring (add_derive, remove_derive, replace_unwrap, rename, change_visibility). \
            Use code_review for code quality, security, and best practices analysis. \
            Use doc_search to look up crate info on docs.rs/crates.io. \
            Use diagram to generate Mermaid architecture diagrams. \
            Use config to persist user preferences across sessions. \
            Use scheduler to manage recurring tasks. \
            Use llm to call LLM APIs (OpenAI/Anthropic/Ollama) for code generation, review, explanation, and Q&A. \
            Use database for SQL queries, table management, CSV import/export. \
            Use db_migration for database migrations: generate SQLx/Diesel/SeaORM migration files, \
            diff schema with code models, generate mock data for testing, and list existing migrations. \
            Use crypto for encrypt/decrypt, hash, sign/verify, JWT, password hashing. \
            Use quantitative_trading for backtesting, strategy comparison, technical indicators. \
            Use env_secret to manage environment variables and secrets (list, get, set, remove). \
            Use notify to send notifications via webhooks (Slack, DingTalk, custom URLs). \
            Use image to analyze and manipulate images (info, resize, thumbnail, convert). \
            Use container for containerization: generate Dockerfiles (multi-stage, MUSL static builds), \
            docker-compose configs, multi-arch build scripts, and analyze Rust projects for containerization. \
            Use cross_compile for cross-compilation: target management, cross-build config generation, \
            Wasm size analysis/optimization, and embedded device configuration. \
            Use fuzz_driver for fuzzing: generate cargo-fuzz targets, manage corpus, parse crash reports, \
            recommend fuzzing strategies, and initialize fuzzing in projects. \
            Use data_processor for CSV/JSON data processing: parse, filter, sort, aggregate, \
            stats (mean/median/std_dev), merge/join, head/tail, unique, rename, add_column. \
            Use chart_generator to visualize data: pie (pie chart), bar (horizontal bar), \
            line (line chart with sparkline), table (markdown table), histogram (from binned data). \
            Use text_processor for advanced text manipulation: case conversion (snake/camel/pascal/kebab/title), \
            count (chars/words/lines/bytes), truncate, base64 encode/decode, URL encode/decode, \
            HTML escape/unescape, reverse, trim, pad, repeat, UUID generation (v4), random string generation, \
            word wrap, indent, slugify, regex find/captures/replace/test/split, join/split arrays, \
            string length, shuffle, substring, simple replace, char_at with Unicode info, \
            escape/unescape special characters (\\n, \\t, \\uXXXX, etc.).
            Use todo_manager for personal todo list management: add/list/get/update/delete \
            todos with priorities (low/medium/high/urgent), categories, tags, due dates, \
            search/filter, completion tracking with notes, statistics (overdue/due today/\
            by status/priority/category), archive completed items, and purge archived items.
            Reflect on your growth with self_reflect, and record evolution events with record_evolution. \
            When you learn something important, store it as a memory. \
            When you improve yourself, record the evolution event. \
            Always run cargo check after code modifications to verify correctness.",
        );

        let config_store = ConfigStore::load();

        Self {
            agent,
            model_router,
            mcp_bridge: None, // Initialized asynchronously in new_async
            config_store,
            loop_manager: LoopManager::new(),
            goal_manager: GoalManager::new(),
        }
    }

    /// Create a new Gateway and asynchronously connect MCP servers.
    ///
    /// This is the preferred constructor when MCP server integration is needed.
    /// It connects to configured MCP servers and registers their tools.
    pub async fn new_async(config: CargoConfig) -> Self {
        let mut gateway = Self::new(config.clone());

        // Connect to configured MCP servers
        if !config.mcp_servers.is_empty() {
            let mut bridge = McpBridge::new();
            for (name, server_config) in &config.mcp_servers {
                tracing::info!("Configuring MCP server: {name}");
                bridge.add_config(name, server_config.clone());
            }

            // Register tools directly into the agent's tool registry
            let registry = &mut gateway.agent.tool_registry;
            bridge.start_all(registry).await;

            tracing::info!(
                "MCP bridge: {} server(s) configured, {} connected, {} MCP tool(s) registered",
                config.mcp_servers.len(),
                bridge.connected_count(),
                bridge.total_mcp_tools(),
            );

            gateway.mcp_bridge = Some(bridge);
        }

        gateway
    }

    /// Access the MCP bridge (if configured).
    pub fn mcp_bridge(&self) -> Option<&McpBridge> {
        self.mcp_bridge.as_ref()
    }

    /// Mutable access to the MCP bridge.
    pub fn mcp_bridge_mut(&mut self) -> Option<&mut McpBridge> {
        self.mcp_bridge.as_mut()
    }

    /// Access the loop manager.
    pub fn loop_manager(&self) -> &LoopManager {
        &self.loop_manager
    }

    /// Access the goal manager.
    pub fn goal_manager(&self) -> &GoalManager {
        &self.goal_manager
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Gateway running");
        Ok(())
    }

    pub async fn handle_message(&mut self, text: &str) -> Result<String> {
        let selected = self.model_router.select(text);
        if selected != self.agent.model_name() {
            tracing::debug!(
                from = %self.agent.model_name(),
                to = %selected,
                "Model router switched model for this message"
            );
            self.agent.set_model(selected);
        }
        self.agent.chat(text).await
    }

    /// Return cumulative token usage info.
    pub fn token_usage_info(&self) -> String {
        let usage = self.agent.token_usage();
        if usage.api_calls == 0 {
            "No API calls made yet.".to_string()
        } else {
            format!(
                "API calls: {}\nPrompt tokens:     {}\nCompletion tokens: {}\nTotal tokens:      {}",
                usage.api_calls,
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
            )
        }
    }

    /// Reset token usage counters (e.g. after /clear).
    pub fn reset_token_usage(&mut self) {
        self.agent.reset_token_usage();
    }

    /// Return the model routing decision for a given message.
    pub fn model_routing_info(&self, message: &str) -> String {
        self.model_router.describe(message)
    }

    /// Override the model for the next request based on message complexity.
    pub fn select_model_for_message(&self, message: &str) -> &str {
        self.model_router.select(message)
    }

    /// Unified slash command dispatcher.
    ///
    /// Takes the raw input (starting with `/`), parses it, and returns
    /// a `SlashAction` that drives the REPL loop in `main.rs`.
    pub async fn handle_slash_command(&mut self, input: &str) -> crate::cli_commands::SlashAction {
        use crate::cli_commands::SlashAction;

        // Expand shortcuts before parsing
        let expanded = self.expand_shortcuts(input);
        let (cmd, args) = crate::cli_commands::parse(&expanded);

        match cmd {
            // ── Lifecycle ────────────────────────────────────
            "quit" | "exit" => SlashAction::Exit,
            "clear" | "cls" => {
                print!("\x1b[2J\x1b[H");
                self.clear_conversation();
                SlashAction::Clear
            }

            // ── Dashboard ────────────────────────────────────
            "dashboard" | "dash" => SlashAction::Dashboard,

            // ── Static info (pure text builders) ─────────────
            "help" | "h" => {
                let text = if args.is_empty() {
                    crate::cli_commands::help_general()
                } else {
                    crate::cli_commands::help_topic(args)
                };
                SlashAction::Output(text)
            }
            "version" | "v" => SlashAction::Output(crate::cli_commands::version_text()),
            "status" => SlashAction::Output(crate::cli_commands::status_text()),
            "config" => SlashAction::Output(crate::cli_commands::config_text()),
            "usage" => SlashAction::Output(
                "Token usage is tracked per conversation.\nAsk the agent: 'show token usage'."
                    .into(),
            ),
            "model" => SlashAction::Output(
                "Model routing is automatic based on task complexity.\n\
                     Ask the agent: 'what model complexity is this task?'"
                    .into(),
            ),

            // ── Dynamic (needs runtime state) ────────────────
            "tools" => SlashAction::Output(self.slash_tools(args)),
            "tool" => SlashAction::Output(self.slash_tool_detail(args)),
            "mem" | "memory" => SlashAction::Output(self.slash_memory(args).await),
            "git" => SlashAction::Output(self.slash_git(args)),
            "tasks" | "task" => SlashAction::Output(self.slash_tasks(args)),
            "skills" | "skill" => SlashAction::Output(self.slash_skills(args)),
            "export" => SlashAction::Output(self.slash_export(args)),
            "stats" => SlashAction::Output(self.slash_stats()),
            "mcp" => SlashAction::Output(self.slash_mcp(args).await),

            // ── Shortcut management ─────────────────────────
            "shortcut" | "sc" => SlashAction::Output(self.slash_shortcut(args)),

            // ── Loop management ─────────────────────────────
            "loop" => {
                if args.is_empty() {
                    SlashAction::Output(self.slash_loop_help())
                } else if let Some((subcmd, subargs)) = args.split_once(' ') {
                    match subcmd {
                        "stop" => {
                            let id = subargs.trim().parse::<u64>().ok();
                            if let Some(id) = id {
                                SlashAction::StopLoop { id: id.to_string() }
                            } else {
                                SlashAction::Output("Usage: /loop:stop <id>".into())
                            }
                        }
                        "stop-all" => SlashAction::StopAllLoops,
                        "list" => SlashAction::ListLoops,
                        _ => SlashAction::Output(self.slash_loop_help()),
                    }
                } else {
                    // Parse interval and command: "/loop 5m cargo test" or "/loop 300 cargo test"
                    match self.parse_loop_args(args) {
                        Ok((interval_secs, command)) => SlashAction::Loop {
                            interval_secs,
                            command,
                        },
                        Err(e) => SlashAction::Output(e),
                    }
                }
            }

            // ── Goal management ─────────────────────────────
            "goal" => {
                if args.is_empty() {
                    SlashAction::ShowGoal
                } else if let Some((subcmd, subargs)) = args.split_once(':') {
                    match subcmd {
                        "clear" => SlashAction::ClearGoal,
                        "done" => {
                            if subargs.is_empty() {
                                SlashAction::GoalDone
                            } else {
                                SlashAction::Output("Usage: /goal:done".into())
                            }
                        }
                        _ => SlashAction::Output(self.slash_goal_help()),
                    }
                } else {
                    // Set new goal with the rest as description
                    SlashAction::SetGoal {
                        description: args.to_string(),
                    }
                }
            }

            // ── Unknown — let LLM try ────────────────────────
            _ => SlashAction::PassThrough,
        }
    }

    /// Expand input by resolving any configured shortcut.
    ///
    /// First checks in-memory shortcuts (updated via `/shortcut` commands).
    /// Falls back to disk so that changes made via the `config` tool (LLM)
    /// are also picked up.
    fn expand_shortcuts(&self, input: &str) -> String {
        if !input.starts_with('/') {
            return input.to_string();
        }

        let rest = &input[1..];
        let first_token = match rest.split_whitespace().next() {
            Some(t) => t,
            None => return input.to_string(),
        };

        // Check in-memory first (hot path: set via /shortcut)
        if let Some(expanded) = self.config_store.get_shortcut(first_token) {
            let expanded_rest = rest.replacen(first_token, &expanded, 1);
            return format!("/{}", expanded_rest);
        }

        // Fall back to disk (picks up changes made via config tool / LLM)
        let disk_store = ConfigStore::load();
        if let Some(expanded) = disk_store.get_shortcut(first_token) {
            let expanded_rest = rest.replacen(first_token, &expanded, 1);
            // Sync into memory for next time
            self.config_store.add_shortcut(first_token, &expanded);
            return format!("/{}", expanded_rest);
        }

        input.to_string()
    }

    /// Clear conversation history and reset token usage (used by /clear command).
    pub fn clear_conversation(&mut self) {
        self.agent.clear_conversation();
        self.reset_token_usage();
        self.agent.session_metrics().reset();
    }

    /// Return a reference to the session metrics.
    pub fn session_metrics(&self) -> &std::sync::Arc<crate::metrics::SessionMetrics> {
        self.agent.session_metrics()
    }

    /// Return a reference to the hook manager.
    pub fn hook_manager(&self) -> &crate::hooks::HookManager {
        self.agent.hook_manager()
    }

    // ── Tool commands ──────────────────────────────────────

    // ── Shortcut commands ──────────────────────────────────

    fn slash_shortcut(&self, args: &str) -> String {
        let mut parts = args.splitn(3, ' ');
        let subcmd = match parts.next() {
            Some(s) => s,
            None => return self.slash_shortcut_help(),
        };

        match subcmd {
            "add" | "set" => {
                let alias = match parts.next() {
                    Some(a) => a,
                    None => return "  Usage: `/shortcut add <alias> <command>` — e.g. `/shortcut add t tools`".to_string(),
                };
                let cmd = match parts.next() {
                    Some(c) => c,
                    None => {
                        return "  Usage: `/shortcut add <alias> <command>` — missing command"
                            .to_string()
                    }
                };
                self.config_store.add_shortcut(alias, cmd);
                format!(
                    "  ✅ Shortcut added: /{} → /{}",
                    alias.magenta().bold(),
                    cmd.magenta().bold()
                )
            }
            "remove" | "rm" | "del" | "delete" => {
                let alias = match parts.next() {
                    Some(a) => a,
                    None => {
                        return "  Usage: `/shortcut remove <alias>` — e.g. `/shortcut remove t`"
                            .to_string()
                    }
                };
                self.config_store.remove_shortcut(alias);
                format!("  ✅ Shortcut removed: /{}", alias.magenta().bold())
            }
            "list" | "ls" | "" => {
                let shortcuts = self.config_store.list_shortcuts();
                if shortcuts.is_empty() {
                    return "  📭 No shortcuts defined.\n  Use `/shortcut add <alias> <command>` to create one.".to_string();
                }
                let mut out = String::with_capacity(512);
                out.push_str(&format!(
                    "  {}  {} ({})\n\n",
                    "🔗".bold(),
                    "Command Shortcuts".cyan().bold(),
                    format!("{} defined", shortcuts.len()).dimmed()
                ));
                let mut pairs: Vec<(&String, &String)> = shortcuts.iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0));
                for (alias, cmd) in &pairs {
                    out.push_str(&format!(
                        "    /{:<12} → /{}\n",
                        format!("{}", alias.magenta().bold()),
                        cmd.magenta().bold()
                    ));
                }
                out.push_str(&"\n  Use `/shortcut add <alias> <command>` to add more.".dimmed());
                out
            }
            other => format!(
                "  ❌ Unknown subcommand: `{other}`\n  {}",
                self.slash_shortcut_help()
            ),
        }
    }

    fn slash_shortcut_help(&self) -> String {
        "Usage:\n    /shortcut add <alias> <command>   Create a shortcut\n    /shortcut remove <alias>          Remove a shortcut\n    /shortcut list                     List all shortcuts\n\nExample: /shortcut add t tools".to_string()
    }

    // ── Tool commands ──────────────────────────────────────

    fn slash_tools(&self, _args: &str) -> String {
        let tools = self.agent.tool_registry.list_tools();
        let mut out = String::with_capacity(2048);
        out.push_str(&format!(
            "  {}  {}\n\n",
            "🔧".bold(),
            format!("Tools ({} registered)", tools.len()).cyan().bold()
        ));

        // Group tools by category for better readability
        let mut categories: Vec<(&str, Vec<(&str, &str)>)> = vec![
            ("Code", vec![]),
            ("Git", vec![]),
            ("Data", vec![]),
            ("System", vec![]),
            ("AI/LLM", vec![]),
            ("Utils", vec![]),
        ];

        for tool in &tools {
            let name = tool.name();
            let desc = tool.description();
            let cat = match name {
                "code_analyze" | "code_analyzer" | "code_transform" | "code_review"
                | "code_execute" | "scaffold" | "dep_manager" | "self_modify" | "test_generate"
                | "benchmark" => 0,
                "git_status" | "git_diff" | "git_log" | "git_clone" | "git_commit" | "git_push" => {
                    1
                }
                "data_processor" | "chart_generator" | "database" | "config_store"
                | "doc_search" | "diagram" => 2,
                "task_planner" | "task_pool" | "scheduler" | "env_secret" | "notify"
                | "memory_store" | "evolution" | "manage_skills" => 3,
                "llm" | "self_reflect" | "record_evolution" => 4,
                _ => 5,
            };
            categories[cat].1.push((name, desc));
        }

        // Determine which label to use for the catch-all
        let labels = ["Code", "Git", "Data", "System", "AI/LLM", "Utils"];

        for (idx, (_, items)) in categories.iter_mut().enumerate() {
            if items.is_empty() {
                continue;
            }
            items.sort_by(|a, b| a.0.cmp(b.0));
            out.push_str(&format!(
                "  {}  {}\n",
                "▸".cyan().bold(),
                labels[idx].bold()
            ));
            for (name, desc) in items {
                out.push_str(&format!(
                    "    {}  {}\n",
                    name.magenta().bold(),
                    desc.dimmed()
                ));
            }
            out.push('\n');
        }

        out.push_str(&format!(
            "  {} {}",
            "💡".dimmed(),
            "Use `/tool:name` for details on a specific tool.".dimmed()
        ));
        out
    }

    fn slash_tool_detail(&self, name: &str) -> String {
        if name.is_empty() {
            return format!(
                "  {}  {}\n\n  {}",
                "❓".bold(),
                "Tool Lookup".cyan().bold(),
                "Usage: `/tool:tool_name` (e.g. `/tool:code_analyze`)".dimmed()
            );
        }
        match self.agent.tool_registry.get(name) {
            Some(tool) => {
                let mut out = String::with_capacity(1024);
                out.push_str(&format!(
                    "  {}  {}\n\n",
                    "🔧".bold(),
                    format!("Tool: {}", tool.name()).cyan().bold()
                ));
                out.push_str(&format!("  {}\n\n", tool.description().dimmed()));

                let params = tool.parameters();
                if params.is_empty() {
                    out.push_str("  No parameters\n");
                } else {
                    out.push_str(&format!("  {} ({})\n\n", "Parameters".bold(), params.len()));
                    for p in &params {
                        let req = if p.required { "required" } else { "optional" };
                        out.push_str(&format!(
                            "    {} [{}] ({})  {}\n",
                            p.name.magenta().bold(),
                            p.parameter_type.dimmed(),
                            req.dimmed(),
                            p.description.dimmed()
                        ));
                    }
                }
                out
            }
            None => {
                format!("  ❌ Tool `{name}` not found.\n  Use `/tools` to see all available tools.")
            }
        }
    }

    // ── Memory commands ────────────────────────────────────

    async fn slash_memory(&self, args: &str) -> String {
        // Access memory store through the agent
        let store: Option<std::sync::Arc<crate::memory::SqliteMemoryStore>> =
            self.agent.memory_store().cloned();
        let store = match store {
            Some(s) => s,
            None => return "  ❌ Memory store is not available (run the agent first).".to_string(),
        };

        let (subcmd, subargs) = if let Some(pos) = args.find(' ') {
            (&args[..pos], args[pos + 1..].trim())
        } else {
            (args, "")
        };

        match subcmd {
            "ns" | "namespaces" | "namespace" => match store.list_namespaces() {
                Ok(ns_list) => {
                    if ns_list.is_empty() {
                        return "  📭 No memories stored yet.\n  Ask the agent to store something!"
                            .to_string();
                    }
                    let total: usize = ns_list.iter().map(|(_, c)| c).sum();
                    let mut out = String::with_capacity(512);
                    out.push_str(&format!(
                        "  {}  {}\n\n",
                        "🗂".bold(),
                        format!("Namespaces ({total} total)").cyan().bold()
                    ));
                    for (ns, count) in &ns_list {
                        out.push_str(&format!(
                            "    {}  {:>4} memories\n",
                            ns.magenta().bold(),
                            count
                        ));
                    }
                    out
                }
                Err(e) => format!("  ❌ Error reading memory store: {e}"),
            },
            "search" | "s" | "find" | "q" => {
                if subargs.is_empty() {
                    return "  Usage: `/mem:search <query>` — e.g. `/mem:search rust ownership`"
                        .to_string();
                }
                match store.search(None, None, Some(subargs), None, 10) {
                    Ok(results) => {
                        if results.is_empty() {
                            return format!("  🔍 No memories found for: `{subargs}`");
                        }
                        let mut out = String::with_capacity(1024);
                        out.push_str(&format!(
                            "  {}  {} results for \"{}\"\n\n",
                            "🔍".bold(),
                            results.len(),
                            subargs.cyan()
                        ));
                        for m in &results {
                            let preview = if m.value.len() > 80 {
                                format!("{}...", &m.value[..77])
                            } else {
                                m.value.clone()
                            };
                            out.push_str(&format!(
                                "    {}  [imp:{}] {}\n",
                                m.key.magenta().bold(),
                                m.importance,
                                preview.dimmed()
                            ));
                        }
                        out
                    }
                    Err(e) => format!("  ❌ Search error: {e}"),
                }
            }
            "stats" | "" => match store.stats() {
                Ok(stats) => {
                    let mut out = String::with_capacity(1024);
                    out.push_str(&format!(
                        "  {}  {}\n\n",
                        "🧠".bold(),
                        "Memory Stats".cyan().bold()
                    ));
                    out.push_str(&format!(
                        "  {} {}\n\n",
                        "Total memories:".dimmed(),
                        stats.total.to_string().bold()
                    ));

                    out.push_str(&format!("  {} \n", "By Namespace:".bold()));
                    for (ns, count) in &stats.by_namespace {
                        out.push_str(&format!("    {} {}\n", ns.magenta().bold(), count));
                    }

                    out.push_str(&format!("\n  {} \n", "By Importance:".bold()));
                    for (imp, count) in &stats.by_importance {
                        let bar = "█".repeat(*count);
                        out.push_str(&format!(
                            "    {}  {} ({})\n",
                            format!("{:>2}", imp).dimmed(),
                            bar,
                            count
                        ));
                    }
                    out
                }
                Err(e) => format!("  ❌ Stats error: {e}"),
            },
            _ => format!(
                "  ❌ Unknown memory subcommand: `{subcmd}`\n  Use: stats, ns, search <query>"
            ),
        }
    }

    // ── Git commands ───────────────────────────────────────

    fn slash_git(&self, args: &str) -> String {
        match args {
            "log" | "l" => {
                let output = std::process::Command::new("git")
                    .args(["log", "--oneline", "--abbrev-commit", "-15"])
                    .output();
                match output {
                    Ok(out) if out.status.success() => {
                        let log = String::from_utf8_lossy(&out.stdout);
                        let mut formatted = String::with_capacity(1024);
                        formatted.push_str(&format!(
                            "  {}  {}\n\n",
                            "📦".bold(),
                            "Recent Commits".cyan().bold()
                        ));
                        for line in log.lines() {
                            formatted.push_str(&format!("    {}\n", line.dimmed()));
                        }
                        formatted
                    }
                    Ok(out) => format!(
                        "  ❌ git log failed:\n  {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                    Err(e) => format!("  ❌ git not available: {e}"),
                }
            }
            "diff" | "d" => {
                let output = std::process::Command::new("git")
                    .args(["diff", "--stat"])
                    .output();
                match output {
                    Ok(out) if out.status.success() => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.trim().is_empty() {
                            return "  ✅ Working tree is clean, no uncommitted changes."
                                .to_string();
                        }
                        let mut formatted = String::with_capacity(512);
                        formatted.push_str(&format!(
                            "  {}  {}\n\n",
                            "📝".bold(),
                            "Uncommitted Changes".cyan().bold()
                        ));
                        for line in diff.lines() {
                            formatted.push_str(&format!("    {}\n", line.dimmed()));
                        }
                        formatted.push_str(
                            &"\n  Use `/git:diff:full` for detailed diff."
                                .dimmed()
                                .to_string(),
                        );
                        formatted
                    }
                    Ok(out) => format!(
                        "  ❌ git diff failed:\n  {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                    Err(e) => format!("  ❌ git not available: {e}"),
                }
            }
            "diff:full" | "df" => {
                let output = std::process::Command::new("git").args(["diff"]).output();
                match output {
                    Ok(out) if out.status.success() => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.trim().is_empty() {
                            return "  ✅ No uncommitted changes.".to_string();
                        }
                        format!("  📝 Full diff:\n\n{}", diff)
                    }
                    Ok(out) => format!(
                        "  ❌ git diff failed:\n  {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                    Err(e) => format!("  ❌ git not available: {e}"),
                }
            }
            "branch" | "b" => {
                let output = std::process::Command::new("git")
                    .args(["branch", "-a"])
                    .output();
                match output {
                    Ok(out) if out.status.success() => {
                        let branches = String::from_utf8_lossy(&out.stdout);
                        let mut formatted = String::with_capacity(512);
                        formatted.push_str(&format!(
                            "  {}  {}\n\n",
                            "🌿".bold(),
                            "Branches".cyan().bold()
                        ));
                        for line in branches.lines() {
                            if line.starts_with('*') {
                                formatted
                                    .push_str(&format!("    {} (current)\n", line.green().bold()));
                            } else {
                                formatted.push_str(&format!("    {}\n", line.dimmed()));
                            }
                        }
                        formatted
                    }
                    Ok(out) => format!(
                        "  ❌ git branch failed:\n  {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                    Err(e) => format!("  ❌ git not available: {e}"),
                }
            }
            "" | "status" | "s" => {
                let output = std::process::Command::new("git")
                    .args(["status", "--short"])
                    .output();
                match output {
                    Ok(out) if out.status.success() => {
                        let status = String::from_utf8_lossy(&out.stdout);
                        let mut formatted = String::with_capacity(512);

                        // Get current branch
                        let branch_output = std::process::Command::new("git")
                            .args(["branch", "--show-current"])
                            .output();
                        let branch = branch_output
                            .ok()
                            .filter(|o| o.status.success())
                            .and_then(|o| String::from_utf8(o.stdout).ok())
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|| "?".to_string());

                        formatted.push_str(&format!(
                            "  {}  {}  {}\n\n",
                            "📦".bold(),
                            "Git Status".cyan().bold(),
                            format!("[{}]", branch).dimmed()
                        ));

                        if status.trim().is_empty() {
                            formatted
                                .push_str(&format!("  {}  Working tree is clean\n", "✅".green()));
                        } else {
                            for line in status.lines() {
                                let (prefix, file) = if line.len() > 3 {
                                    (&line[..2], &line[3..])
                                } else {
                                    (line, "")
                                };
                                let icon = match prefix.trim() {
                                    "M" | "MM" => "📝",
                                    "A" => "➕",
                                    "D" => "🗑",
                                    "??" => "❓",
                                    "R" => "🔄",
                                    _ => "  ",
                                };
                                formatted.push_str(&format!(
                                    "    {} {}  {}\n",
                                    icon,
                                    prefix.dimmed(),
                                    file.yellow()
                                ));
                            }
                        }
                        formatted.push_str(
                            &"\n  Use `/git:log` for commit history, `/git:diff` for changes."
                                .dimmed(),
                        );
                        formatted
                    }
                    Ok(out) => format!(
                        "  ❌ git status failed:\n  {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                    Err(e) => format!("  ❌ git not available: {e}"),
                }
            }
            _ => format!("  ❌ Unknown git subcommand: `{args}`\n  Use: status, log, diff, branch"),
        }
    }

    // ── Task commands ──────────────────────────────────────

    fn slash_tasks(&self, args: &str) -> String {
        // Task stats require querying the task planner's SQLite DB
        let db_path = crate::constants::AGENT_DIR.as_str().to_owned() + "/tasks.db";
        let path = std::path::Path::new(&db_path);

        if !path.exists() {
            return "  📋 No tasks database found.\n  Ask the agent to create a task first using task_planner.".to_string();
        }

        match args {
            "todo" | "pending" | "in_progress" => {
                // Try to read tasks from the database
                match rusqlite::Connection::open(path) {
                    Ok(conn) => {
                        let mut stmt = match conn.prepare(
                            "SELECT id, title, status, created_at FROM tasks WHERE status IN ('pending', 'in_progress') ORDER BY created_at DESC LIMIT 15"
                        ) {
                            Ok(s) => s,
                            Err(e) => return format!("  ❌ Failed to query tasks: {e}"),
                        };

                        let rows = match stmt.query_map([], |row| {
                            let id: String = row.get(0)?;
                            let title: String = row.get(1)?;
                            let status: String = row.get(2)?;
                            let created: String = row.get(3)?;
                            Ok((id, title, status, created))
                        }) {
                            Ok(r) => r,
                            Err(e) => return format!("  ❌ Failed to read tasks: {e}"),
                        };

                        let mut out = String::with_capacity(512);
                        out.push_str(&format!(
                            "  {}  {}\n\n",
                            "📋".bold(),
                            "Pending Tasks".cyan().bold()
                        ));

                        let mut count = 0;
                        for (_id, title, status, _created) in rows.flatten() {
                            let icon = if status == "in_progress" {
                                "🔄"
                            } else {
                                "⏳"
                            };
                            out.push_str(&format!(
                                "    {}  {}  ({})\n",
                                icon,
                                title.bold(),
                                status.dimmed()
                            ));
                            count += 1;
                        }

                        if count == 0 {
                            out.push_str(&format!("  {}  No pending tasks! 🎉\n", "  "));
                        }
                        out
                    }
                    Err(e) => format!("  ❌ Failed to open tasks database: {e}"),
                }
            }
            "" | "stats" | "all" => {
                match rusqlite::Connection::open(path) {
                    Ok(conn) => {
                        // Get counts by status
                        let total: usize = conn
                            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
                            .unwrap_or(0);
                        let pending: usize = conn
                            .query_row(
                                "SELECT COUNT(*) FROM tasks WHERE status = 'pending'",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        let in_progress: usize = conn
                            .query_row(
                                "SELECT COUNT(*) FROM tasks WHERE status = 'in_progress'",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        let completed: usize = conn
                            .query_row(
                                "SELECT COUNT(*) FROM tasks WHERE status = 'completed'",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);

                        let mut out = String::with_capacity(256);
                        out.push_str(&format!(
                            "  {}  {}\n\n",
                            "📊".bold(),
                            "Task Overview".cyan().bold()
                        ));
                        out.push_str(&format!(
                            "  {} {}\n",
                            "Total tasks:".dimmed(),
                            total.to_string().bold()
                        ));
                        out.push_str(&format!(
                            "  {} {} {}\n",
                            "⏳".dimmed(),
                            "Pending:".dimmed(),
                            pending
                        ));
                        out.push_str(&format!(
                            "  {} {} {}\n",
                            "🔄".dimmed(),
                            "In Progress:".dimmed(),
                            in_progress
                        ));
                        out.push_str(&format!(
                            "  {} {} {}\n",
                            "✅".dimmed(),
                            "Completed:".dimmed(),
                            completed
                        ));

                        if pending + in_progress > 0 {
                            out.push_str(&"\n  Use `/tasks:todo` to see pending tasks.".dimmed());
                        }
                        out
                    }
                    Err(e) => format!("  ❌ Failed to open tasks database: {e}"),
                }
            }
            _ => format!("  ❌ Unknown tasks subcommand: `{args}`\n  Use: stats, todo"),
        }
    }

    // ── Skills commands ────────────────────────────────────

    fn slash_skills(&self, _args: &str) -> String {
        let skills = self.agent.skill_registry.list();
        let active_count = self.agent.skill_registry.active_skills().len();

        let mut out = String::with_capacity(512);
        out.push_str(&format!(
            "  {}  {} ({})\n\n",
            "🎯".bold(),
            "Skills".cyan().bold(),
            format!("{} loaded, {} active", skills.len(), active_count).dimmed()
        ));

        if skills.is_empty() {
            out.push_str(&format!("  {}  No skills loaded.\n", "📭".dimmed()));
            out.push_str(&format!(
                "  {}  Ask the agent to create a skill with manage_skills.\n",
                "💡".dimmed()
            ));
            return out;
        }

        // Group by active/inactive
        for (name, desc, active) in &skills {
            let status = if *active {
                "🟢 always-on"
            } else {
                "🔵 on-demand"
            };
            out.push_str(&format!(
                "    {}  {}  {}\n",
                name.magenta().bold(),
                status.dimmed(),
                desc.dimmed()
            ));
        }

        out.push_str(&"\n  Use `/help:skills` for more info.".dimmed());
        out
    }

    // ── Export command ─────────────────────────────────────

    fn slash_export(&self, args: &str) -> String {
        let path = if args.is_empty() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("conversation_{ts}.json")
        } else {
            args.to_string()
        };

        match self.agent.export_conversation(&path) {
            Ok(()) => format!(
                "  ✅ Conversation exported to: {}\n  ({:.1} messages, {} tokens)",
                path.cyan(),
                self.agent.messages().len(),
                self.agent.token_usage().total_tokens,
            ),
            Err(e) => format!("  ❌ Export failed: {e}"),
        }
    }

    // ── Stats command ──────────────────────────────────────

    fn slash_stats(&self) -> String {
        let usage = self.agent.token_usage();
        let msg_count = self.agent.messages().len();
        let metrics = self.agent.session_metrics();

        let mut out = String::with_capacity(1024);
        out.push_str(&format!(
            "  {}  {}\n\n",
            "📊".bold(),
            "Session Statistics".cyan().bold()
        ));

        // Session duration
        out.push_str(&format!(
            "  {}  {}  {:.1}s\n\n",
            "⏱".dimmed(),
            "Session Duration".dimmed(),
            metrics.session_duration_secs()
        ));

        // Conversation
        out.push_str(&format!("  {}  {}\n", "Conversation".bold(), ""));
        out.push_str(&format!("    Messages:       {}\n", msg_count));
        out.push_str(&format!(
            "    User messages:  {}\n",
            metrics
                .user_messages
                .load(std::sync::atomic::Ordering::Relaxed)
        ));

        // Token usage
        out.push_str(&format!("\n  {}  {}\n", "Token Usage".bold(), ""));
        if usage.api_calls == 0 {
            out.push_str("    No API calls made yet.\n");
        } else {
            out.push_str(&format!("    API calls:      {}\n", usage.api_calls));
            out.push_str(&format!(
                "    Prompt:         {} tokens\n",
                usage.prompt_tokens
            ));
            out.push_str(&format!(
                "    Completion:     {} tokens\n",
                usage.completion_tokens
            ));
            out.push_str(&format!(
                "    Total:          {} tokens\n",
                usage.total_tokens
            ));
        }

        // Tool metrics
        out.push_str(&format!("\n  {}  {}\n", "Tool Calls".bold(), ""));
        let tool_calls = metrics
            .tool_calls
            .load(std::sync::atomic::Ordering::Relaxed);
        let tool_errors = metrics
            .tool_errors
            .load(std::sync::atomic::Ordering::Relaxed);
        out.push_str(&format!("    Total:          {}\n", tool_calls));
        out.push_str(&format!(
            "    Success rate:   {:.1}%\n",
            metrics.tool_success_rate()
        ));
        out.push_str(&format!(
            "    Avg latency:    {:.0}ms\n",
            metrics.avg_tool_latency_ms()
        ));
        if tool_errors > 0 {
            out.push_str(&format!("    Errors:         {}\n", tool_errors));
        }

        // Latency
        out.push_str(&format!("\n  {}  {}\n", "Latency".bold(), ""));
        out.push_str(&format!(
            "    Avg chat:       {:.0}ms\n",
            metrics.avg_chat_latency_ms()
        ));

        // System
        let tool_count = self.agent.tool_registry.list_tools().len();
        out.push_str(&format!("\n  {}  {}\n", "System".bold(), ""));
        out.push_str(&format!("    Tools:          {}\n", tool_count));
        out.push_str(&format!(
            "    Hooks:          {}\n",
            self.agent.hook_manager().hook_count()
        ));

        let skills_count = self.agent.skill_registry.list().len();
        out.push_str(&format!("    Skills:         {}\n", skills_count));

        // Memory
        if let Some(store) = self.agent.memory_store() {
            if let Ok(stats) = (*store).stats() {
                out.push_str(&format!("    Memories:       {}\n", stats.total));
            }
        }

        out
    }

    // ─── MCP commands ─────────────────────────────────────────

    async fn slash_mcp(&mut self, args: &str) -> String {
        let (subcmd, subargs) = if let Some(pos) = args.find(' ') {
            (&args[..pos], args[pos + 1..].trim())
        } else {
            (args, "")
        };

        match subcmd {
            "list" | "ls" | "" => self.slash_mcp_list(),
            "status" => self.slash_mcp_status(),
            "restart" | "restart_server" => {
                if subargs.is_empty() {
                    return "  Usage: `/mcp:restart <server-name>`".to_string();
                }
                self.slash_mcp_restart(subargs).await
            }
            "stop" | "disconnect" => {
                if subargs.is_empty() {
                    return "  Usage: `/mcp:stop <server-name>`".to_string();
                }
                self.slash_mcp_stop(subargs).await
            }
            "start" | "connect" => {
                if subargs.is_empty() {
                    return "  Usage: `/mcp:start <server-name>`".to_string();
                }
                self.slash_mcp_start(subargs).await
            }
            _ => format!(
                "  ❌ Unknown MCP subcommand: `{subcmd}`\n  Use: list, status, start, stop, restart"
            ),
        }
    }

    fn slash_mcp_list(&self) -> String {
        let mut out = String::with_capacity(512);
        out.push_str(&format!(
            "  {}  {}\n\n",
            "🔌".bold(),
            "MCP Servers".cyan().bold()
        ));

        if let Some(bridge) = &self.mcp_bridge {
            let status = bridge.status();
            if status.is_empty() {
                out.push_str("  📭 No MCP servers configured.\n");
                out.push_str(&format!(
                    "  {}  Add servers to ~/.cargo-agent/config.yaml under `mcp_servers:`.\n",
                    "💡".dimmed()
                ));
                return out;
            }

            for s in &status {
                let icon = if s.connected { "🟢" } else { "🔴" };
                out.push_str(&format!(
                    "    {}  {}  ({} tool(s))\n",
                    icon,
                    s.name.bold(),
                    s.tool_count
                ));
                if let Some(ref err) = s.error {
                    out.push_str(&format!("        {} {err}\n", "⚠".dimmed()));
                }
            }

            out.push_str(&format!(
                "\n  {} {} connected, {} tool(s) total\n",
                "📊".dimmed(),
                bridge.connected_count(),
                bridge.total_mcp_tools()
            ));
        } else {
            out.push_str("  📭 MCP bridge not initialized.\n");
        }

        out
    }

    fn slash_mcp_status(&self) -> String {
        if let Some(bridge) = &self.mcp_bridge {
            let status = bridge.status();
            let mut out = String::with_capacity(1024);
            out.push_str(&format!(
                "  {}  {}\n\n",
                "📊".bold(),
                "MCP Bridge Status".cyan().bold()
            ));

            out.push_str(&format!(
                "  {} {} server(s)\n",
                "Configured:".dimmed(),
                status.len()
            ));
            out.push_str(&format!(
                "  {} {} server(s) connected\n",
                "Connected:".dimmed(),
                bridge.connected_count()
            ));
            out.push_str(&format!(
                "  {} {} tool(s) registered\n",
                "Tools:".dimmed(),
                bridge.total_mcp_tools()
            ));

            out.push_str(&format!(
                "\n  {} Use `/mcp:list` for server details, `/mcp:restart <name>` to reconnect.\n",
                "💡".dimmed()
            ));
            out
        } else {
            "  🔌 MCP bridge not initialized.\n  No MCP servers configured in config.yaml."
                .to_string()
        }
    }

    async fn slash_mcp_restart(&mut self, name: &str) -> String {
        if let Some(bridge) = &mut self.mcp_bridge {
            match bridge
                .restart_server(name, &mut self.agent.tool_registry)
                .await
            {
                Ok(()) => format!("  ✅ MCP server '{name}' restarted successfully."),
                Err(e) => format!("  ❌ Failed to restart '{name}': {e}"),
            }
        } else {
            "  ❌ MCP bridge not initialized.".to_string()
        }
    }

    async fn slash_mcp_stop(&mut self, name: &str) -> String {
        if let Some(bridge) = &mut self.mcp_bridge {
            match bridge.stop_server(name).await {
                Ok(()) => format!("  ✅ MCP server '{name}' stopped."),
                Err(e) => format!("  ❌ Failed to stop '{name}': {e}"),
            }
        } else {
            "  ❌ MCP bridge not initialized.".to_string()
        }
    }

    async fn slash_mcp_start(&mut self, name: &str) -> String {
        if let Some(bridge) = &mut self.mcp_bridge {
            match bridge
                .start_server(name, &mut self.agent.tool_registry)
                .await
            {
                Ok(()) => format!(
                    "  ✅ MCP server '{name}' started. {} tool(s) registered.",
                    bridge
                        .status()
                        .iter()
                        .find(|s| s.name == name)
                        .map(|s| s.tool_count)
                        .unwrap_or(0)
                ),
                Err(e) => format!("  ❌ Failed to start '{name}': {e}"),
            }
        } else {
            "  ❌ MCP bridge not initialized.".to_string()
        }
    }

    /// Return a reference to the agent for dashboard display.
    pub fn agent(&self) -> &AIAgent {
        &self.agent
    }

    /// Return the configured model name for dashboard display.
    pub fn model_name(&self) -> &str {
        &self.model_router.default_model
    }

    // ========================================================================
    // Loop commands
    // ========================================================================

    fn slash_loop_help(&self) -> String {
        format!(
            "  {}  {}\n\n\
             {}  Start a recurring loop that executes a command at a fixed interval.\n\
             {}  /loop <interval> <command>    Start a new loop (e.g. /loop 5m cargo test)\n\
             {}  /loop:list                    List active loops\n\
             {}  /loop:stop <id>               Stop a loop by ID\n\
             {}  /loop:stop-all                Stop all active loops\n\n\
             {}  Interval format: <number>[s|m|h] — e.g. 30s, 5m, 1h\n",
            "🔄".bold(),
            "Loop Commands".cyan().bold(),
            "Description:".dimmed(),
            "Start:".magenta().bold(),
            "List:".magenta().bold(),
            "Stop:".magenta().bold(),
            "Stop All:".magenta().bold(),
            "Tip:".dimmed(),
        )
    }

    fn parse_loop_args(&self, args: &str) -> Result<(u64, String), String> {
        let args = args.trim();
        if args.is_empty() {
            return Err("Usage: /loop <interval> <command>\nExample: /loop 5m cargo test".into());
        }

        // Split into interval and command
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return Err("Usage: /loop <interval> <command>\nExample: /loop 5m cargo test".into());
        }

        let interval_str = parts[0];
        let command = parts[1].to_string();

        // Parse interval: supports "30s", "5m", "1h", or raw seconds
        let interval_secs = if let Some(secs) = interval_str.strip_suffix('s') {
            secs.parse::<u64>()
                .map_err(|_| format!("Invalid interval: {interval_str}"))?
        } else if let Some(mins) = interval_str.strip_suffix('m') {
            mins.parse::<u64>()
                .map(|m| m * 60)
                .map_err(|_| format!("Invalid interval: {interval_str}"))?
        } else if let Some(hours) = interval_str.strip_suffix('h') {
            hours
                .parse::<u64>()
                .map(|h| h * 3600)
                .map_err(|_| format!("Invalid interval: {interval_str}"))?
        } else {
            interval_str.parse::<u64>().map_err(|_| {
                format!(
                    "Invalid interval: {interval_str}. Use format like 30s, 5m, 1h, or raw seconds"
                )
            })?
        };

        if interval_secs < 1 {
            return Err("Interval must be at least 1 second".into());
        }

        Ok((interval_secs, command))
    }

    /// List all active loops.
    pub async fn slash_loop_list(&self) -> String {
        let loops = self.loop_manager.list().await;
        if loops.is_empty() {
            return "  📭 No active loops.\n  Use /loop <interval> <command> to start one."
                .to_string();
        }

        let mut out = String::with_capacity(512);
        out.push_str(&format!(
            "  {}  {}\n\n",
            "🔄".bold(),
            format!("Active Loops ({})", loops.len()).cyan().bold()
        ));

        for loop_task in &loops {
            let _status = if loop_task.enabled {
                "● running".green().to_string()
            } else {
                "○ paused".red().to_string()
            };
            out.push_str(&format!(
                "  {}  ID: {}\n  {}  {}\n  {}  Every {} seconds\n  {}  {} runs\n\n",
                "ID:".dimmed(),
                loop_task.id.to_string().magenta().bold(),
                "Cmd:".dimmed(),
                loop_task.command.dimmed(),
                "Interval:".dimmed(),
                loop_task.interval_secs.to_string().yellow(),
                "Runs:".dimmed(),
                loop_task.run_count.to_string().yellow(),
            ));
        }

        out.push_str(&format!(
            "  {}  /loop:stop <id> to stop a loop.\n",
            "Tip:".dimmed()
        ));
        out
    }

    /// Stop a loop by ID.
    pub async fn slash_loop_stop(&self, id: u64) -> String {
        match self.loop_manager.stop(id).await {
            Ok(()) => format!("  ✅ Loop {id} stopped."),
            Err(e) => format!("  ❌ {e}"),
        }
    }

    /// Stop all loops.
    pub async fn slash_loop_stop_all(&self) -> String {
        let count = self.loop_manager.stop_all().await;
        format!("  ✅ Stopped {count} active loop(s).")
    }

    // ========================================================================
    // Goal commands
    // ========================================================================

    fn slash_goal_help(&self) -> String {
        format!(
            "  {}  {}\n\n\
             {}  Track the current session goal.\n\
             {}  /goal <description>          Set a new goal\n\
             {}  /goal                        Show current goal\n\
             {}  /goal:clear                  Clear the current goal\n\
             {}  /goal:done                   Mark the goal as completed\n",
            "🎯".bold(),
            "Goal Commands".cyan().bold(),
            "Description:".dimmed(),
            "Set:".magenta().bold(),
            "Show:".magenta().bold(),
            "Clear:".magenta().bold(),
            "Done:".magenta().bold(),
        )
    }

    pub async fn slash_goal_set(&self, description: String) -> String {
        self.goal_manager.set(description.clone()).await;
        format!("  🎯 Goal set: {}\n", description)
    }

    pub async fn slash_goal_show(&self) -> String {
        match self.goal_manager.get().await {
            Some(goal) => {
                let status = if goal.completed {
                    "✅ Completed"
                } else {
                    "🔄 In Progress"
                };
                let completed_info = if let Some(ref completed_at) = goal.completed_at {
                    format!("\n  {}  {}", "Completed at:".dimmed(), completed_at)
                } else {
                    String::new()
                };
                format!(
                    "  {}  {}\n\n  {}  {}\n  {}  {}{}\n",
                    "🎯".bold(),
                    "Current Goal".cyan().bold(),
                    "Status:".dimmed(),
                    status,
                    "Goal:".dimmed(),
                    goal.description.bold(),
                    completed_info,
                )
            }
            None => "  📭 No active goal.\n  Use /goal <description> to set one.".to_string(),
        }
    }

    pub async fn slash_goal_clear(&self) -> String {
        if self.goal_manager.clear().await {
            "  ✅ Goal cleared.".to_string()
        } else {
            "  📭 No goal to clear.".to_string()
        }
    }

    pub async fn slash_goal_done(&self) -> String {
        match self.goal_manager.mark_done().await {
            Ok(()) => "  🎉 Goal marked as completed!".to_string(),
            Err(e) => format!("  ❌ {e}"),
        }
    }
}
