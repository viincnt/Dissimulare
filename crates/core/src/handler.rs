use crate::chaos::build_injection_script;
use crate::fingerprint::{FingerprintPolicy, ResolvedIdentity};
use crate::html::inject_after_head_open;
use crate::stats::Stats;
use crate::tracking_params::strip_tracking_params;
use crate::youtube;
use dissimulare_filters::FilterService;
use http::header::{ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, TRANSFER_ENCODING};
use http::{HeaderMap, HeaderValue, Request, Response, StatusCode};
use http_body_util::BodyExt;
use hudsucker::{Body, HttpContext, HttpHandler, RequestOrResponse};
use std::sync::{Arc, Mutex};

/// Whatever `handle_request` figures out that `handle_response` needs for
/// the same exchange. Handed off through a mutex; sound only because this
/// proxy never enables hudsucker's `http2` feature, so requests on one
/// connection — and therefore on one cloned handler — are always handled
/// one at a time.
#[derive(Default)]
struct PendingExchange {
    identity: Option<ResolvedIdentity>,
    is_youtube_player: bool,
}

/// The proxy's request pipeline: block known ads/trackers via the
/// `adblock-rust` engine, strip tracking query parameters, normalize
/// fingerprintable headers, strip ad-break scheduling out of YouTube's
/// player API responses, and — in chaos mode — inject a script that makes
/// the page's own JS agree with the absurd identity just sent over the
/// wire.
#[derive(Clone)]
pub struct DissimulareHandler {
    filters: FilterService,
    policy: Arc<FingerprintPolicy>,
    stats: Stats,
    strip_youtube_ads: bool,
    pending: Arc<Mutex<PendingExchange>>,
}

impl DissimulareHandler {
    pub fn new(filters: FilterService, policy: FingerprintPolicy, stats: Stats, strip_youtube_ads: bool) -> Self {
        Self {
            filters,
            policy: Arc::new(policy),
            stats,
            strip_youtube_ads,
            pending: Arc::new(Mutex::new(PendingExchange::default())),
        }
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }
}

impl HttpHandler for DissimulareHandler {
    async fn handle_request(&mut self, _ctx: &HttpContext, mut req: Request<Body>) -> RequestOrResponse {
        self.stats.record_request();

        let method = req.method().to_string();
        let request_type = resource_type_from_headers(req.headers());
        let source_url = req
            .headers()
            .get(http::header::REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let url = req.uri().to_string();

        let decision = self
            .filters
            .check(url.clone(), source_url, request_type, method)
            .await;

        if let Some(decision) = decision {
            if decision.blocked {
                self.stats.record_blocked();
                tracing::debug!(url = %url, request_type, "blocked");
                return synthetic_block_response(request_type, decision.redirect.as_deref()).into();
            }
            if let Some(rewritten) = decision.rewritten_url {
                if let Ok(uri) = rewritten.parse::<http::Uri>() {
                    *req.uri_mut() = uri;
                }
            }
        }

        if let Some(stripped) = strip_tracking_params(req.uri()) {
            *req.uri_mut() = stripped;
        }

        let uri = req.uri().clone();
        let identity = self.policy.apply(req.headers_mut(), &uri);
        let is_youtube_player = self.strip_youtube_ads && youtube::is_player_endpoint(&uri);

        // Chaos identities are only worth injecting into actual navigations,
        // and only chaos mode needs the body at all — asking every image/
        // script/XHR response to arrive uncompressed too would just waste
        // bandwidth for no benefit. The YouTube player response needs the
        // same treatment so its JSON body can be read as-is, with no
        // gzip/br decoding of our own to deal with.
        let wants_injection = matches!(request_type, "document" | "sub_frame")
            && identity.as_ref().is_some_and(|i| i.chaos.is_some());
        if wants_injection || is_youtube_player {
            req.headers_mut().insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        }

        *self.pending.lock().unwrap() = PendingExchange { identity, is_youtube_player };

        req.into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        let pending = std::mem::take(&mut *self.pending.lock().unwrap());

        if pending.is_youtube_player {
            return strip_youtube_ads(res).await;
        }

        let Some(chaos) = pending.identity.as_ref().and_then(|i| i.chaos.as_ref()) else {
            return res;
        };

        let is_html = res
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.to_ascii_lowercase().contains("text/html"))
            .unwrap_or(false);
        if !is_html {
            return res;
        }

        let (mut parts, body) = res.into_parts();
        let original = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err) => {
                tracing::warn!(error = %err, "failed to read response body for injection");
                return Response::from_parts(parts, Body::empty());
            }
        };

        let script = build_injection_script(chaos, &pending.identity.as_ref().unwrap().user_agent);
        let final_body = inject_after_head_open(&original, &script).unwrap_or_else(|| original.to_vec());

        parts.headers.remove(CONTENT_LENGTH);
        parts.headers.remove(TRANSFER_ENCODING);
        parts
            .headers
            .insert(CONTENT_LENGTH, HeaderValue::from_str(&final_body.len().to_string()).unwrap());

        Response::from_parts(parts, Body::from(final_body))
    }
}

