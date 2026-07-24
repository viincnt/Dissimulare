mod consent;
mod dashboard;
mod domain_list;
mod menu;
mod message;

use crate::app::{App, Screen};
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Menu => menu::draw(frame, app),
        Screen::Consent { thumbprint, input, error } => {
            consent::draw(frame, app, thumbprint, input, error.as_deref())
        }
        Screen::Busy(label) => message::draw_busy(frame, app, label),
        Screen::Message { title, lines, tone } => message::draw(frame, app, title, lines, tone),
        Screen::ConfirmUninstall { selected } => message::draw_confirm_uninstall(frame, app, *selected),
        Screen::DomainList { kind, entries, selected, status } => {
            domain_list::draw(frame, app, *kind, entries, *selected, status.as_deref())
        }
        Screen::DomainListAdd { kind, input } => domain_list::draw_text_input(
            frame,
            app,
            &format!("{} \u{203a} Add", kind.title()),
            "Enter a domain (e.g. example.com):",
            input,
            &[("type", "domain"), ("\u{21b5}", "add"), ("Esc", "cancel")],
        ),
        Screen::DomainListImportPath { kind, input } => domain_list::draw_text_input(
            frame,
            app,
            &format!("{} \u{203a} Import", kind.title()),
            "Path to a plain-text domain list (one per line):",
            input,
            &[("type", "path"), ("\u{21b5}", "import"), ("Esc", "cancel")],
        ),
        Screen::DomainListExportPath { kind, input } => domain_list::draw_text_input(
            frame,
            app,
            &format!("{} \u{203a} Export", kind.title()),
            "Path to write the enabled domains to:",
            input,
            &[("type", "path"), ("\u{21b5}", "export"), ("Esc", "cancel")],
        ),
        Screen::Dashboard => dashboard::draw(frame, app),
    }
}

/// Builds the single panel every screen renders into: one bordered frame
/// spanning the terminal (mark + "Dissimulare" on the title's left, the
/// current screen name on its right), a compact status row, a thin rule,
/// the screen's own content, another thin rule, and a help row — instead
/// of each screen owning its own nested boxes. Returns the (content, help)
/// rects for the caller to fill in.
pub fn frame(f: &mut Frame, app: &App, screen_name: &str) -> (Rect, Rect) {
    let area = f.area();
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent_dim())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("\u{25c6}", theme::bold_accent()),
            Span::raw(" "),
            Span::styled("Dissimulare", theme::bold_white()),
            Span::raw(" "),
        ]))
        .title(Line::from(format!(" {screen_name} ")).alignment(Alignment::Right));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status row
            Constraint::Length(1), // rule
            Constraint::Min(1),    // screen content
            Constraint::Length(1), // rule
            Constraint::Length(1), // help row
        ])
        .split(inner);

    draw_status_row(f, rows[0], app);
    draw_rule(f, rows[1]);
    draw_rule(f, rows[3]);

    (rows[2], rows[4])
}

fn draw_status_row(f: &mut Frame, area: Rect, app: &App) {
    let running = app.running.is_some();
    let proxy_style = if running { theme::bold_success() } else { theme::muted() };
    let proxy_text = if running { "\u{25cf} proxy up" } else { "\u{25cb} proxy down" };
    let ca_style = if app.ca_installed() { theme::success() } else { theme::muted() };
    let ca_text = if app.ca_installed() { "CA trusted" } else { "CA untrusted" };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(ca_text, ca_style),
        Span::styled("   ", theme::muted()),
        Span::styled(proxy_text, proxy_style),
    ];
    if let Some(running) = app.running.as_ref() {
        spans.push(Span::styled("   ", theme::muted()));
        spans.push(Span::styled(running.listen_addr.to_string(), theme::muted()));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn draw_rule(f: &mut Frame, area: Rect) {
    let rule = "\u{2500}".repeat(area.width as usize);
    f.render_widget(Paragraph::new(Span::styled(rule, theme::subtle())), area);
}

pub fn draw_help(f: &mut Frame, area: Rect, bindings: &[(&str, &str)]) {
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (key, desc)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   \u{b7}   ", theme::muted()));
        }
        spans.push(Span::styled(*key, theme::accent()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, theme::muted()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), area);
}

/// A compact, unboxed `[ Option ]  [ Option ]` choice row — used for the
/// Yes/No uninstall confirmation. Deliberately plain text rather than
/// bordered buttons: the outer panel is already the frame everything
/// lives in, so a nested box here would just be visual noise.
pub fn draw_choice(f: &mut Frame, area: Rect, options: &[&str], selected: usize) {
    let mut spans = Vec::new();
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("    "));
        }
        let style = if i == selected { theme::bold_accent() } else { theme::muted() };
        spans.push(Span::styled(format!("[ {option} ]"), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center), area);
}
