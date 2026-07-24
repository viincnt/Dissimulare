use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};

use crate::app::App;
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], "Run");

    let running = app.running.as_ref();
    let snapshot = running.map(|r| r.stats.snapshot());
    let elapsed = app.started_at.map(|t| t.elapsed()).unwrap_or_default();
    let secs = elapsed.as_secs();

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(4), // big blocked-count number
            Constraint::Length(1), // "requests blocked" caption
            Constraint::Length(1), // spacer
            Constraint::Length(1), // total/uptime/listen-address summary
            Constraint::Fill(1),
        ])
        .split(areas[1]);

    let blocked = snapshot.map(|s| s.blocked_requests).unwrap_or(0);
    let big = BigText::builder()
        .pixel_size(PixelSize::HalfHeight)
        .centered()
        .lines([Line::from(blocked.to_string()).style(theme::bold_accent())])
        .build();
    frame.render_widget(big, v[1]);

    frame.render_widget(
        Paragraph::new("requests blocked").alignment(Alignment::Center).style(theme::muted()),
        v[2],
    );

    let total = snapshot.map(|s| s.total_requests).unwrap_or(0);
    let summary = format!(
        "{total} total requests \u{b7} {:02}:{:02}:{:02} uptime \u{b7} {}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60,
        running.map(|r| r.listen_addr.to_string()).unwrap_or_default()
    );
    frame.render_widget(
        Paragraph::new(summary).alignment(Alignment::Center).style(theme::bright()),
        v[4],
    );

    super::draw_help(frame, areas[2], &[("q / Esc / Ctrl-C", "stop the proxy")]);
}
