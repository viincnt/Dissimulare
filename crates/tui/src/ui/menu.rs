use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};

use crate::app::App;
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = super::centered_area(frame.area());

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),   // [0] top spacer
            Constraint::Length(4), // [1] logo
            Constraint::Length(1), // [2] subtitle
            Constraint::Length(2), // [3] gap
            Constraint::Length(3), // [4] menu buttons
            Constraint::Length(1), // [5] description of selected item
            Constraint::Fill(1),   // [6] bottom spacer
            Constraint::Length(2), // [7] help bar
        ])
        .split(area);

    let big_text = BigText::builder()
        .pixel_size(PixelSize::HalfHeight)
        .centered()
        .lines([Line::from("Dissimulare").style(theme::bold_accent())])
        .build();
    frame.render_widget(big_text, outer[1]);

    frame.render_widget(
        Paragraph::new("Ad/Tracker Blocking \u{b7} Fingerprint Chaos")
            .alignment(Alignment::Center)
            .style(theme::muted()),
        outer[2],
    );

    let items = app.menu_items();
    super::draw_buttons(frame, outer[4], items, app.menu_index);

    if let Some((_, desc)) = items.get(app.menu_index) {
        frame.render_widget(
            Paragraph::new(*desc).alignment(Alignment::Center).style(theme::muted()),
            outer[5],
        );
    }

    super::draw_help(frame, outer[7], &[("\u{2190}\u{2192}/hl", "navigate"), ("\u{21b5}", "select"), ("q", "quit")]);
}
