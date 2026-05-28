use std::io::{self, BufRead};
use std::time::Instant;

use crossterm::style::Stylize;
use cargo_agent::cli_commands::{SlashResult, handle as handle_slash};
use cargo_agent::config::CargoConfig;
use cargo_agent::gateway::Gateway;
use cargo_agent::ui;

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

        // Check for local slash commands first
        if input == "/usage" {
            println!("\n{}\n", gateway.token_usage_info());
            ui::thin_separator();
            continue;
        }

        match handle_slash(input) {
            SlashResult::Handled(output) => {
                if output.is_empty() {
                    // /quit or /clear — handle exit separately
                    if input == "/quit" || input == "/exit" {
                        println!();
                        ui::print_info("Goodbye!");
                        println!();
                        break;
                    }
                    // /clear — reset token usage
                    if input == "/clear" || input == "/cls" {
                        gateway.reset_token_usage();
                    }
                    // /clear already printed its escape sequence
                    continue;
                }
                println!("\n{output}\n");
                ui::thin_separator();
                continue;
            }
            SlashResult::PassThrough => {}
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
