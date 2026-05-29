//! TUI Dashboard: terminal-based status display for the cargo-agent.
//!
//! Uses crossterm to render a full-screen dashboard with auto-refresh,
//! progress bars, and boxed panels showing agent status, tool usage,
//! memory stats, and token consumption.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetAttributes, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

/// Dashboard summary payload.
#[derive(Default)]
pub struct DashboardState {
    pub version: String,
    pub model_name: String,
    pub uptime_secs: u64,
    pub total_api_calls: u64,
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_memories: usize,
    pub total_tools: usize,
    pub health_status: String,
    pub conversation_messages: usize,
    pub context_max: usize,
    pub skills_loaded: usize,
    pub skills_active: usize,
    pub memory_by_namespace: Vec<(String, usize)>,
    pub memory_bytes: u64,
}

impl DashboardState {
    pub fn render_loop(&self) {
        let mut stdout = stdout();
        let _ = execute!(stdout, EnterAlternateScreen, cursor::Hide);
        let _ = terminal::enable_raw_mode();

        // Render the first frame immediately
        let _ = render_screen(&mut stdout, self);
        let _ = stdout.flush();

        let mut last_refresh = Instant::now();
        let mut paused = false;
        let refresh_interval = Duration::from_secs(5);

        loop {
            let remaining = if paused {
                refresh_interval
            } else {
                refresh_interval.saturating_sub(last_refresh.elapsed())
            };

            if event::poll(remaining).unwrap_or(false) {
                if let Event::Key(KeyEvent {
                    code, modifiers: _, ..
                }) = event::read()
                    .unwrap_or(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
                {
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => {
                            last_refresh = Instant::now();
                            let _ = render_screen(&mut stdout, self);
                            let _ = stdout.flush();
                        }
                        KeyCode::Char(' ') => {
                            paused = !paused;
                            last_refresh = Instant::now();
                        }
                        _ => {}
                    }
                }
            }

            if !paused && last_refresh.elapsed() >= refresh_interval {
                last_refresh = Instant::now();
                let _ = render_screen(&mut stdout, self);
                let _ = stdout.flush();
            }

            // Always render countdown
            let countdown = if paused {
                "paused".to_string()
            } else {
                let secs = remaining.as_secs();
                format!("{secs}s")
            };
            render_status_bar(&mut stdout, &self.health_status, &countdown, paused).ok();
            let _ = stdout.flush();
        }

        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
    }
}

fn render_screen(stdout: &mut impl Write, state: &DashboardState) -> std::io::Result<()> {
    let (w, _h) = terminal::size().unwrap_or((100, 30));

    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;

    // Title bar
    let title = " Cargo Agent Dashboard ";
    let title_line = format!("{}═", "═".repeat(w.saturating_sub(2) as usize));
    draw_text(stdout, 0, 0, &title_line, Color::Cyan)?;
    draw_text_centered(stdout, 0, title, Color::White)?;

    // Calculate panel widths
    let half = (w as usize / 2).saturating_sub(1);
    let col1_x = 2usize;
    let col2_x = half + 2;
    let panel_w = half.saturating_sub(2);

    // Row positions
    let mut y: u16 = 2;

    // ── Panel row 1: Agent Info | Token Usage ──
    y = draw_panel_border(stdout, y, col1_x, panel_w, " Agent Info ")?;
    draw_panel_border(
        stdout,
        y.saturating_sub(1),
        col2_x,
        panel_w,
        " Token Usage ",
    )?;

    let inner_y = y;
    draw_kv(stdout, inner_y, col1_x + 2, "Version", &state.version)?;
    draw_kv(stdout, inner_y + 1, col1_x + 2, "Model", &state.model_name)?;
    draw_kv(
        stdout,
        inner_y + 2,
        col1_x + 2,
        "Uptime",
        &format_uptime(state.uptime_secs),
    )?;

    let token_total = state.prompt_tokens + state.completion_tokens;
    let efficiency = if token_total > 0 {
        (state.completion_tokens as f64 / token_total as f64 * 100.0) as u64
    } else {
        0
    };

    draw_kv(
        stdout,
        inner_y,
        col2_x + 2,
        "Prompt",
        &format_tokens(state.prompt_tokens),
    )?;
    draw_kv(
        stdout,
        inner_y + 1,
        col2_x + 2,
        "Completion",
        &format_tokens(state.completion_tokens),
    )?;
    draw_kv(
        stdout,
        inner_y + 2,
        col2_x + 2,
        "Total",
        &format_tokens(state.total_tokens),
    )?;
    draw_kv(
        stdout,
        inner_y + 3,
        col2_x + 2,
        "Efficiency",
        &format!("{efficiency}% completion"),
    )?;

    y = inner_y + 4;

    // ── Panel row 2: Conversation | System ──
    y += 1;
    y = draw_panel_border(stdout, y, col1_x, panel_w, " Conversation ")?;
    draw_panel_border(stdout, y.saturating_sub(1), col2_x, panel_w, " System ")?;

    let inner_y2 = y;
    let ctx_pct = if state.context_max > 0 {
        (state.conversation_messages as f64 / state.context_max as f64 * 100.0).round() as u16
    } else {
        0
    };
    let bar_w = panel_w.saturating_sub(14);
    draw_kv(
        stdout,
        inner_y2,
        col1_x + 2,
        "Messages",
        &format!(
            "{}/{} ({}%)",
            state.conversation_messages, state.context_max, ctx_pct
        ),
    )?;
    draw_bar(
        stdout,
        inner_y2 + 1,
        col1_x + 2,
        bar_w as u16,
        ctx_pct,
        Color::Cyan,
    )?;

    draw_kv(
        stdout,
        inner_y2,
        col2_x + 2,
        "Tools",
        &format!("{} registered", state.total_tools),
    )?;
    draw_kv(
        stdout,
        inner_y2 + 1,
        col2_x + 2,
        "Skills",
        &format!("{} ({} active)", state.skills_loaded, state.skills_active),
    )?;
    draw_kv(
        stdout,
        inner_y2 + 2,
        col2_x + 2,
        "Memories",
        &format!("{} total", state.total_memories),
    )?;
    if state.memory_bytes > 0 {
        let mb = state.memory_bytes as f64 / 1024.0 / 1024.0;
        draw_kv(
            stdout,
            inner_y2 + 3,
            col2_x + 2,
            "Memory",
            &format!("{mb:.1} MB"),
        )?;
    }

    y = inner_y2 + 5;

    // ── Panel row 3: Memory Breakdown (full width) ──
    if !state.memory_by_namespace.is_empty() {
        y += 1;
        let full_w = w as usize - 4;
        y = draw_panel_border(stdout, y, col1_x, full_w, " Memory Breakdown ")?;

        let max_count = state
            .memory_by_namespace
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1);
        let bar_area_w = (full_w / 2).saturating_sub(16);

        for (i, (ns, count)) in state.memory_by_namespace.iter().enumerate() {
            let ny = y + 1 + i as u16;
            let pct = (*count as f64 / max_count as f64 * 100.0).round() as u16;
            let bar_chars = (pct as usize * bar_area_w / 100).max(1);
            let bar_str = format!(
                "{}{}",
                "█".repeat(bar_chars),
                "░".repeat(bar_area_w.saturating_sub(bar_chars))
            );
            draw_text(
                stdout,
                ny,
                col1_x + 2,
                &format!("{ns:<16} {:>4}", count),
                Color::White,
            )?;
            draw_text(
                stdout,
                ny,
                col1_x + 22,
                &format!(" {bar_str}"),
                Color::DarkGrey,
            )?;
        }
        y += state.memory_by_namespace.len() as u16 + 1;
    }

    // ── API Calls footer ──
    y += 1;
    if state.total_api_calls > 0 {
        draw_text(
            stdout,
            y,
            col1_x,
            &format!("Total API calls: {}", state.total_api_calls),
            Color::DarkGrey,
        )?;
    }

    Ok(())
}

