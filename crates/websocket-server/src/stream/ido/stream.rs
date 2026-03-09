use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::primitives::Address;
use alloy::transports::ws::WsConnect;
use futures_util::StreamExt;

use openlaunch_shared::config;

use sqlx::PgPool;

use crate::cache::PriceCache;
use crate::candle::CandleManager;
use crate::event::EventProducers;
use super::receive;

/// Start streaming IDO contract events from the blockchain via WebSocket RPC.
///
/// Subscribes to logs from the IDO contract and forwards parsed events
/// to the appropriate event producers. Automatically reconnects on failure.
pub async fn start_ido_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
    candle_mgr: Arc<CandleManager>,
    db_pool: PgPool,
) -> anyhow::Result<()> {
    let rpc_url = config::MAIN_RPC_URL.clone();
    let ws_url = crate::stream::rpc_url_to_ws(&rpc_url);

    tracing::info!(url = %ws_url, "Connecting to IDO event stream");

    loop {
        match run_ido_subscription(&ws_url, &producers, &price_cache, &candle_mgr, &db_pool).await {
            Ok(()) => {
                tracing::warn!("IDO stream ended unexpectedly, reconnecting in 5s...");
            }
            Err(e) => {
                tracing::error!(error = %e, "IDO stream error, reconnecting in 5s...");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn run_ido_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let ido_address: Address = config::IDO_CONTRACT.parse()?;
    let filter = Filter::new().address(ido_address);

    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    tracing::info!("IDO event stream connected");

    while let Some(log) = stream.next().await {
        if let Err(e) = receive::handle_ido_log(&log, producers, price_cache, candle_mgr, db_pool).await {
            tracing::error!(error = %e, "Failed to handle IDO log");
        }
    }

    Ok(())
}

