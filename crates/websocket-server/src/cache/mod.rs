use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Cached price snapshot for a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub token_address: String,
    pub price: String,
    pub updated_at: i64,
}

/// In-memory cache for the latest price per token.
/// Used by the price event producer to provide initial state on new subscriptions.
pub struct PriceCache {
    prices: DashMap<String, PriceSnapshot>,
}

impl PriceCache {
    pub fn new() -> Self {
        Self {
            prices: DashMap::new(),
        }
    }

    /// Update the cached price for a token.
    pub fn set_price(&self, token_address: &str, price: String) -> PriceSnapshot {
        let snapshot = PriceSnapshot {
            token_address: token_address.to_lowercase(),
            price,
            updated_at: openlaunch_shared::types::common::current_unix_timestamp(),
        };
        self.prices
            .insert(token_address.to_lowercase(), snapshot.clone());
        snapshot
    }

    /// Get the latest cached price for a token, if available.
    pub fn get_price(&self, token_address: &str) -> Option<PriceSnapshot> {
        self.prices
            .get(&token_address.to_lowercase())
            .map(|entry| entry.clone())
    }

    /// Remove a token's cached price.
    pub fn remove_price(&self, token_address: &str) -> Option<PriceSnapshot> {
        self.prices
            .remove(&token_address.to_lowercase())
            .map(|(_, v)| v)
    }

    /// Returns the number of cached prices.
    pub fn len(&self) -> usize {
        self.prices.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_cache_set_and_get() {
        let cache = PriceCache::new();
        cache.set_price("0xABC", "1.25".to_string());

        let snapshot = cache.get_price("0xabc").unwrap();
        assert_eq!(snapshot.price, "1.25");
        assert_eq!(snapshot.token_address, "0xabc");
    }

    #[test]
    fn test_price_cache_missing() {
        let cache = PriceCache::new();
        assert!(cache.get_price("0xnope").is_none());
    }

    #[test]
    fn test_price_cache_update() {
        let cache = PriceCache::new();
        cache.set_price("0xABC", "1.00".to_string());
        cache.set_price("0xabc", "2.00".to_string());

        let snapshot = cache.get_price("0xabc").unwrap();
        assert_eq!(snapshot.price, "2.00");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_price_cache_remove() {
        let cache = PriceCache::new();
        cache.set_price("0xABC", "1.00".to_string());
        let removed = cache.remove_price("0xabc").unwrap();
        assert_eq!(removed.price, "1.00");
        assert!(cache.is_empty());
    }
}
