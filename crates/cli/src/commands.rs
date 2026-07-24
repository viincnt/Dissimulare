use crate::cli::ExceptionsAction;
use crate::config::{AppConfig, ChaosException};
use crate::seed::load_or_generate_chaos_seed;
use anyhow::{bail, Context, Result};
use dissimulare_ca::RootCa;
use dissimulare_core::{DissimulareHandler, Stats, StatsSnapshot};
use dissimulare_filters::{FilterManager, FilterService};
use dissimulare_platform::{system_proxy, AppPaths};
use hudsucker::rustls::crypto::aws_lc_rs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

/// Whether the local root CA is currently trusted by the OS. Front-ends
/// (e.g. the TUI) use this to decide whether `setup` still needs to run,
/// instead of duplicating any CA logic themselves.
pub fn ca_installed() -> Result<bool> {
    let paths = AppPaths::new()?;
    let root_ca = RootCa::load_or_generate(&paths)?;
    root_ca.is_installed()
}

/// What a consent screen needs before installing the CA: the thumbprint to
/// display, and whether consent is actually needed (`false` if the CA is
/// already trusted, e.g. a stale menu selection).
pub struct ConsentState {
    pub thumbprint: String,
    pub needed: bool,
}

pub fn consent_state() -> Result<ConsentState> {
    let paths = AppPaths::new()?;
    paths.ensure_dirs()?;
    let root_ca = RootCa::load_or_generate(&paths)?;
    let needed = !root_ca.is_installed()?;
    Ok(ConsentState { thumbprint: root_ca.sha1_thumbprint().to_string(), needed })
}

/// Result of a completed [`complete_setup`] run: the CA's thumbprint and,
/// per configured filter list, whether it ended up downloaded.
pub struct SetupOutcome {
    pub ca_thumbprint: String,
    pub filters: Vec<(String, bool)>,
}

/// Installs the CA (assuming the caller already obtained consent — see
/// [`consent_state`]) and syncs the configured filter lists. Does no I/O
/// beyond that: no printing, no stdin reads, so any front-end can drive it.
pub async fn complete_setup() -> Result<SetupOutcome> {
    let paths = AppPaths::new()?;
    paths.ensure_dirs()?;

    let root_ca = RootCa::load_or_generate(&paths)?;
    if !root_ca.is_installed()? {
        root_ca.install()?;
    }

    let config = AppConfig::load_or_default(&paths)?;
    let sources = config.filters.sources();
    let filter_manager = FilterManager::new(paths.clone(), config.filters.refresh_interval())?;
    filter_manager.sync_lists(&sources).await?;
    let engine_source = filter_manager.engine_source(&sources)?;
    let filter_service = FilterService::spawn(engine_source);
    // Force the engine to finish building (and its cache to be written)
    // before reporting success, instead of leaving it to happen lazily.
    filter_service
        .check(
            "https://example.com/".to_string(),
            String::new(),
            "document",
            "GET".to_string(),
        )
        .await;

    config.save(&paths)?;

    let filters = sources.iter().map(|s| (s.name.to_string(), true)).collect();
    Ok(SetupOutcome { ca_thumbprint: root_ca.sha1_thumbprint().to_string(), filters })
}

pub async fn setup() -> Result<()> {
    let consent = consent_state()?;
    if consent.needed {
        print_consent_notice(&consent.thumbprint);
        if !prompt_confirmation()? {
            println!("Setup cancelled — no certificate was installed.");
            return Ok(());
        }
    } else {
        println!("Root CA already trusted (SHA-1 {}).", consent.thumbprint);
    }

    println!("Downloading filter lists...");
    let outcome = complete_setup().await?;
    for (name, downloaded) in &outcome.filters {
        println!("Filter list {name}: {}", if *downloaded { "downloaded" } else { "missing" });
    }
    println!("Setup complete. Run `dissimulare run` to start the proxy.");
    Ok(())
}

/// A proxy started via [`start`], running in the background until [`stop`]
/// is called. Splitting this out of [`run`] lets a caller drive its own
/// event loop (e.g. a TUI dashboard polling `stats`) instead of being stuck
/// waiting on Ctrl-C inside this function.
pub struct RunningProxy {
    pub stats: Stats,
    pub listen_addr: SocketAddr,
    set_system_proxy: bool,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    proxy_task: tokio::task::JoinHandle<()>,
}

