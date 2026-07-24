use anyhow::{Context, Result};
use dissimulare_core::{FingerprintPolicy, IdentityMode, DEFAULT_USER_AGENT};
use dissimulare_filters::{FilterListSource, DEFAULT_LISTS};
use dissimulare_platform::AppPaths;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub proxy: ProxyConfig,
    pub filters: FiltersConfig,
    pub fingerprint: FingerprintConfig,
    pub youtube: YoutubeConfig,
}

impl AppConfig {
    pub fn load_or_default(paths: &AppPaths) -> Result<Self> {
        let path = paths.config_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let path = paths.config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    pub listen_addr: SocketAddr,
    pub set_system_proxy: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8443".parse().expect("valid default address"),
            set_system_proxy: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YoutubeConfig {
    /// Strips ad-break scheduling metadata from YouTube's internal player
    /// API response so the player never learns there's an ad to insert.
    /// Doesn't touch server-stitched ad formats, where no separate signal
    /// exists to remove — disable if a YouTube API change ever makes this
    /// misbehave, without needing a rebuild.
    pub strip_ad_metadata: bool,
}

impl Default for YoutubeConfig {
    fn default() -> Self {
        Self { strip_ad_metadata: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FiltersConfig {
    pub lists: Vec<String>,
    pub refresh_hours: u64,
}

impl Default for FiltersConfig {
    fn default() -> Self {
        Self {
            lists: DEFAULT_LISTS.iter().map(|s| s.name.to_string()).collect(),
            refresh_hours: 24,
        }
    }
}

impl FiltersConfig {
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.refresh_hours.max(1) * 3600)
    }

    pub fn sources(&self) -> Vec<FilterListSource> {
        DEFAULT_LISTS
            .iter()
            .filter(|s| self.lists.iter().any(|name| name == s.name))
            .copied()
            .collect()
    }
}

/// A single chaos-mode exception entry: a domain (and its subdomains) that
/// always gets a normal browser identity, plus whether it's currently
/// active. Kept separate from simply removing the domain so a user can
/// toggle a predefined entry off without losing it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosException {
    pub domain: String,
    pub enabled: bool,
}

impl ChaosException {
    fn enabled(domain: &str) -> Self {
        Self { domain: domain.to_string(), enabled: true }
    }
}

/// Domains known to break under chaos mode's absurd User-Agent (heavy
/// server-side UA sniffing falling back to a legacy/basic page being the
/// usual symptom) — pre-populated but each individually toggleable, and
/// the user can add their own on top.
const DEFAULT_CHAOS_EXCEPTIONS: &[&str] = &["google.com"];

/// Which identity strategy to use. `"chaos"` (the default) feeds every
/// domain a different, deliberately absurd hardware/OS combination instead
/// of trying to blend into a crowd; `"uniform"` falls back to a single
/// common UA for every site; `"off"` leaves User-Agent/client hints alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FingerprintConfig {
    pub identity_mode: String,
    pub uniform_user_agent: String,
    pub minimal_accept_language: bool,
    pub strip_client_hints: bool,
    pub trim_cross_site_referer: bool,
    pub send_gpc: bool,
    /// Sites exempted from chaos mode. Fully user-editable: predefined
    /// entries can be disabled instead of removed, and custom domains can
    /// be added — see the `dissimulare exceptions` subcommand for
    /// add/remove/enable/disable/import/export.
    pub chaos_exceptions: Vec<ChaosException>,
}

impl Default for FingerprintConfig {
    fn default() -> Self {
        let defaults = FingerprintPolicy::default();
        Self {
            identity_mode: "chaos".to_string(),
            uniform_user_agent: DEFAULT_USER_AGENT.to_string(),
            minimal_accept_language: defaults.minimal_accept_language,
            strip_client_hints: defaults.strip_client_hints,
            trim_cross_site_referer: defaults.trim_cross_site_referer,
            send_gpc: defaults.send_gpc,
            chaos_exceptions: DEFAULT_CHAOS_EXCEPTIONS.iter().map(|d| ChaosException::enabled(d)).collect(),
        }
    }
}

impl FingerprintConfig {
    /// Builds the runtime policy. `chaos_seed` is only used when
    /// `identity_mode == "chaos"`.
    pub fn to_policy(&self, chaos_seed: Vec<u8>) -> FingerprintPolicy {
        let identity_mode = match self.identity_mode.as_str() {
            "off" => IdentityMode::Off,
            "chaos" => IdentityMode::Chaos { seed: chaos_seed },
            _ => IdentityMode::Uniform(self.uniform_user_agent.clone()),
        };

        let chaos_exceptions = self
            .chaos_exceptions
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.domain.clone())
            .collect();

        FingerprintPolicy {
            identity_mode,
            minimal_accept_language: self.minimal_accept_language,
            strip_client_hints: self.strip_client_hints,
            trim_cross_site_referer: self.trim_cross_site_referer,
            send_gpc: self.send_gpc,
            chaos_exceptions,
        }
    }
}
