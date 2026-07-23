//! adblock-rust integration: default filter lists, on-disk download/cache
//! with periodic refresh, and the compiled engine used to decide whether a
//! given network request should be blocked.

mod engine;
mod lists;
mod manager;
mod service;

pub use engine::BlockDecision;
pub use lists::{FilterListSource, DEFAULT_LISTS};
pub use manager::{EngineSource, FilterManager};
pub use service::FilterService;