/// Reads a YouTube player-API response and, if it's JSON carrying any of
/// the known ad-break keys, rewrites it without them. Falls back to
/// passing the response through untouched for anything unexpected (wrong
/// content-type, a body read error, non-JSON, or JSON with none of the
/// keys) — a format change on YouTube's end should degrade to "ads come
/// back", never to a broken player.
async fn strip_youtube_ads(res: Response<Body>) -> Response<Body> {
    let is_json = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.to_ascii_lowercase().contains("json"))
        .unwrap_or(false);
    if !is_json {
        return res;
    }

    let (mut parts, body) = res.into_parts();
    let original = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => {
            tracing::warn!(error = %err, "failed to read YouTube player response body");
            return Response::from_parts(parts, Body::empty());
        }
    };

    let Some(stripped) = youtube::strip_ad_metadata(&original) else {
        return Response::from_parts(parts, Body::from(original));
    };

    tracing::debug!("stripped ad-break metadata from a YouTube player response");
    parts.headers.remove(CONTENT_LENGTH);
    parts.headers.remove(TRANSFER_ENCODING);
    parts
        .headers
        .insert(CONTENT_LENGTH, HeaderValue::from_str(&stripped.len().to_string()).unwrap());
    Response::from_parts(parts, Body::from(stripped))
}

/// Maps the standard `Sec-Fetch-Dest` header (sent by every modern browser)
/// to the resource-type vocabulary `adblock-rust`'s filter lists expect.
/// Falls back to `"other"` for requests that don't send it (some non-browser
/// clients, or older navigations).
fn resource_type_from_headers(headers: &HeaderMap) -> &'static str {
    match headers
        .get("sec-fetch-dest")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
    {
        "document" => "document",
        "iframe" | "frame" => "sub_frame",
        "script" | "worker" | "sharedworker" | "serviceworker" => "script",
        "style" => "stylesheet",
        "image" => "image",
        "font" => "font",
        "audio" | "video" | "track" => "media",
        "object" | "embed" => "object",
        "empty" => "xmlhttprequest",
        _ => "other",
    }
}

fn synthetic_block_response(request_type: &str, redirect: Option<&str>) -> Response<Body> {
    if let Some(resource) = redirect {
        tracing::debug!(
            resource,
            "redirect-resource requested but no resource library is loaded yet; \
             falling back to a generic block response"
        );
    }
    empty_block_response(request_type)
}

/// A body-shaped no-op response per resource type, so pages that expect a
/// blocked script/image to still "exist" don't throw load errors — the same
/// approach uBlock Origin uses instead of returning hard failures.
fn empty_block_response(request_type: &str) -> Response<Body> {
    let builder = Response::builder();
    let response = match request_type {
        "image" => builder
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "image/gif")
            .body(Body::from(TRANSPARENT_GIF)),
        "script" => builder
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/javascript")
            .body(Body::empty()),
        "stylesheet" => builder
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/css")
            .body(Body::empty()),
        _ => builder.status(StatusCode::NO_CONTENT).body(Body::empty()),
    };
    response.expect("static block response is always valid")
}

const TRANSPARENT_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3B,
];
