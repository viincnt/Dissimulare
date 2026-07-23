use crate::cert_store::{CertStore, CertStoreError, Result as CertResult};
use crate::system_proxy::{Result as ProxyResult, SystemProxy, SystemProxyError};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Output};

fn login_keychain() -> CertResult<String> {
    let home = std::env::var("HOME")
        .map_err(|_| CertStoreError::Io(std::io::Error::other("HOME is not set")))?;
    Ok(format!("{home}/Library/Keychains/login.keychain-db"))
}

fn write_temp_der(der: &[u8]) -> std::io::Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("dissimulare-ca-{}-{:x}.der", std::process::id(), der.len()));
    std::fs::write(&path, der)?;
    Ok(path)
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

/// Installs/removes the app's root CA in the *current user's* login
/// keychain, never the System keychain, so no admin password is required
/// (mirrors the no-elevation rule the Windows implementation follows).
pub struct MacOsCertStore;

impl MacOsCertStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsCertStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CertStore for MacOsCertStore {
    fn install_root_ca(&self, der: &[u8]) -> CertResult<()> {
        let keychain = login_keychain()?;
        let cert_path =
            write_temp_der(der).map_err(CertStoreError::Io)?;

        let result = Command::new("security")
            .args(["add-trusted-cert", "-r", "trustRoot", "-k", &keychain])
            .arg(&cert_path)
            .output();
        let _ = std::fs::remove_file(&cert_path);

        let output = result.map_err(CertStoreError::Io)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CertStoreError::InvalidCertificate(stderr_of(&output)))
        }
    }

    fn uninstall_root_ca(&self, sha1_thumbprint: &str) -> CertResult<()> {
        let keychain = login_keychain()?;
        let output = Command::new("security")
            .args(["delete-certificate", "-Z", sha1_thumbprint, "-t"])
            .arg(&keychain)
            .output()
            .map_err(CertStoreError::Io)?;

        if output.status.success() {
            return Ok(());
        }
        // Already gone is not a failure for an idempotent uninstall.
        let stderr = stderr_of(&output);
        if stderr.contains("not find") || stderr.contains("not found") {
            Ok(())
        } else {
            Err(CertStoreError::Io(std::io::Error::other(stderr)))
        }
    }

    fn is_installed(&self, sha1_thumbprint: &str) -> CertResult<bool> {
        let keychain = login_keychain()?;
        let output = Command::new("security")
            .args(["find-certificate", "-a", "-Z", "-k"])
            .arg(&keychain)
            .output()
            .map_err(CertStoreError::Io)?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        let needle = format!("sha-1 hash: {}", sha1_thumbprint.to_ascii_lowercase());
        Ok(stdout.contains(&needle))
    }
}

/// Points every active network service's web proxy settings (the ones
/// System Settings / Safari / most macOS apps read) at the local proxy via
/// `networksetup`, and clears them back on disable.
pub struct MacOsSystemProxy;

impl MacOsSystemProxy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsSystemProxy {
    fn default() -> Self {
        Self::new()
    }
}

fn run_networksetup(args: &[&str]) -> ProxyResult<()> {
    let output = Command::new("networksetup")
        .args(args)
        .output()
        .map_err(SystemProxyError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SystemProxyError::Io(std::io::Error::other(stderr_of(&output))))
    }
}

/// Network service names (e.g. "Wi-Fi", "Ethernet"), skipping the disabled
/// ones `networksetup` prefixes with `*`.
fn network_services() -> ProxyResult<Vec<String>> {
    let output = Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .map_err(SystemProxyError::Io)?;
    if !output.status.success() {
        return Err(SystemProxyError::Io(std::io::Error::other(stderr_of(&output))));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .skip(1) // header line: "An asterisk (*) denotes that a network service is disabled."
        .filter(|line| !line.starts_with('*') && !line.trim().is_empty())
        .map(str::to_string)
        .collect())
}

impl SystemProxy for MacOsSystemProxy {
    fn enable(&self, addr: SocketAddr) -> ProxyResult<()> {
        let host = addr.ip().to_string();
        let port = addr.port().to_string();
        for service in network_services()? {
            run_networksetup(&["-setwebproxy", &service, &host, &port])?;
            run_networksetup(&["-setsecurewebproxy", &service, &host, &port])?;
            // Never proxy loopback/local traffic; also avoids the proxy
            // trying to route requests to itself.
            run_networksetup(&[
                "-setproxybypassdomains",
                &service,
                "127.0.0.1",
                "localhost",
                "*.local",
            ])?;
        }
        Ok(())
    }

    fn disable(&self) -> ProxyResult<()> {
        for service in network_services()? {
            run_networksetup(&["-setwebproxystate", &service, "off"])?;
            run_networksetup(&["-setsecurewebproxystate", &service, "off"])?;
        }
        Ok(())
    }
}
