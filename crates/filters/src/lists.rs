/// A single filter list the engine can be built from.
#[derive(Debug, Clone, Copy)]
pub struct FilterListSource {
    pub name: &'static str,
    pub url: &'static str,
}

/// Default set of lists: general ad blocking (EasyList) plus tracker/beacon
/// blocking (EasyPrivacy) — the two lists uBlock Origin/Brave use as their
/// baseline for "ads and privacy".
pub const DEFAULT_LISTS: &[FilterListSource] = &[
    FilterListSource {
        name: "easylist",
        url: "https://easylist.to/easylist/easylist.txt",
    },
    FilterListSource {
        name: "easyprivacy",
        url: "https://easylist.to/easylist/easyprivacy.txt",
    },
];
