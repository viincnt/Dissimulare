use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// Central place for every on-disk path the app uses, backed by
/// `directories::ProjectDirs`. Nothing here is Windows-specific: this is
/// what makes the rest of the workspace portable to macOS/Linux without
/// touching path logic.
#[derive(Clone)]
pub struct AppPaths {
    dirs: ProjectDirs,
}

impl AppPaths {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "Dissimulare", "Dissimulare")
            .context("could not determine the system's application data directories")?;
        Ok(Self { dirs })
    }

    pub fn config_dir(&self) -> &Path {
        self.dirs.config_dir()
    }

    pub fn data_dir(&self) -> &Path {
        self.dirs.data_dir()
    }

    pub fn cache_dir(&self) -> &Path {
        self.dirs.cache_dir()
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    pub fn ca_cert_file(&self) -> PathBuf {
        self.data_dir().join("ca").join("ca_cert.pem")
    }

    pub fn ca_key_file(&self) -> PathBuf {
        self.data_dir().join("ca").join("ca_key.pem")
    }

    pub fn ca_cert_der_file(&self) -> PathBuf {
        self.data_dir().join("ca").join("ca_cert.der")
    }

    /// Per-installation random seed used to derive a stable-per-domain
    /// chaos identity (see `dissimulare-core::IdentityMode::Chaos`).
    pub fn chaos_seed_file(&self) -> PathBuf {
        self.data_dir().join("chaos_seed.bin")
    }

    pub fn filter_engine_cache(&self) -> PathBuf {
        self.cache_dir().join("filters").join("engine.bin")
    }

    pub fn filter_lists_dir(&self) -> PathBuf {
        self.cache_dir().join("filters").join("lists")
    }

    /// Creates every directory this struct hands out paths inside of.
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.config_dir())?;
        std::fs::create_dir_all(self.ca_cert_file().parent().unwrap())?;
        std::fs::create_dir_all(self.filter_lists_dir())?;
        Ok(())
    }
}
