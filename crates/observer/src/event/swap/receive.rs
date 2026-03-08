use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::db::postgres::controller::{
    chart as chart_ctrl, market as market_ctrl, swap as swap_ctrl,
};
use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::trading::ChartBar;

use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::event::swap::stream::RawSwapEvent;
use crate::sync::receive::ReceiveManager;

/// Mapping from pool address to (token_id, is_token0).
/// In production this would be loaded from the DB; here we accept it as a parameter.
pub struct PoolTokenMapping {
    pub pool_address: String,
    pub token_id: String,
    pub is_token0: bool,
}

/// Process Swap event batches received from the stream.
pub async fn process_swap_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<RawSwapEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
    mappings: &[PoolTokenMapping],
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing Swap event batch"
        );

        for event in &batch.events {
            if let Err(e) = handle_swap(pool, event, mappings).await {
                if e.is_skippable() {
                    tracing::warn!(error = %e, "Skipping Swap event");
                    continue;
                }
                return Err(e);
            }
        }

        receive_mgr.mark_completed(EventType::Swap, batch.to_block);
    }

    Ok(())
}

async fn handle_swap(
    pool: &PgPool,
    event: &RawSwapEvent,
    mappings: &[PoolTokenMapping],
) -> Result<(), ObserverError> {
    let mapping = mappings
        .iter()
        .find(|m| m.pool_address == event.pool)
        .ok_or_else(|| {
            ObserverError::skippable(format!("Unknown pool address: {}", event.pool))
        })?;

    let (native_amount, token_amount, event_type) = parse_swap_amounts(event, mapping.is_token0);

    let price = compute_price(&native_amount, &token_amount);
    let value = native_amount.clone();
    let now = current_unix_timestamp();

    // Insert swap record
    swap_ctrl::insert(
        pool,
        &mapping.token_id,
        &event.sender,
        &event_type,
        &native_amount,
        &token_amount,
        &price,
        &value,
        &event.tx_hash,
        event.block_number as i64,
        now,
    )
    .await
    .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Swap insert failed: {e}")))?;

    // Update chart bar (1-minute resolution)
    let bar_time = (now / 60) * 60; // Round to minute
    let bar = ChartBar {
        time: bar_time,
        open: price.clone(),
        high: price.clone(),
        low: price.clone(),
        close: price.clone(),
        volume: value.clone(),
    };
    chart_ctrl::upsert_bar(pool, &mapping.token_id, "1m", &bar)
        .await
        .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Chart upsert failed: {e}")))?;

    // Update market_data with latest price
    if let Ok(Some(existing)) = market_ctrl::find_by_token(pool, &mapping.token_id).await {
        let updated = market_ctrl::MarketDataRow {
            token_price: price.clone(),
            native_price: price.clone(),
            ath_price: price.clone(),
            ..existing
        };
        market_ctrl::upsert(pool, &updated)
            .await
            .map_err(|e| {
                ObserverError::retriable(anyhow::anyhow!("Market data upsert failed: {e}"))
            })?;
    }

    tracing::debug!(
        token = %mapping.token_id,
        event_type = %event_type,
        price = %price,
        "Swap processed"
    );

    Ok(())
}

/// Parse swap amounts into (native_amount, token_amount, event_type).
/// If the project token is token0, a positive amount0 means the pool received tokens
/// (user sold), and negative means user bought.
fn parse_swap_amounts(event: &RawSwapEvent, is_token0: bool) -> (String, String, String) {
    let amount0: i128 = event.amount0.parse().unwrap_or(0);
    let amount1: i128 = event.amount1.parse().unwrap_or(0);

    if is_token0 {
        let token_amount = amount0.unsigned_abs().to_string();
        let native_amount = amount1.unsigned_abs().to_string();
        let event_type = if amount0 > 0 { "SELL" } else { "BUY" };
        (native_amount, token_amount, event_type.to_string())
    } else {
        let token_amount = amount1.unsigned_abs().to_string();
        let native_amount = amount0.unsigned_abs().to_string();
        let event_type = if amount1 > 0 { "SELL" } else { "BUY" };
        (native_amount, token_amount, event_type.to_string())
    }
}

fn compute_price(native_amount: &str, token_amount: &str) -> String {
    let native: f64 = native_amount.parse().unwrap_or(0.0);
    let token: f64 = token_amount.parse().unwrap_or(1.0);
    if token == 0.0 {
        return "0".to_string();
    }
    format!("{:.18}", native / token)
}
