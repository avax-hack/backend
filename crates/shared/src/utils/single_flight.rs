use moka::future::Cache;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// SingleFlight cache that deduplicates concurrent identical requests.
/// Uses Moka's `try_get_with` to ensure only one request is made
/// for the same key, even with thousands of concurrent callers.
pub struct SingleFlightCache {
    cache: Cache<String, Arc<String>>,
}

impl SingleFlightCache {
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self { cache }
    }

    /// Get a value from cache, or compute it if not present.
    /// Concurrent requests for the same key will wait for the first
    /// computation to complete and share the result.
    pub async fn get_or_insert<T, F, Fut>(
        &self,
        key: &str,
        f: F,
    ) -> anyhow::Result<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let key = key.to_string();
        let result = self
            .cache
            .try_get_with(key, async {
                let value = f().await?;
                let json = serde_json::to_string(&value)?;
                Ok::<_, anyhow::Error>(Arc::new(json))
            })
            .await
            .map_err(|e| anyhow::anyhow!("SingleFlight error: {e}"))?;

        let value: T = serde_json::from_str(&result)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_flight_cache() {
        let cache = SingleFlightCache::new(100, Duration::from_secs(1));

        let result: String = cache
            .get_or_insert("key1", || async { Ok("hello".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "hello");

        // Second call should return cached value
        let result2: String = cache
            .get_or_insert("key1", || async { Ok("world".to_string()) })
            .await
            .unwrap();
        assert_eq!(result2, "hello"); // Still "hello" from cache
    }

    #[tokio::test]
    async fn test_single_flight_different_keys() {
        let cache = SingleFlightCache::new(100, Duration::from_secs(10));

        let r1: String = cache
            .get_or_insert("key_a", || async { Ok("alpha".to_string()) })
            .await
            .unwrap();
        let r2: String = cache
            .get_or_insert("key_b", || async { Ok("beta".to_string()) })
            .await
            .unwrap();
        assert_eq!(r1, "alpha");
        assert_eq!(r2, "beta");
    }

    #[tokio::test]
    async fn test_single_flight_numeric_type() {
        let cache = SingleFlightCache::new(100, Duration::from_secs(10));

        let result: i64 = cache
            .get_or_insert("num_key", || async { Ok(42i64) })
            .await
            .unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_single_flight_struct_type() {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let cache = SingleFlightCache::new(100, Duration::from_secs(10));
        let data = TestData {
            name: "test".to_string(),
            value: 99,
        };
        let expected = data.clone();

        let result: TestData = cache
            .get_or_insert("struct_key", || async { Ok(data) })
            .await
            .unwrap();
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_single_flight_error_propagation() {
        let cache = SingleFlightCache::new(100, Duration::from_secs(10));

        let result: Result<String, _> = cache
            .get_or_insert("err_key", || async {
                Err(anyhow::anyhow!("computation failed"))
            })
            .await;
        assert!(result.is_err());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_single_flight_ttl_expiry() {
        let cache = SingleFlightCache::new(100, Duration::from_millis(50));

        let r1: String = cache
            .get_or_insert("ttl_key", || async { Ok("first".to_string()) })
            .await
            .unwrap();
        assert_eq!(r1, "first");

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        let r2: String = cache
            .get_or_insert("ttl_key", || async { Ok("second".to_string()) })
            .await
            .unwrap();
        assert_eq!(r2, "second");
    }

    #[tokio::test]
    async fn test_single_flight_concurrent_dedup() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let cache = Arc::new(SingleFlightCache::new(100, Duration::from_secs(10)));
        let call_count = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let cache = cache.clone();
            let count = call_count.clone();
            handles.push(tokio::spawn(async move {
                let _result: String = cache
                    .get_or_insert("shared_key", || {
                        let count = count.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            Ok("shared_value".to_string())
                        }
                    })
                    .await
                    .unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // Moka's try_get_with should deduplicate: only 1 call (or very few)
        let calls = call_count.load(Ordering::SeqCst);
        assert!(calls <= 2, "Expected deduplication, but got {calls} calls");
    }

    #[tokio::test]
    async fn test_single_flight_vec_type() {
        let cache = SingleFlightCache::new(100, Duration::from_secs(10));

        let result: Vec<String> = cache
            .get_or_insert("vec_key", || async {
                Ok(vec!["a".to_string(), "b".to_string()])
            })
            .await
            .unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }
}
