use thiserror::Error;

#[derive(Debug, Error)]
pub enum CertStoreError {
    #[error("operation not supported on this platform")]
    Unsupported,
    #[error("I/O error in the certificate store: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid certificate: {0}")]
    InvalidCertificate(String),
}

pub type Result<T> = std::result::Result<T, CertStoreError>;

/// Abstraction over the OS trust store. Only one implementation of this
/// trait should exist per platform, so the rest of the workspace never has
/// to know how a given OS trusts a root CA.
///
/// Implementations MUST target a per-user store (never require admin/root
/// elevation) so the app keeps working inside a sandboxed Microsoft Store
/// (MSIX) container.
pub trait CertStore: Send + Sync {
    /// Installs `der` (a DER-encoded X.509 certificate) as a trusted root CA
    /// for the current user.
    fn install_root_ca(&self, der: &[u8]) -> Result<()>;

    /// Removes any previously installed root CA matching `sha1_thumbprint`
    /// (lowercase hex, 40 chars).
    fn uninstall_root_ca(&self, sha1_thumbprint: &str) -> Result<()>;

    /// Returns whether a root CA with the given thumbprint is currently trusted.
    fn is_installed(&self, sha1_thumbprint: &str) -> Result<bool>;

    /// Removes any trusted root CA whose subject contains `common_name` but
    /// whose SHA-1 thumbprint isn't `keep_thumbprint`. Cleans up orphaned
    /// trust anchors left behind when the local CA material is regenerated
    /// (e.g. the app's data directory was wiped) without going through
    /// `uninstall_root_ca` first — otherwise every regeneration would add a
    /// new trusted CA next to the old one instead of replacing it. Best
    /// effort: failing to remove a stale entry should not block installing
    /// the current one.
    fn prune_stale(&self, common_name: &str, keep_thumbprint: &str) -> Result<()>;
}
