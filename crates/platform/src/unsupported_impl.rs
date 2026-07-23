use crate::cert_store::{CertStore, CertStoreError, Result as CertResult};
use crate::system_proxy::{Result as ProxyResult, SystemProxy, SystemProxyError};
use std::net::SocketAddr;

/// Placeholder used on any platform without a dedicated implementation yet
/// (everything except Windows and macOS, for now — e.g. Linux).
///
/// To add support for another OS: create a new `<os>_impl.rs` implementing
/// `CertStore` and `SystemProxy`, then wire both in from `lib.rs` the same
/// way the Windows and macOS modules are wired in.
pub struct UnsupportedCertStore;
pub struct UnsupportedSystemProxy;

impl CertStore for UnsupportedCertStore {
    fn install_root_ca(&self, _der: &[u8]) -> CertResult<()> {
        Err(CertStoreError::Unsupported)
    }

    fn uninstall_root_ca(&self, _sha1_thumbprint: &str) -> CertResult<()> {
        Err(CertStoreError::Unsupported)
    }

    fn is_installed(&self, _sha1_thumbprint: &str) -> CertResult<bool> {
        Err(CertStoreError::Unsupported)
    }
}

impl SystemProxy for UnsupportedSystemProxy {
    fn enable(&self, _addr: SocketAddr) -> ProxyResult<()> {
        Err(SystemProxyError::Unsupported)
    }

    fn disable(&self) -> ProxyResult<()> {
        Err(SystemProxyError::Unsupported)
    }
}
