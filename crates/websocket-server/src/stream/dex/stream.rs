use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use alloy::transports::ws::WsConnect;
use futures_util::StreamExt;
use sqlx::PgPool;

use openlaunch_shared::config;

use crate::cache::PriceCache;
use crate::candle::CandleManager;
use crate::event::EventProducers;

use super::receive;

/// Pool ID to Token mapping loaded from database.
#[derive(Debug, Clone)]
pub struct PoolMapping {
    pub pool_id: String,
    pub token_id: String,
    pub is_token0: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct PoolMappingRow {
    pool_id: String,
    token_id: String,
    is_token0: bool,
}

/// Load pool mappings from the database.
async fn load_mappings(db: &PgPool) -> Vec<PoolMapping> {
    let rows = sqlx::query_as::<_, PoolMappingRow>(
        "SELECT pool_id, token_id, is_token0 FROM pool_mappings",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| PoolMapping {
            pool_id: r.pool_id,
            token_id: r.token_id,
            is_token0: r.is_token0,
        })
        .collect()
}

/// Start streaming PoolManager Swap events from the blockchain.
pub async fn start_dex_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
    candle_mgr: Arc<CandleManager>,
    db: PgPool,
) -> anyhow::Result<()> {
    let rpc_url = config::MAIN_RPC_URL.clone();
    let ws_url = rpc_url_to_ws(&rpc_url);

    tracing::info!(url = %ws_url, "Connecting to DEX Swap event stream");

    loop {
        match run_dex_subscription(&ws_url, &producers, &price_cache, &candle_mgr, &db).await {
            Ok(()) => {
                tracing::warn!("DEX stream ended unexpectedly, reconnecting...");
            }
            Err(e) => {
                tracing::error!(error = %e, "DEX stream error, reconnecting in 5s...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn run_dex_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db: &PgPool,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let pool_manager_addr: Address = config::POOL_MANAGER_CONTRACT.parse()?;
    let filter = Filter::new()
        .address(pool_manager_addr)
        .event_signature(receive::Swap::SIGNATURE_HASH);

    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    let mut mappings = load_mappings(db).await;
    tracing::info!(count = mappings.len(), "DEX stream connected, loaded pool mappings");

    let mut event_count: u64 = 0;
    let reload_interval: u64 = 100;

    while let Some(log) = stream.next().await {
        event_count += 1;

        if event_count % reload_interval == 0 {
            mappings = load_mappings(db).await;
            tracing::info!(count = mappings.len(), "Reloaded pool mappings (periodic)");
        }

        if let Err(e) =
            receive::handle_swap_log(&log, &mappings, producers, price_cache, candle_mgr)
        {
            tracing::error!(error = %e, "Failed to handle DEX Swap log");
        }
    }

    Ok(())
}

fn rpc_url_to_ws(url: &str) -> String {
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return url.to_string();
    }
    let ws = url
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    if ws.ends_with("/rpc") {
        ws[..ws.len() - 4].to_string() + "/ws"
    } else {
        ws
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_url_to_ws() {
        assert_eq!(
            rpc_url_to_ws("https://api.avax-test.network/ext/bc/C/rpc"),
            "wss://api.avax-test.network/ext/bc/C/ws"
        );
        assert_eq!(
            rpc_url_to_ws("https://rpc.example.com"),
            "wss://rpc.example.com"
        );
        assert_eq!(rpc_url_to_ws("http://localhost:8545"), "ws://localhost:8545");
        assert_eq!(rpc_url_to_ws("wss://already.ws"), "wss://already.ws");
    }
}
