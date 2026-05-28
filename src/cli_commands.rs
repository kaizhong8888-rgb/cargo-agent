//! Slash commands: local CLI commands that bypass the LLM.
//!
//! Commands:
//! - `/help` — list all available commands
//! - `/quit` or `/exit` — exit the CLI
//! - `/clear` or `/cls` — clear the screen
//! - `/status` — show agent status (version, OS, paths)
//! - `/tools` — list available tools
//! - `/model` — show current model info
//! - `/config` — show config file location
//! - `/usage` — show token usage statistics

use colored::Colorize;

/// Result of attempting to handle a slash command.
pub enum SlashResult {
    /// Command was handled — here's the output to show.
    Handled(String),
    /// Not a slash command — pass through to the LLM.
    PassThrough,
}

/// Handle a slash command if the input starts with `/`.
pub fn handle(input: &str) -> SlashResult {
    if !input.starts_with('/') {
        return SlashResult::PassThrough;
    }

    let (cmd, _rest) = if let Some(pos) = input.find(' ') {
        (&input[..pos], input[pos + 1..].trim())
    } else {
        (input, "")
    };

    match cmd {
        "/help" => SlashResult::Handled(help_text()),
        "/quit" | "/exit" => SlashResult::Handled(String::new()),
        "/clear" | "/cls" => {
            print!("\x1b[2J\x1b[H");
            SlashResult::Handled(String::new())
        }
        "/status" => SlashResult::Handled(status_text()),
        "/tools" => SlashResult::Handled(tools_text()),
        "/model" => SlashResult::Handled("Ask the agent: what model are you running on?".into()),
        "/config" => SlashResult::Handled(
            "Config file: ~/.cargo-agent/config.yaml\n\
                     Skills dir:  ~/.cargo-agent/skills/\n\
                     Memories:    ~/.cargo-agent/memories/memories.db".to_string(),
        ),
        "/usage" => SlashResult::Handled(
            "Token usage is tracked per conversation. Ask the agent: 'show token usage'.".into(),
        ),
        other => SlashResult::Handled(format!("Unknown command: {other}\nType /help for available commands.")),
    }
}

fn help_text() -> String {
    let lines = vec![
        ("Available Commands", true),
        ("", false),
        ("/help          Show this help message", false),
        ("/clear, /cls   Clear the terminal screen", false),
        ("/status        Show agent status", false),
        ("/tools         List available tools", false),
        ("/quit, /exit   Exit the agent", false),
        ("", false),
        ("Note: Commands like /skills, /memory, /tasks, /prompt", false),
        ("are handled through tools — just ask the agent!", false),
    ];

    let mut out = String::new();
    for (line, is_header) in lines {
        if is_header {
            out.push_str(&format!("  {}\n", line.cyan().bold()));
        } else {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out
}

fn tools_text() -> String {
    let tools = [
        ("code_analyze", "Analyze Rust code structure and patterns"),
        ("task_planner", "Create and track tasks with SQLite persistence"),
        ("memory_store", "Store and retrieve memories by namespace/tag"),
        ("file_read", "Read file contents"),
        ("file_write", "Write/create files"),
        ("file_list", "List directory contents"),
        ("file_grep", "Search files for patterns"),
        ("self_modify", "Modify agent's own source code"),
        ("self_reflect", "Reflect on agent growth and identify gaps"),
        ("record_evolution", "Record evolution events"),
        ("manage_skills", "List/show/create/update/delete skills"),
        ("task_pool", "Execute concurrent shell commands"),
        ("url_fetch", "Fetch content from URLs (GET only)"),
        ("http_client", "Full HTTP client (GET/POST/PUT/DELETE, JSON, headers, cookies, multipart)"),
        ("git_status", "Show Git working tree status"),
        ("git_diff", "Show changes between commits or working tree"),
        ("git_log", "Show commit history with filtering"),
        ("git_clone", "Clone a Git repository"),
        ("git_commit", "Stage files and create a commit"),
        ("git_push", "Push commits to remote"),
        ("code_execute", "Compile and run Rust code in isolated sandbox"),
        ("project_scaffold", "Generate project structures (cli/lib/web/game)"),
        ("dep_manager", "Manage dependencies (add/rm/update/tree/audit)"),
        ("code_transform", "Safe code refactoring (derive, unwrap, rename, visibility)"),
        ("code_review", "Review code for quality, security, and best practices"),
        ("doc_search", "Search docs.rs and crates.io for crate info"),
        ("diagram", "Generate Mermaid architecture diagrams"),
        ("config", "Persist user preferences across sessions"),
        ("scheduler", "Manage recurring scheduled tasks"),
        ("llm", "Call external LLMs for code generation, review, Q&A"),
        ("database", "SQL queries, table management, CSV import/export"),
        ("crypto", "Encrypt/decrypt, hash, sign/verify, JWT, password hashing"),
        ("quantitative_trading", "Backtesting, strategy comparison, technical indicators"),
        ("env_secret", "Manage environment variables and secrets"),
        ("notify", "Send notifications via webhooks (Slack, DingTalk, custom)"),
        ("image", "Analyze and manipulate images (info, resize, thumbnail, convert)"),
        ("hello", "Greeting tool (demo)"),
    ];

    let mut out = String::new();
    out.push_str(&format!("  {}\n\n", "Available Tools".cyan().bold()));
    for (name, desc) in tools {
        out.push_str(&format!("  {}  {}\n", name.magenta().bold(), desc.dimmed()));
    }
    out.push_str(&format!("\n  {}", "Ask the agent to use any of these tools.".dimmed()));
    out
}

fn status_text() -> String {
    let mut out = String::new();
    out.push_str(&format!("  {}\n\n", "Agent Status".cyan().bold()));

    // Show version
    let version = env!("CARGO_PKG_VERSION");
    out.push_str(&format!("  {} v{}\n", "Version".dimmed(), version.bold()));

    out.push_str(&format!("\n  {}", "Run `/help` for available commands.".dimmed()));
    out
}
