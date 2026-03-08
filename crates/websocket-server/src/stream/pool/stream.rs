use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::primitives::Address;
use alloy::transports::ws::WsConnect;
use futures_util::StreamExt;

use openlaunch_shared::config;

use crate::cache::PriceCache;
use crate::event::EventProducers;
use super::receive;

/// Start streaming LpManager contract events from the blockchain via WebSocket RPC.
///
/// Subscribes to logs from the LP Manager contract and forwards parsed events
/// to the appropriate event producers. Automatically reconnects on failure.
pub async fn start_pool_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
) -> anyhow::Result<()> {
    let rpc_url = config::MAIN_RPC_URL.clone();
    let ws_url = rpc_url_to_ws(&rpc_url);

    tracing::info!(url = %ws_url, "Connecting to Pool event stream");

    loop {
        match run_pool_subscription(&ws_url, &producers, &price_cache).await {
            Ok(()) => {
                tracing::warn!("Pool stream ended unexpectedly, reconnecting...");
            }
            Err(e) => {
                tracing::error!(error = %e, "Pool stream error, reconnecting in 5s...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn run_pool_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let lp_manager_address: Address = config::LP_MANAGER_CONTRACT.parse()?;
    let filter = Filter::new().address(lp_manager_address);

    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    tracing::info!("Pool event stream connected");

    while let Some(log) = stream.next().await {
        if let Err(e) = receive::handle_pool_log(&log, producers, price_cache) {
            tracing::error!(error = %e, "Failed to handle Pool log");
        }
    }

    Ok(())
}

/// Convert an HTTP RPC URL to a WebSocket URL.
fn rpc_url_to_ws(url: &str) -> String {
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return url.to_string();
    }
    url.replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_url_to_ws() {
        assert_eq!(rpc_url_to_ws("https://rpc.example.com"), "wss://rpc.example.com");
        assert_eq!(rpc_url_to_ws("http://localhost:8545"), "ws://localhost:8545");
    }
}
