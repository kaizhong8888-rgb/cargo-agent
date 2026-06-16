//! Terminal UI: colors, formatting, and progress indicators for the CLI.
//!
//! Provides colored output, separators, headers, and loading spinners
//! to make the cargo-agent CLI more visually polished.
//!
//! When the `tui` feature is disabled, falls back to simple text-only output.

use colored::Colorize;
use std::io::{self, Write};
use std::sync::Arc;

// crossterm imports — gated behind `tui` feature
#[cfg(feature = "tui")]
use crossterm::cursor;
#[cfg(feature = "tui")]
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
#[cfg(feature = "tui")]
use crossterm::terminal;
#[cfg(feature = "tui")]
use crossterm::QueueableCommand;

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
#[cfg(feature = "tui")]
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A spinner that shows progress on the terminal.
pub struct Spinner {
    message: String,
    running: bool,

    #[cfg(feature = "tui")]
    stdout: io::Stdout,
    #[cfg(feature = "tui")]
    frame: usize,
}

impl Spinner {
    /// Create a new spinner with the given message.
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            running: false,
            #[cfg(feature = "tui")]
            stdout: io::stdout(),
            #[cfg(feature = "tui")]
            frame: 0,
        }
    }

    /// Start the spinner.
    pub fn start(&mut self) {
        self.running = true;
        #[cfg(feature = "tui")]
        {
            let _ = terminal::enable_raw_mode();
            let _ = self.stdout.queue(cursor::Hide);
            self.render_frame();
        }
        #[cfg(not(feature = "tui"))]
        {
            let _ = write!(io::stdout(), "  ⏳ {} ...", self.message);
        }
    }

    /// Stop the spinner and clear the line.
    pub fn stop(&mut self) {
        self.running = false;
        #[cfg(feature = "tui")]
        {
            let _ = self.stdout.queue(cursor::Show);
            let _ = self.stdout.write_all(b"\r\x1b[K");
            let _ = self.stdout.flush();
            let _ = terminal::disable_raw_mode();
        }
        #[cfg(not(feature = "tui"))]
        {
            println!();
        }
    }

    /// Advance to the next frame.
    pub fn tick(&mut self) {
        #[cfg(feature = "tui")]
        {
            if !self.running {
                return;
            }
            self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
            self.render_frame();
        }
    }

    #[cfg(feature = "tui")]
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

// ============================================================================
// Blinking Status LED — shows real-time agent state during processing
// ============================================================================

/// Color codes for the status LED (ANSI SGR codes).
#[cfg(feature = "tui")]
mod led_colors {
    pub const CYAN: &str = "36";
    pub const MAGENTA: &str = "35";
    pub const YELLOW: &str = "33";
    pub const BLUE: &str = "34";
    pub const ORANGE: &str = "38;5;208";
    pub const GREY: &str = "90";
}

/// Timing constants for LED blinking.
#[cfg(feature = "tui")]
mod led_timing {
    pub const SLOW_ON_MS: u64 = 500;
    pub const SLOW_OFF_MS: u64 = 300;
    pub const FAST_ON_MS: u64 = 200;
    pub const FAST_OFF_MS: u64 = 150;
}

/// A blinking LED indicator that reflects the agent's current runtime state.
///
/// Renders a colored dot (●) on its own line. Blinks at different speeds
/// depending on the state. When `tui` feature is disabled, this is a no-op.
pub struct StatusIndicator {
    #[cfg(feature = "tui")]
    handle: Option<std::thread::JoinHandle<()>>,
    #[cfg(feature = "tui")]
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    #[allow(dead_code)]
    status: Arc<std::sync::atomic::AtomicU8>,
    #[allow(dead_code)]
    current_tool: Arc<std::sync::Mutex<String>>,
}

impl StatusIndicator {
    /// Create a new status indicator for the given agent.
    pub fn new(
        status: Arc<std::sync::atomic::AtomicU8>,
        current_tool: Arc<std::sync::Mutex<String>>,
    ) -> Self {
        Self {
            #[cfg(feature = "tui")]
            handle: None,
            #[cfg(feature = "tui")]
            stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            status,
            current_tool,
        }
    }

