//! Removes ad-break scheduling metadata from YouTube's internal player API
//! response before it reaches the browser, so the player itself never
//! learns there's an ad to insert — the same idea uBlock Origin/Brave rely
//! on for YouTube, just done as a response-body rewrite here instead of a
//! page-side scriptlet patching the same JSON after the fact.
//!
//! This only removes *scheduling* metadata, not video content: formats
//! where the ad is stitched directly into the video stream bytes (no
//! separate signal to remove) aren't affected by this and aren't blockable
//! by any request/response-level technique.

use http::Uri;

/// Whether `uri` looks like a call to YouTube's internal player API — the
/// endpoint whose JSON response carries ad-break scheduling alongside the
/// actual video/caption/streaming data.
pub fn is_player_endpoint(uri: &Uri) -> bool {
    let host = uri.host().unwrap_or("");
    let is_youtube = host == "youtube.com" || host.ends_with(".youtube.com");
    is_youtube && uri.path().contains("/youtubei/v1/player")
}

/// Top-level keys in the player response that carry ad-break scheduling.
/// Removing them doesn't touch `streamingData`/captions/video metadata —
/// just the information the player uses to decide *when* to cut to an ad.
const AD_METADATA_KEYS: &[&str] = &["adPlacements", "adSlots", "adBreakHeartbeatParams", "playerAds"];

/// Strips ad-break metadata from a player response body, returning the
/// rewritten JSON. Returns `None` (meaning: leave the original body
/// untouched) if the body isn't the JSON object shape this expects, or if
/// none of the known keys were present — a format change on YouTube's end
/// should degrade to "ads come back", never to a broken player.
pub fn strip_ad_metadata(body: &[u8]) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let object = value.as_object_mut()?;

    let mut removed_any = false;
    for key in AD_METADATA_KEYS {
        if object.remove(*key).is_some() {
            removed_any = true;
        }
    }
    if !removed_any {
        return None;
    }

    serde_json::to_vec(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(s: &str) -> Uri {
        s.parse().unwrap()
    }

    #[test]
    fn recognizes_the_player_endpoint_on_any_youtube_subdomain() {
        assert!(is_player_endpoint(&uri("https://www.youtube.com/youtubei/v1/player?key=abc")));
        assert!(is_player_endpoint(&uri("https://m.youtube.com/youtubei/v1/player")));
        assert!(!is_player_endpoint(&uri("https://www.youtube.com/youtubei/v1/next")));
        assert!(!is_player_endpoint(&uri("https://example.com/youtubei/v1/player")));
    }

    #[test]
    fn strips_known_ad_keys_and_leaves_everything_else() {
        let body = br#"{"adPlacements":[{"x":1}],"playerAds":[1,2],"videoDetails":{"title":"hi"}}"#;
        let stripped = strip_ad_metadata(body).expect("should strip");
        let value: serde_json::Value = serde_json::from_slice(&stripped).unwrap();
        assert!(value.get("adPlacements").is_none());
        assert!(value.get("playerAds").is_none());
        assert_eq!(value["videoDetails"]["title"], "hi");
    }

    #[test]
    fn leaves_bodies_without_ad_keys_untouched() {
        let body = br#"{"videoDetails":{"title":"hi"}}"#;
        assert!(strip_ad_metadata(body).is_none());
    }

    #[test]
    fn degrades_gracefully_on_non_json_or_non_object_bodies() {
        assert!(strip_ad_metadata(b"not json").is_none());
        assert!(strip_ad_metadata(b"[1,2,3]").is_none());
    }
}
