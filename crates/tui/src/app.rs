use crate::terminal::Term;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use dissimulare_cli::commands::{self, RunningProxy};
use dissimulare_cli::config::DomainEntry;
use std::path::Path;
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
    ("Exceptions", "Sites that always get a normal identity in chaos mode"),
    ("Bypass", "Sites never intercepted at all (fixes TLS-fingerprint captcha loops)"),
    ("Uninstall", "Remove the CA and clear system proxy settings"),
    ("Quit", "Exit"),
];

/// Color/emphasis a [`Screen::Message`] renders with.
pub enum Tone {
    Info,
    Success,
    Error,
}

/// A single line in a [`Screen::Message`] body. `Ok`/`Fail` get a colored
/// checkmark/cross — for yes-or-no facts like "CA trusted" or "filter list
/// downloaded" — instead of reading as plain text indistinguishable from
/// everything else on the screen.
pub enum StatusLine {
    Plain(String),
    Ok(String),
    Fail(String),
}

/// The two user-editable domain lists share an identical shape (toggleable
/// entries + add/remove/import/export), just backed by different
/// `dissimulare_cli::commands` functions and config sections — this is the
/// single place that difference lives, so the `Screen`/UI/key-handling
/// code below is written once and works for both.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DomainListKind {
    ChaosExceptions,
    Bypass,
}

impl DomainListKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::ChaosExceptions => "Exceptions",
            Self::Bypass => "Bypass",
        }
    }

    pub fn empty_hint(self) -> &'static str {
        match self {
            Self::ChaosExceptions => "No exceptions configured. Press [a] to add one.",
            Self::Bypass => "No bypass domains configured. Press [a] to add one.",
        }
    }

    fn list(self) -> Result<Vec<DomainEntry>> {
        match self {
            Self::ChaosExceptions => commands::exceptions_list(),
            Self::Bypass => commands::bypass_list(),
        }
    }

    fn add(self, domain: &str) -> Result<()> {
        match self {
            Self::ChaosExceptions => commands::exceptions_add(domain),
            Self::Bypass => commands::bypass_add(domain),
        }
    }

    fn remove(self, domain: &str) -> Result<()> {
        match self {
            Self::ChaosExceptions => commands::exceptions_remove(domain),
            Self::Bypass => commands::bypass_remove(domain),
        }
    }

    fn set_enabled(self, domain: &str, enabled: bool) -> Result<bool> {
        match self {
            Self::ChaosExceptions => commands::exceptions_set_enabled(domain, enabled),
            Self::Bypass => commands::bypass_set_enabled(domain, enabled),
        }
    }

    fn import(self, path: &Path) -> Result<usize> {
        match self {
            Self::ChaosExceptions => commands::exceptions_import(path),
            Self::Bypass => commands::bypass_import(path),
        }
    }

    fn export(self, path: &Path) -> Result<usize> {
        match self {
            Self::ChaosExceptions => commands::exceptions_export(path),
            Self::Bypass => commands::bypass_export(path),
        }
    }
}

pub enum Screen {
    Menu,
    /// First-run consent: the user must type `I AGREE` before the CA is
    /// installed. Mirrors the CLI's typed-confirmation requirement on
    /// purpose — this is the one action that touches the system trust
    /// store.
    Consent { thumbprint: String, input: String, error: Option<String> },
    /// A transient "please wait" screen shown while an async command runs;
    /// never actually waits for input itself (see [`App::run`]).
    Busy(String),
    /// Generic result/info screen: title + body lines + a tone, used for
    /// Setup/Status/Uninstall results and errors alike.
    Message { title: String, lines: Vec<StatusLine>, tone: Tone },
    /// `selected`: 0 = Yes, 1 = No. Defaults to No.
    ConfirmUninstall { selected: usize },
    /// Toggleable domain-list checklist — chaos exceptions or the bypass
    /// list, see [`DomainListKind`]: toggle entries on/off, or jump into
    /// `a`/`d`/`i`/`e` for add/remove/import/export.
    DomainList { kind: DomainListKind, entries: Vec<DomainEntry>, selected: usize, status: Option<String> },
    DomainListAdd { kind: DomainListKind, input: String },
    DomainListImportPath { kind: DomainListKind, input: String },
    DomainListExportPath { kind: DomainListKind, input: String },
    Dashboard,
}

pub struct App {
    pub screen: Screen,
    pub menu_index: usize,
    pub running: Option<RunningProxy>,
    pub started_at: Option<Instant>,
    ca_installed: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Menu,
            menu_index: 0,
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

    /// Whether the CA is currently trusted, for the persistent status bar.
    pub fn ca_installed(&self) -> bool {
        self.ca_installed
    }

    fn refresh_ca_state(&mut self) {
        self.ca_installed = commands::ca_installed().unwrap_or(self.ca_installed);
    }