    /// Start the blinking indicator.
    pub fn start(&mut self) {
        #[cfg(feature = "tui")]
        {
            let stop = Arc::clone(&self.stop_flag);
            let status = Arc::clone(&self.status);
            let tool = Arc::clone(&self.current_tool);

            let handle = std::thread::spawn(move || {
                let _ = terminal::enable_raw_mode();
                let mut stdout = io::stdout();
                let _ = stdout.queue(cursor::Hide);
                let _ = stdout.flush();

                let mut visible = true;

                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    let status_code = status.load(std::sync::atomic::Ordering::Acquire);
                    let state = match status_code {
                        1 => LedState::SearchingMemories,
                        2 => LedState::CallingLLM,
                        3 => LedState::ExecutingTool,
                        4 => LedState::GeneratingResponse,
                        5 => LedState::TruncatingContext,
                        _ => LedState::Idle,
                    };

                    let tool_name = tool.lock().ok().map(|g| g.clone()).unwrap_or_default();
                    let (color, label, fast_blink) = state.render(&tool_name);
                    let (on_ms, off_ms) = if fast_blink {
                        (led_timing::FAST_ON_MS, led_timing::FAST_OFF_MS)
                    } else {
                        (led_timing::SLOW_ON_MS, led_timing::SLOW_OFF_MS)
                    };

                    if state == LedState::Idle {
                        visible = false;
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    if visible {
                        let output =
                            format!("\r\x1b[K  \x1b[{}m●\x1b[0m {}", color, label.dimmed());
                        let _ = stdout.write_all(output.as_bytes());
                        let _ = stdout.flush();
                        std::thread::sleep(std::time::Duration::from_millis(on_ms));
                    } else {
                        let _ = stdout.write_all(b"\r\x1b[K");
                        let _ = stdout.flush();
                        std::thread::sleep(std::time::Duration::from_millis(off_ms));
                    }
                    visible = !visible;
                }

                let _ = stdout.write_all(b"\r\x1b[K");
                let _ = stdout.queue(cursor::Show);
                let _ = stdout.flush();
                let _ = terminal::disable_raw_mode();
            });

            self.handle = Some(handle);
        }
    }

    /// Stop the indicator and wait for the background thread to exit.
    pub fn stop(self) {
        #[cfg(feature = "tui")]
        {
            self.stop_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(handle) = self.handle {
                let _ = handle.join();
            }
        }
    }
}

/// Represents the visual state of the LED indicator.
#[cfg(feature = "tui")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LedState {
    Idle,
    SearchingMemories,
    CallingLLM,
    ExecutingTool,
    GeneratingResponse,
    TruncatingContext,
}

#[cfg(feature = "tui")]
impl LedState {
    fn render(&self, tool_name: &str) -> (&'static str, String, bool) {
        match self {
            Self::Idle => (led_colors::GREY, String::new(), false),
            Self::SearchingMemories => (
                led_colors::BLUE,
                "🔍 Searching memories...".to_string(),
                false,
            ),
            Self::CallingLLM => (led_colors::CYAN, "🤖 Calling LLM...".to_string(), false),
            Self::ExecutingTool => (
                led_colors::MAGENTA,
                if tool_name.is_empty() {
                    "⚙️  Executing tool...".to_string()
                } else {
                    format!("⚙️  Executing {}...", tool_name)
                },
                true,
            ),
            Self::GeneratingResponse => (
                led_colors::YELLOW,
                "✍️  Composing response...".to_string(),
                false,
            ),
            Self::TruncatingContext => (
                led_colors::ORANGE,
                "📏 Managing context...".to_string(),
                true,
            ),
        }
    }
}

// ============================================================================
// Status bar — shows token usage, context length, model info after response
// ============================================================================

/// Information needed to render the status bar.
#[derive(Debug, Clone, Default)]
pub struct StatusInfo {
    pub api_calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub messages_count: usize,
    pub messages_max: usize,
    pub model_name: String,
    pub elapsed_secs: f32,
}

/// Print a compact status bar showing token usage and context info.
pub fn print_status_bar(info: &StatusInfo) {
    let token_str = format_tokens(info.total_tokens);
    let msg_pct = if info.messages_max > 0 {
        (info.messages_count as f64 / info.messages_max as f64 * 100.0).round() as u16
    } else {
        0
    };

    let mut parts: Vec<String> = Vec::new();

    if info.total_tokens > 0 {
        let icon = if info.prompt_tokens > info.completion_tokens {
            "📤"
        } else {
            "📊"
        };
        parts.push(format!("{} {} tokens", icon, token_str.bold()));
    }

    if info.messages_count > 0 {
        parts.push(format!(
            "💬 {} msgs ({}%)",
            info.messages_count.to_string().bold(),
            msg_pct
        ));
    }

    if !info.model_name.is_empty() {
        let model_short = info
            .model_name
            .split('/')
            .next_back()
            .unwrap_or(&info.model_name);
        parts.push(format!("🤖 {}", model_short.dimmed()));
    }

    if info.elapsed_secs > 0.0 {
        let color = if info.elapsed_secs > 10.0 {
            "🔴"
        } else if info.elapsed_secs > 5.0 {
            "🟡"
        } else {
            "⚡"
        };
        parts.push(format!("{} {:.1}s", color, info.elapsed_secs));
    }

    if info.api_calls > 1 {
        parts.push(format!("🔄 {} calls", info.api_calls));
    }

    if parts.is_empty() {
        return;
    }

    println!();
    println!("  {}", format!("  {}", parts.join("  │  ")).dimmed());
}

/// Format token count for display.
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Print a "now thinking" line with the current agent status.
pub fn print_thinking_status(status_display: &str) {
    println!("  ⏳ {} {}", "⟐".dimmed(), status_display.dimmed());
}

/// Print a status update during processing (replaces previous line).
pub fn print_status_update(status_display: &str) {
    #[cfg(feature = "tui")]
    {
        print!("\r\x1b[K  ⏳ {} {}", "⟐".dimmed(), status_display.dimmed());
        let _ = io::stdout().flush();
    }
    #[cfg(not(feature = "tui"))]
    {
        println!("  ⏳ {}", status_display.dimmed());
    }
}

