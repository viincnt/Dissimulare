//! Root CA lifecycle: generate once, persist to disk, reload on every
//! subsequent run, and hand out both a hudsucker signing authority (for
//! live MITM certs) and OS trust-store install/uninstall via
//! `dissimulare-platform`.

use anyhow::{Context, Result};
use dissimulare_platform::{cert_store, AppPaths};
use hudsucker::certificate_authority::RcgenAuthority;
use hudsucker::rustls::crypto::aws_lc_rs;
use rcgen::{BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair};
use sha1::{Digest, Sha1};

/// The app's persisted root CA: same key material every run, so the
/// certificate the user trusted on setup keeps working across restarts.
pub struct RootCa {
    cert_pem: String,
    key_pem: String,
    der: Vec<u8>,
    thumbprint: String,
}

impl RootCa {
    /// Loads the CA from disk, or generates and persists a brand new one if
    /// this is the first run.
    pub fn load_or_generate(paths: &AppPaths) -> Result<Self> {
        let cert_path = paths.ca_cert_file();
        let key_path = paths.ca_key_file();
        let der_path = paths.ca_cert_der_file();

        if cert_path.exists() && key_path.exists() && der_path.exists() {
            tracing::debug!("loading existing CA from {}", cert_path.display());
            let cert_pem = std::fs::read_to_string(&cert_path)
                .with_context(|| format!("reading {}", cert_path.display()))?;
            let key_pem = std::fs::read_to_string(&key_path)
                .with_context(|| format!("reading {}", key_path.display()))?;
            let der = std::fs::read(&der_path)
                .with_context(|| format!("reading {}", der_path.display()))?;
            let thumbprint = sha1_hex(&der);
            return Ok(Self { cert_pem, key_pem, der, thumbprint });
        }

        tracing::info!("generating new local root CA at {}", cert_path.display());
        let (cert_pem, key_pem, der) = generate_root_ca()?;

        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&cert_path, &cert_pem)
            .with_context(|| format!("writing {}", cert_path.display()))?;
        std::fs::write(&key_path, &key_pem)
            .with_context(|| format!("writing {}", key_path.display()))?;
        std::fs::write(&der_path, &der)
            .with_context(|| format!("writing {}", der_path.display()))?;
        restrict_private_key_permissions(&key_path)?;

        let thumbprint = sha1_hex(&der);
        Ok(Self { cert_pem, key_pem, der, thumbprint })
    }

    /// SHA-1 thumbprint (lowercase hex) of the CA certificate, used to
    /// find/verify/remove it in the OS trust store.
    pub fn sha1_thumbprint(&self) -> &str {
        &self.thumbprint
    }

    /// Raw DER bytes of the CA certificate.
    pub fn der(&self) -> &[u8] {
        &self.der
    }

    /// Builds the hudsucker signing authority that mints per-domain leaf
    /// certificates on the fly, signed by this root CA.
    pub fn to_authority(&self) -> Result<RcgenAuthority> {
        let key_pair =
            KeyPair::from_pem(&self.key_pem).context("corrupted CA private key")?;
        let issuer = Issuer::from_ca_cert_pem(&self.cert_pem, key_pair)
            .context("corrupted CA certificate")?;
        Ok(RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider()))
    }

    /// Installs this CA as a trusted root for the current user. Never
    /// requires admin/root elevation (see `dissimulare-platform::CertStore`).
    pub fn install(&self) -> Result<()> {
        cert_store()
            .install_root_ca(&self.der)
            .context("failed to install the CA into the system certificate store")
    }

    /// Removes this CA from the current user's trust store.
    pub fn uninstall(&self) -> Result<()> {
        cert_store()
            .uninstall_root_ca(&self.thumbprint)
            .context("failed to remove the CA from the system certificate store")
    }

    pub fn is_installed(&self) -> Result<bool> {
        cert_store()
            .is_installed(&self.thumbprint)
            .context("failed to query the system certificate store")
    }
}

fn generate_root_ca() -> Result<(String, String, Vec<u8>)> {
    let key_pair = KeyPair::generate().context("generating CA key pair")?;

    let mut params =
        CertificateParams::new(Vec::<String>::new()).context("invalid certificate parameters")?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "Dissimulare Local CA");
    dn.push(DnType::OrganizationName, "Dissimulare");
    params.distinguished_name = dn;

    let cert = params
        .self_signed(&key_pair)
        .context("self-signing the CA certificate")?;

    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();
    let der = cert.der().to_vec();

    Ok((cert_pem, key_pem, der))
}

fn sha1_hex(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(unix)]
fn restrict_private_key_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("restricting permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_private_key_permissions(_path: &std::path::Path) -> Result<()> {
    // On Windows the key already lives under the per-user AppData profile,
    // which other user accounts can't read by default; NTFS ACL tightening
    // could be added here later for defense in depth.
    Ok(())
}
