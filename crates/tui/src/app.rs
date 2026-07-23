use crate::terminal::{resume, suspend, Term};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use dissimulare_cli::commands::{self, RunningProxy};
use std::time::{Duration, Instant};

/// Menu shown before the CA is trusted: `setup` is the only thing that
/// makes sense to offer, since `Run` would just refuse to start and
/// `Status`/`Uninstall` have nothing to act on yet.
const MENU_ITEMS_NO_CA: &[(&str, &str)] = &[
    ("Setup", "Generate/trust the local CA and download filter lists"),
    ("Quit", "Exit"),
];

/// Menu shown once the CA is trusted: `Setup` drops out (it's already
/// done) and the rest of the day-to-day commands take over.
const MENU_ITEMS_WITH_CA: &[(&str, &str)] = &[
    ("Run", "Start the proxy and watch live stats"),
    ("Status", "Show CA trust state and cached filter lists"),
    ("Uninstall", "Remove the CA and clear system proxy settings"),
    ("Quit", "Exit"),
];

pub enum Screen {
    Menu,
    Dashboard,
}

pub struct App {
    pub screen: Screen,
    pub menu_index: usize,
    pub message: Option<String>,
    pub running: Option<RunningProxy>,
    pub started_at: Option<Instant>,
    ca_installed: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Menu,
            menu_index: 0,
            message: None,
            running: None,
            started_at: None,
            // A failed check is treated the same as "not installed": the
            // worst case is Setup runs again, which is harmless and
            // idempotent, rather than hiding it when it might be needed.
            ca_installed: commands::ca_installed().unwrap_or(false),
        }
    }

    pub fn menu_items(&self) -> &'static [(&'static str, &'static str)] {
        if self.ca_installed {
            MENU_ITEMS_WITH_CA
        } else {
            MENU_ITEMS_NO_CA
        }
    }

    /// Re-checks whether the CA is trusted and, if that flips which menu
    /// applies, snaps the selection back to the top so it can't end up
    /// pointing past the end of a shorter list.
    fn refresh_ca_state(&mut self) {
        let now_installed = commands::ca_installed().unwrap_or(self.ca_installed);
        if now_installed != self.ca_installed {
            self.ca_installed = now_installed;
            self.menu_index = 0;
        }
    }

    /// Drives the whole TUI: render, wait for an event (or a ~250ms tick so
    /// the dashboard's live stats keep advancing), handle it, repeat.
    pub async fn run(mut self, term: &mut Term) -> Result<()> {
        loop {
            term.draw(|frame| crate::ui::draw(frame, &self))?;

            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let keep_going = match self.screen {
                Screen::Menu => self.handle_menu_key(key.code, term).await?,
                Screen::Dashboard => {
                    self.handle_dashboard_key(key.code, key.modifiers).await;
                    true
                }
            };
            if !keep_going {
                // Leaving the dashboard running unattended would leak the
                // proxy task and, worse, leave the system proxy pointed at
                // a process that's about to exit.
                if let Some(running) = self.running.take() {
                    running.stop().await;
                }
                return Ok(());
            }
        }
    }

    async fn handle_menu_key(&mut self, code: KeyCode, term: &mut Term) -> Result<bool> {
        let item_count = self.menu_items().len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_index = if self.menu_index == 0 { item_count - 1 } else { self.menu_index - 1 };
                self.message = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu_index = (self.menu_index + 1) % item_count;
                self.message = None;
            }
            KeyCode::Enter => {
                let selected = self.menu_items()[self.menu_index].0;
                if selected == "Quit" {
                    return Ok(false);
                }
                self.activate(selected, term).await?;
            }
            KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
            _ => {}
        }
        Ok(true)
    }

    async fn handle_dashboard_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let is_ctrl_c = modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c');
        if code != KeyCode::Char('q') && code != KeyCode::Esc && !is_ctrl_c {
            return;
        }
        if let Some(running) = self.running.take() {
            let snapshot = running.stop().await;
            self.message = Some(format!(
                "Proxy stopped — {} requests seen, {} blocked.",
                snapshot.total_requests, snapshot.blocked_requests
            ));
        }
        self.started_at = None;
        self.screen = Screen::Menu;
    }

    /// Runs whichever `dissimulare-cli` command the menu selection maps to.
    /// `Setup`/`Status`/`Uninstall` are plain, synchronous-feeling commands
    /// that print straight to the terminal, so this drops out of TUI mode
    /// for them; `Run` hands off to the interactive dashboard instead.
    async fn activate(&mut self, selected: &str, term: &mut Term) -> Result<()> {
        match selected {
            "Setup" => {
                suspend(term)?;
                let result = commands::setup().await;
                let message = finish_plain_command("Setup", result);
                resume(term)?;
                self.message = Some(message);
                self.refresh_ca_state();
            }
            "Status" => {
                suspend(term)?;
                let result = commands::status();
                let message = finish_plain_command("Status", result);
                resume(term)?;
                self.message = Some(message);
            }
            "Uninstall" => {
                suspend(term)?;
                let result = commands::uninstall();
                let message = finish_plain_command("Uninstall", result);
                resume(term)?;
                self.message = Some(message);
                self.refresh_ca_state();
            }
            "Run" => {
                self.message = Some("Starting proxy…".to_string());
                term.draw(|frame| crate::ui::draw(frame, self))?;
                match commands::start(None).await {
                    Ok(running) => {
                        self.running = Some(running);
                        self.started_at = Some(Instant::now());
                        self.message = None;
                        self.screen = Screen::Dashboard;
                    }
                    Err(err) => self.message = Some(format!("Failed to start: {err:#}")),
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn finish_plain_command(action: &str, result: Result<()>) -> String {
    if let Err(err) = &result {
        println!("\nError: {err:#}");
    }
    println!("\nPress Enter to return to the menu...");
    let _ = std::io::stdin().read_line(&mut String::new());

    match result {
        Ok(()) => format!("{action} completed."),
        Err(err) => format!("{action} failed: {err:#}"),
    }
}
