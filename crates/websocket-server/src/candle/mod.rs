use dashmap::DashMap;
use sqlx::PgPool;

/// Supported candle intervals: `(label, seconds)`.
pub const INTERVALS: [(&str, i64); 6] = [
    ("1m", 60),
    ("5m", 300),
    ("15m", 900),
    ("1h", 3600),
    ("4h", 14400),
    ("1d", 86400),
];

/// OHLCV candle for a single time bucket.
#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Thread-safe in-memory OHLCV candle store keyed by `(token_id, interval)`.
pub struct CandleManager {
    candles: DashMap<(String, String), Candle>,
}

impl CandleManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            candles: DashMap::new(),
        }
    }

    /// Update candles for all intervals given a new trade tick.
    ///
    /// - Same time bucket: `high = max`, `low = min`, `close = price`, `volume += volume`.
    ///   `open` is never changed.
    /// - New time bucket: `open = previous close`, reset candle.
    pub fn update(&self, token_id: &str, price: f64, volume: f64, timestamp: i64) {
        let token_lower = token_id.to_lowercase();
        for &(interval_label, interval_secs) in &INTERVALS {
            let bucket_time = timestamp - (timestamp % interval_secs);
            let key = (token_lower.clone(), interval_label.to_string());

            self.candles
                .entry(key)
                .and_modify(|existing| {
                    if existing.time == bucket_time {
                        // Same bucket — update in place (conceptually; DashMap gives &mut).
                        existing.high = existing.high.max(price);
                        existing.low = existing.low.min(price);
                        existing.close = price;
                        existing.volume += volume;
                    } else {
                        // New bucket — open from previous close.
                        *existing = Candle {
                            time: bucket_time,
                            open: existing.close,
                            high: existing.close.max(price),
                            low: existing.close.min(price),
                            close: price,
                            volume,
                        };
                    }
                })
                .or_insert_with(|| Candle {
                    time: bucket_time,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                });
        }
    }

    /// Retrieve the current candle for a token and interval.
    #[must_use]
    pub fn get(&self, token_id: &str, interval: &str) -> Option<Candle> {
        let key = (token_id.to_lowercase(), interval.to_string());
        self.candles.get(&key).map(|entry| *entry)
    }

    /// Return the supported interval definitions.
    #[must_use]
    pub fn intervals() -> &'static [(&'static str, i64)] {
        &INTERVALS
    }

    /// Load the latest candle per (token_id, interval) from the database.
    /// Called once at startup so that candles survive server restarts.
    pub async fn load_from_db(&self, db: &PgPool) {
        let rows = sqlx::query_as::<_, LatestCandleRow>(
            r#"
            SELECT DISTINCT ON (token_id, interval)
                   token_id, interval, time,
                   open::TEXT, high::TEXT, low::TEXT, close::TEXT, volume::TEXT
            FROM charts
            ORDER BY token_id, interval, time DESC
            "#,
        )
        .fetch_all(db)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "Failed to load candles from database, starting with empty state");
            Vec::new()
        });

        let mut count = 0u64;
        for row in rows {
            let open: f64 = row.open.parse().unwrap_or(0.0);
            let high: f64 = row.high.parse().unwrap_or(0.0);
            let low: f64 = row.low.parse().unwrap_or(0.0);
            let close: f64 = row.close.parse().unwrap_or(0.0);
            let volume: f64 = row.volume.parse().unwrap_or(0.0);

            let key = (row.token_id.to_lowercase(), row.interval);
            self.candles.insert(key, Candle {
                time: row.time,
                open,
                high,
                low,
                close,
                volume,
            });
            count += 1;
        }

        tracing::info!(count, "Loaded candles from database");
    }
}

impl Default for CandleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, sqlx::FromRow)]
struct LatestCandleRow {
    token_id: String,
    interval: String,
    time: i64,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_creates_new_candle() {
        let mgr = CandleManager::new();
        mgr.update("token_a", 100.0, 10.0, 1_000_000);

        let candle = mgr.get("token_a", "1m").expect("candle should exist");
        assert_eq!(candle.open, 100.0);
        assert_eq!(candle.high, 100.0);
        assert_eq!(candle.low, 100.0);
        assert_eq!(candle.close, 100.0);
        assert_eq!(candle.volume, 10.0);
    }

    #[test]
    fn test_update_existing_candle_same_bucket() {
        let mgr = CandleManager::new();
        // Both timestamps fall in the same 1-minute bucket (bucket starts at 1_000_020 / 60 * 60 = 999_960).
        let ts = 999_960; // exactly on a 1m boundary
        mgr.update("token_a", 100.0, 10.0, ts);
        mgr.update("token_a", 110.0, 5.0, ts + 10);
        mgr.update("token_a", 90.0, 3.0, ts + 20);

        let candle = mgr.get("token_a", "1m").expect("candle should exist");
        assert_eq!(candle.open, 100.0, "open must not change");
        assert_eq!(candle.high, 110.0, "high = max of all prices");
        assert_eq!(candle.low, 90.0, "low = min of all prices");
        assert_eq!(candle.close, 90.0, "close = latest price");
        assert_eq!(candle.volume, 18.0, "volume accumulated");
    }

    #[test]
    fn test_update_new_bucket_resets_candle() {
        let mgr = CandleManager::new();
        let bucket1_ts = 60; // 1m bucket starting at 60
        mgr.update("token_a", 100.0, 10.0, bucket1_ts);

        // Next 1m bucket.
        let bucket2_ts = 120;
        mgr.update("token_a", 105.0, 7.0, bucket2_ts);

        let candle = mgr.get("token_a", "1m").expect("candle should exist");
        assert_eq!(candle.time, 120, "new bucket time");
        assert_eq!(candle.open, 100.0, "open = previous close");
        assert_eq!(candle.high, 105.0);
        assert_eq!(candle.low, 100.0, "low = min(prev_close, new_price)");
        assert_eq!(candle.close, 105.0);
        assert_eq!(candle.volume, 7.0, "volume reset for new bucket");
    }

    #[test]
    fn test_all_six_intervals_updated() {
        let mgr = CandleManager::new();
        mgr.update("token_b", 50.0, 1.0, 100_000);

        for &(label, _) in &INTERVALS {
            assert!(
                mgr.get("token_b", label).is_some(),
                "candle missing for interval {label}"
            );
        }
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = CandleManager::new();
        assert!(mgr.get("no_such_token", "1m").is_none());
    }
}
