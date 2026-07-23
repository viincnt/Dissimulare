use super::data::{CHAOS_HARDWARES, CHAOS_OPERATING_SYSTEMS};
use sha1::{Digest, Sha1};

/// A deliberately absurd hardware/OS combination, deterministically picked
/// per domain from a per-installation seed. Same domain + same seed always
/// yields the same combination (so a site doesn't see its "browser" change
/// identity mid-session); different domains get different, unrelated
/// combinations, so nothing about the pattern is stitchable across sites.
#[derive(Debug, Clone, Copy)]
pub struct ChaosIdentity {
    pub hardware: &'static str,
    pub os: &'static str,
}

impl ChaosIdentity {
    pub fn for_domain(seed: &[u8], host: &str) -> Self {
        let host = host.to_ascii_lowercase();
        let hardware = CHAOS_HARDWARES[pick_index(seed, &host, "hardware", CHAOS_HARDWARES.len())];
        let os = CHAOS_OPERATING_SYSTEMS
            [pick_index(seed, &host, "os", CHAOS_OPERATING_SYSTEMS.len())];
        Self { hardware, os }
    }

    /// A `User-Agent`-shaped string (still starts with `Mozilla/5.0 (...)`
    /// so naive server-side sniffing doesn't hard-fail) with the chaos OS
    /// name sitting where a real platform token would go.
    pub fn user_agent(&self) -> String {
        format!(
            "Mozilla/5.0 ({}; {}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            self.os, self.hardware
        )
    }

    /// Value for the `Sec-CH-UA-Platform` request header (expects a quoted
    /// string per the Client Hints spec).
    pub fn sec_ch_ua_platform(&self) -> String {
        format!("\"{}\"", escape_quotes(self.os))
    }

    /// Value for the `Sec-CH-UA-Model` request header.
    pub fn sec_ch_ua_model(&self) -> String {
        format!("\"{}\"", escape_quotes(self.hardware))
    }
}

fn escape_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn pick_index(seed: &[u8], host: &str, salt: &str, len: usize) -> usize {
    debug_assert!(len > 0);
    let mut hasher = Sha1::new();
    hasher.update(seed);
    hasher.update(host.as_bytes());
    hasher.update(salt.as_bytes());
    let digest = hasher.finalize();
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    (n as usize) % len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_domain_same_seed_is_stable() {
        let seed = b"some-seed";
        let a = ChaosIdentity::for_domain(seed, "example.com");
        let b = ChaosIdentity::for_domain(seed, "example.com");
        assert_eq!(a.hardware, b.hardware);
        assert_eq!(a.os, b.os);
    }

    #[test]
    fn different_domains_can_differ() {
        let seed = b"some-seed";
        let identities: Vec<_> = ["a.com", "b.com", "c.com", "d.com", "e.com"]
            .iter()
            .map(|h| ChaosIdentity::for_domain(seed, h))
            .collect();
        let all_same = identities
            .windows(2)
            .all(|w| w[0].hardware == w[1].hardware && w[0].os == w[1].os);
        assert!(!all_same, "expected at least some variation across domains");
    }

    #[test]
    fn different_seed_can_change_the_pick() {
        let a = ChaosIdentity::for_domain(b"seed-one", "example.com");
        let b = ChaosIdentity::for_domain(b"seed-two", "example.com");
        // Not guaranteed to differ (both are drawn from the same list), but
        // over a handful of domains at least one should.
        let differs = (0..8).any(|i| {
            let host = format!("site{i}.example");
            let x = ChaosIdentity::for_domain(b"seed-one", &host);
            let y = ChaosIdentity::for_domain(b"seed-two", &host);
            x.hardware != y.hardware || x.os != y.os
        });
        assert!(differs);
        let _ = (a, b);
    }

    #[test]
    fn user_agent_still_looks_like_a_user_agent() {
        let identity = ChaosIdentity::for_domain(b"seed", "example.com");
        assert!(identity.user_agent().starts_with("Mozilla/5.0 ("));
    }
}
