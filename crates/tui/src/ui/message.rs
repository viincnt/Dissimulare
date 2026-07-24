use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, StatusLine, Tone};
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App, title: &str, lines: &[StatusLine], tone: &Tone) {
    let (content, help) = super::frame(frame, app, title);

    // `Plain` lines take the overall tone's color (an error screen's text
    // reads as an error; a success screen's plain facts read as neutral);
    // `Ok`/`Fail` always get their own checkmark/cross regardless of tone.
    let plain_style = match tone {
        Tone::Error => theme::err(),
        _ => theme::bright(),
    };

    let rendered: Vec<Line> = lines
        .iter()
        .map(|line| match line {
            StatusLine::Plain(text) => Line::from(Span::styled(text.clone(), plain_style)),
            StatusLine::Ok(text) => Line::from(vec![
                Span::styled("\u{2713} ", theme::bold_success()),
                Span::styled(text.clone(), theme::bright()),
            ]),
            StatusLine::Fail(text) => Line::from(vec![
                Span::styled("\u{2717} ", theme::bold_err()),
                Span::styled(text.clone(), theme::bright()),
            ]),
        })
        .collect();

    frame.render_widget(Paragraph::new(rendered), content);

    super::draw_help(frame, help, &[("\u{21b5} / Esc / Space", "back to menu")]);
}

pub fn draw_busy(frame: &mut Frame, app: &App, label: &str) {
    let (content, _help) = super::frame(frame, app, "Please wait");
    frame.render_widget(Paragraph::new(label).style(theme::bold_accent()), content);
}

pub fn draw_confirm_uninstall(frame: &mut Frame, app: &App, selected: usize) {
    let (content, help) = super::frame(frame, app, "Uninstall");

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(content);

    frame.render_widget(
        Paragraph::new("Remove the CA from the trust store and clear system proxy settings?").style(theme::bright()),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new("A fresh CA is generated the next time you run Setup.").style(theme::muted()),
        rows[1],
    );

    super::draw_choice(frame, rows[3], &["Yes", "No"], selected);

    super::draw_help(
        frame,
        help,
        &[("\u{2190}\u{2192}/hl", "choose"), ("\u{21b5}", "confirm"), ("Esc", "cancel")],
    );
}
