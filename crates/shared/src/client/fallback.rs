use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use alloy::providers::{Provider, ProviderBuilder, RootProvider};

use crate::config;
use super::RpcClient;
use super::provider::ProviderId;

/// The concrete provider type for HTTP connections.
pub type HttpProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    RootProvider,
>;

impl RpcClient {
    /// Execute an async operation with automatic fallback to the next-best provider on failure.
    ///
    /// Tries providers in score order (best first). On failure or timeout, penalizes the
    /// current provider and moves to the next. On success, rewards the provider.
    pub async fn execute_with_fallback<F, Fut, T>(
        self: &Arc<Self>,
        operation: F,
    ) -> anyhow::Result<T>
    where
        F: Fn(HttpProvider) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let timeout_duration = Duration::from_millis(*config::RPC_TIMEOUT_MS);

        // Collect providers sorted by score (best first)
        let sorted_providers = self.providers_sorted_by_score();

        if sorted_providers.is_empty() {
            anyhow::bail!("No RPC providers available");
        }

        let mut last_error = None;

        for provider_id in &sorted_providers {
            let url = match self.providers.get(provider_id) {
                Some(state) => state.url.clone(),
                None => continue,
            };

            let provider_url: alloy::transports::http::reqwest::Url = match url.parse() {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!(provider = ?provider_id, error = %e, "Invalid provider URL, skipping");
                    continue;
                }
            };

            let provider = ProviderBuilder::new().connect_http(provider_url);

            match tokio::time::timeout(timeout_duration, operation(provider)).await {
                Ok(Ok(result)) => {
                    self.reward_provider(provider_id);
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        provider = ?provider_id,
                        error = %e,
                        "RPC call failed, trying next provider"
                    );
                    self.penalize_provider(provider_id);
                    last_error = Some(e);
                }
                Err(_timeout) => {
                    tracing::warn!(
                        provider = ?provider_id,
                        timeout_ms = *config::RPC_TIMEOUT_MS,
                        "RPC call timed out, trying next provider"
                    );
                    self.penalize_provider(provider_id);
                    last_error = Some(anyhow::anyhow!(
                        "RPC timeout after {}ms for provider {:?}",
                        *config::RPC_TIMEOUT_MS,
                        provider_id
                    ));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All RPC providers failed")))
    }

    /// Returns provider IDs sorted by score (highest first).
    fn providers_sorted_by_score(&self) -> Vec<ProviderId> {
        let mut entries: Vec<(ProviderId, i32)> = self
            .providers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().score()))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.into_iter().map(|(id, _)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::provider::ProviderState;

    fn make_client_with_providers() -> Arc<RpcClient> {
        let client = Arc::new(RpcClient::new());
        client.add_provider(
            ProviderId::Main,
            ProviderState::new("http://main.rpc", &ProviderId::Main).unwrap(),
        );
        client.add_provider(
            ProviderId::Sub1,
            ProviderState::new("http://sub1.rpc", &ProviderId::Sub1).unwrap(),
        );
        client.add_provider(
            ProviderId::Sub2,
            ProviderState::new("http://sub2.rpc", &ProviderId::Sub2).unwrap(),
        );
        client
    }

    #[test]
    fn test_providers_sorted_by_score() {
        let client = make_client_with_providers();
        let sorted = client.providers_sorted_by_score();
        // Main (80) > Sub1 (70) > Sub2 (60)
        assert_eq!(sorted[0], ProviderId::Main);
        assert_eq!(sorted[1], ProviderId::Sub1);
        assert_eq!(sorted[2], ProviderId::Sub2);
    }

    #[test]
    fn test_providers_sorted_after_penalize() {
        let client = make_client_with_providers();
        // Penalize Main heavily so it drops below Sub2
        for _ in 0..11 {
            client.penalize_provider(&ProviderId::Main);
        }
        let sorted = client.providers_sorted_by_score();
        // Main should now be last
        assert_eq!(sorted[0], ProviderId::Sub1);
        assert_eq!(sorted[1], ProviderId::Sub2);
        assert_eq!(sorted[2], ProviderId::Main);
    }

    #[tokio::test]
    async fn test_execute_with_fallback_no_providers() {
        let client = Arc::new(RpcClient::new());
        let result: anyhow::Result<u64> = client
            .execute_with_fallback(|_provider| async { Ok(42u64) })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No RPC providers"));
    }
}
