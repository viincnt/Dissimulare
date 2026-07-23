use crate::app::{App, Screen};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Menu => draw_menu(frame, app),
        Screen::Dashboard => draw_dashboard(frame, app),
    }
}

fn draw_menu(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    let title = Paragraph::new("🎭 Dissimulare")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app
        .menu_items()
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let selected = i == app.menu_index;
            let name_style = if selected {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{name:<11}"), name_style),
                Span::raw(format!(" {desc}")),
            ]))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Menu"));
    frame.render_widget(list, chunks[1]);

    let footer_text = app
        .message
        .clone()
        .unwrap_or_else(|| "↑/↓ (or j/k) move · Enter select · q quit".to_string());
    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

fn draw_dashboard(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    let title = Paragraph::new("🎭 Dissimulare — proxy running")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let running = app.running.as_ref();
    let snapshot = running.map(|r| r.stats.snapshot());
    let elapsed = app.started_at.map(|t| t.elapsed()).unwrap_or_default();
    let secs = elapsed.as_secs();

    let lines = vec![
        Line::from(format!(
            "Listen address:   {}",
            running.map(|r| r.listen_addr.to_string()).unwrap_or_default()
        )),
        Line::from(format!(
            "Uptime:           {:02}:{:02}:{:02}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        )),
        Line::from(""),
        Line::from(format!(
            "Total requests:   {}",
            snapshot.map(|s| s.total_requests).unwrap_or(0)
        )),
        Line::from(format!(
            "Blocked requests: {}",
            snapshot.map(|s| s.blocked_requests).unwrap_or(0)
        )),
    ];
    let body = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Stats"));
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new("q / Esc / Ctrl-C  stop the proxy and return to the menu")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}
