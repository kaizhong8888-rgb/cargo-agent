use crate::agent::core::AIAgent;
use crate::config::CargoConfig;
use crate::model::client::ModelClient;
use crate::model::router::ModelRouter;
use crate::skills::SkillRegistry;
use crate::tools::builtin::env_secret::SecretStore;
use crate::tools::ToolRegistry;
use anyhow::Result;
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

        // Data Processing: parse, filter, sort, aggregate, stats, merge, convert CSV/JSON
        crate::tools::builtin::data_processor::register_all(&mut tool_registry);

        // Load skills from ~/.cargo-agent/skills/
        let skills_dir = crate::constants::skills_dir();
        let skill_registry = Arc::new(
            SkillRegistry::load_from_dir(&skills_dir).unwrap_or_else(|_| {
                tracing::warn!("Failed to load skills from {}", skills_dir.display());
                SkillRegistry::new()
            }),
        );

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
            Reflect on your growth with self_reflect, and record evolution events with record_evolution. \
            When you learn something important, store it as a memory. \
            When you improve yourself, record the evolution event. \
            Always run cargo check after code modifications to verify correctness.",
        );

        Self { agent, model_router }
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
}
