use std::sync::Arc;
use std::time::Duration;

use alloy::providers::{Provider, ProviderBuilder};

use super::RpcClient;

/// Background health check loop that monitors provider health and updates latest block.
pub async fn run_health_check(client: Arc<RpcClient>, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;

        // Update latest block from best provider
        match client.get_block_number().await {
            Ok(block) => {
                client.set_latest_block(block);
                tracing::debug!(block, "Updated latest block");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to update latest block");
            }
        }

        // Check each provider's liveness by querying block number
        for entry in client.providers.iter() {
            let id = entry.key().clone();
            let url = entry.value().url.clone();

            let provider_url: alloy::transports::http::reqwest::Url = match url.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };

            let provider = ProviderBuilder::new().connect_http(provider_url);

            // Query block number with timeout to detect stale/dead providers
            match tokio::time::timeout(Duration::from_secs(5), provider.get_block_number()).await {
                Ok(Ok(block)) => {
                    let latest = client.latest_block();
                    if latest > 0 && block + 10 <= latest {
                        // Provider is more than 10 blocks behind — mark as stale
                        tracing::warn!(
                            provider = ?id,
                            provider_block = block,
                            latest_block = latest,
                            "Provider is stale"
                        );
                        client.penalize_provider(&id);
                    } else {
                        tracing::debug!(
                            provider = ?id,
                            block,
                            score = entry.value().score(),
                            "Provider healthy"
                        );
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!(provider = ?id, error = %e, "Provider health check failed");
                    client.penalize_provider(&id);
                }
                Err(_) => {
                    tracing::warn!(provider = ?id, "Provider health check timed out");
                    client.penalize_provider(&id);
                }
            }
        }
    }
}