// ============================================================================
// Multi-line input reader
// ============================================================================

/// Read multi-line user input from the terminal.
///
/// When `tui` feature is enabled, supports Shift+Enter for newlines,
/// arrow keys, and visual cursor. Otherwise falls back to simple stdin line.
pub fn read_multiline_input() -> Option<String> {
    #[cfg(feature = "tui")]
    {
        tui_read_multiline_input()
    }
    #[cfg(not(feature = "tui"))]
    {
        simple_read_line()
    }
}

/// Simple stdin line reader (fallback when `tui` is disabled).
#[cfg(not(feature = "tui"))]
fn simple_read_line() -> Option<String> {
    use std::io::BufRead;
    print!("  {} ", "▸".green().bold());
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) => None, // EOF
        Ok(_) => {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}

/// Full-featured multi-line reader (crossterm-based).
#[cfg(feature = "tui")]
fn tui_read_multiline_input() -> Option<String> {
    let _ = terminal::enable_raw_mode();
    let mut stdout = io::stdout();
    let _ = stdout.queue(cursor::Show);
    let _ = stdout.flush();

    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut cursor_pos: usize = 0;

    print!("  {} ", "▸".green().bold());
    let _ = stdout.flush();

    loop {
        if event::poll(std::time::Duration::from_millis(50)).is_err() {
            continue;
        }
        let Ok(ev) = event::read() else { continue };
        let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = ev
        else {
            continue;
        };

        match (code, modifiers) {
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if lines.is_empty() && current_line.is_empty() {
                    terminal::disable_raw_mode().ok();
                    println!();
                    return None;
                }
                lines.push(current_line);
                let result = lines.join("\n");
                terminal::disable_raw_mode().ok();
                println!();
                return Some(result);
            }

            (KeyCode::Enter, KeyModifiers::SHIFT)
            | (KeyCode::Enter, KeyModifiers::ALT)
            | (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                lines.push(current_line.clone());
                current_line.clear();
                cursor_pos = 0;
                print!("\n  {} ", "···".dimmed());
                let _ = stdout.flush();
            }

            (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                terminal::disable_raw_mode().ok();
                println!();
                return None;
            }

            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                let chars: Vec<char> = current_line.chars().collect();
                let mut new_line: Vec<char> = chars;
                new_line.insert(cursor_pos, c);
                current_line = new_line.into_iter().collect();
                cursor_pos += 1;
                redraw_line(&current_line, cursor_pos, &mut stdout);
            }

            (KeyCode::Backspace, KeyModifiers::NONE) => {
                if cursor_pos > 0 {
                    let chars: Vec<char> = current_line.chars().collect();
                    let mut new_line: Vec<char> = chars;
                    new_line.remove(cursor_pos - 1);
                    current_line = new_line.into_iter().collect();
                    cursor_pos -= 1;
                    redraw_line(&current_line, cursor_pos, &mut stdout);
                }
            }

            (KeyCode::Delete, KeyModifiers::NONE) => {
                let chars: Vec<char> = current_line.chars().collect();
                if cursor_pos < chars.len() {
                    let mut new_line: Vec<char> = chars;
                    new_line.remove(cursor_pos);
                    current_line = new_line.into_iter().collect();
                    redraw_line(&current_line, cursor_pos, &mut stdout);
                }
            }

            (KeyCode::Left, KeyModifiers::NONE) => {
                if cursor_pos > 0 {
                    cursor_pos -= 1;
                    redraw_line(&current_line, cursor_pos, &mut stdout);
                }
            }

            (KeyCode::Right, KeyModifiers::NONE) => {
                let chars: Vec<char> = current_line.chars().collect();
                if cursor_pos < chars.len() {
                    cursor_pos += 1;
                    redraw_line(&current_line, cursor_pos, &mut stdout);
                }
            }

            (KeyCode::Home, _) => {
                cursor_pos = 0;
                let _ = write!(stdout, "\r\x1b[4C");
                let _ = stdout.flush();
            }

            (KeyCode::End, _) => {
                cursor_pos = current_line.chars().count();
                let prefix_len = 4;
                let target = prefix_len + current_line.chars().map(|c| c.len_utf8()).sum::<usize>();
                let _ = write!(stdout, "\r\x1b[{}C", target);
                let _ = stdout.flush();
            }

            (KeyCode::Esc, _) => {
                current_line.clear();
                cursor_pos = 0;
                redraw_line(&current_line, cursor_pos, &mut stdout);
            }

            _ => {}
        }
    }
}

/// Redraw the current line with cursor at the correct position.
#[cfg(feature = "tui")]
fn redraw_line(line: &str, cursor_pos: usize, stdout: &mut io::Stdout) {
    print!("\r\x1b[K");
    print!("  {} {}", "▸".green().bold(), line);
    let prefix_len = 4;
    let target = prefix_len
        + line
            .chars()
            .take(cursor_pos)
            .map(|c| c.len_utf8())
            .sum::<usize>();
    print!("\r\x1b[{}C", target);
    let _ = stdout.flush();
}
