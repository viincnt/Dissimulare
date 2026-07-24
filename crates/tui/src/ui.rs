mod consent;
mod dashboard;
mod domain_list;
mod menu;
mod message;

use crate::app::{App, Screen};
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Menu => menu::draw(frame, app),
        Screen::Consent { thumbprint, input, error } => {
            consent::draw(frame, thumbprint, input, error.as_deref())
        }
        Screen::Busy(label) => message::draw_busy(frame, label),
        Screen::Message { title, lines, tone } => message::draw(frame, title, lines, tone),
        Screen::ConfirmUninstall { selected } => message::draw_confirm_uninstall(frame, *selected),
        Screen::DomainList { kind, entries, selected, status } => {
            domain_list::draw(frame, *kind, entries, *selected, status.as_deref())
        }
        Screen::DomainListAdd { kind, input } => domain_list::draw_text_input(
            frame,
            &format!("{} \u{203a} Add", kind.title()),
            "Enter a domain (e.g. example.com):",
            input,
            &[("type", "domain"), ("\u{21b5}", "add"), ("Esc", "cancel")],
        ),
        Screen::DomainListImportPath { kind, input } => domain_list::draw_text_input(
            frame,
            &format!("{} \u{203a} Import", kind.title()),
            "Path to a plain-text domain list (one per line):",
            input,
            &[("type", "path"), ("\u{21b5}", "import"), ("Esc", "cancel")],
        ),
        Screen::DomainListExportPath { kind, input } => domain_list::draw_text_input(
            frame,
            &format!("{} \u{203a} Export", kind.title()),
            "Path to write the enabled domains to:",
            input,
            &[("type", "path"), ("\u{21b5}", "export"), ("Esc", "cancel")],
        ),
        Screen::Dashboard => dashboard::draw(frame, app),
    }
}

/// Every screen's content lives in this centered column rather than
/// stretching edge-to-edge, matching the rest of the project's TUI style.
pub fn centered_area(area: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(80), Constraint::Percentage(10)])
        .split(area)[1]
}

/// header / content / help split within [`centered_area`], used by every
/// screen except the main menu (which has its own bespoke logo layout).
pub fn outer_layout(area: Rect) -> [Rect; 3] {
    let area = centered_area(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(area);
    [chunks[0], chunks[1], chunks[2]]
}

pub fn draw_header(frame: &mut Frame, area: Rect, breadcrumb: &str) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled("\u{1F3AD}", theme::bold_accent()),
            Span::raw(" "),
            Span::styled("Dissimulare", theme::bold_white()),
            Span::styled("  \u{203A}  ", theme::muted()),
            Span::styled(breadcrumb.to_string(), theme::bright()),
        ]))
        .block(Block::default().borders(Borders::BOTTOM).border_style(theme::subtle())),
        area,
    );
}

pub fn draw_help(frame: &mut Frame, area: Rect, bindings: &[(&str, &str)]) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, desc)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("    ", theme::muted()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme::accent()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, theme::muted()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP).border_style(theme::subtle())),
        area,
    );
}

/// A row of bordered, centered "buttons" — used for the main menu and for
/// the Yes/No uninstall confirmation alike.
pub fn draw_buttons(frame: &mut Frame, area: Rect, options: &[(&str, &str)], selected: usize) {
    const GAP: u16 = 3;
    const PAD: u16 = 2;

    let btn_widths: Vec<u16> =
        options.iter().map(|(label, _)| (label.chars().count() as u16) + PAD * 2 + 2).collect();

    let total_width: u16 =
        btn_widths.iter().sum::<u16>() + GAP * (options.len().saturating_sub(1) as u16);

    let center = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(total_width), Constraint::Fill(1)])
        .split(area)[1];

    let mut constraints: Vec<Constraint> = Vec::new();
    for (i, &w) in btn_widths.iter().enumerate() {
        constraints.push(Constraint::Length(w));
        if i < options.len() - 1 {
            constraints.push(Constraint::Length(GAP));
        }
    }

    let btn_areas = Layout::default().direction(Direction::Horizontal).constraints(constraints).split(center);

    for (i, (label, _)) in options.iter().enumerate() {
        let btn_rect = btn_areas[i * 2];
        let style = if i == selected { theme::bold_accent() } else { theme::muted() };

        frame.render_widget(
            Paragraph::new(format!("  {label}  "))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).border_style(style))
                .style(style),
            btn_rect,
        );
    }
}
