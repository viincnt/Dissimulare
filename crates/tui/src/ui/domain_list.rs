use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::DomainListKind;
use crate::theme;
use dissimulare_cli::config::DomainEntry;

pub fn draw(frame: &mut Frame, kind: DomainListKind, entries: &[DomainEntry], selected: usize, status: Option<&str>) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], kind.title());

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(areas[1]);

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new(kind.empty_hint()).alignment(Alignment::Center).style(theme::muted()),
            v[0],
        );
    } else {
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
        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), v[0]);
    }

    if let Some(status) = status {
        frame.render_widget(
            Paragraph::new(status).alignment(Alignment::Center).style(theme::bold_success()),
            v[1],
        );
    }

    super::draw_help(
        frame,
        areas[2],
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

/// Shared by the Add/Import/Export screens: a header, a one-line prompt, a
/// bordered single-line text box, and a help bar.
pub fn draw_text_input(frame: &mut Frame, breadcrumb: &str, prompt: &str, input: &str, help: &[(&str, &str)]) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], breadcrumb);

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(3), Constraint::Fill(1)])
        .split(areas[1]);

    frame.render_widget(Paragraph::new(prompt).alignment(Alignment::Center).style(theme::bright()), v[1]);

    frame.render_widget(
        Paragraph::new(input)
            .alignment(Alignment::Center)
            .style(theme::bold_white())
            .block(Block::default().borders(Borders::ALL).border_style(theme::accent())),
        v[2],
    );

    super::draw_help(frame, areas[2], help);
}
