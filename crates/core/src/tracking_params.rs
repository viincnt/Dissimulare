use http::Uri;

/// Query parameter names (case-insensitive) known to exist purely to track
/// users across clicks/campaigns, not to affect what content is served.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "msclkid",
    "mc_eid",
    "mc_cid",
    "igshid",
    "ref_src",
    "yclid",
    "twclid",
    "vero_id",
    "_hsenc",
    "_hsmi",
    "mkt_tok",
    "oly_anon_id",
    "oly_enc_id",
    "wickedid",
    "epik",
];

/// Strips known tracking query parameters from `uri`. Returns `None` when
/// there was nothing to strip, so callers can skip rebuilding the request.
pub fn strip_tracking_params(uri: &Uri) -> Option<Uri> {
    let query = uri.query()?;
    if !query.split('&').any(is_tracking_param) {
        return None;
    }

    let kept: Vec<&str> = query.split('&').filter(|p| !is_tracking_param(p)).collect();

    let path = uri.path();
    let new_path_and_query = if kept.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", kept.join("&"))
    };

    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(new_path_and_query.parse().ok()?);
    Uri::from_parts(parts).ok()
}

fn is_tracking_param(pair: &str) -> bool {
    let name = pair.split('=').next().unwrap_or(pair);
    TRACKING_PARAMS.iter().any(|t| t.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_known_tracking_params_and_keeps_the_rest() {
        let uri: Uri = "https://example.com/page?id=42&utm_source=newsletter&fbclid=abc"
            .parse()
            .unwrap();

        let stripped = strip_tracking_params(&uri).expect("should strip something");
        assert_eq!(stripped, "https://example.com/page?id=42");
    }

    #[test]
    fn returns_none_when_nothing_to_strip() {
        let uri: Uri = "https://example.com/page?id=42".parse().unwrap();
        assert!(strip_tracking_params(&uri).is_none());
    }

    #[test]
    fn drops_query_entirely_when_everything_was_tracking() {
        let uri: Uri = "https://example.com/page?utm_source=x&utm_medium=y".parse().unwrap();
        let stripped = strip_tracking_params(&uri).unwrap();
        assert_eq!(stripped, "https://example.com/page");
    }
}
