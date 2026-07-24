use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let (content, help) = super::frame(frame, app, "Menu");

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // wordmark
            Constraint::Length(1), // tagline
            Constraint::Length(1), // spacer
            Constraint::Min(1),    // menu items
        ])
        .split(content);

    // A letter-spaced wordmark rather than a full ASCII-art logo: this is
    // the one screen that gets a bit of an "arrival" moment, the rest stay
    // dense/functional.
    let wordmark: String = "DISSIMULARE".chars().map(|c| format!("{c} ")).collect::<String>().trim_end().to_string();
    frame.render_widget(
        Paragraph::new(wordmark).style(theme::bold_accent()).alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new("Ad/Tracker Blocking \u{b7} Fingerprint Chaos")
            .style(theme::muted())
            .alignment(Alignment::Center),
        rows[1],
    );

    let items = app.menu_items();
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let selected = i == app.menu_index;
            let marker = if selected { "\u{203a} " } else { "  " };
            let name_style = if selected { theme::bold_accent() } else { theme::bright() };
            Line::from(vec![
                Span::styled(marker, name_style),
                Span::styled(format!("{name:<11}"), name_style),
                Span::styled(*desc, theme::muted()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), rows[3]);

    super::draw_help(frame, help, &[("\u{2191}\u{2193}/jk", "navigate"), ("\u{21b5}", "select"), ("q", "quit")]);
}
