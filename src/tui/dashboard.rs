//! TUI Dashboard: terminal-based status display for the cargo-agent.
//!
//! Uses crossterm to render a full-screen dashboard showing agent status,
//! tool usage, memory stats, and token consumption.

use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};

/// Dashboard summary payload.
#[derive(Default)]
pub struct DashboardState {
    pub version: String,
    pub model: String,
    pub uptime_secs: u64,
    pub total_api_calls: u64,
    pub total_tokens: u64,
    pub total_memories: usize,
    pub total_tools: usize,
    pub health_status: String,
}

impl DashboardState {
    pub fn render(&self) {
        let mut stdout = stdout();
        let _ = execute!(stdout, EnterAlternateScreen);

        let _ = render_screen(&mut stdout, self);

        let _ = write!(stdout, "\n\n  Press any key to return to REPL...");
        let _ = stdout.flush();

        // Wait for a keypress
        let _buf = [0u8; 1];
        let _ = terminal::enable_raw_mode();
        let _ = crossterm::event::read();
        let _ = terminal::disable_raw_mode();

        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
    }
}

fn render_screen(stdout: &mut impl Write, state: &DashboardState) -> std::io::Result<()> {
    let (w, _h) = terminal::size().unwrap_or((80, 24));

    execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::Hide)?;

    // Title bar
    let title = " Cargo Agent Dashboard ";
    let title_line = format!("══{}═", "═".repeat(w.saturating_sub(title.len() as u16 + 4) as usize));
    draw_line(stdout, 0, &title_line, Color::Cyan)?;
    draw_text_centered(stdout, 0, title, Color::Cyan)?;

    // Row 1: Basic info
    let y = start_row(2);
    draw_section_title(stdout, y, "Agent Info")?;
    let y = y + 1;
    draw_kv(stdout, y, 2, "Version", &state.version)?;
    draw_kv(stdout, y + 1, 2, "Model", &state.model)?;
    draw_kv(stdout, y + 2, 2, "Uptime", &format_uptime(state.uptime_secs))?;

    // Row 1 right: Stats
    let x2 = w as usize / 2 + 4;
    draw_section_title_at(stdout, y, x2, "Usage Stats")?;
    draw_kv(stdout, y + 1, x2, "API Calls", &state.total_api_calls.to_string())?;
    draw_kv(stdout, y + 2, x2, "Total Tokens", &format_tokens(state.total_tokens))?;
    draw_kv(stdout, y + 3, x2, "Memories", &state.total_memories.to_string())?;

    // Row 2: Tools
    let y2 = y + 6;
    draw_section_title(stdout, y2, &format!("Tools ({} registered)", state.total_tools))?;

    // Row 3: Health
    let y3 = y2 + 2;
    let health_color = if state.health_status == "ok" {
        Color::Green
    } else {
        Color::Red
    };
    draw_text(stdout, y3, 2, &format!("Health: {}", state.health_status), health_color)?;

    Ok(())
}

fn draw_line(stdout: &mut impl Write, y: u16, text: &str, color: Color) -> std::io::Result<()> {
    execute!(
        stdout,
        cursor::MoveTo(0, y),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    )
}

fn draw_text_centered(stdout: &mut impl Write, y: u16, text: &str, color: Color) -> std::io::Result<()> {
    let (w, _) = terminal::size().unwrap_or((80, 24));
    let x = (w.saturating_sub(text.len() as u16)) / 2;
    execute!(
        stdout,
        cursor::MoveTo(x, y),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    )
}

fn draw_text(stdout: &mut impl Write, y: u16, x: usize, text: &str, color: Color) -> std::io::Result<()> {
    execute!(
        stdout,
        cursor::MoveTo(x as u16, y),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    )
}

fn draw_kv(stdout: &mut impl Write, y: u16, x: usize, key: &str, value: &str) -> std::io::Result<()> {
    execute!(
        stdout,
        cursor::MoveTo(x as u16, y),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{key}: ")),
        SetForegroundColor(Color::White),
        Print(value),
        ResetColor
    )
}

fn draw_section_title(stdout: &mut impl Write, y: u16, title: &str) -> std::io::Result<()> {
    draw_text(stdout, y, 2, title, Color::Yellow)
}

fn draw_section_title_at(stdout: &mut impl Write, y: u16, x: usize, title: &str) -> std::io::Result<()> {
    draw_text(stdout, y, x, title, Color::Yellow)
}

fn start_row(base: u16) -> u16 {
    base
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}
