use std::io::{self, BufRead};
use std::time::Instant;

use cargo_agent::cli_commands::{handle as handle_slash, parse as parse_slash, SlashResult};
use cargo_agent::config::CargoConfig;
use cargo_agent::gateway::Gateway;
use cargo_agent::ui;
use crossterm::style::Stylize;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_run_mode = args.len() > 1 && args[1] == "run";

    // In run mode, suppress tracing output for clean UI
    if is_run_mode {
        // No-op subscriber: write to /dev/null, level OFF
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("off"))
            .without_time()
            .with_target(false)
            .with_writer(std::io::sink)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("cargo_agent=info".parse()?),
            )
            .init();
    }

    let config = CargoConfig::load()?;

    if is_run_mode {
        let prompt = args[2..].join(" ");
        ui::show_response_banner();
        ui::print_prompt(&prompt);
        println!();

        let mut gateway = Gateway::new(config);
        match gateway.handle_message(&prompt).await {
            Ok(response) => {
                ui::print_response(&response);
            }
            Err(e) => ui::print_error(&format!("{e}")),
        }
        return Ok(());
    }

    // Interactive mode
    ui::show_banner();

    // Start health endpoint on port 8787 (non-blocking)
    if let Err(e) = cargo_agent::health::start_status_server(8787).await {
        tracing::warn!("Could not start health server on port 8787: {e}");
    }

    let mut gateway = Gateway::new(config);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // ── Slash command handling ──────────────────────────
        // Three-tier: static commands → dynamic commands → pass through to LLM
        if input.starts_with('/') {
            // Tier 1: Static commands (help, clear, quit, version, status, config, etc.)
            match handle_slash(input) {
                SlashResult::Handled(output) => {
                    if output.is_empty() {
                        // /quit or /exit — handle exit separately
                        if input == "/quit" || input == "/exit" {
                            println!();
                            ui::print_info("Goodbye!");
                            println!();
                            break;
                        }
                        // /clear or /cls — already printed escape sequences
                        if input == "/clear" || input == "/cls" {
                            gateway.reset_token_usage();
                        }
                        continue;
                    }
                    println!("\n{output}\n");
                    ui::thin_separator();
                    continue;
                }
                SlashResult::PassThrough => {}
            }

            // Tier 2: Dynamic commands (tools, mem, git, tasks, skills, export, stats, dashboard)
            let (cmd, args) = parse_slash(input);
            if cmd == "dashboard" || cmd == "dash" {
                show_dashboard(&gateway);
                continue;
            }
            if !cmd.is_empty() {
                if let Some(output) = gateway.handle_slash(cmd, args).await {
                    println!("\n{output}\n");
                    ui::thin_separator();
                    continue;
                }
            }

            // Tier 3: Unknown /command — pass through to LLM
            // The LLM might understand it
        }

        // Pass through to the agent
        ui::print_prompt(input);
        println!();

        let mut spinner = ui::Spinner::new("Thinking...");
        spinner.start();
        let start = Instant::now();

        let result = gateway.handle_message(input).await;

        spinner.stop();

        let elapsed = start.elapsed();
        println!("  {}", format!("({:.2}s)", elapsed.as_secs_f32()).dim());
        println!();

        match result {
            Ok(response) => ui::print_response(&response),
            Err(e) => ui::print_error(&format!("{e}")),
        }

        ui::separator();
    }

    Ok(())
}

fn show_dashboard(gateway: &Gateway) {
    let agent = gateway.agent();
    let usage = agent.token_usage();
    let health = cargo_agent::health::current_health();

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
        uptime_secs: health.uptime_seconds,
        total_api_calls: usage.api_calls,
        total_tokens: usage.total_tokens,
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_memories,
        total_tools,
        health_status: health.status.to_string(),
        conversation_messages: agent.messages().len(),
        context_max: 200,
        skills_loaded,
        skills_active,
        memory_by_namespace,
        memory_bytes: health.memory_bytes,
    };
    state.render_loop();
}
