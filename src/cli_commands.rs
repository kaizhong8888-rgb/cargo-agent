//! Slash commands: local CLI shortcuts that bypass the LLM.
//!
//! This module provides:
//! - `SlashAction` enum — the result of handling a slash command
//! - `parse()` — split `/cmd:args` or `/cmd args` into components
//! - Help text builders — rich formatted strings for `/help` topics
//!
//! Dispatch is owned by `Gateway::handle_slash_command()`.
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

// ── SlashAction — result of handling a slash command ──────

/// The outcome of handling a slash command.
///
/// `Gateway::handle_slash_command()` returns this to `main.rs`,
/// which then drives the REPL loop accordingly.
pub enum SlashAction {
    /// Display this text and continue the REPL.
    Output(String),
    /// Exit the REPL loop (`/quit`, `/exit`).
    Exit,
    /// Clear the screen and reset conversation state (`/clear`, `/cls`).
    Clear,
    /// Start the TUI dashboard (hands over terminal control).
    Dashboard,
    /// Not a slash command — pass through to the LLM.
    PassThrough,
}

// ── Command parsing ───────────────────────────────────────

/// Parse a slash command into `(command, args)`.
///
/// Supports both `/cmd:args` and `/cmd args` syntax.
/// Returns `("", "")` if the input doesn't start with `/`.
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

// ============================================================================
// Help system
// ============================================================================

/// Build the general help overview.
pub fn help_general() -> String {
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

    // MCP
    section(
        &mut out,
        "MCP Servers",
        &[
            ("/mcp", "List MCP server connections"),
            ("/mcp:list", "Show all MCP servers and status"),
            ("/mcp:status", "MCP bridge statistics"),
            ("/mcp:start <name>", "Start an MCP server"),
            ("/mcp:stop <name>", "Stop an MCP server"),
            ("/mcp:restart <name>", "Restart an MCP server"),
        ],
    );

    // Shortcuts
    section(
        &mut out,
        "Shortcuts",
        &[
            ("/shortcut add", "Add a command shortcut"),
            ("/shortcut remove", "Remove a shortcut"),
            ("/shortcut list", "List all shortcuts"),
            ("/shortcut | /sc", "Manage custom command aliases"),
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

/// Build help text for a specific topic.
pub fn help_topic(topic: &str) -> String {
    match topic {
        "tools" | "tool" => help_tools_detail(),
        "mem" | "memory" => help_memory_detail(),
        "git" => help_git_detail(),
        "tasks" | "task" | "task_planner" => help_tasks_detail(),
        "skills" | "skill" => help_skills_detail(),
        "shortcut" | "shortcuts" => help_shortcut_detail(),
        "commands" | "all" => help_general(),
        _ => format!(
            "No help available for `{topic}`.\nTry: tools, memory, git, tasks, skills, shortcuts, commands"
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

fn help_shortcut_detail() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {}  {}\n\n",
        "🔗".bold(),
        "Shortcuts — Quick Reference".cyan().bold()
    ));
    out.push_str("  /shortcut add <a> <c>    Create shortcut: /<a> → /<c>\n");
    out.push_str("  /shortcut remove <a>    Remove a shortcut\n");
    out.push_str("  /shortcut list          List all shortcuts\n");
    out.push_str("  /shortcut | /sc         Manage custom command aliases\n");
    out.push_str(&format!(
        "\n  {}\n",
        "Shortcuts persist in ~/.cargo-agent/shortcuts.json.".dimmed()
    ));
    out.push_str(&format!(
        "  {}\n",
        "Also accessible via the `config` tool with action: add_shortcut, remove_shortcut, list_shortcuts.".dimmed()
    ));
    out
}

// ============================================================================
// Info text builders
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

/// Build version info text.
pub fn version_text() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");
    let desc = env!("CARGO_PKG_DESCRIPTION");
    format!(
        "  {} {} v{}\n  {}\n\n  {} https://github.com/cargo-agent/cargo-agent\n",
        "🚀".bold(),
        name.cyan().bold(),
        version.bold(),
        desc.dimmed(),
        "🔗".dimmed(),
    )
}

/// Build agent status text.
pub fn status_text() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let mut out = format!(
        "  {}  {}\n\n  {} v{}\n  {} {}/{}\n",
        "📊".bold(),
        "Agent Status".cyan().bold(),
        "Version".dimmed(),
        version.bold(),
        "Platform".dimmed(),
        os,
        arch,
    );

    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        out.push_str(&format!("  {} {}/.cargo-agent\n", "Data Dir".dimmed(), home));
    }

    out.push_str(&format!(
        "  {} {}\n  {} {}\n  {} {}\n\n  {} Use `/help` for all available commands.\n",
        "Config".dimmed(),
        "~/.cargo-agent/config.yaml".dimmed(),
        "Memories".dimmed(),
        "~/.cargo-agent/memories/memories.db".dimmed(),
        "Skills".dimmed(),
        "~/.cargo-agent/skills/".dimmed(),
        "💡".dimmed(),
    ));
    out
}

/// Build config paths text.
pub fn config_text() -> String {
    format!(
        "  {}  {}\n\n  Config file:   ~/.cargo-agent/config.yaml\n  Skills dir:    ~/.cargo-agent/skills/\n  Memories DB:   ~/.cargo-agent/memories/memories.db\n  Secrets:       ~/.cargo-agent/secrets.json\n  Preferences:   ~/.cargo-agent/preferences.json\n\n  {}\n",
        "⚙".bold(),
        "Configuration Paths".cyan().bold(),
        "https://docs.rs/cargo_agent".dimmed(),
    )
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
    fn test_parse_colon_with_spaces() {
        let (cmd, args) = parse("/mem:search hello world");
        assert_eq!(cmd, "mem");
        assert_eq!(args, "search hello world");
    }

    #[test]
    fn test_section_format() {
        let mut out = String::new();
        section(&mut out, "Test", &[("/cmd", "description")]);
        assert!(out.contains("Test"));
        assert!(out.contains("/cmd"));
        assert!(out.contains("description"));
    }

    #[test]
    fn test_help_general_contains_commands() {
        let out = help_general();
        assert!(out.contains("Slash Commands"));
        assert!(out.contains("/version"));
        assert!(out.contains("/tools"));
    }

    #[test]
    fn test_help_topic_valid() {
        let out = help_topic("memory");
        assert!(out.contains("Memory"));
        assert!(out.contains("/mem:ns"));
    }

    #[test]
    fn test_help_topic_invalid() {
        let result = help_topic("nonexistent");
        assert!(result.contains("No help available"));
    }

    #[test]
    fn test_version_text_contains_version() {
        let out = version_text();
        assert!(out.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_status_text_contains_sections() {
        let out = status_text();
        assert!(out.contains("Agent Status"));
        assert!(out.contains("Version"));
        assert!(out.contains("Platform"));
    }

    #[test]
    fn test_config_text_contains_paths() {
        let out = config_text();
        assert!(out.contains("config.yaml"));
        assert!(out.contains("memories/memories.db"));
    }
}
