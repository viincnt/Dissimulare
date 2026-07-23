use adblock::engine::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use anyhow::{anyhow, Result};

/// Outcome of checking a single network request against the filter engine.
#[derive(Debug, Clone)]
pub struct BlockDecision {
    /// The request should be dropped/short-circuited entirely.
    pub blocked: bool,
    /// A substitute resource to serve instead (from a `$redirect=` rule),
    /// when one is registered and matched. Not all blocked requests have one.
    pub redirect: Option<String>,
    /// The URL with tracking query parameters stripped by a `$removeparam`
    /// rule; present even for requests that are not otherwise blocked.
    pub rewritten_url: Option<String>,
}

/// Thin wrapper around adblock-rust's `Engine`, scoped to exactly what the
/// proxy handler needs: "should this request be blocked, and with what".
pub struct FilterEngine {
    engine: Engine,
}

impl FilterEngine {
    /// Compiles a fresh engine from raw filter-list text (one string per list).
    pub fn build(list_texts: impl IntoIterator<Item = String>) -> Self {
        let mut set = FilterSet::new(false);
        for text in list_texts {
            set.add_filter_list(text, ParseOptions::default());
        }
        let engine = Engine::new_with_filter_set(set);
        Self { engine }
    }

    /// Loads a previously serialized engine, skipping the cost of
    /// re-parsing filter list text on every startup.
    pub fn load_cached(bytes: &[u8]) -> Result<Self> {
        let mut engine = Engine::default();
        engine
            .deserialize(bytes)
            .map_err(|e| anyhow!("corrupted filter engine cache: {e:?}"))?;
        Ok(Self { engine })
    }

    /// Serializes the engine for on-disk caching.
    pub fn to_cache_bytes(&self) -> Vec<u8> {
        self.engine.serialize()
    }

    /// Checks a single network request.
    ///
    /// `request_type` follows adblock-rust's resource-type strings
    /// (`"document"`, `"script"`, `"image"`, `"xmlhttprequest"`, `"sub_frame"`, ...).
    pub fn check(
        &self,
        url: &str,
        source_url: &str,
        request_type: &str,
        method: &str,
    ) -> Option<BlockDecision> {
        let request = Request::new(url, source_url, request_type, method).ok()?;
        let result = self.engine.check_network_request(&request);

        let blocked = result.should_block();
        if !blocked && result.redirect.is_none() && result.rewritten_url.is_none() {
            return None;
        }

        Some(BlockDecision {
            blocked,
            redirect: result.redirect,
            rewritten_url: result.rewritten_url,
        })
    }
}
