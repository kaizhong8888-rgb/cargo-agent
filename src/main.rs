use std::time::Instant;

use cargo_agent::cli_commands::SlashAction;
use cargo_agent::config::CargoConfig;
use cargo_agent::gateway::Gateway;
use cargo_agent::ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_run_mode = args.len() > 1 && args[1] == "run";
    let is_mcp_server = args.iter().any(|a| a == "--mcp-server");

    // In run mode or MCP server mode, suppress tracing output for clean UI
    if is_run_mode || is_mcp_server {
        // No-op subscriber: write to /dev/null, level OFF
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("off"))
            .without_time()
            .with_target(false)
            .with_writer(std::io::sink)
            .init();
    } else {
        // Write tracing to stderr so it doesn't interleave with the colored banner on stdout
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("cargo_agent=info".parse()?),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    let config = CargoConfig::load()?;

    // ── MCP Server mode ─────────────────────────────────────
    if is_mcp_server {
        let gateway = Gateway::new(config);
        let registry = &gateway.agent().tool_registry;
        eprintln!("🚀 Starting cargo-agent MCP server on stdio...");
        return cargo_agent::mcp::server::run_stdio_server(registry).await;
    }

    if is_run_mode {
        let prompt = args[2..].join(" ");
        ui::show_response_banner();
        ui::print_prompt(&prompt);
        println!();

        let start = Instant::now();
        let mut gateway = Gateway::new(config);

        let mut indicator = ui::StatusIndicator::new(
            gateway.agent().current_status.clone(),
            gateway.agent().current_tool.clone(),
        );
        indicator.start();
        let result = gateway.handle_message(&prompt).await;
        indicator.stop();
        let elapsed = start.elapsed();
        let usage = gateway.agent().token_usage();

        match &result {
            Ok(response) => {
                ui::print_response(response);
                // Show status bar
                let status_info = ui::StatusInfo {
                    api_calls: usage.api_calls,
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    total_tokens: usage.total_tokens,
                    messages_count: gateway.agent().messages().len(),
                    messages_max: 200,
                    model_name: gateway.model_name().to_string(),
                    elapsed_secs: elapsed.as_secs_f32(),
                };
                ui::print_status_bar(&status_info);
                println!();
            }
            Err(e) => ui::print_error(&format!("{e}")),
        }
        return Ok(());
    }

    // Interactive mode
    ui::show_banner();

    let mut gateway = Gateway::new_async(config).await;

    // Report MCP server connections
    if let Some(bridge) = gateway.mcp_bridge() {
        if bridge.connected_count() > 0 {
            tracing::info!(
                "MCP bridge: {} server(s) connected, {} tool(s) registered",
                bridge.connected_count(),
                bridge.total_mcp_tools()
            );
        }
    }

    // ── REPL loop with multi-line input support ─────────
    loop {
        let input = match ui::read_multiline_input() {
            Some(s) => s,
            None => {
                // Ctrl+C or Ctrl+D — exit gracefully
                println!();
                ui::print_info("Goodbye!");
                println!();
                break;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // ── Slash command handling ──────────────────────────
        if input.starts_with('/') {
            match gateway.handle_slash_command(input).await {
                SlashAction::Output(text) => {
                    println!("\n{text}\n");
                    ui::thin_separator();
                    continue;
                }
                SlashAction::Exit => {
                    println!();
                    ui::print_info("Goodbye!");
                    println!();
                    break;
                }
                SlashAction::Clear => {
                    continue;
                }
                SlashAction::Dashboard => {
                    #[cfg(feature = "tui")] {
                        show_dashboard(&gateway);
                    }
                    #[cfg(not(feature = "tui"))] {
                        println!(
                            "\n  Dashboard requires the `tui` feature.\n  Rebuild with default features or `--features tui`.\n"
                        );
                    }
                    continue;
                }
                SlashAction::PassThrough => {
                    // Unknown /command — LLM might understand it
                }
            }
        }

        // Pass through to the agent
        ui::print_prompt(input);
        println!();

        // Show routing info before processing
        let model_info = gateway.model_routing_info(input);
        let msg_count = gateway.agent().messages().len();
        ui::print_thinking_status(&format!(
            "{} | 💬 {} messages in context",
            model_info, msg_count,
        ));

        let mut indicator = ui::StatusIndicator::new(
            gateway.agent().current_status.clone(),
            gateway.agent().current_tool.clone(),
        );
        indicator.start();

        let mut spinner = ui::Spinner::new("Thinking...");
        spinner.start();
        let start = Instant::now();

        let result = gateway.handle_message(input).await;

        spinner.stop();
        indicator.stop();

        let elapsed = start.elapsed();
        let usage = gateway.agent().token_usage();

        match &result {
            Ok(response) => ui::print_response(response),
            Err(e) => ui::print_error(&format!("{e}")),
        }

        // Show compact status bar with token usage & context info
        let status_info = ui::StatusInfo {
            api_calls: usage.api_calls,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            messages_count: gateway.agent().messages().len(),
            messages_max: 200,
            model_name: gateway.model_name().to_string(),
            elapsed_secs: elapsed.as_secs_f32(),
        };
        ui::print_status_bar(&status_info);

        ui::separator();
    }

    Ok(())
}

#[cfg(feature = "tui")]
fn show_dashboard(gateway: &Gateway) {
    let agent = gateway.agent();
    let usage = agent.token_usage();

    let memory_store = agent.memory_store();
    let total_memories = memory_store
        .and_then(|s| s.stats().ok())
        .map(|s| s.total)
        .unwrap_or(0);

    let memory_by_namespace = memory_store
        .and_then(|s| s.stats().ok())
        .map(|s| s.by_namespace.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    let total_tools = agent.tool_registry.list_tools().len();
    let skills = agent.skill_registry.list();
    let skills_loaded = skills.len();
    let skills_active = agent.skill_registry.active_skills().len();

    let state = cargo_agent::tui::dashboard::DashboardState {
        version: env!("CARGO_PKG_VERSION").to_string(),
        model_name: gateway.model_name().to_string(),
        uptime_secs: gateway.session_metrics().session_duration_secs() as u64,
        total_api_calls: usage.api_calls,
        total_tokens: usage.total_tokens,
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_memories,
        total_tools,
        health_status: "ok".to_string(),
        conversation_messages: agent.messages().len(),
        context_max: 200,
        skills_loaded,
        skills_active,
        memory_by_namespace,
        memory_bytes: memory_usage_bytes(),
    };
    state.render_loop();
}

/// Get current process memory usage in bytes (macOS only, returns 0 otherwise).
#[cfg(feature = "tui")]
fn memory_usage_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    #[allow(deprecated)] // mach_task_self is deprecated in favor of mach2, but pulling mach2 just for this is overkill
    {
        let mut info = unsafe { std::mem::zeroed::<libc::mach_task_basic_info_data_t>() };
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        let ret = unsafe {
            libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as libc::task_info_t,
                &mut count,
            )
        };
        if ret == libc::KERN_SUCCESS {
            return info.resident_size as u64;
        }
    }
    0
}
