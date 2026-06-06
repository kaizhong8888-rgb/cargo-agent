//! Built-in tool registration for the agent gateway.

use crate::memory::SqliteMemoryStore;
use crate::tools::builtin::env_secret::SecretStore;
use crate::tools::ToolRegistry;
use std::path::PathBuf;
use std::sync::Arc;

/// Register all built-in tools on the registry.
pub fn register_builtin_tools(
    tool_registry: &mut ToolRegistry,
    memory_store: Option<Arc<SqliteMemoryStore>>,
) {
    // ── Always-registered tools ──────────────────────────────
    crate::tools::builtin::hello_tool::register_all(tool_registry);
    crate::tools::builtin::file_tools::register_all(tool_registry);

    if let Some(store) = memory_store {
        crate::tools::builtin::memory_tool::register_all(tool_registry, store.clone());
        crate::tools::builtin::evolution_tools::register_all(tool_registry, store.clone());
        crate::tools::builtin::task_planner::register(tool_registry, store);
    }

    crate::tools::builtin::task_pool::register_all(tool_registry);
    crate::tools::builtin::net_tools::register_all(tool_registry);
    crate::tools::builtin::code_analyzer_tool::register_all(tool_registry);
    crate::tools::builtin::code_quality_tool::register_all(tool_registry);
    crate::tools::builtin::ci_cd_tool::register_all(tool_registry);
    crate::tools::builtin::security_scanner::register_all(tool_registry);
    crate::tools::builtin::git_workflow_tool::register_all(tool_registry);
    crate::tools::builtin::doc_generator::register_all(tool_registry);
    crate::tools::builtin::smart_refactor::register_all(tool_registry);
    crate::tools::builtin::git_tools::register_all(tool_registry);
    crate::tools::builtin::code_executor::register_all(tool_registry);
    crate::tools::builtin::scaffold::register_all(tool_registry);
    crate::tools::builtin::dep_manager::register_all(tool_registry);
    crate::tools::builtin::code_transform::register_all(tool_registry);
    crate::tools::builtin::code_review::register_all(tool_registry);
    crate::tools::builtin::doc_search::register_all(tool_registry);
    crate::tools::builtin::diagram::register_all(tool_registry);
    crate::tools::builtin::config_store::register_all(tool_registry);
    crate::tools::builtin::scheduler::register_all(tool_registry);
    crate::tools::builtin::cron_tool::register_all(tool_registry);
    crate::tools::builtin::json_schema_tool::register_all(tool_registry);
    crate::tools::builtin::llm_tool::register_all(tool_registry);
    crate::tools::builtin::database_tool::register_all(tool_registry);
    crate::tools::builtin::db_migration::register_all(tool_registry);

    let agent_dir = PathBuf::from(&*crate::constants::AGENT_DIR);
    let secret_store = SecretStore::new(agent_dir.clone());
    crate::tools::builtin::env_secret::register_all(tool_registry, secret_store);

    crate::tools::builtin::notify::register_all(tool_registry);
    crate::tools::builtin::license_audit::register_all(tool_registry);
    crate::tools::builtin::container_tool::register_all(tool_registry);
    crate::tools::builtin::data_processor::register_all(tool_registry);
    crate::tools::builtin::chart_generator::register_all(tool_registry);
    crate::tools::builtin::text_processor::register_all(tool_registry);
    crate::tools::builtin::todo_manager::register_all(tool_registry);
    crate::tools::builtin::date_time_tool::register_all(tool_registry);
    crate::tools::builtin::fortune_tool::register_all(tool_registry);

    let plugins_dir = agent_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).ok();
    crate::tools::builtin::plugin_tool::register_all(tool_registry, plugins_dir);

    crate::tools::builtin::test_generator::register_all(tool_registry);
    crate::tools::builtin::ast_analyzer::register_all(tool_registry);
    crate::tools::builtin::async_profiler::register_all(tool_registry);
    crate::tools::builtin::cross_compile::register_all(tool_registry);
    crate::tools::builtin::fuzz_driver::register_all(tool_registry);
    crate::tools::builtin::benchmark_tool::register_all(tool_registry);
    crate::tools::builtin::clippy_lint_tool::register_all(tool_registry);
    crate::tools::builtin::regex_tool::register_all(tool_registry);
    crate::tools::builtin::diff_tool::register_all(tool_registry);
    crate::tools::builtin::log_analyzer::register_all(tool_registry);
    crate::tools::builtin::markdown_tool::register_all(tool_registry);
    crate::tools::builtin::openapi_tool::register_all(tool_registry);
    crate::tools::builtin::env_file_tool::register_all(tool_registry);

    // ── Feature-gated tools ─────────────────────────────────
    #[cfg(feature = "crypto")]
    crate::tools::builtin::crypto_tool::register_all(tool_registry);

    #[cfg(feature = "crypto")]
    crate::tools::builtin::hash_tool::register_all(tool_registry);

    #[cfg(feature = "image")]
    crate::tools::builtin::image_tool::register_all(tool_registry);

    #[cfg(feature = "email")]
    crate::tools::builtin::mail_tool::register_all(tool_registry);

    #[cfg(feature = "pdf")]
    crate::tools::builtin::pdf_tool::register_all(tool_registry);

    #[cfg(feature = "scraper")]
    crate::tools::builtin::browser_tool::register_all(tool_registry);

    #[cfg(feature = "browser")]
    crate::tools::builtin::browser_automation::register_all(tool_registry);

    #[cfg(feature = "compression")]
    crate::tools::builtin::archive_tool::register_all(tool_registry);

    #[cfg(feature = "sysinfo")]
    crate::tools::builtin::sysmonitor_tool::register_all(tool_registry);

    #[cfg(feature = "sysinfo")]
    crate::tools::builtin::process_tool::register_all(tool_registry);

    #[cfg(feature = "templates")]
    crate::tools::builtin::template_tool::register_all(tool_registry);

    #[cfg(feature = "trading")]
    crate::tools::builtin::quantitative_trading_tool::register_all(tool_registry);
}

/// Tool names referenced in the gateway system prompt (must be registered).
#[allow(dead_code)]
pub const PROMPT_ADVERTISED_TOOLS: &[&str] = &["code_review", "quantitative_trading"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_advertised_tools_are_registered() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry, None);
        for name in PROMPT_ADVERTISED_TOOLS {
            if cfg!(feature = "trading") || *name != "quantitative_trading" {
                assert!(
                    registry.get(name).is_some(),
                    "tool {name} is advertised in the system prompt but not registered"
                );
            }
        }
    }
}
