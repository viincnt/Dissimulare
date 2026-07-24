use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::Tone;
use crate::theme;

pub fn draw(frame: &mut Frame, title: &str, lines: &[String], tone: &Tone) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], title);

    let title_style = match tone {
        Tone::Info => theme::bright(),
        Tone::Success => theme::bold_success(),
        Tone::Error => theme::bold_err(),
    };

    let body: Vec<Line> = lines.iter().map(|l| Line::from(l.as_str())).collect();
    let body_height = body.len() as u16;

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(body_height), Constraint::Fill(1)])
        .split(areas[1]);

    frame.render_widget(Paragraph::new(body).alignment(Alignment::Center).style(title_style), v[1]);

    super::draw_help(frame, areas[2], &[("\u{21b5} / Esc / Space", "back to menu")]);
}

pub fn draw_busy(frame: &mut Frame, label: &str) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], "Please wait");

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1)])
        .split(areas[1]);

    frame.render_widget(Paragraph::new(label).alignment(Alignment::Center).style(theme::bold_accent()), v[1]);
}

pub fn draw_confirm_uninstall(frame: &mut Frame, selected: usize) {
    let areas = super::outer_layout(frame.area());
    super::draw_header(frame, areas[0], "Uninstall");

    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(areas[1]);

    frame.render_widget(
        Paragraph::new("Remove the CA from the trust store and clear system proxy settings?")
            .alignment(Alignment::Center)
            .style(theme::bright()),
        v[1],
    );
    frame.render_widget(
        Paragraph::new("A fresh CA is generated the next time you run Setup.")
            .alignment(Alignment::Center)
            .style(theme::muted()),
        v[2],
    );

    super::draw_buttons(frame, v[4], &[("Yes", ""), ("No", "")], selected);

    super::draw_help(
        frame,
        areas[2],
        &[("\u{2190}\u{2192}/hl", "choose"), ("\u{21b5}", "confirm"), ("Esc", "cancel")],
    );
}
