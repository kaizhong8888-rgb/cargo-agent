//! Slash commands: local CLI shortcuts that bypass the LLM.
//!
//! # Quick Reference
//!
//! | Command | Shortcut | Description |
//! |---------|----------|-------------|
//! | `/help` | `/h` | Show categorized help |
//! | `/help:tools` | — | Tool-specific help |
//! | `/help:memory` | — | Memory commands help |
//! | `/help:git` | — | Git shortcuts help |
//! | `/version` | `/v` | Show version info |
//! | `/status` | — | Agent status |
//! | `/clear` | `/cls` | Clear screen |
//! | `/quit` | `/exit` | Exit agent |
//! | `/tools` | — | List available tools |
//! | `/tool:name` | — | Show tool details |
//! | `/mem` | — | Memory overview |
//! | `/mem:ns` | — | List namespaces |
//! | `/mem:search q` | — | Search memories |
//! | `/git` | — | Git status summary |
//! | `/git:log` | — | Recent commits |
//! | `/tasks` | — | Task overview |
//! | `/tasks:todo` | — | Pending tasks |
//! | `/skills` | — | List skills |
//! | `/usage` | — | Token usage |
//! | `/model` | — | Model info |
//! | `/config` | — | Config paths |
//! | `/export` | — | Export conversation |

use colored::Colorize;

/// Result of attempting to handle a slash command.
pub enum SlashResult {
    /// Command was handled — here's the output to show.
    Handled(String),
    /// Not a slash command — pass through to the LLM.
    PassThrough,
}

/// Parse a slash command into (command, args).
/// Supports both `/cmd args` and `/cmd:args` syntax.
pub fn parse(input: &str) -> (&str, &str) {
    if !input.starts_with('/') {
        return ("", "");
    }
    let rest = &input[1..];

    // Try `/cmd:args` syntax first
    if let Some(pos) = rest.find(':') {
        let cmd = &rest[..pos];
        let args = rest[pos + 1..].trim();
        return (cmd, args);
    }

    // Fallback to `/cmd args` syntax
    if let Some(pos) = rest.find(' ') {
        (&rest[..pos], rest[pos + 1..].trim())
    } else {
        (rest, "")
    }
}

/// Handle a slash command if the input starts with `/`.
///
/// Returns `SlashResult::Handled(output)` for recognized commands,
/// or `SlashResult::PassThrough` if it's not a slash command.
/// Commands that need dynamic data (like tool registry, memory store)
/// should return `SlashResult::PassThrough` to be handled by `Gateway::handle_slash()`.
pub fn handle(input: &str) -> SlashResult {
    if !input.starts_with('/') {
        return SlashResult::PassThrough;
    }

    let (cmd, args) = parse(input);

    match cmd {
        // ── Navigation / Meta ──────────────────────────────
        "help" | "h" => {
            if args.is_empty() {
                SlashResult::Handled(help_general())
            } else {
                SlashResult::Handled(help_topic(args))
            }
        }

        "version" | "v" => SlashResult::Handled(version_text()),

        "status" => SlashResult::Handled(status_text()),

        "clear" | "cls" => {
            print!("\x1b[2J\x1b[H");
            SlashResult::Handled(String::new())
        }

        "quit" | "exit" => SlashResult::Handled(String::new()),

        // ── Session / Usage ───────────────────────────────
        "usage" => SlashResult::Handled(
            "Token usage is tracked per conversation.\nAsk the agent: 'show token usage'.".into(),
        ),

        "model" => SlashResult::Handled(
            "Model routing is automatic based on task complexity.\n\
                 Ask the agent: 'what model complexity is this task?'"
                .into(),
        ),

        "config" => SlashResult::Handled(config_text()),

        // ── Dynamic commands (handled by Gateway) ─────────
        // These pass through because they need access to the tool registry,
        // memory store, git, or other runtime state.
        "tools" | "tool" | "mem" | "memory" | "git" | "tasks" | "task" | "skills" | "skill"
        | "export" | "stats" => SlashResult::PassThrough,

        // ── Unknown ───────────────────────────────────────
        other => SlashResult::Handled(format!(
            "❌ Unknown command: `/{other}`\n  Type `/help` for available commands."
        )),
    }
}

// ============================================================================
// Help system
// ============================================================================

