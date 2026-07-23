//! The MITM proxy pipeline itself: a `hudsucker::HttpHandler` that blocks
//! ads/trackers via `dissimulare-filters`, strips tracking query
//! parameters, and normalizes fingerprintable headers — including, in
//! chaos mode, feeding every domain a different absurd hardware/OS
//! identity instead of trying to blend into a crowd.

mod chaos;
mod fingerprint;
mod handler;
mod html;
mod stats;
mod tracking_params;

pub use fingerprint::{FingerprintPolicy, IdentityMode, ResolvedIdentity, DEFAULT_USER_AGENT};
pub use handler::DissimulareHandler;
pub use stats::{Stats, StatsSnapshot};
pub use tracking_params::strip_tracking_params;
