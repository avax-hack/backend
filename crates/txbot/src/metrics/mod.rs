use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Tracks transaction bot operation counters.
///
/// All counters use `AtomicU64` for lock-free concurrent access.
pub struct TxBotMetrics {
    pub graduate_attempts: AtomicU64,
    pub graduate_successes: AtomicU64,
    pub graduate_failures: AtomicU64,
    pub collect_attempts: AtomicU64,
    pub collect_successes: AtomicU64,
    pub collect_failures: AtomicU64,
}

impl TxBotMetrics {
    pub fn new() -> Self {
        Self {
            graduate_attempts: AtomicU64::new(0),
            graduate_successes: AtomicU64::new(0),
            graduate_failures: AtomicU64::new(0),
            collect_attempts: AtomicU64::new(0),
            collect_successes: AtomicU64::new(0),
            collect_failures: AtomicU64::new(0),
        }
    }

    pub fn record_graduate_attempt(&self) {
        self.graduate_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_graduate_success(&self) {
        self.graduate_successes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_graduate_failure(&self) {
        self.graduate_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_collect_attempt(&self) {
        self.collect_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_collect_success(&self) {
        self.collect_successes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_collect_failure(&self) {
        self.collect_failures.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            graduate_attempts: self.graduate_attempts.load(Ordering::Relaxed),
            graduate_successes: self.graduate_successes.load(Ordering::Relaxed),
            graduate_failures: self.graduate_failures.load(Ordering::Relaxed),
            collect_attempts: self.collect_attempts.load(Ordering::Relaxed),
            collect_successes: self.collect_successes.load(Ordering::Relaxed),
            collect_failures: self.collect_failures.load(Ordering::Relaxed),
        }
    }
}

/// A point-in-time snapshot of all metrics counters.
#[derive(Debug)]
struct MetricsSnapshot {
    graduate_attempts: u64,
    graduate_successes: u64,
    graduate_failures: u64,
    collect_attempts: u64,
    collect_successes: u64,
    collect_failures: u64,
}

/// Spawn a background task that logs metrics every 60 seconds.
pub fn spawn_reporter(metrics: Arc<TxBotMetrics>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_secs(60);
        loop {
            tokio::time::sleep(interval).await;

            let snap = metrics.snapshot();
            tracing::info!(
                graduate_attempts = snap.graduate_attempts,
                graduate_successes = snap.graduate_successes,
                graduate_failures = snap.graduate_failures,
                collect_attempts = snap.collect_attempts,
                collect_successes = snap.collect_successes,
                collect_failures = snap.collect_failures,
                "TxBot metrics report"
            );
        }
    })
}
