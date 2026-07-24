mod app;
mod terminal;
mod theme;
mod ui;

use anyhow::Result;
use app::App;
use dissimulare_platform::AppPaths;
use std::sync::Mutex;

/// Logs go to a file, never stdout/stderr: those share the terminal with
/// ratatui's alternate screen, and any stray write would corrupt the UI.
fn init_tracing() -> Result<()> {
    let paths = AppPaths::new()?;
    paths.ensure_dirs()?;
    let log_path = paths.data_dir().join("tui.log");
    let file = std::fs::OpenOptions::new().create(true).append(true).open(log_path)?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .init();
    Ok(())
}

/// Restores the terminal on panic so a bug in the TUI doesn't leave the
/// user's shell stuck in raw mode / the alternate screen.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        default_hook(info);
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    // A logging setup failure (e.g. an unwritable data dir) shouldn't stop
    // the TUI from at least showing the menu.
    if let Err(err) = init_tracing() {
        eprintln!("warning: could not set up TUI logging: {err:#}");
    }
    install_panic_hook();

    let mut term = terminal::init()?;
    let result = App::new().run(&mut term).await;
    terminal::restore(&mut term)?;
    result
}