    /// Lands back on the menu, with the cursor clamped to a valid item in
    /// case the menu just changed size (e.g. Setup finished and dropped
    /// out of the list).
    fn back_to_menu(&mut self) {
        self.menu_index = self.menu_index.min(self.menu_items().len() - 1);
        self.screen = Screen::Menu;
    }

    /// Re-reads `kind`'s domain list from disk and shows it, keeping the
    /// cursor as close as possible to `keep_selected` (clamped, since an
    /// add/remove may have changed how many entries there are), with an
    /// optional status line (e.g. "Imported 3 domain(s)."). Used both when
    /// first entering the screen and after every action that changes it,
    /// so what's on screen never drifts from what's actually saved.
    fn reload_domain_list(&mut self, kind: DomainListKind, keep_selected: usize, status: Option<String>) {
        match kind.list() {
            Ok(entries) => {
                let selected = if entries.is_empty() { 0 } else { keep_selected.min(entries.len() - 1) };
                self.screen = Screen::DomainList { kind, entries, selected, status };
            }
            Err(err) => self.show_error(kind.title(), err),
        }
    }

    fn show_error(&mut self, title: &str, err: anyhow::Error) {
        self.screen = Screen::Message {
            title: title.to_string(),
            lines: vec![StatusLine::Plain(format!("{err:#}"))],
            tone: Tone::Error,
        };
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

            let keep_going = self.handle_key(key.code, key.modifiers, term).await?;
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

    async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, term: &mut Term) -> Result<bool> {
        // `Screen` owns growable state (Strings/Vecs), so it can't be
        // `Copy`; swap in a placeholder to move the real value out and
        // match on it, instead of juggling borrows of `self` in every arm.
        match std::mem::replace(&mut self.screen, Screen::Menu) {
            Screen::Menu => self.handle_menu_key(code, term).await,
            Screen::Consent { thumbprint, input, error } => {
                self.handle_consent_key(code, thumbprint, input, error, term).await
            }
            Screen::Busy(label) => {
                self.screen = Screen::Busy(label);
                Ok(true)
            }
            Screen::Message { title, lines, tone } => {
                self.handle_message_key(code, title, lines, tone);
                Ok(true)
            }
            Screen::ConfirmUninstall { selected } => self.handle_confirm_uninstall_key(code, selected).await,
            Screen::DomainList { kind, entries, selected, status } => {
                self.handle_domain_list_key(code, kind, entries, selected, status)
            }
            Screen::DomainListAdd { kind, input } => {
                self.handle_domain_list_add_key(code, kind, input);
                Ok(true)
            }
            Screen::DomainListImportPath { kind, input } => {
                self.handle_domain_list_import_key(code, kind, input);
                Ok(true)
            }
            Screen::DomainListExportPath { kind, input } => {
                self.handle_domain_list_export_key(code, kind, input);
                Ok(true)
            }
            Screen::Dashboard => {
                self.screen = Screen::Dashboard;
                self.handle_dashboard_key(code, modifiers).await;
                Ok(true)
            }
        }
    }