impl RunningProxy {
    /// Signals the proxy to shut down, waits for it to finish, and reverts
    /// the system proxy settings if this run enabled them. Returns the
    /// final stats snapshot.
    pub async fn stop(self) -> StatsSnapshot {
        let _ = self.shutdown_tx.send(());
        let _ = self.proxy_task.await;

        if self.set_system_proxy {
            if let Err(err) = system_proxy().disable() {
                tracing::warn!(error = %err, "failed to disable the system proxy on shutdown");
            } else {
                tracing::info!("system proxy disabled");
            }
        }

        self.stats.snapshot()
    }
}

/// Builds and starts the proxy (installing the system proxy if configured),
/// returning immediately with a handle instead of waiting for a shutdown
/// signal — callers decide how they want to be told to stop.
pub async fn start(set_system_proxy_override: Option<bool>) -> Result<RunningProxy> {
    let paths = AppPaths::new()?;
    paths.ensure_dirs()?;
    let config = AppConfig::load_or_default(&paths)?;

    let root_ca = RootCa::load_or_generate(&paths)?;
    if !root_ca.is_installed()? {
        bail!("the Dissimulare root CA isn't trusted yet — run `dissimulare setup` first");
    }

    let sources = config.filters.sources();
    let filter_manager = FilterManager::new(paths.clone(), config.filters.refresh_interval())?;
    if let Err(err) = filter_manager.sync_lists(&sources).await {
        tracing::warn!(error = %err, "could not refresh filter lists, falling back to cached copies");
    }
    let engine_source = filter_manager.engine_source(&sources)?;
    let filter_service = FilterService::spawn(engine_source);

    let stats = Stats::new();
    let chaos_seed = load_or_generate_chaos_seed(&paths)?;
    let policy = config.fingerprint.to_policy(chaos_seed);
    let handler =
        DissimulareHandler::new(filter_service, policy, stats.clone(), config.youtube.strip_ad_metadata);

    let authority = root_ca.to_authority()?;
    let listen_addr = config.proxy.listen_addr;
    let set_system_proxy = set_system_proxy_override.unwrap_or(config.proxy.set_system_proxy);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let proxy = hudsucker::Proxy::builder()
        .with_addr(listen_addr)
        .with_ca(authority)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(handler)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .build()
        .context("building proxy")?;

    if set_system_proxy {
        system_proxy()
            .enable(listen_addr)
            .context("enabling the system proxy")?;
        tracing::info!(%listen_addr, "system proxy enabled");
    }

    tracing::info!(%listen_addr, "Dissimulare proxy listening");
    let proxy_task = tokio::spawn(async move {
        let _ = proxy.start().await;
    });

    Ok(RunningProxy {
        stats,
        listen_addr,
        set_system_proxy,
        shutdown_tx,
        proxy_task,
    })
}

pub async fn run(set_system_proxy_override: Option<bool>) -> Result<()> {
    let running = start(set_system_proxy_override).await?;

    tokio::signal::ctrl_c().await.context("waiting for ctrl-c")?;
    tracing::info!("shutting down");

    let snapshot = running.stop().await;
    tracing::info!(
        total_requests = snapshot.total_requests,
        blocked_requests = snapshot.blocked_requests,
        "final stats"
    );

    Ok(())
}

/// Everything a status view needs, gathered without printing anything.
pub struct StatusInfo {
    pub ca_thumbprint: String,
    pub ca_trusted: bool,
    pub listen_addr: SocketAddr,
    pub system_proxy_on_start: bool,
    pub filters: Vec<(String, bool)>,
}

pub fn status_info() -> Result<StatusInfo> {
    let paths = AppPaths::new()?;
    let root_ca = RootCa::load_or_generate(&paths)?;
    let ca_trusted = root_ca.is_installed()?;

    let config = AppConfig::load_or_default(&paths)?;
    let lists_dir = paths.filter_lists_dir();
    let filters = config
        .filters
        .sources()
        .iter()
        .map(|source| {
            let path = lists_dir.join(format!("{}.txt", source.name));
            (source.name.to_string(), path.exists())
        })
        .collect();

    Ok(StatusInfo {
        ca_thumbprint: root_ca.sha1_thumbprint().to_string(),
        ca_trusted,
        listen_addr: config.proxy.listen_addr,
        system_proxy_on_start: config.proxy.set_system_proxy,
        filters,
    })
}

