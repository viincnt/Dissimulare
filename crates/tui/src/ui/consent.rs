use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme;

const NOTICE: &[&str] = &[
    "Dissimulare needs to install a local Certificate Authority (CA) so it",
    "can inspect and rewrite your own HTTPS traffic, in order to block",
    "ads/trackers and normalize fingerprinting-related headers.",
    "",
    "This certificate is trusted ONLY for your current user account (never",
    "system-wide), and is used solely to decrypt/re-encrypt HTTPS",
    "connections made from this device. Remove it any time from Uninstall.",
];

pub fn draw(frame: &mut Frame, thumbprint: &str, input: &str, error: Option<&str>) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], "Setup");

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(NOTICE.len() as u16), // notice text
            Constraint::Length(1),                   // spacer
            Constraint::Length(1),                   // thumbprint
            Constraint::Length(1),                   // spacer
            Constraint::Length(3),                   // input box
            Constraint::Length(1),                   // error line
            Constraint::Min(0),
        ])
        .split(areas[1]);

    frame.render_widget(
        Paragraph::new(NOTICE.iter().map(|&l| Line::from(l)).collect::<Vec<_>>())
            .alignment(Alignment::Center)
            .style(theme::bright()),
        v[0],
    );

    frame.render_widget(
        Paragraph::new(format!("SHA-1 thumbprint: {thumbprint}"))
            .alignment(Alignment::Center)
            .style(theme::muted()),
        v[2],
    );

    let input_style = if error.is_some() { theme::err() } else { theme::accent() };
    frame.render_widget(
        Paragraph::new(input)
            .alignment(Alignment::Center)
            .style(theme::bold_white())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(input_style)
                    .title(" Type I AGREE to continue "),
            ),
        v[4],
    );

    if let Some(err) = error {
        frame.render_widget(Paragraph::new(err).alignment(Alignment::Center).style(theme::err()), v[5]);
    }

    super::draw_help(
        frame,
        areas[2],
        &[("type", "\"I AGREE\""), ("\u{21b5}", "confirm"), ("Esc", "cancel")],
    );
}
