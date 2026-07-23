use crate::cert_store::{CertStore, CertStoreError, Result as CertResult};
use crate::system_proxy::{Result as ProxyResult, SystemProxy, SystemProxyError};
use std::net::SocketAddr;

use schannel::cert_context::{CertContext, HashAlgorithm};
use schannel::cert_store::{CertAdd, CertStore as SchannelStore};
use winreg::enums::*;
use winreg::RegKey;

const ROOT_STORE_NAME: &str = "Root";
const INTERNET_SETTINGS_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Installs/removes the app's root CA in the *current user's* "Root" store
/// (`CERT_SYSTEM_STORE_CURRENT_USER`), never `LocalMachine`. This is what
/// lets the app trust its own MITM certificates without an admin prompt,
/// which matters both for everyday UX and for running inside an MSIX
/// sandbox where elevation isn't available.
pub struct WindowsCertStore;

impl WindowsCertStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsCertStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CertStore for WindowsCertStore {
    fn install_root_ca(&self, der: &[u8]) -> CertResult<()> {
        let ctx = CertContext::new(der)
            .map_err(|e| CertStoreError::InvalidCertificate(e.to_string()))?;
        let mut store = SchannelStore::open_current_user(ROOT_STORE_NAME)?;
        store.add_cert(&ctx, CertAdd::ReplaceExisting)?;
        Ok(())
    }

    fn uninstall_root_ca(&self, sha1_thumbprint: &str) -> CertResult<()> {
        let store = SchannelStore::open_current_user(ROOT_STORE_NAME)?;
        let target = sha1_thumbprint.to_ascii_lowercase();
        for cert in store.certs() {
            let matches = cert
                .fingerprint(HashAlgorithm::sha1())
                .map(|h| to_hex(&h) == target)
                .unwrap_or(false);
            if matches {
                cert.delete()?;
            }
        }
        Ok(())
    }

    fn is_installed(&self, sha1_thumbprint: &str) -> CertResult<bool> {
        let store = SchannelStore::open_current_user(ROOT_STORE_NAME)?;
        let target = sha1_thumbprint.to_ascii_lowercase();
        for cert in store.certs() {
            if cert
                .fingerprint(HashAlgorithm::sha1())
                .map(|h| to_hex(&h) == target)
                .unwrap_or(false)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn prune_stale(&self, common_name: &str, keep_thumbprint: &str) -> CertResult<()> {
        let store = SchannelStore::open_current_user(ROOT_STORE_NAME)?;
        let keep = keep_thumbprint.to_ascii_lowercase();
        let needle = common_name.as_bytes();

        for cert in store.certs() {
            // `schannel` doesn't expose subject/CN parsing, but the DER
            // encoding of an X.509 Name attribute embeds the string's raw
            // bytes verbatim, so a substring search over the encoded
            // certificate is enough to recognize "one of ours".
            let looks_like_ours = cert.to_der().windows(needle.len()).any(|w| w == needle);
            if !looks_like_ours {
                continue;
            }

            let is_current = cert
                .fingerprint(HashAlgorithm::sha1())
                .map(|h| to_hex(&h) == keep)
                .unwrap_or(false);
            if is_current {
                continue;
            }

            // Best-effort: an orphaned old CA shouldn't block installing
            // the current one.
            let _ = cert.delete();
        }

        Ok(())
    }
}

/// Points the per-user WinINet proxy settings (the ones Edge/Chrome/most
/// Windows apps read) at the local proxy, and notifies WinINet so running
/// processes pick it up without a reboot/relogin.
pub struct WindowsSystemProxy;

impl WindowsSystemProxy {
    pub fn new() -> Self {
        Self
    }

    fn notify_system(&self) -> ProxyResult<()> {
        use windows::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };
        unsafe {
            InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0)
                .map_err(|e| SystemProxyError::Io(std::io::Error::other(e.to_string())))?;
            InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0)
                .map_err(|e| SystemProxyError::Io(std::io::Error::other(e.to_string())))?;
        }
        Ok(())
    }
}

impl Default for WindowsSystemProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemProxy for WindowsSystemProxy {
    fn enable(&self, addr: SocketAddr) -> ProxyResult<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(INTERNET_SETTINGS_PATH)?;
        key.set_value("ProxyServer", &addr.to_string())?;
        // Never proxy loopback/local traffic; also avoids the proxy trying
        // to route requests to itself.
        key.set_value("ProxyOverride", &"<local>")?;
        key.set_value("ProxyEnable", &1u32)?;
        self.notify_system()
    }

    fn disable(&self) -> ProxyResult<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(INTERNET_SETTINGS_PATH)?;
        key.set_value("ProxyEnable", &0u32)?;
        self.notify_system()
    }
}
