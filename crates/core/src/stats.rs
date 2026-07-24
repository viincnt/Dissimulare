use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// How many recent blocks [`Stats::recent_blocks`] keeps around for a live
/// dashboard to display — old enough to feel like a scrolling log, bounded
/// so it can't grow forever on a long-running proxy.
const RECENT_BLOCKS_CAPACITY: usize = 200;

#[derive(Debug, Clone)]
pub struct BlockedRequest {
    pub url: String,
    pub at: Instant,
}

#[derive(Debug, Default)]
struct Counters {
    total: AtomicU64,
    blocked: AtomicU64,
    recent_blocks: Mutex<VecDeque<BlockedRequest>>,
}

/// Cheaply cloneable, shared request counters (and a bounded recent-blocks
/// log) for the `status` command and the TUI dashboard to read while the
/// proxy is running.
#[derive(Debug, Clone, Default)]
pub struct Stats(Arc<Counters>);

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_request(&self) {
        self.0.total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_blocked(&self, url: &str) {
        self.0.blocked.fetch_add(1, Ordering::Relaxed);

        let mut recent = self.0.recent_blocks.lock().unwrap();
        if recent.len() >= RECENT_BLOCKS_CAPACITY {
            recent.pop_front();
        }
        recent.push_back(BlockedRequest { url: url.to_string(), at: Instant::now() });
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            total_requests: self.0.total.load(Ordering::Relaxed),
            blocked_requests: self.0.blocked.load(Ordering::Relaxed),
        }
    }

    /// The most recent blocked requests, oldest first.
    pub fn recent_blocks(&self) -> Vec<BlockedRequest> {
        self.0.recent_blocks.lock().unwrap().iter().cloned().collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatsSnapshot {
    pub total_requests: u64,
    pub blocked_requests: u64,
}
