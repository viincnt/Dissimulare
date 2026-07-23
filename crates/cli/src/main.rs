mod cli;
mod commands;
mod config;
mod seed;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Setup => commands::setup().await,
        Command::Run { set_system_proxy } => commands::run(set_system_proxy).await,
        Command::Status => commands::status(),
        Command::Uninstall => commands::uninstall(),
    }
}
