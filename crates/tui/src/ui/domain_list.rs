use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, DomainListKind};
use crate::theme;
use dissimulare_cli::config::DomainEntry;

pub fn draw(
    frame: &mut Frame,
    app: &App,
    kind: DomainListKind,
    entries: &[DomainEntry],
    selected: usize,
    status: Option<&str>,
) {
    let (content, help) = super::frame(frame, app, kind.title());

    if entries.is_empty() {
        frame.render_widget(Paragraph::new(kind.empty_hint()).style(theme::muted()), content);
    } else {
        let rows =
            Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(content);

        let lines: Vec<Line> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_selected = i == selected;
                let checkbox = if entry.enabled { "[x]" } else { "[ ]" };
                let base_style = if entry.enabled { theme::bright() } else { theme::muted() };
                let row_style = if is_selected { theme::bold_accent() } else { base_style };
                let marker = if is_selected { "\u{203a} " } else { "  " };
                Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(format!("{checkbox} "), row_style),
                    Span::styled(entry.domain.clone(), row_style),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), rows[0]);

        if let Some(status) = status {
            frame.render_widget(Paragraph::new(status).style(theme::bold_success()), rows[1]);
        }
    }

    super::draw_help(
        frame,
        help,
        &[
            ("\u{2191}\u{2193}/jk", "move"),
            ("\u{21b5}/space", "toggle"),
            ("a", "add"),
            ("d", "remove"),
            ("i", "import"),
            ("e", "export"),
            ("Esc", "back"),
        ],
    );
}

/// Shared by the Add/Import/Export screens: a one-line prompt and a
/// bordered single-line text box.
pub fn draw_text_input(
    frame: &mut Frame,
    app: &App,
    breadcrumb: &str,
    prompt: &str,
    input: &str,
    help: &[(&str, &str)],
) {
    let (content, help_area) = super::frame(frame, app, breadcrumb);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(3)])
        .split(content);

    frame.render_widget(Paragraph::new(prompt).style(theme::bright()), rows[0]);

    frame.render_widget(
        Paragraph::new(input)
            .style(theme::bold_white())
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(theme::accent())),
        rows[2],
    );

    super::draw_help(frame, help_area, help);
}