fn help_general() -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "  {}  {}\n\n",
        "⌨".bold(),
        "Slash Commands".cyan().bold()
    ));

    // Navigation
    section(
        &mut out,
        "Navigation",
        &[
            ("/help", "Show this help"),
            ("/help:topic", "Help on a specific topic"),
            ("/version | /v", "Show version info"),
            ("/status", "Agent status & paths"),
        ],
    );

    // Session
    section(
        &mut out,
        "Session",
        &[
            ("/clear | /cls", "Clear terminal screen"),
            ("/quit | /exit", "Exit the agent"),
            ("/usage", "Token usage statistics"),
            ("/model", "Model routing info"),
            ("/config", "Show config file paths"),
        ],
    );

    // Tools
    section(
        &mut out,
        "Tools & Skills",
        &[
            ("/tools", "List all available tools"),
            ("/tool:name", "Show details for a specific tool"),
            ("/skills", "List loaded skills"),
        ],
    );

    // Memory
    section(
        &mut out,
        "Memory",
        &[
            ("/mem", "Memory overview & stats"),
            ("/mem:ns", "List all namespaces"),
            ("/mem:search q", "Search memories"),
        ],
    );

    // Git
    section(
        &mut out,
        "Git",
        &[
            ("/git", "Git status summary"),
            ("/git:log", "Recent commit history"),
        ],
    );

    // Tasks
    section(
        &mut out,
        "Tasks",
        &[
            ("/tasks", "Task overview & stats"),
            ("/tasks:todo", "Show pending tasks"),
        ],
    );

    out.push_str(&format!(
        "\n  {}",
        "Tip: Use `/help:topic` — e.g. `/help:memory`, `/help:git`".dimmed()
    ));

    out
}

fn help_topic(topic: &str) -> String {
    match topic {
        "tools" | "tool" => help_tools_detail(),
        "mem" | "memory" => help_memory_detail(),
        "git" => help_git_detail(),
        "tasks" | "task" | "task_planner" => help_tasks_detail(),
        "skills" | "skill" => help_skills_detail(),
        "commands" | "shortcuts" | "all" => help_general(),
        _ => format!(
            "No help available for `{topic}`.\nTry: tools, memory, git, tasks, skills, commands"
        ),
    }
}

fn help_tools_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "🔧".bold(),
        "Tools — Quick Reference".cyan().bold()
    ));
    out.push_str("  /tools              List all registered tools\n");
    out.push_str("  /tool:name          Show tool parameters & description\n");
    out.push_str(&format!(
        "\n  {}\n",
        "All tools can be invoked by asking the agent directly.".dimmed()
    ));
    out
}

fn help_memory_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "🧠".bold(),
        "Memory — Quick Reference".cyan().bold()
    ));
    out.push_str("  /mem                Show memory stats (total, per-namespace)\n");
    out.push_str("  /mem:ns             List all namespaces with counts\n");
    out.push_str("  /mem:search <q>     Search memories by query text\n");
    out.push_str(&format!(
        "\n  {}\n",
        "For full memory operations, ask the agent to use memory tools.".dimmed()
    ));
    out
}

fn help_git_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "📦".bold(),
        "Git — Quick Reference".cyan().bold()
    ));
    out.push_str("  /git                Show working tree status\n");
    out.push_str("  /git:log            Show recent commits\n");
    out.push_str(&format!(
        "\n  {}\n",
        "Full Git operations (commit, push, diff, clone) — ask the agent.".dimmed()
    ));
    out
}

fn help_tasks_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "📋".bold(),
        "Tasks — Quick Reference".cyan().bold()
    ));
    out.push_str("  /tasks              Task overview (total, by status)\n");
    out.push_str("  /tasks:todo         List pending/in-progress tasks\n");
    out.push_str(&format!(
        "\n  {}\n",
        "Full task management — ask the agent to use task_planner.".dimmed()
    ));
    out
}

fn help_skills_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "🎯".bold(),
        "Skills — Quick Reference".cyan().bold()
    ));
    out.push_str("  /skills             List all loaded skills\n");
    out.push_str(&format!(
        "\n  {}\n",
        "Skill management — ask the agent to use manage_skills.".dimmed()
    ));
    out
}

// ============================================================================
// Info text helpers
// ============================================================================

fn section(out: &mut String, title: &str, items: &[(&str, &str)]) {
    out.push_str(&format!("  {}  {}\n", "▸".cyan().bold(), title.bold()));
    for (cmd, desc) in items {
        out.push_str(&format!(
            "    {}  {}\n",
            cmd.magenta().bold(),
            desc.dimmed()
        ));
    }
    out.push('\n');
}

fn version_text() -> String {
    let mut out = String::new();
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");
    let desc = env!("CARGO_PKG_DESCRIPTION");
    out.push_str(&format!(
        "  {}  {} v{}\n",
        "🚀".bold(),
        name.cyan().bold(),
        version.bold()
    ));
    out.push_str(&format!("  {}\n", desc.dimmed()));
    out.push_str(&format!(
        "\n  {} https://github.com/cargo-agent/cargo-agent\n",
        "🔗".dimmed()
    ));
    out
}

