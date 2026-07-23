use crate::config::AppConfig;
use crate::seed::load_or_generate_chaos_seed;
use anyhow::{bail, Context, Result};
use dissimulare_ca::RootCa;
use dissimulare_core::{DissimulareHandler, Stats, StatsSnapshot};
use dissimulare_filters::{FilterManager, FilterService};
use dissimulare_platform::{system_proxy, AppPaths};
use hudsucker::rustls::crypto::aws_lc_rs;
use std::io::Write;
use std::net::SocketAddr;

pub async fn setup() -> Result<()> {
    let paths = AppPaths::new()?;
    paths.ensure_dirs()?;

    let root_ca = RootCa::load_or_generate(&paths)?;

    if root_ca.is_installed()? {
        println!("Root CA already trusted (SHA-1 {}).", root_ca.sha1_thumbprint());
    } else {
        print_consent_notice(root_ca.sha1_thumbprint());
        if !prompt_confirmation()? {
            println!("Setup cancelled — no certificate was installed.");
            return Ok(());
        }
        root_ca.install()?;
        println!("Root CA installed for the current user.");
    }

    let config = AppConfig::load_or_default(&paths)?;
    let sources = config.filters.sources();
    let refresh_interval = config.filters.refresh_interval();

    println!("Downloading filter lists ({})...", sources.iter().map(|s| s.name).collect::<Vec<_>>().join(", "));
    let filter_manager = FilterManager::new(paths.clone(), refresh_interval)?;
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
    println!("Filter lists ready.");

    config.save(&paths)?;
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
    let handler = DissimulareHandler::new(filter_service, policy, stats.clone());

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

pub fn status() -> Result<()> {
    let paths = AppPaths::new()?;
    let root_ca = RootCa::load_or_generate(&paths)?;
    let installed = root_ca.is_installed()?;

    println!("CA thumbprint (SHA-1): {}", root_ca.sha1_thumbprint());
    println!("CA trusted: {}", if installed { "yes" } else { "no" });

    let config = AppConfig::load_or_default(&paths)?;
    println!("Listen address: {}", config.proxy.listen_addr);
    println!("System proxy on start: {}", config.proxy.set_system_proxy);

    let lists_dir = paths.filter_lists_dir();
    for source in config.filters.sources() {
        let path = lists_dir.join(format!("{}.txt", source.name));
        println!(
            "Filter list {}: {}",
            source.name,
            if path.exists() { "downloaded" } else { "missing" }
        );
    }

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let paths = AppPaths::new()?;
    let root_ca = RootCa::load_or_generate(&paths)?;

    if root_ca.is_installed()? {
        root_ca.uninstall()?;
        println!("Root CA removed from the trust store.");
    } else {
        println!("Root CA was not installed.");
    }

    match system_proxy().disable() {
        Ok(()) => println!("System proxy settings cleared."),
        Err(err) => tracing::warn!(error = %err, "could not clear system proxy settings"),
    }

    for path in [
        paths.ca_cert_file(),
        paths.ca_key_file(),
        paths.ca_cert_der_file(),
        paths.chaos_seed_file(),
    ] {
        let _ = std::fs::remove_file(path);
    }
    println!("Local CA material removed. A fresh CA will be generated on the next `setup`.");

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
