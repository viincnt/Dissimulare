//! Shared "does this host fall under that domain (or one of its
//! subdomains)?" check, used by both chaos-mode exceptions and the
//! no-intercept bypass list — anywhere a user configures a plain list of
//! domains that should each also cover their subdomains.

/// Whether `host` is `domain` itself or a subdomain of it (case-insensitive).
pub fn matches_domain(host: &str, domain: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let domain = domain.to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Whether `host` matches any entry in `domains`.
pub fn matches_any(host: &str, domains: &[String]) -> bool {
    domains.iter().any(|domain| matches_domain(host, domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_and_subdomains_case_insensitively() {
        assert!(matches_domain("Google.com", "google.com"));
        assert!(matches_domain("www.google.com", "google.com"));
        assert!(matches_domain("accounts.google.com", "GOOGLE.COM"));
        assert!(!matches_domain("notgoogle.com", "google.com"));
        assert!(!matches_domain("google.com.evil.example", "google.com"));
    }

    #[test]
    fn matches_any_checks_every_entry() {
        let domains = vec!["a.example".to_string(), "b.example".to_string()];
        assert!(matches_any("sub.b.example", &domains));
        assert!(!matches_any("c.example", &domains));
    }
}
