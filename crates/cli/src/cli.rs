use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    /// Manage the chaos-mode domain exception list — sites that always get
    /// a normal browser identity instead of an absurd one (useful for
    /// sites whose own User-Agent sniffing breaks otherwise).
    Exceptions {
        #[command(subcommand)]
        action: ExceptionsAction,
    },
}

#[derive(Subcommand)]
pub enum ExceptionsAction {
    /// List every entry and whether it's currently enabled.
    List,
    /// Add a domain to the list (enabled by default; re-enables it if it
    /// was already present but disabled).
    Add { domain: String },
    /// Remove a domain from the list entirely.
    Remove { domain: String },
    /// Enable an existing entry without removing it.
    Enable { domain: String },
    /// Disable an existing entry without removing it.
    Disable { domain: String },
    /// Merge domains from a plain-text file (one per line, blank lines and
    /// `#` comments ignored) into the list, enabled.
    Import { path: PathBuf },
    /// Write every currently-enabled domain to a plain-text file, one per
    /// line, so the list can be shared or reused elsewhere.
    Export { path: PathBuf },
}
