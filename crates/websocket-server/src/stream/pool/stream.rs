use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::primitives::Address;
use alloy::transports::ws::WsConnect;
use futures_util::StreamExt;

use openlaunch_shared::config;

use crate::event::EventProducers;
use super::receive;

/// Start streaming LpManager contract events from the blockchain via WebSocket RPC.
///
/// Subscribes to logs from the LP Manager contract and forwards parsed events
/// to the appropriate event producers. Automatically reconnects on failure.
pub async fn start_pool_stream(
    producers: Arc<EventProducers>,
) -> anyhow::Result<()> {
    let rpc_url = config::MAIN_RPC_URL.clone();
    let ws_url = crate::stream::rpc_url_to_ws(&rpc_url);

    tracing::info!(url = %ws_url, "Connecting to Pool event stream");

    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match run_pool_subscription(&ws_url, &producers).await {
            Ok(()) => {
                tracing::warn!(attempt, "Pool stream disconnected (stream ended), reconnecting...");
            }
            Err(e) => {
                tracing::error!(attempt, error = %e, "Pool stream connection failed, reconnecting...");
            }
        }
        let delay = std::cmp::min(5 * 2u64.saturating_pow(attempt.min(5) - 1), 60);
        tracing::info!(delay_secs = delay, attempt, "Pool stream reconnecting after backoff");
        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
    }
}

async fn run_pool_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let lp_manager_address: Address = config::LP_MANAGER_CONTRACT.parse()?;
    let filter = Filter::new().address(lp_manager_address);

    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    tracing::info!(url = %ws_url, "Pool event stream connected successfully");

    while let Some(log) = stream.next().await {
        if let Err(e) = receive::handle_pool_log(&log, producers) {
            tracing::error!(error = %e, "Failed to handle Pool log");
        }
    }

    Ok(())
}

