/// A single filter list the engine can be built from.
#[derive(Debug, Clone, Copy)]
pub struct FilterListSource {
    pub name: &'static str,
    pub url: &'static str,
}

/// Default set of lists: general ad blocking (EasyList), tracker/beacon
/// blocking (EasyPrivacy), uBlock Origin's own supplementary rules (broader
/// site-specific coverage, e.g. YouTube), cookie-notice removal (Fanboy's
/// Cookiemonster), and uBO's other-annoyances list (anti-adblock warnings,
/// forced modals, etc.).
pub const DEFAULT_LISTS: &[FilterListSource] = &[
    FilterListSource {
        name: "easylist",
        url: "https://easylist.to/easylist/easylist.txt",
    },
    FilterListSource {
        name: "easyprivacy",
        url: "https://easylist.to/easylist/easyprivacy.txt",
    },
    FilterListSource {
        name: "ublock-filters",
        url: "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/filters.txt",
    },
    FilterListSource {
        name: "fanboy-cookiemonster",
        url: "https://secure.fanboy.co.nz/fanboy-cookiemonster.txt",
    },
    FilterListSource {
        name: "ubo-annoyances-other",
        url: "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/annoyances-others.txt",
    },
];