    async fn handle_menu_key(&mut self, code: KeyCode, term: &mut Term) -> Result<bool> {
        self.screen = Screen::Menu;
        let item_count = self.menu_items().len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_index = if self.menu_index == 0 { item_count - 1 } else { self.menu_index - 1 };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu_index = (self.menu_index + 1) % item_count;
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

    async fn activate(&mut self, selected: &str, term: &mut Term) -> Result<()> {
        match selected {
            "Setup" => match commands::consent_state() {
                Ok(state) if state.needed => {
                    self.screen = Screen::Consent {
                        thumbprint: state.thumbprint,
                        input: String::new(),
                        error: None,
                    };
                }
                // Already trusted (stale menu selection) — nothing to
                // consent to, just run the rest of setup.
                Ok(_) => self.run_setup(term).await?,
                Err(err) => self.show_error("Setup", err),
            },
            "Status" => match commands::status_info() {
                Ok(info) => {
                    let mut lines = vec![
                        StatusLine::Plain(format!("CA thumbprint (SHA-1): {}", info.ca_thumbprint)),
                        if info.ca_trusted {
                            StatusLine::Ok("CA trusted".to_string())
                        } else {
                            StatusLine::Fail("CA trusted".to_string())
                        },
                        StatusLine::Plain(format!("Listen address: {}", info.listen_addr)),
                        StatusLine::Plain(format!(
                            "System proxy on start: {}",
                            if info.system_proxy_on_start { "yes" } else { "no" }
                        )),
                    ];
                    for (name, downloaded) in &info.filters {
                        lines.push(if *downloaded {
                            StatusLine::Ok(format!("Filter list {name}"))
                        } else {
                            StatusLine::Fail(format!("Filter list {name}"))
                        });
                    }
                    self.screen = Screen::Message { title: "Status".to_string(), lines, tone: Tone::Info };
                }
                Err(err) => self.show_error("Status", err),
            },
            "Exceptions" => self.reload_domain_list(DomainListKind::ChaosExceptions, 0, None),
            "Bypass" => self.reload_domain_list(DomainListKind::Bypass, 0, None),
            "Uninstall" => {
                self.screen = Screen::ConfirmUninstall { selected: 1 };
            }
            "Run" => {
                self.screen = Screen::Busy("Starting proxy…".to_string());
                term.draw(|frame| crate::ui::draw(frame, self))?;
                match commands::start(None).await {
                    Ok(running) => {
                        self.running = Some(running);
                        self.started_at = Some(Instant::now());
                        self.screen = Screen::Dashboard;
                    }
                    Err(err) => self.show_error("Run", err),
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn run_setup(&mut self, term: &mut Term) -> Result<()> {
        self.screen = Screen::Busy("Installing CA and downloading filter lists…".to_string());
        term.draw(|frame| crate::ui::draw(frame, self))?;

        match commands::complete_setup().await {
            Ok(outcome) => {
                let mut lines = vec![StatusLine::Plain(format!("CA thumbprint (SHA-1): {}", outcome.ca_thumbprint))];
                for (name, downloaded) in &outcome.filters {
                    lines.push(if *downloaded {
                        StatusLine::Ok(format!("Filter list {name}"))
                    } else {
                        StatusLine::Fail(format!("Filter list {name}"))
                    });
                }
                lines.push(StatusLine::Plain("Setup complete.".to_string()));
                self.screen = Screen::Message { title: "Setup".to_string(), lines, tone: Tone::Success };
            }
            Err(err) => self.show_error("Setup", err),
        }
        self.refresh_ca_state();
        Ok(())
    }

    async fn handle_consent_key(
        &mut self,
        code: KeyCode,
        thumbprint: String,
        mut input: String,
        mut error: Option<String>,
        term: &mut Term,
    ) -> Result<bool> {
        match code {
            KeyCode::Char(c) => {
                input.push(c);
                error = None;
                self.screen = Screen::Consent { thumbprint, input, error };
            }
            KeyCode::Backspace => {
                input.pop();
                self.screen = Screen::Consent { thumbprint, input, error };
            }
            KeyCode::Enter => {
                if input.trim() == "I AGREE" {
                    self.run_setup(term).await?;
                } else {
                    self.screen = Screen::Consent {
                        thumbprint,
                        input: String::new(),
                        error: Some("Type exactly \"I AGREE\" to continue.".to_string()),
                    };
                }
            }
            KeyCode::Esc => self.back_to_menu(),
            _ => {
                self.screen = Screen::Consent { thumbprint, input, error };
            }
        }
        Ok(true)
    }

    fn handle_message_key(&mut self, code: KeyCode, title: String, lines: Vec<StatusLine>, tone: Tone) {
        match code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => self.back_to_menu(),
            _ => self.screen = Screen::Message { title, lines, tone },
        }
    }

    async fn handle_confirm_uninstall_key(&mut self, code: KeyCode, selected: usize) -> Result<bool> {
        match code {
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                self.screen = Screen::ConfirmUninstall { selected: if selected == 0 { 1 } else { 0 } };
            }
            KeyCode::Enter => {
                if selected == 0 {
                    match commands::perform_uninstall() {
                        Ok(outcome) => {
                            let mut lines = vec![if outcome.ca_was_installed {
                                StatusLine::Ok("Root CA removed from the trust store".to_string())
                            } else {
                                StatusLine::Plain("Root CA was not installed.".to_string())
                            }];
                            if outcome.system_proxy_cleared {
                                lines.push(StatusLine::Ok("System proxy settings cleared".to_string()));
                            }
                            lines.push(StatusLine::Plain(
                                "Local CA material removed. A fresh CA will be generated on the next setup."
                                    .to_string(),
                            ));
                            self.screen =
                                Screen::Message { title: "Uninstall".to_string(), lines, tone: Tone::Success };
                        }
                        Err(err) => self.show_error("Uninstall", err),
                    }
                    self.refresh_ca_state();
                } else {
                    self.back_to_menu();
                }
            }
            KeyCode::Esc => self.back_to_menu(),
            _ => self.screen = Screen::ConfirmUninstall { selected },
        }
        Ok(true)
    }

    fn handle_domain_list_key(
        &mut self,
        code: KeyCode,
        kind: DomainListKind,
        entries: Vec<DomainEntry>,
        selected: usize,
        status: Option<String>,
    ) -> Result<bool> {
        let item_count = entries.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') if item_count > 0 => {
                let selected = if selected == 0 { item_count - 1 } else { selected - 1 };
                self.screen = Screen::DomainList { kind, entries, selected, status: None };
            }
            KeyCode::Down | KeyCode::Char('j') if item_count > 0 => {
                let selected = (selected + 1) % item_count;
                self.screen = Screen::DomainList { kind, entries, selected, status: None };
            }
            KeyCode::Enter | KeyCode::Char(' ') if item_count > 0 => {
                let domain = entries[selected].domain.clone();
                let now_enabled = !entries[selected].enabled;
                match kind.set_enabled(&domain, now_enabled) {
                    Ok(_) => self.reload_domain_list(kind, selected, None),
                    Err(err) => self.show_error(kind.title(), err),
                }
            }
            KeyCode::Char('d') if item_count > 0 => {
                let domain = entries[selected].domain.clone();
                match kind.remove(&domain) {
                    Ok(()) => self.reload_domain_list(kind, selected, Some(format!("Removed {domain}."))),
                    Err(err) => self.show_error(kind.title(), err),
                }
            }
            KeyCode::Char('a') => self.screen = Screen::DomainListAdd { kind, input: String::new() },
            KeyCode::Char('i') => self.screen = Screen::DomainListImportPath { kind, input: String::new() },
            KeyCode::Char('e') => self.screen = Screen::DomainListExportPath { kind, input: String::new() },
            KeyCode::Esc | KeyCode::Char('q') => self.back_to_menu(),
            _ => self.screen = Screen::DomainList { kind, entries, selected, status },
        }
        Ok(true)
    }

    fn handle_domain_list_add_key(&mut self, code: KeyCode, kind: DomainListKind, mut input: String) {
        match code {
            KeyCode::Char(c) => {
                input.push(c);
                self.screen = Screen::DomainListAdd { kind, input };
            }
            KeyCode::Backspace => {
                input.pop();
                self.screen = Screen::DomainListAdd { kind, input };
            }
            KeyCode::Enter if !input.trim().is_empty() => match kind.add(&input) {
                Ok(()) => {
                    let domain = commands::normalize_domain(&input);
                    self.reload_domain_list(kind, 0, Some(format!("Added {domain}.")));
                }
                Err(err) => self.show_error(kind.title(), err),
            },
            KeyCode::Esc => self.reload_domain_list(kind, 0, None),
            _ => self.screen = Screen::DomainListAdd { kind, input },
        }
    }

    fn handle_domain_list_import_key(&mut self, code: KeyCode, kind: DomainListKind, mut input: String) {
        match code {
            KeyCode::Char(c) => {
                input.push(c);
                self.screen = Screen::DomainListImportPath { kind, input };
            }
            KeyCode::Backspace => {
                input.pop();
                self.screen = Screen::DomainListImportPath { kind, input };
            }
            KeyCode::Enter if !input.trim().is_empty() => match kind.import(Path::new(input.trim())) {
                Ok(count) => self.reload_domain_list(kind, 0, Some(format!("Imported {count} domain(s)."))),
                Err(err) => self.show_error(kind.title(), err),
            },
            KeyCode::Esc => self.reload_domain_list(kind, 0, None),
            _ => self.screen = Screen::DomainListImportPath { kind, input },
        }
    }

    fn handle_domain_list_export_key(&mut self, code: KeyCode, kind: DomainListKind, mut input: String) {
        match code {
            KeyCode::Char(c) => {
                input.push(c);
                self.screen = Screen::DomainListExportPath { kind, input };
            }
            KeyCode::Backspace => {
                input.pop();
                self.screen = Screen::DomainListExportPath { kind, input };
            }
            KeyCode::Enter if !input.trim().is_empty() => match kind.export(Path::new(input.trim())) {
                Ok(count) => self.reload_domain_list(kind, 0, Some(format!("Exported {count} domain(s)."))),
                Err(err) => self.show_error(kind.title(), err),
            },
            KeyCode::Esc => self.reload_domain_list(kind, 0, None),
            _ => self.screen = Screen::DomainListExportPath { kind, input },
        }
    }

    async fn handle_dashboard_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let is_ctrl_c = modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c');
        if code != KeyCode::Char('q') && code != KeyCode::Esc && !is_ctrl_c {
            return;
        }
        if let Some(running) = self.running.take() {
            let snapshot = running.stop().await;
            self.screen = Screen::Message {
                title: "Proxy stopped".to_string(),
                lines: vec![
                    StatusLine::Plain(format!("Total requests: {}", snapshot.total_requests)),
                    StatusLine::Plain(format!("Blocked requests: {}", snapshot.blocked_requests)),
                ],
                tone: Tone::Success,
            };
        } else {
            self.screen = Screen::Menu;
        }
        self.started_at = None;
    }
}
