use crate::engine::{BlockDecision, FilterEngine};
use crate::manager::EngineSource;
use tokio::sync::{mpsc, oneshot};

struct CheckJob {
    url: String,
    source_url: String,
    request_type: &'static str,
    method: String,
    respond_to: oneshot::Sender<Option<BlockDecision>>,
}

/// Runs `adblock-rust`'s engine on a dedicated OS thread and exposes a
/// `Send + Sync + Clone` handle any async task can use to query it.
///
/// `adblock::Engine` uses `Rc`/`RefCell` internally and is neither `Send`
/// nor `Sync`, so it can't be shared (or even moved) across threads the way
/// `hudsucker::HttpHandler` requires. Instead of fighting that, it gets
/// exactly one thread to live on and is talked to over a channel — the
/// engine itself is built on that thread and never crosses a thread boundary.
#[derive(Clone)]
pub struct FilterService {
    tx: mpsc::UnboundedSender<CheckJob>,
}

impl FilterService {
    pub fn spawn(source: EngineSource) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<CheckJob>();

        std::thread::Builder::new()
            .name("dissimulare-filter-engine".into())
            .spawn(move || {
                let engine = build_engine(source);
                while let Some(job) = rx.blocking_recv() {
                    let decision =
                        engine.check(&job.url, &job.source_url, job.request_type, &job.method);
                    let _ = job.respond_to.send(decision);
                }
            })
            .expect("failed to spawn the filter engine thread");

        Self { tx }
    }

    pub async fn check(
        &self,
        url: String,
        source_url: String,
        request_type: &'static str,
        method: String,
    ) -> Option<BlockDecision> {
        let (respond_to, response) = oneshot::channel();
        let job = CheckJob { url, source_url, request_type, method, respond_to };
        if self.tx.send(job).is_err() {
            return None;
        }
        response.await.ok().flatten()
    }
}

fn build_engine(source: EngineSource) -> FilterEngine {
    match source {
        EngineSource::Cached(bytes) => FilterEngine::load_cached(&bytes).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "cached filter engine unreadable, starting with an empty engine");
            FilterEngine::build(std::iter::empty::<String>())
        }),
        EngineSource::Build { texts, cache_path } => {
            let engine = FilterEngine::build(texts);
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(err) = std::fs::write(&cache_path, engine.to_cache_bytes()) {
                tracing::warn!(
                    error = %err,
                    path = %cache_path.display(),
                    "failed to write filter engine cache"
                );
            }
            engine
        }
    }
}