fn status_text() -> String {
    let mut out = String::new();
    let version = env!("CARGO_PKG_VERSION");
    out.push_str(&format!(
        "  {}  {}\n\n",
        "📊".bold(),
        "Agent Status".cyan().bold()
    ));
    out.push_str(&format!("  {} v{}\n", "Version".dimmed(), version.bold()));

    // OS info
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    out.push_str(&format!("  {} {}/{}\n", "Platform".dimmed(), os, arch));

    // Home dir
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        out.push_str(&format!(
            "  {} {}/.cargo-agent\n",
            "Data Dir".dimmed(),
            home
        ));
    }

    out.push_str(&format!(
        "  {} {}\n",
        "Config".dimmed(),
        "~/.cargo-agent/config.yaml".dimmed()
    ));
    out.push_str(&format!(
        "  {} {}\n",
        "Memories".dimmed(),
        "~/.cargo-agent/memories.db".dimmed()
    ));
    out.push_str(&format!(
        "  {} {}\n",
        "Skills".dimmed(),
        "~/.cargo-agent/skills/".dimmed()
    ));

    out.push_str(&format!(
        "\n  {} Use `/help` for all available commands.\n",
        "💡".dimmed()
    ));
    out
}

fn config_text() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "⚙".bold(),
        "Configuration Paths".cyan().bold()
    ));
    out.push_str("  Config file:   ~/.cargo-agent/config.yaml\n");
    out.push_str("  Skills dir:    ~/.cargo-agent/skills/\n");
    out.push_str("  Memories DB:   ~/.cargo-agent/memories.db\n");
    out.push_str("  Secrets:       ~/.cargo-agent/secrets.json\n");
    out.push_str("  Preferences:   ~/.cargo-agent/preferences.json\n");
    out.push_str(&format!("\n  {}\n", "https://docs.rs/cargo_agent".dimmed()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slash_colon() {
        let (cmd, args) = parse("/tool:code_analyze");
        assert_eq!(cmd, "tool");
        assert_eq!(args, "code_analyze");
    }

    #[test]
    fn test_parse_slash_space() {
        let (cmd, args) = parse("/help tools");
        assert_eq!(cmd, "help");
        assert_eq!(args, "tools");
    }

    #[test]
    fn test_parse_no_args() {
        let (cmd, args) = parse("/help");
        assert_eq!(cmd, "help");
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_not_slash() {
        let (cmd, args) = parse("hello world");
        assert!(cmd.is_empty());
        assert!(args.is_empty());
    }

    #[test]
    fn test_handle_pass_through() {
        let result = handle("hello world");
        assert!(matches!(result, SlashResult::PassThrough));
    }

    #[test]
    fn test_handle_help() {
        let result = handle("/help");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains("Slash Commands"));
            assert!(output.contains("/version"));
            assert!(output.contains("/tools"));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_help_alias() {
        let result = handle("/h");
        assert!(matches!(result, SlashResult::Handled(_)));
    }

    #[test]
    fn test_handle_help_topic() {
        let result = handle("/help:memory");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains("Memory"));
            assert!(output.contains("/mem:ns"));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_version() {
        let result = handle("/version");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains(env!("CARGO_PKG_VERSION")));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_version_alias() {
        let result = handle("/v");
        assert!(matches!(result, SlashResult::Handled(_)));
    }

    #[test]
    fn test_handle_status() {
        let result = handle("/status");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains("Agent Status"));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_config() {
        let result = handle("/config");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains("config.yaml"));
            assert!(output.contains("memories.db"));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_unknown() {
        let result = handle("/xyzzy");
        if let SlashResult::Handled(output) = result {
            assert!(output.contains("Unknown command"));
            assert!(output.contains("/xyzzy"));
        } else {
            panic!("Expected Handled");
        }
    }

    #[test]
    fn test_handle_dynamic_commands_pass_through() {
        // These need Gateway access, so they should pass through
        for cmd in &[
            "/tools", "/mem", "/git", "/tasks", "/skills", "/export", "/stats",
        ] {
            let result = handle(cmd);
            assert!(
                matches!(result, SlashResult::PassThrough),
                "Expected PassThrough for `{cmd}`"
            );
        }
    }

    #[test]
    fn test_parse_colon_with_spaces() {
        let (cmd, args) = parse("/mem:search hello world");
        assert_eq!(cmd, "mem");
        assert_eq!(args, "search hello world");
    }

    #[test]
    fn test_help_topic_invalid() {
        let result = help_topic("nonexistent");
        assert!(result.contains("No help available"));
    }

    #[test]
    fn test_section_format() {
        let mut out = String::new();
        section(&mut out, "Test", &[("/cmd", "description")]);
        assert!(out.contains("Test"));
        assert!(out.contains("/cmd"));
        assert!(out.contains("description"));
    }
}
