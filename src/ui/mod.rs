//! Terminal UI: colors, formatting, and progress indicators for the CLI.
//!
//! Provides colored output, separators, headers, and loading spinners
//! to make the cargo-agent CLI more visually polished.

use colored::Colorize;
use crossterm::cursor;
use crossterm::terminal;
use crossterm::QueueableCommand;
use std::io::{self, Write};

// ============================================================================
// Banner
// ============================================================================

/// Show the application banner.
pub fn show_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!(
        "  {} {}",
        "Cargo Agent".bold().cyan(),
        format!("v{version}").dimmed()
    );
    println!("  {}", "Self-evolving AI assistant".dimmed());
    println!("  {}", "Type /quit to exit".dimmed());
    println!();
}

/// Show the one-shot response banner (for `cargo-agent run`).
pub fn show_response_banner() {
    println!(
        "  {} {}\n",
        "Cargo Agent".bold().cyan(),
        env!("CARGO_PKG_VERSION").dimmed()
    );
}

// ============================================================================
// Separators
// ============================================================================

const SEPARATOR: &str = "─";
const SEPARATOR_WIDTH: usize = 60;

/// Print a horizontal separator line.
pub fn separator() {
    println!("{}", SEPARATOR.repeat(SEPARATOR_WIDTH).dimmed());
}

/// Print a thin separator (lighter visual weight).
pub fn thin_separator() {
    println!("{}", "·".repeat(SEPARATOR_WIDTH).dimmed());
}

// ============================================================================
// Output helpers
// ============================================================================

/// Print the user's input prompt with a visual marker.
pub fn print_prompt(input: &str) {
    println!("\n  {} {}", "▸".green().bold(), input);
    thin_separator();
}

/// Print the agent's response header.
pub fn print_response_header() {
    println!("\n  {} {}", "◆".cyan().bold(), "Response".dimmed());
}

/// Print an error message in red.
pub fn print_error(msg: &str) {
    eprintln!("\n  {} {}", "✗".red().bold(), msg.red());
}

/// Print a success/confirmation message in green.
pub fn print_success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

/// Print an informational message in yellow.
pub fn print_info(msg: &str) {
    println!("  {} {}", "ℹ".yellow().bold(), msg.yellow());
}

/// Print a tool execution indicator in magenta.
pub fn print_tool_call(name: &str, summary: &str) {
    println!(
        "  {} {} {}",
        "⚙".magenta().bold(),
        name.magenta(),
        format!("— {summary}").dimmed(),
    );
}

/// Print formatted response text with markdown-aware rendering.
pub fn print_response(text: &str) {
    // Split on code blocks (``` ... ```) to preserve them
    let parts: Vec<&str> = text.split("```").collect();

    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 1 {
            // Code block — dim background
            let code = part.trim_start_matches('\n').trim_end().to_string();
            // Try to extract language
            let (lang, body) = if let Some(newline_pos) = code.find('\n') {
                let first_line = code[..newline_pos].trim().to_string();
                let rest = code[newline_pos + 1..].to_string();
                (
                    if first_line.is_empty() {
                        None
                    } else {
                        Some(first_line)
                    },
                    rest,
                )
            } else {
                (None, code)
            };

            if let Some(lang) = lang {
                println!(
                    "  {} {}",
                    "┌".cyan().bold(),
                    format!("[{lang}]").cyan().bold()
                );
            }
            for line in body.lines() {
                println!("  {}", line.on_black().bright_yellow());
            }
            println!("  {}", "└".cyan().bold());
        } else {
            // Regular text — render markdown
            render_markdown_lines(part);
        }
    }
}

/// Render markdown-formatted lines (headers, bold, lists).
fn render_markdown_lines(text: &str) {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            println!();
            continue;
        }

        // Headers: ### or ## or #
        if let Some(stripped) = trimmed.strip_prefix("### ") {
            println!("  {}", format!("  {stripped}").cyan().bold());
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("## ") {
            println!("  {}", format!("▎ {stripped}").blue().bold());
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("# ") {
            println!("  {}", format!("▎ {stripped}").bright_blue().bold());
            println!("  {}", "─".repeat(40).dimmed());
            continue;
        }

        // Bold: **text** → bold
        let rendered = render_inline_bold(trimmed);
        println!("  {rendered}");
    }
}

/// Render inline markdown: **bold** → bold text.
fn render_inline_bold(text: &str) -> String {
    // Simple: replace **text** with colored/bold text
    // We use regex to find **...** patterns
    static BOLD_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = BOLD_RE
        .get_or_init(|| regex::Regex::new(r"\*\*(.+?)\*\*").expect("invalid regex: bold markdown"));

    let mut result = String::new();
    let mut last_end = 0;

    for cap in re.captures_iter(text) {
        let Some(m) = cap.get(0) else { continue };
        let inner = &cap[1];
        result.push_str(&text[last_end..m.start()]);
        result.push_str(&format!("{}{}", "\x1b[1m", inner));
        last_end = m.end();
    }

    // Strip remaining ** markers
    result.push_str(&text[last_end..]);
    result = result.replace("**", "");

    if result.contains('\x1b') {
        format!("{result}\x1b[0m")
    } else {
        result
    }
}

// ============================================================================
// Loading spinner
// ============================================================================

/// Simple spinner for indicating the agent is thinking.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A spinner that shows progress on the terminal.
pub struct Spinner {
    stdout: io::Stdout,
    frame: usize,
    message: String,
    running: bool,
}

impl Spinner {
    /// Create a new spinner with the given message.
    pub fn new(message: &str) -> Self {
        Self {
            stdout: io::stdout(),
            frame: 0,
            message: message.to_string(),
            running: false,
        }
    }

    /// Start the spinner.
    pub fn start(&mut self) {
        self.running = true;
        let _ = terminal::enable_raw_mode();
        let _ = self.stdout.queue(cursor::Hide);
        self.render_frame();
    }

    /// Stop the spinner and clear the line.
    pub fn stop(&mut self) {
        self.running = false;
        let _ = self.stdout.queue(cursor::Show);
        let _ = self.stdout.write_all(b"\r\x1b[K");
        let _ = self.stdout.flush();
        let _ = terminal::disable_raw_mode();
    }

    /// Advance to the next frame.
    pub fn tick(&mut self) {
        if !self.running {
            return;
        }
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
        self.render_frame();
    }

    fn render_frame(&mut self) {
        let frame = SPINNER_FRAMES[self.frame];
        let output = format!("\r\x1b[K  {} {}", frame.cyan(), self.message.dimmed());
        let _ = self.stdout.write_all(output.as_bytes());
        let _ = self.stdout.flush();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if self.running {
            self.stop();
        }
    }
}
