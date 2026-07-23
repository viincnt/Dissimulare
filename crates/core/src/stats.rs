use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Default)]
struct Counters {
    total: AtomicU64,
    blocked: AtomicU64,
}

/// Cheaply cloneable, shared request counters for the `status` command / a
/// future UI to read while the proxy is running.
#[derive(Debug, Clone, Default)]
pub struct Stats(Arc<Counters>);

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_request(&self) {
        self.0.total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_blocked(&self) {
        self.0.blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            total_requests: self.0.total.load(Ordering::Relaxed),
            blocked_requests: self.0.blocked.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatsSnapshot {
    pub total_requests: u64,
    pub blocked_requests: u64,
}
