use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dissimulare", version, about = "A privacy-first local MITM proxy: strict ad/tracker blocking and fingerprint hardening.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// First-run flow: generate the local root CA, ask for explicit consent,
    /// install it for the current user, and download the default filter lists.
    Setup,
    /// Start the proxy. Requires `setup` to have been run at least once.
    Run {
        /// Override the config's `set_system_proxy` setting for this run.
        #[arg(long)]
        set_system_proxy: Option<bool>,
    },
    /// Show whether the CA is trusted and which filter lists are cached.
    Status,
    /// Remove the root CA from the trust store and clear system proxy settings.
    Uninstall,
}