/// Draw a boxed panel border with a title.
fn draw_panel_border(
    stdout: &mut impl Write,
    y: u16,
    x: usize,
    width: usize,
    title: &str,
) -> std::io::Result<u16> {
    let inner_y = y + 1;
    let w = width.saturating_sub(2);
    let title_display = if title.len() < w {
        let pad_right = w - title.len();
        if pad_right > 1 {
            format!("─{title}{}", "─".repeat(pad_right - 1))
        } else {
            title.to_string()
        }
    } else {
        format!("─{title}").chars().take(w).collect()
    };
    draw_text(stdout, y, x, &format!("┌{title_display}┐"), Color::DarkGrey)?;

    Ok(inner_y)
}

fn draw_text(
    stdout: &mut impl Write,
    y: u16,
    x: usize,
    text: &str,
    color: Color,
) -> std::io::Result<()> {
    execute!(
        stdout,
        cursor::MoveTo(x as u16, y),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    )
}

use crossterm::style::Attribute;

fn draw_text_centered(
    stdout: &mut impl Write,
    y: u16,
    text: &str,
    color: Color,
) -> std::io::Result<()> {
    let (w, _) = terminal::size().unwrap_or((100, 24));
    let x = (w.saturating_sub(text.len() as u16 + 2)) / 2;
    execute!(
        stdout,
        cursor::MoveTo(x, y),
        SetForegroundColor(color),
        SetAttributes(Attribute::Bold.into()),
        Print(text),
        ResetColor
    )
}

fn draw_kv(
    stdout: &mut impl Write,
    y: u16,
    x: usize,
    key: &str,
    value: &str,
) -> std::io::Result<()> {
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

fn draw_bar(
    stdout: &mut impl Write,
    y: u16,
    x: usize,
    width: u16,
    percent: u16,
    color: Color,
) -> std::io::Result<()> {
    let filled = ((percent as f64 * width as f64 / 100.0).round() as u16).min(width);
    let empty = width.saturating_sub(filled);

    let bar = format!(
        "[{}{}]",
        "█".repeat(filled as usize),
        "░".repeat(empty as usize)
    );

    execute!(
        stdout,
        cursor::MoveTo(x as u16, y),
        SetForegroundColor(color),
        Print(bar),
        ResetColor
    )
}

fn render_status_bar(
    stdout: &mut impl Write,
    health: &str,
    countdown: &str,
    paused: bool,
) -> std::io::Result<()> {
    let (w, h) = terminal::size().unwrap_or((100, 30));
    let y = h.saturating_sub(1);
    let _ = y; // used for positioning

    let health_icon = "\u{25cf}"; // ●
    let health_color = if health == "ok" {
        Color::Green
    } else {
        Color::Red
    };
    let pause_label = if paused { "⏸ paused" } else { "↻ auto" };

    let status_text = format!(
        " {} {}  |  Refresh: {} {}  |  Press 'q' quit, 'r' refresh, 'Space' pause  ",
        health_icon, health, countdown, pause_label,
    );

    // Pad to full width
    let padded = format!("{:<width$}", status_text, width = w as usize);

    execute!(
        stdout,
        cursor::MoveTo(0, y),
        SetForegroundColor(health_color),
        Print(&padded[..health_icon.len() + 1]),
        SetForegroundColor(Color::DarkGrey),
        Print(&padded[health_icon.len() + 1..]),
        ResetColor
    )
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
