use std::sync::atomic::{AtomicU64, Ordering};

pub struct Metrics {
    pub db_queries: AtomicU64,
    pub db_errors: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub rpc_requests: AtomicU64,
    pub rpc_errors: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            db_queries: AtomicU64::new(0),
            db_errors: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            rpc_requests: AtomicU64::new(0),
            rpc_errors: AtomicU64::new(0),
        }
    }

    pub fn record_db_query(&self) {
        self.db_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_db_error(&self) {
        self.db_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rpc_request(&self) {
        self.rpc_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rpc_error(&self) {
        self.rpc_errors.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_new_all_zero() {
        let m = Metrics::new();
        assert_eq!(m.db_queries.load(Ordering::Relaxed), 0);
        assert_eq!(m.db_errors.load(Ordering::Relaxed), 0);
        assert_eq!(m.cache_hits.load(Ordering::Relaxed), 0);
        assert_eq!(m.cache_misses.load(Ordering::Relaxed), 0);
        assert_eq!(m.rpc_requests.load(Ordering::Relaxed), 0);
        assert_eq!(m.rpc_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_default_same_as_new() {
        let m = Metrics::default();
        assert_eq!(m.db_queries.load(Ordering::Relaxed), 0);
        assert_eq!(m.rpc_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_db_query() {
        let m = Metrics::new();
        m.record_db_query();
        m.record_db_query();
        m.record_db_query();
        assert_eq!(m.db_queries.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_record_db_error() {
        let m = Metrics::new();
        m.record_db_error();
        assert_eq!(m.db_errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_record_cache_hit() {
        let m = Metrics::new();
        m.record_cache_hit();
        m.record_cache_hit();
        assert_eq!(m.cache_hits.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_record_cache_miss() {
        let m = Metrics::new();
        m.record_cache_miss();
        assert_eq!(m.cache_misses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_record_rpc_request() {
        let m = Metrics::new();
        for _ in 0..5 {
            m.record_rpc_request();
        }
        assert_eq!(m.rpc_requests.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_record_rpc_error() {
        let m = Metrics::new();
        m.record_rpc_error();
        m.record_rpc_error();
        assert_eq!(m.rpc_errors.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_metrics_independent_counters() {
        let m = Metrics::new();
        m.record_db_query();
        m.record_cache_hit();
        m.record_rpc_request();
        assert_eq!(m.db_queries.load(Ordering::Relaxed), 1);
        assert_eq!(m.db_errors.load(Ordering::Relaxed), 0);
        assert_eq!(m.cache_hits.load(Ordering::Relaxed), 1);
        assert_eq!(m.cache_misses.load(Ordering::Relaxed), 0);
        assert_eq!(m.rpc_requests.load(Ordering::Relaxed), 1);
        assert_eq!(m.rpc_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_large_count() {
        let m = Metrics::new();
        for _ in 0..1000 {
            m.record_db_query();
        }
        assert_eq!(m.db_queries.load(Ordering::Relaxed), 1000);
    }
}
