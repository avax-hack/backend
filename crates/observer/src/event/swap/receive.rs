use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::db::postgres::controller::{
    chart as chart_ctrl, market as market_ctrl, swap as swap_ctrl,
};
use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::trading::ChartBar;
use openlaunch_shared::utils::price::wei_to_display;

use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::event::swap::stream::RawSwapEvent;
use crate::sync::receive::ReceiveManager;

const USDC_DECIMALS: u32 = 6;
const TOKEN_DECIMALS: u32 = 18;

/// Mapping from V4 pool ID to (token_address, is_token0).
/// Loaded from DB based on LiquidityAllocated events.
pub struct PoolTokenMapping {
    pub pool_id: String,
    pub token_id: String,
    pub is_token0: bool,
}

/// How often (in batches) to reload pool mappings from the database.
const MAPPING_RELOAD_INTERVAL: u64 = 50;

/// Load pool mappings from the database.
async fn load_mappings(reader_pool: &PgPool) -> Vec<PoolTokenMapping> {
    let rows = crate::controller::lp::load_pool_mappings(reader_pool)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|r| PoolTokenMapping {
            pool_id: r.pool_id,
            token_id: r.token_id,
            is_token0: r.is_token0,
        })
        .collect()
}

/// Process Swap event batches received from the stream.
///
/// Pool mappings are reloaded from `reader_pool` every [`MAPPING_RELOAD_INTERVAL`]
/// batches, and also whenever an unknown pool ID is encountered, so that newly
/// created pools are picked up without restarting the observer.
pub async fn process_swap_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<RawSwapEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
    reader_pool: &PgPool,
) -> Result<(), ObserverError> {
    let mut mappings = load_mappings(reader_pool).await;
    tracing::info!(count = mappings.len(), "Loaded initial pool mappings for swap filtering");
    let mut batch_count: u64 = 0;

    while let Some(batch) = rx.recv().await {
        // Wait until dependencies are met before processing
        while !receive_mgr.can_process(EventType::Swap, batch.to_block) {
            tracing::warn!(
                event_type = "Swap",
                to_block = batch.to_block,
                "Dependencies not met, waiting before processing"
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Periodically reload pool mappings to pick up newly created pools
        batch_count += 1;
        if batch_count % MAPPING_RELOAD_INTERVAL == 0 {
            mappings = load_mappings(reader_pool).await;
            tracing::info!(count = mappings.len(), "Reloaded pool mappings (periodic)");
        }

        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing Swap event batch"
        );

        let mut reloaded_for_unknown = false;
        for event in &batch.events {
            if let Err(e) = handle_swap(pool, event, &mappings).await {
                if e.is_skippable() {
                    // On unknown pool, reload mappings once per batch and retry
                    if !reloaded_for_unknown {
                        reloaded_for_unknown = true;
                        mappings = load_mappings(reader_pool).await;
                        tracing::info!(
                            count = mappings.len(),
                            "Reloaded pool mappings (unknown pool encountered)"
                        );
                        // Retry this event with fresh mappings
                        if let Err(e2) = handle_swap(pool, event, &mappings).await {
                            if e2.is_skippable() {
                                tracing::warn!(error = %e2, "Skipping Swap event after mapping reload");
                                continue;
                            }
                            return Err(e2);
                        }
                        continue;
                    }
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
        .find(|m| m.pool_id == event.pool_id)
        .ok_or_else(|| {
            ObserverError::skippable(format!("Unknown pool ID: {}", event.pool_id))
        })?;

    let (raw_native, raw_token, event_type) = parse_swap_amounts(event, mapping.is_token0);

    let native_amount = wei_to_display(&raw_native, USDC_DECIMALS)
        .map_err(|e| ObserverError::skippable(format!("Invalid native amount: {e}")))?;
    let token_amount = wei_to_display(&raw_token, TOKEN_DECIMALS)
        .map_err(|e| ObserverError::skippable(format!("Invalid token amount: {e}")))?;

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

    // Update chart bars for all timeframes
    let intervals: &[(&str, i64)] = &[
        ("1m", 60),
        ("5m", 300),
        ("15m", 900),
        ("1h", 3600),
        ("4h", 14400),
        ("1d", 86400),
    ];
    for &(interval, secs) in intervals {
        let bar_time = (now / secs) * secs;
        let bar = ChartBar {
            time: bar_time,
            open: price.clone(),
            high: price.clone(),
            low: price.clone(),
            close: price.clone(),
            volume: value.clone(),
        };
        chart_ctrl::upsert_bar(pool, &mapping.token_id, interval, &bar)
            .await
            .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Chart upsert {interval} failed: {e}")))?;
    }

    // Update market_data with latest price
    if let Ok(Some(existing)) = market_ctrl::find_by_token(pool, &mapping.token_id).await {
        let ath_price = max_numeric_str(&existing.ath_price, &price);
        let updated = market_ctrl::MarketDataRow {
            token_price: price.clone(),
            ath_price,
            ..existing
        };
        market_ctrl::upsert(pool, &updated)
            .await
            .map_err(|e| {
                ObserverError::retriable(anyhow::anyhow!("Market data upsert failed: {e}"))
            })?;
    }

    // Recalculate 24h volume from swaps table
    market_ctrl::refresh_volume_24h(pool, &mapping.token_id)
        .await
        .map_err(|e| {
            ObserverError::retriable(anyhow::anyhow!("Volume refresh failed: {e}"))
        })?;

    tracing::debug!(
        token = %mapping.token_id,
        event_type = %event_type,
        price = %price,
        "Swap processed"
    );

    Ok(())
}

/// Parse swap amounts into (native_amount, token_amount, event_type).
fn parse_swap_amounts(event: &RawSwapEvent, is_token0: bool) -> (String, String, String) {
    let amount0: i128 = event.amount0.parse().unwrap_or(0);
    let amount1: i128 = event.amount1.parse().unwrap_or(0);

    // Uniswap V4 sign convention (from Pool.sol):
    //   amount > 0 → user RECEIVES (gains) that token
    //   amount < 0 → user SENDS (pays) that token
    if is_token0 {
        let token_amount = amount0.unsigned_abs().to_string();
        let native_amount = amount1.unsigned_abs().to_string();
        let event_type = if amount0 > 0 { "BUY" } else { "SELL" };
        (native_amount, token_amount, event_type.to_string())
    } else {
        let token_amount = amount1.unsigned_abs().to_string();
        let native_amount = amount0.unsigned_abs().to_string();
        let event_type = if amount1 > 0 { "BUY" } else { "SELL" };
        (native_amount, token_amount, event_type.to_string())
    }
}

fn max_numeric_str(existing: &str, new_val: &str) -> String {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    let existing_bd = BigDecimal::from_str(existing).unwrap_or_default();
    let new_bd = BigDecimal::from_str(new_val).unwrap_or_default();
    if existing_bd >= new_bd {
        existing.to_string()
    } else {
        new_val.to_string()
    }
}

fn compute_price(native_amount: &str, token_amount: &str) -> String {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    let native = BigDecimal::from_str(native_amount).unwrap_or_default();
    let token = BigDecimal::from_str(token_amount).unwrap_or_default();
    if token == BigDecimal::from(0) {
        return "0".to_string();
    }
    let price = (native / token).round(10);
    format!("{price}")
}
