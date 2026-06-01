use crate::agent::core::AIAgent;
use crate::config::CargoConfig;
use crate::model::client::ModelClient;
use crate::model::router::ModelRouter;
use crate::skills::SkillRegistry;
use crate::tools::builtin::env_secret::SecretStore;
use crate::tools::ToolRegistry;
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Gateway {
    agent: AIAgent,
    model_router: ModelRouter,
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
        crate::tools::builtin::hello_tool::register_all(&mut tool_registry);
        crate::tools::builtin::file_tools::register_all(&mut tool_registry);

        // Memory and evolution tools share the same SQLite-backed store
        if let Some(store) = &memory_store {
            crate::tools::builtin::memory_tool::register_all(&mut tool_registry, store.clone());
            crate::tools::builtin::evolution_tools::register_all(&mut tool_registry, store.clone());
            crate::tools::builtin::task_planner::register(&mut tool_registry, store.clone());
        }

        crate::tools::builtin::task_pool::register_all(&mut tool_registry);
        crate::tools::builtin::fs_tools::register_all(&mut tool_registry);
        crate::tools::builtin::net_tools::register_all(&mut tool_registry);
        crate::tools::builtin::code_analyzer_tool::register_all(&mut tool_registry);

        // Code Quality: quality scoring, duplicate detection, dependency visualization
        crate::tools::builtin::code_quality_tool::register_all(&mut tool_registry);

        // CI/CD Integration: generate CI configs, run tests/builds, coverage, audit, pre-release checks
        crate::tools::builtin::ci_cd_tool::register_all(&mut tool_registry);

        // Security Scanner: code security patterns, dependency audit, hardcoded secrets detection
        crate::tools::builtin::security_scanner::register_all(&mut tool_registry);

        // Git Workflow: branch management, changelog, release automation, merge, PR description
        crate::tools::builtin::git_workflow_tool::register_all(&mut tool_registry);

        // Documentation Generator: API docs, README, architecture docs, module documentation
        crate::tools::builtin::doc_generator::register_all(&mut tool_registry);

        // Smart Refactor: detect code smells and suggest idiomatic Rust improvements
        crate::tools::builtin::smart_refactor::register_all(&mut tool_registry);

        // Tier 1: Git integration, code execution, full HTTP client
        crate::tools::builtin::git_tools::register_all(&mut tool_registry);
        crate::tools::builtin::code_executor::register_all(&mut tool_registry);
        // (http_client already registered via net_tools)

        // Tier 2: Scaffold, dependency manager, code transform
        crate::tools::builtin::scaffold::register_all(&mut tool_registry);
        crate::tools::builtin::dep_manager::register_all(&mut tool_registry);
        crate::tools::builtin::code_transform::register_all(&mut tool_registry);

        // Tier 3: Doc search, diagrams, config persistence, scheduler
        crate::tools::builtin::doc_search::register_all(&mut tool_registry);
        crate::tools::builtin::diagram::register_all(&mut tool_registry);
        crate::tools::builtin::config_store::register_all(&mut tool_registry);
        crate::tools::builtin::scheduler::register_all(&mut tool_registry);

        // LLM Integration: call OpenAI/Anthropic/Ollama for code gen, review, Q&A
        crate::tools::builtin::llm_tool::register_all(&mut tool_registry);

        // Database: SQL queries, table management, CSV import/export, backup
        crate::tools::builtin::database_tool::register_all(&mut tool_registry);

        // Cryptography: encrypt/decrypt, hash, sign/verify, JWT, password hashing
        crate::tools::builtin::crypto_tool::register_all(&mut tool_registry);

        // Quantitative Trading: backtesting, strategy comparison, technical indicators
        crate::tools::builtin::quantitative_trading_tool::register_all(&mut tool_registry);

        // Environment/Secret Management: manage API keys, tokens securely
        let agent_dir = PathBuf::from(&*crate::constants::AGENT_DIR);
        let secret_store = SecretStore::new(agent_dir);
        crate::tools::builtin::env_secret::register_all(&mut tool_registry, secret_store);

        // Notifications: send alerts via webhooks (Slack, DingTalk, custom)
        crate::tools::builtin::notify::register_all(&mut tool_registry);

        // Image: analyze and manipulate images (info, resize, thumbnail, convert)
        crate::tools::builtin::image_tool::register_all(&mut tool_registry);
        crate::tools::builtin::mail_tool::register_all(&mut tool_registry);
        crate::tools::builtin::archive_tool::register_all(&mut tool_registry);
        crate::tools::builtin::pdf_tool::register_all(&mut tool_registry);
        crate::tools::builtin::template_tool::register_all(&mut tool_registry);
        crate::tools::builtin::sysmonitor_tool::register_all(&mut tool_registry);

        // Data Processing: parse, filter, sort, aggregate, stats, merge, convert CSV/JSON
        crate::tools::builtin::data_processor::register_all(&mut tool_registry);

        // Chart Generation: visualize data with pie, bar, line, table, histogram charts
        crate::tools::builtin::chart_generator::register_all(&mut tool_registry);

        // Text Processing: case conversion, counting, base64/url/html encoding,
        // UUID generation, regex, string padding/truncation, word wrapping, and more
        crate::tools::builtin::text_processor::register_all(&mut tool_registry);

        // Todo Manager: personal todo list with priorities, categories, tags,
        // due dates, search/filter, and statistics
        crate::tools::builtin::todo_manager::register_all(&mut tool_registry);

        // Date/Time: timezone conversions, date arithmetic, calendar operations, duration formatting
        crate::tools::builtin::date_time_tool::register_all(&mut tool_registry);

        // GitHub API: list PRs, issues, check CI status, get repo info
        crate::tools::builtin::github_tool::register_all(&mut tool_registry);

        // Plugin Marketplace: browse, search, install, uninstall community plugins
        let plugins_dir = PathBuf::from(&*crate::constants::AGENT_DIR).join("plugins");
        std::fs::create_dir_all(&plugins_dir).ok();
        crate::tools::builtin::plugin_tool::register_all(&mut tool_registry, plugins_dir);

        // Test Generator: analyze Rust source and generate unit/integration/property tests
        crate::tools::builtin::test_generator::register_all(&mut tool_registry);

        // AST Analyzer: AST-level code analysis using syn (analyze, unused_imports, public_api, dependencies, complexity)
        crate::tools::builtin::ast_analyzer::register_all(&mut tool_registry);

        // Benchmark: performance analysis, micro-benchmarks, criterion code generation, hotspot detection
        crate::tools::builtin::benchmark_tool::register_all(&mut tool_registry);

        // Clippy Lint: run clippy, categorize lints, suggest fixes, quality scoring, auto-fix
        crate::tools::builtin::clippy_lint_tool::register_all(&mut tool_registry);

        // Regex: advanced regex testing, validation, replacement, and Rust code generation
        crate::tools::builtin::regex_tool::register_all(&mut tool_registry);

        // Diff: compare text/code, unified diff, side-by-side view, patch generation/application
        crate::tools::builtin::diff_tool::register_all(&mut tool_registry);

        // Log Analyzer: parse, filter, analyze, and visualize log files
        crate::tools::builtin::log_analyzer::register_all(&mut tool_registry);

        // Markdown Processor: convert to HTML/text, generate TOC, lint, stats, transform, validate links
        crate::tools::builtin::markdown_tool::register_all(&mut tool_registry);

        // OpenAPI/Swagger: generate, validate, and analyze OpenAPI specs from Rust web projects
        crate::tools::builtin::openapi_tool::register_all(&mut tool_registry);

        // Hash Tool: compute file and string checksums (MD5, SHA-1, SHA-256, SHA-512, BLAKE3)
        crate::tools::builtin::hash_tool::register_all(&mut tool_registry);

        // EnvFile Tool: parse, validate, generate, merge .env files
        crate::tools::builtin::env_file_tool::register_all(&mut tool_registry);

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
            Use crypto for encrypt/decrypt, hash, sign/verify, JWT, password hashing. \
            Use quantitative_trading for backtesting, strategy comparison, technical indicators. \
            Use env_secret to manage environment variables and secrets (list, get, set, remove). \
            Use notify to send notifications via webhooks (Slack, DingTalk, custom URLs). \
            Use image to analyze and manipulate images (info, resize, thumbnail, convert). \
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

        Self {
            agent,
            model_router,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Gateway running");
        Ok(())
    }

    pub async fn handle_message(&mut self, text: &str) -> Result<String> {
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

    /// Handle a dynamic slash command that needs access to runtime state.
    ///
    /// These are commands that cannot be handled statically in `cli_commands::handle()`
    /// because they need the tool registry, memory store, or other runtime data.
    ///
    /// Returns `None` if the command was not recognized (should pass through to LLM).
    pub async fn handle_slash(&self, cmd: &str, args: &str) -> Option<String> {
        match cmd {
            "tools" => Some(self.slash_tools(args)),
            "tool" => Some(self.slash_tool_detail(args)),
            "mem" | "memory" => Some(self.slash_memory(args).await),
            "git" => Some(self.slash_git(args)),
            "tasks" | "task" => Some(self.slash_tasks(args)),
            "skills" | "skill" => Some(self.slash_skills(args)),
            "export" => Some(self.slash_export(args)),
            "stats" => Some(self.slash_stats()),
            _ => None,
        }
    }

    /// Clear conversation history and reset token usage (used by /clear command).
    pub fn clear_conversation(&mut self) {
        self.agent.clear_conversation();
        self.reset_token_usage();
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
                | "code_execute" | "scaffold" | "dep_manager" | "self_modify"
                | "test_generate" | "benchmark" => 0,
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

        let mut out = String::with_capacity(512);
        out.push_str(&format!(
            "  {}  {}\n\n",
            "📊".bold(),
            "Session Statistics".cyan().bold()
        ));

        // Conversation
        out.push_str(&format!("  {}  {}\n", "Conversation".bold(), ""));
        out.push_str(&format!("    Messages:     {}\n", msg_count));

        // Token usage
        out.push_str(&format!("\n  {}  {}\n", "Token Usage".bold(), ""));
        if usage.api_calls == 0 {
            out.push_str("    No API calls made yet.\n");
        } else {
            out.push_str(&format!("    API calls:    {}\n", usage.api_calls));
            out.push_str(&format!(
                "    Prompt:       {} tokens\n",
                usage.prompt_tokens
            ));
            out.push_str(&format!(
                "    Completion:   {} tokens\n",
                usage.completion_tokens
            ));
            out.push_str(&format!(
                "    Total:        {} tokens\n",
                usage.total_tokens
            ));
        }

        // Tools
        let tool_count = self.agent.tool_registry.list_tools().len();
        out.push_str(&format!("\n  {}  {}\n", "System".bold(), ""));
        out.push_str(&format!("    Tools registered: {}\n", tool_count));

        let skills_count = self.agent.skill_registry.list().len();
        out.push_str(&format!("    Skills loaded:    {}\n", skills_count));

        // Memory
        if let Some(store) = self.agent.memory_store() {
            if let Ok(stats) = (*store).stats() {
                out.push_str(&format!("    Memories stored:  {}\n", stats.total));
            }
        }

        out
    }

    /// Return a reference to the agent for dashboard display.
    pub fn agent(&self) -> &AIAgent {
        &self.agent
    }

    /// Return the configured model name for dashboard display.
    pub fn model_name(&self) -> &str {
        &self.model_router.default_model
    }
}