pub fn status() -> Result<()> {
    let info = status_info()?;
    println!("CA thumbprint (SHA-1): {}", info.ca_thumbprint);
    println!("CA trusted: {}", if info.ca_trusted { "yes" } else { "no" });
    println!("Listen address: {}", info.listen_addr);
    println!("System proxy on start: {}", info.system_proxy_on_start);
    for (name, downloaded) in &info.filters {
        println!("Filter list {name}: {}", if *downloaded { "downloaded" } else { "missing" });
    }
    Ok(())
}

/// Result of a completed [`perform_uninstall`] run.
pub struct UninstallOutcome {
    pub ca_was_installed: bool,
    pub system_proxy_cleared: bool,
}

/// Removes the CA from the trust store, clears system proxy settings, and
/// deletes the local CA material — no printing, so any front-end can
/// render its own result.
pub fn perform_uninstall() -> Result<UninstallOutcome> {
    let paths = AppPaths::new()?;
    let root_ca = RootCa::load_or_generate(&paths)?;

    let ca_was_installed = root_ca.is_installed()?;
    if ca_was_installed {
        root_ca.uninstall()?;
    }

    let system_proxy_cleared = match system_proxy().disable() {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(error = %err, "could not clear system proxy settings");
            false
        }
    };

    for path in [
        paths.ca_cert_file(),
        paths.ca_key_file(),
        paths.ca_cert_der_file(),
        paths.chaos_seed_file(),
    ] {
        let _ = std::fs::remove_file(path);
    }

    Ok(UninstallOutcome { ca_was_installed, system_proxy_cleared })
}

pub fn uninstall() -> Result<()> {
    let outcome = perform_uninstall()?;
    if outcome.ca_was_installed {
        println!("Root CA removed from the trust store.");
    } else {
        println!("Root CA was not installed.");
    }
    if outcome.system_proxy_cleared {
        println!("System proxy settings cleared.");
    }
    println!("Local CA material removed. A fresh CA will be generated on the next `setup`.");
    Ok(())
}

/// Accepts a bare domain, a domain with a path (`google.com/search`), or a
/// full URL (`https://google.com/search`) alike, and normalizes down to
/// just the lowercase host.
pub fn normalize_domain(input: &str) -> String {
    let input = input.trim();
    let without_scheme = input.split_once("://").map(|(_, rest)| rest).unwrap_or(input);
    let host = without_scheme.split(['/', '?', '#']).next().unwrap_or(without_scheme);
    host.trim().to_ascii_lowercase()
}

/// The chaos-mode exception list as currently saved (or the defaults, if
/// nothing's been saved yet).
pub fn exceptions_list() -> Result<Vec<ChaosException>> {
    let paths = AppPaths::new()?;
    let config = AppConfig::load_or_default(&paths)?;
    Ok(config.fingerprint.chaos_exceptions)
}

pub fn exceptions_add(domain: &str) -> Result<()> {
    let paths = AppPaths::new()?;
    let mut config = AppConfig::load_or_default(&paths)?;
    let domain = normalize_domain(domain);
    match config.fingerprint.chaos_exceptions.iter_mut().find(|e| e.domain == domain) {
        Some(existing) => existing.enabled = true,
        None => config.fingerprint.chaos_exceptions.push(ChaosException { domain, enabled: true }),
    }
    config.save(&paths)
}

pub fn exceptions_remove(domain: &str) -> Result<()> {
    let paths = AppPaths::new()?;
    let mut config = AppConfig::load_or_default(&paths)?;
    let domain = normalize_domain(domain);
    config.fingerprint.chaos_exceptions.retain(|e| e.domain != domain);
    config.save(&paths)
}

