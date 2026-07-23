use crate::lists::FilterListSource;
use anyhow::{Context, Result};
use dissimulare_platform::AppPaths;
use std::path::PathBuf;
use std::time::Duration;

/// Everything needed to build the `adblock-rust` engine, handed to
/// [`crate::FilterService::spawn`] so construction happens on its dedicated
/// thread instead of here (the engine type can't cross a thread boundary).
pub enum EngineSource {
    Cached(Vec<u8>),
    Build { texts: Vec<String>, cache_path: PathBuf },
}

/// Downloads/caches filter list text on disk and builds/caches the compiled
/// [`FilterEngine`], so a normal startup only re-parses lists when they've
/// actually changed.
pub struct FilterManager {
    paths: AppPaths,
    client: reqwest::Client,
    refresh_interval: Duration,
}

impl FilterManager {
    pub fn new(paths: AppPaths, refresh_interval: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Dissimulare/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client for filter list downloads")?;
        Ok(Self { paths, client, refresh_interval })
    }

    /// Ensures every configured list is present on disk, downloading any
    /// that are missing or older than the configured refresh interval. A
    /// failed refresh keeps the previously cached copy rather than failing
    /// outright, since a stale list is far better than no blocking at all.
    pub async fn sync_lists(&self, sources: &[FilterListSource]) -> Result<()> {
        std::fs::create_dir_all(self.paths.filter_lists_dir())?;

        for source in sources {
            let path = self.list_path(source.name);
            if !self.is_stale(&path) {
                continue;
            }

            tracing::info!(list = source.name, url = source.url, "downloading filter list");
            match self.download(source.url).await {
                Ok(text) => std::fs::write(&path, text)
                    .with_context(|| format!("writing {}", path.display()))?,
                Err(err) if path.exists() => {
                    tracing::warn!(
                        list = source.name,
                        error = %err,
                        "refresh failed, keeping previously cached list"
                    );
                }
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }

    /// Decides whether the engine should be rebuilt from list text or can
    /// reuse the on-disk cache, reading whichever raw input is needed.
    /// Reuses the cache whenever every source list is at least as old as it.
    pub fn engine_source(&self, sources: &[FilterListSource]) -> Result<EngineSource> {
        let cache_path = self.paths.filter_engine_cache();

        if self.cache_is_fresh(&cache_path, sources) {
            if let Ok(bytes) = std::fs::read(&cache_path) {
                tracing::debug!("reusing cached filter engine");
                return Ok(EngineSource::Cached(bytes));
            }
        }

        let mut texts = Vec::with_capacity(sources.len());
        for source in sources {
            let path = self.list_path(source.name);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            texts.push(text);
        }

        Ok(EngineSource::Build { texts, cache_path })
    }

    fn cache_is_fresh(&self, cache_path: &PathBuf, sources: &[FilterListSource]) -> bool {
        let Ok(cache_time) = std::fs::metadata(cache_path).and_then(|m| m.modified()) else {
            return false;
        };

        sources.iter().all(|s| {
            std::fs::metadata(self.list_path(s.name))
                .and_then(|m| m.modified())
                .map(|list_time| list_time <= cache_time)
                .unwrap_or(false)
        })
    }

    fn is_stale(&self, path: &PathBuf) -> bool {
        match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(modified) => modified.elapsed().unwrap_or(Duration::MAX) > self.refresh_interval,
            Err(_) => true,
        }
    }

    async fn download(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("requesting {url}"))?
            .error_for_status()
            .with_context(|| format!("bad status from {url}"))?;
        response
            .text()
            .await
            .with_context(|| format!("reading body from {url}"))
    }

    fn list_path(&self, name: &str) -> PathBuf {
        self.paths.filter_lists_dir().join(format!("{name}.txt"))
    }
}
