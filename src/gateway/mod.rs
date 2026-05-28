use crate::agent::core::AIAgent;
use crate::config::CargoConfig;
use crate::model::client::ModelClient;
use crate::skills::SkillRegistry;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

pub struct Gateway {
    agent: AIAgent,
}

impl Gateway {
    pub fn new(config: CargoConfig) -> Self {
        let api_key = config.resolve_api_key().unwrap_or_default();
        let model_name = config.model.name.clone();
        let base_url = config.resolve_base_url();

        let client = ModelClient::new(api_key, model_name, base_url);

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
            Use doc_search to look up crate info on docs.rs/crates.io. \
            Use diagram to generate Mermaid architecture diagrams. \
            Use config to persist user preferences across sessions. \
            Use scheduler to manage recurring tasks. \
            Use llm to call LLM APIs (OpenAI/Anthropic/Ollama) for code generation, review, explanation, and Q&A. \
            Reflect on your growth with self_reflect, and record evolution events with record_evolution. \
            When you learn something important, store it as a memory. \
            When you improve yourself, record the evolution event. \
            Always run cargo check after code modifications to verify correctness.",
        );

        Self { agent }
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Gateway running");
        Ok(())
    }

    pub async fn handle_message(&mut self, text: &str) -> Result<String> {
        self.agent.chat(text).await
    }
}
