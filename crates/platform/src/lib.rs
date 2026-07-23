//! Platform integration for Dissimulare.
//!
//! This is the *only* crate in the workspace allowed to contain
//! `#[cfg(target_os = "...")]` code. Everything else talks to the
//! [`CertStore`] and [`SystemProxy`] traits, so porting to a new OS means
//! adding one module here and wiring it into [`cert_store()`] /
//! [`system_proxy()`] — no changes anywhere else.

mod cert_store;
mod paths;
mod system_proxy;

pub use cert_store::{CertStore, CertStoreError};
pub use paths::AppPaths;
pub use system_proxy::{SystemProxy, SystemProxyError};

#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(target_os = "windows")]
use windows_impl::{WindowsCertStore, WindowsSystemProxy};

#[cfg(not(target_os = "windows"))]
mod unsupported_impl;
#[cfg(not(target_os = "windows"))]
use unsupported_impl::{UnsupportedCertStore, UnsupportedSystemProxy};

/// Returns the [`CertStore`] implementation for the current platform.
pub fn cert_store() -> Box<dyn CertStore> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsCertStore::new())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(UnsupportedCertStore)
    }
}

/// Returns the [`SystemProxy`] implementation for the current platform.
pub fn system_proxy() -> Box<dyn SystemProxy> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsSystemProxy::new())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(UnsupportedSystemProxy)
    }
}
