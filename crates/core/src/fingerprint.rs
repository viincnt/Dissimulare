use crate::chaos::ChaosIdentity;
use http::header::{ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use http::{HeaderMap, HeaderName, HeaderValue, Uri};

/// A generic, common Chrome-on-Windows UA string, used only in [`IdentityMode::Uniform`].
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

const CLIENT_HINT_HEADERS: &[&str] = &[
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "sec-ch-ua-platform",
    "sec-ch-ua-platform-version",
    "sec-ch-ua-full-version",
    "sec-ch-ua-full-version-list",
    "sec-ch-ua-arch",
    "sec-ch-ua-bitness",
    "sec-ch-ua-model",
    "sec-ch-ua-wow64",
];

/// How the proxy should present the browser's identity to sites.
#[derive(Debug, Clone)]
pub enum IdentityMode {
    /// Don't touch User-Agent/client hints at all.
    Off,
    /// Look like the same low-entropy, common browser on every site — the
    /// "hide in the crowd" approach.
    Uniform(String),
    /// Give every domain a different, deliberately absurd hardware/OS
    /// combination instead — not hiding data, poisoning it. Same domain
    /// always gets the same combination (derived from `seed`); different
    /// domains get unrelated ones.
    Chaos { seed: Vec<u8> },
}

impl Default for IdentityMode {
    fn default() -> Self {
        IdentityMode::Uniform(DEFAULT_USER_AGENT.to_string())
    }
}

/// The identity actually picked for one request, so callers (header
/// rewriting now, HTML injection later) stay in sync with each other.
#[derive(Debug, Clone)]
pub struct ResolvedIdentity {
    pub user_agent: String,
    pub chaos: Option<ChaosIdentity>,
}

/// Rules for rewriting outgoing request headers so they carry consistent
/// values instead of whatever uniquely identifies this particular
/// browser/OS/build — never simply stripped, always replaced with
/// *something* (an absent User-Agent is its own fingerprint).
#[derive(Debug, Clone)]
pub struct FingerprintPolicy {
    pub identity_mode: IdentityMode,
    pub minimal_accept_language: bool,
    pub strip_client_hints: bool,
    pub trim_cross_site_referer: bool,
    pub send_gpc: bool,
    /// Domains (and their subdomains) that always get a normal, common
    /// browser identity even in [`IdentityMode::Chaos`] — an escape hatch
    /// for sites whose own User-Agent sniffing breaks when it doesn't
    /// recognize a real browser/OS, Google search being the prototypical
    /// case. Ignored in [`IdentityMode::Off`]/[`IdentityMode::Uniform`],
    /// since those already send the same identity everywhere.
    pub chaos_exceptions: Vec<String>,
}

impl Default for FingerprintPolicy {
    fn default() -> Self {
        Self {
            identity_mode: IdentityMode::default(),
            minimal_accept_language: true,
            strip_client_hints: true,
            trim_cross_site_referer: true,
            send_gpc: true,
            chaos_exceptions: Vec::new(),
        }
    }
}

impl FingerprintPolicy {
    /// Resolves the identity that should be presented to `host`.
    pub fn resolve_identity(&self, host: &str) -> Option<ResolvedIdentity> {
        match &self.identity_mode {
            IdentityMode::Off => None,
            IdentityMode::Uniform(ua) => Some(ResolvedIdentity { user_agent: ua.clone(), chaos: None }),
            IdentityMode::Chaos { seed } => {
                if self.is_chaos_exception(host) {
                    return Some(ResolvedIdentity {
                        user_agent: DEFAULT_USER_AGENT.to_string(),
                        chaos: None,
                    });
                }
                let chaos = ChaosIdentity::for_domain(seed, host);
                Some(ResolvedIdentity { user_agent: chaos.user_agent(), chaos: Some(chaos) })
            }
        }
    }

    /// Whether `host` (or a parent of it — `chaos_exceptions` entries match
    /// their subdomains too) is on the exception list.
    fn is_chaos_exception(&self, host: &str) -> bool {
        crate::domain_match::matches_any(host, &self.chaos_exceptions)
    }

    /// Rewrites `headers` in place for a request headed to `request_uri`,
    /// returning the identity that was applied (if any) so the caller can
    /// keep the response body's injected JS consistent with it.
    pub fn apply(&self, headers: &mut HeaderMap, request_uri: &Uri) -> Option<ResolvedIdentity> {
        if self.strip_client_hints {
            for name in CLIENT_HINT_HEADERS {
                headers.remove(*name);
            }
        }

        let identity = request_uri.host().and_then(|host| self.resolve_identity(host));
        if let Some(identity) = &identity {
            if let Ok(value) = HeaderValue::from_str(&identity.user_agent) {
                headers.insert(USER_AGENT, value);
            }
            if let Some(chaos) = &identity.chaos {
                insert_header(headers, "sec-ch-ua-platform", &chaos.sec_ch_ua_platform());
                insert_header(headers, "sec-ch-ua-model", &chaos.sec_ch_ua_model());
            }
        }

        if self.minimal_accept_language && headers.contains_key(ACCEPT_LANGUAGE) {
            headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        }

        if self.trim_cross_site_referer {
            self.trim_referer(headers, request_uri);
        }

        if self.send_gpc {
            if let Ok(name) = HeaderName::from_bytes(b"sec-gpc") {
                headers.insert(name, HeaderValue::from_static("1"));
            }
        }

        identity
    }

    /// Reduces the `Referer` header to just its origin when the referring
    /// page's host differs from the request's own host, instead of leaking
    /// the exact page (path/query) the user was on to a third-party site.
    fn trim_referer(&self, headers: &mut HeaderMap, request_uri: &Uri) {
        let Some(target_host) = request_uri.host() else { return };
        let Some(referer_value) = headers.get(REFERER) else { return };
        let Ok(referer_str) = referer_value.to_str() else { return };
        let Ok(referer_uri) = referer_str.parse::<Uri>() else { return };
        let (Some(scheme), Some(referer_host)) =
            (referer_uri.scheme_str(), referer_uri.host())
        else {
            return;
        };

        if target_host.eq_ignore_ascii_case(referer_host) {
            return;
        }

        let trimmed = format!("{scheme}://{referer_host}/");
        if let Ok(value) = HeaderValue::from_str(&trimmed) {
            headers.insert(REFERER, value);
        }
    }
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
        headers.insert(name, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(s: &str) -> Uri {
        s.parse().unwrap()
    }

    #[test]
    fn trims_cross_site_referer_to_origin() {
        let policy = FingerprintPolicy::default();
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_static("https://referrer.example/secret/page?x=1"));

        policy.apply(&mut headers, &uri("https://target.example/thing"));

        assert_eq!(headers.get(REFERER).unwrap(), "https://referrer.example/");
    }

    #[test]
    fn leaves_same_site_referer_untouched() {
        let policy = FingerprintPolicy::default();
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_static("https://target.example/secret/page"));

        policy.apply(&mut headers, &uri("https://target.example/thing"));

        assert_eq!(
            headers.get(REFERER).unwrap(),
            "https://target.example/secret/page"
        );
    }

    #[test]
    fn strips_client_hints_and_spoofs_ua_in_uniform_mode() {
        let policy = FingerprintPolicy::default();
        let mut headers = HeaderMap::new();
        headers.insert("sec-ch-ua-platform-version", HeaderValue::from_static("\"15.2.1\""));
        headers.insert(USER_AGENT, HeaderValue::from_static("some very specific real UA"));

        policy.apply(&mut headers, &uri("https://target.example/"));

        assert!(headers.get("sec-ch-ua-platform-version").is_none());
        assert_eq!(headers.get(USER_AGENT).unwrap(), DEFAULT_USER_AGENT);
    }

    #[test]
    fn chaos_mode_sets_absurd_platform_and_model_headers() {
        let policy = FingerprintPolicy {
            identity_mode: IdentityMode::Chaos { seed: b"test-seed".to_vec() },
            ..FingerprintPolicy::default()
        };
        let mut headers = HeaderMap::new();

        let identity = policy
            .apply(&mut headers, &uri("https://target.example/"))
            .expect("chaos mode always resolves an identity");
        let chaos = identity.chaos.expect("chaos identity present");

        assert_eq!(headers.get(USER_AGENT).unwrap(), identity.user_agent.as_str());
        assert!(headers
            .get("sec-ch-ua-platform")
            .unwrap()
            .to_str()
            .unwrap()
            .contains(chaos.os));
        assert!(headers
            .get("sec-ch-ua-model")
            .unwrap()
            .to_str()
            .unwrap()
            .contains(chaos.hardware));
    }

    #[test]
    fn chaos_exception_gets_a_normal_identity_instead() {
        let policy = FingerprintPolicy {
            identity_mode: IdentityMode::Chaos { seed: b"test-seed".to_vec() },
            chaos_exceptions: vec!["google.com".to_string()],
            ..FingerprintPolicy::default()
        };
        let mut headers = HeaderMap::new();

        // Exact match and subdomain both fall under the exception.
        let identity = policy
            .apply(&mut headers, &uri("https://www.google.com/search"))
            .expect("still resolves an identity, just not a chaos one");
        assert!(identity.chaos.is_none());
        assert_eq!(identity.user_agent, DEFAULT_USER_AGENT);
        assert_eq!(headers.get(USER_AGENT).unwrap(), DEFAULT_USER_AGENT);

        // An unrelated host still gets the chaos treatment.
        let mut other_headers = HeaderMap::new();
        let other_identity = policy
            .apply(&mut other_headers, &uri("https://unrelated.example/"))
            .expect("chaos mode always resolves an identity");
        assert!(other_identity.chaos.is_some());
    }
}
