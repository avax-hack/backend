pub mod api;
pub mod fallback;
pub mod health;
pub mod provider;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;

use self::provider::{ProviderId, ProviderState};

/// Multi-provider RPC client with health scoring and automatic failover.
pub struct RpcClient {
    pub providers: DashMap<ProviderId, ProviderState>,
    latest_block: AtomicU64,
}

impl RpcClient {
    pub fn new() -> Self {
        Self {
            providers: DashMap::new(),
            latest_block: AtomicU64::new(0),
        }
    }

    pub fn latest_block(&self) -> u64 {
        self.latest_block.load(Ordering::Relaxed)
    }

    pub fn set_latest_block(&self, block: u64) {
        self.latest_block.store(block, Ordering::Relaxed);
    }

    /// Select the provider with the highest health score.
    pub fn best_provider(&self) -> Option<ProviderId> {
        self.providers
            .iter()
            .max_by_key(|entry| entry.value().score())
            .map(|entry| entry.key().clone())
    }

    pub fn add_provider(&self, id: ProviderId, state: ProviderState) {
        self.providers.insert(id, state);
    }

    pub fn get_provider(&self, id: &ProviderId) -> Option<dashmap::mapref::one::Ref<ProviderId, ProviderState>> {
        self.providers.get(id)
    }

    pub fn penalize_provider(&self, id: &ProviderId) {
        if let Some(mut entry) = self.providers.get_mut(id) {
            entry.record_failure();
        }
    }

    pub fn reward_provider(&self, id: &ProviderId) {
        if let Some(mut entry) = self.providers.get_mut(id) {
            entry.record_success();
        }
    }

    pub async fn init(rpc_urls: Vec<(ProviderId, String)>) -> anyhow::Result<Arc<Self>> {
        let client = Arc::new(Self::new());

        for (id, url) in rpc_urls {
            if url.is_empty() {
                continue;
            }
            let state = ProviderState::new(&url, &id)?;
            client.add_provider(id, state);
        }

        if client.providers.is_empty() {
            anyhow::bail!("No RPC providers configured");
        }

        tracing::info!("RPC client initialized with {} providers", client.providers.len());
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider::ProviderState;

    #[test]
    fn test_rpc_client_new() {
        let client = RpcClient::new();
        assert_eq!(client.latest_block(), 0);
        assert!(client.providers.is_empty());
    }

    #[test]
    fn test_set_and_get_latest_block() {
        let client = RpcClient::new();
        client.set_latest_block(12345);
        assert_eq!(client.latest_block(), 12345);
    }

    #[test]
    fn test_set_latest_block_overwrites() {
        let client = RpcClient::new();
        client.set_latest_block(100);
        client.set_latest_block(200);
        assert_eq!(client.latest_block(), 200);
    }

    #[test]
    fn test_add_and_get_provider() {
        let client = RpcClient::new();
        let state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        client.add_provider(ProviderId::Main, state);

        let provider = client.get_provider(&ProviderId::Main);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().url, "http://rpc.test");
    }

    #[test]
    fn test_get_provider_missing() {
        let client = RpcClient::new();
        assert!(client.get_provider(&ProviderId::Sub1).is_none());
    }

    #[test]
    fn test_best_provider_single() {
        let client = RpcClient::new();
        let state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        client.add_provider(ProviderId::Main, state);

        assert_eq!(client.best_provider(), Some(ProviderId::Main));
    }

    #[test]
    fn test_best_provider_multiple() {
        let client = RpcClient::new();
        client.add_provider(
            ProviderId::Main,
            ProviderState::new("http://main.test", &ProviderId::Main).unwrap(),
        );
        client.add_provider(
            ProviderId::Sub1,
            ProviderState::new("http://sub1.test", &ProviderId::Sub1).unwrap(),
        );
        client.add_provider(
            ProviderId::Sub2,
            ProviderState::new("http://sub2.test", &ProviderId::Sub2).unwrap(),
        );

        // Main has highest initial score (80 vs 70 vs 60)
        assert_eq!(client.best_provider(), Some(ProviderId::Main));
    }

    #[test]
    fn test_best_provider_empty() {
        let client = RpcClient::new();
        assert_eq!(client.best_provider(), None);
    }

    #[test]
    fn test_penalize_provider() {
        let client = RpcClient::new();
        client.add_provider(
            ProviderId::Main,
            ProviderState::new("http://main.test", &ProviderId::Main).unwrap(),
        );
        client.add_provider(
            ProviderId::Sub1,
            ProviderState::new("http://sub1.test", &ProviderId::Sub1).unwrap(),
        );

        // Penalize main heavily
        for _ in 0..10 {
            client.penalize_provider(&ProviderId::Main);
        }

        // Now Sub1 should be best
        assert_eq!(client.best_provider(), Some(ProviderId::Sub1));
    }

    #[test]
    fn test_penalize_nonexistent_provider() {
        let client = RpcClient::new();
        // Should not panic
        client.penalize_provider(&ProviderId::Sub2);
    }

    #[test]
    fn test_reward_provider() {
        let client = RpcClient::new();
        client.add_provider(
            ProviderId::Sub2,
            ProviderState::new("http://sub2.test", &ProviderId::Sub2).unwrap(),
        );

        let initial_score = client.get_provider(&ProviderId::Sub2).unwrap().score();
        client.reward_provider(&ProviderId::Sub2);
        let new_score = client.get_provider(&ProviderId::Sub2).unwrap().score();
        assert_eq!(new_score, initial_score + 2);
    }

    #[test]
    fn test_reward_nonexistent_provider() {
        let client = RpcClient::new();
        // Should not panic
        client.reward_provider(&ProviderId::Main);
    }

    #[tokio::test]
    async fn test_init_with_providers() {
        let urls = vec![
            (ProviderId::Main, "http://main.rpc".to_string()),
            (ProviderId::Sub1, "http://sub1.rpc".to_string()),
        ];
        let client = RpcClient::init(urls).await.unwrap();
        assert_eq!(client.providers.len(), 2);
    }

    #[tokio::test]
    async fn test_init_skips_empty_urls() {
        let urls = vec![
            (ProviderId::Main, "http://main.rpc".to_string()),
            (ProviderId::Sub1, "".to_string()),
        ];
        let client = RpcClient::init(urls).await.unwrap();
        assert_eq!(client.providers.len(), 1);
    }

    #[tokio::test]
    async fn test_init_fails_with_no_providers() {
        let urls: Vec<(ProviderId, String)> = vec![
            (ProviderId::Main, "".to_string()),
        ];
        let result = RpcClient::init(urls).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_init_empty_list() {
        let urls: Vec<(ProviderId, String)> = vec![];
        let result = RpcClient::init(urls).await;
        assert!(result.is_err());
    }
}
