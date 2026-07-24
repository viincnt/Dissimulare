use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let (content, help) = super::frame(frame, app, "Run");

    let running = app.running.as_ref();
    let snapshot = running.map(|r| r.stats.snapshot());
    let elapsed = app.started_at.map(|t| t.elapsed()).unwrap_or_default();
    let secs = elapsed.as_secs();
    let total = snapshot.map(|s| s.total_requests).unwrap_or(0);
    let blocked = snapshot.map(|s| s.blocked_requests).unwrap_or(0);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // stats line
            Constraint::Length(1), // rule
            Constraint::Length(1), // "recent" label
            Constraint::Min(1),    // live scrolling log, Claude-Code-transcript style
        ])
        .split(content);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{total} total"), theme::bright()),
            Span::styled("  \u{b7}  ", theme::muted()),
            Span::styled(format!("{blocked} blocked"), theme::bold_err()),
            Span::styled("  \u{b7}  ", theme::muted()),
            Span::styled(
                format!("{:02}:{:02}:{:02} uptime", secs / 3600, (secs / 60) % 60, secs % 60),
                theme::bright(),
            ),
        ])),
        rows[0],
    );

    super::draw_rule(frame, rows[1]);
    frame.render_widget(Paragraph::new("recent").style(theme::muted()), rows[2]);

    // Deliberately unboxed, plain lines flowing in — the one place this
    // TUI shows a growing log rather than a static screen, so it should
    // read like a transcript, not another bordered widget.
    let log_area = rows[3];
    let recent = running.map(|r| r.stats.recent_blocks()).unwrap_or_default();
    let visible_rows = log_area.height as usize;
    let url_width = log_area.width.saturating_sub(2) as usize;
    let tail: Vec<Line> = recent
        .iter()
        .rev()
        .take(visible_rows)
        .rev()
        .map(|entry| {
            Line::from(vec![
                Span::styled("\u{2717} ", theme::err()),
                Span::styled(truncate(&entry.url, url_width), theme::muted()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(tail), log_area);

    super::draw_help(frame, help, &[("q / Esc / Ctrl-C", "stop the proxy")]);
}

/// Truncates `s` to at most `max` characters, adding an ellipsis when it
/// doesn't fit — long blocked URLs shouldn't wrap and eat extra log rows.
fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    format!("{}\u{2026}", s.chars().take(keep).collect::<String>())
}