/// Returns whether the domain was found (and therefore actually changed).
pub fn exceptions_set_enabled(domain: &str, enabled: bool) -> Result<bool> {
    let paths = AppPaths::new()?;
    let mut config = AppConfig::load_or_default(&paths)?;
    let domain = normalize_domain(domain);
    let Some(existing) = config.fingerprint.chaos_exceptions.iter_mut().find(|e| e.domain == domain) else {
        return Ok(false);
    };
    existing.enabled = enabled;
    config.save(&paths)?;
    Ok(true)
}

/// Merges domains from a plain-text file (one per line; blank lines and
/// `#` comments ignored) into the list, enabled. Returns how many lines
/// were recognized as domains.
pub fn exceptions_import(path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let paths = AppPaths::new()?;
    let mut config = AppConfig::load_or_default(&paths)?;

    let mut imported = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let domain = normalize_domain(line);
        if domain.is_empty() {
            continue;
        }
        match config.fingerprint.chaos_exceptions.iter_mut().find(|e| e.domain == domain) {
            Some(existing) => existing.enabled = true,
            None => config.fingerprint.chaos_exceptions.push(ChaosException { domain, enabled: true }),
        }
        imported += 1;
    }

    config.save(&paths)?;
    Ok(imported)
}

/// Writes every currently-enabled domain to `path`, one per line, so the
/// list can be shared/reused independently of the rest of the app config.
/// Disabled entries are intentionally left out: from outside this app, a
/// plain domain list has no way to represent "present but inactive".
pub fn exceptions_export(path: &Path) -> Result<usize> {
    let paths = AppPaths::new()?;
    let config = AppConfig::load_or_default(&paths)?;
    let domains: Vec<&str> =
        config.fingerprint.chaos_exceptions.iter().filter(|e| e.enabled).map(|e| e.domain.as_str()).collect();

    let mut text = domains.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(domains.len())
}

pub fn exceptions(action: ExceptionsAction) -> Result<()> {
    match action {
        ExceptionsAction::List => {
            let entries = exceptions_list()?;
            if entries.is_empty() {
                println!("No chaos-mode exceptions configured.");
            }
            for exception in &entries {
                println!(
                    "{} [{}]",
                    exception.domain,
                    if exception.enabled { "enabled" } else { "disabled" }
                );
            }
        }
        ExceptionsAction::Add { domain } => {
            exceptions_add(&domain)?;
            println!("Added {} to the chaos-mode exception list.", normalize_domain(&domain));
        }
        ExceptionsAction::Remove { domain } => {
            exceptions_remove(&domain)?;
            println!("Removed {} from the chaos-mode exception list.", normalize_domain(&domain));
        }
        ExceptionsAction::Enable { domain } => {
            let domain = normalize_domain(&domain);
            if exceptions_set_enabled(&domain, true)? {
                println!("Enabled {domain}.");
            } else {
                println!("{domain} isn't in the list — add it with `dissimulare exceptions add {domain}`.");
            }
        }
        ExceptionsAction::Disable { domain } => {
            let domain = normalize_domain(&domain);
            if exceptions_set_enabled(&domain, false)? {
                println!("Disabled {domain}.");
            } else {
                println!("{domain} isn't in the list.");
            }
        }
        ExceptionsAction::Import { path } => {
            let count = exceptions_import(&path)?;
            println!("Imported {count} domain(s) from {}.", path.display());
        }
        ExceptionsAction::Export { path } => {
            let count = exceptions_export(&path)?;
            println!("Exported {count} enabled domain(s) to {}.", path.display());
        }
    }
    Ok(())
}

fn print_consent_notice(thumbprint: &str) {
    println!("=================================================================");
    println!(" Dissimulare needs to install a local Certificate Authority (CA)");
    println!(" so it can inspect and rewrite your own HTTPS traffic, in order to");
    println!(" block ads/trackers and normalize fingerprinting-related headers.");
    println!();
    println!(" This certificate is trusted ONLY for your current user account");
    println!(" (never system-wide), and Dissimulare uses it solely to");
    println!(" decrypt and re-encrypt HTTPS connections made from this device.");
    println!();
    println!(" SHA-1 thumbprint: {thumbprint}");
    println!();
    println!(" You can remove it at any time by running `dissimulare uninstall`.");
    println!("=================================================================");
}

fn prompt_confirmation() -> Result<bool> {
    print!("Type I AGREE to install the certificate and continue: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim() == "I AGREE")
}
