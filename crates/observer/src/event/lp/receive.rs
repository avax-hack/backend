use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::event::OnChainEvent;
use openlaunch_shared::utils::price::wei_to_display;

use crate::controller::lp as lp_ctrl;
use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::sync::receive::ReceiveManager;

const USDC_DECIMALS: u32 = 6;
const TOKEN_DECIMALS: u32 = 18;

/// Process LP event batches received from the stream.
pub async fn process_lp_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
    reader_pool: &PgPool,
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        // Wait until dependencies are met before processing
        while !receive_mgr.can_process(EventType::Lp, batch.to_block) {
            tracing::warn!(
                event_type = "Lp",
                to_block = batch.to_block,
                "Dependencies not met, waiting before processing"
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing LP event batch"
        );

        for event in &batch.events {
            if let Err(e) = handle_lp_event(pool, event, reader_pool).await {
                if e.is_skippable() {
                    tracing::warn!(error = %e, "Skipping LP event");
                    continue;
                }
                return Err(e);
            }
        }

        receive_mgr.mark_completed(EventType::Lp, batch.to_block);
    }

    Ok(())
}

async fn handle_lp_event(pool: &PgPool, event: &OnChainEvent, reader_pool: &PgPool) -> Result<(), ObserverError> {
    match event {
        OnChainEvent::LiquidityAllocated(e) => {
            let now = current_unix_timestamp();

            let token_display = wei_to_display(&e.token_amount, TOKEN_DECIMALS)
                .map_err(|err| ObserverError::skippable(format!("Invalid token amount: {err}")))?;

            // Store liquidity position
            lp_ctrl::insert_liquidity_position(
                pool,
                &e.token,
                &e.pool_id,
                e.tick_lower,
                e.tick_upper,
                &token_display,
                now,
            )
            .await
            .map_err(|err| ObserverError::retriable(err))?;

            // Store pool → token mapping for swap event filtering
            lp_ctrl::insert_pool_mapping(
                pool,
                &e.pool_id,
                &e.token,
                e.token_is_currency0,
                now,
            )
            .await
            .map_err(|err| ObserverError::retriable(err))?;

            tracing::info!(
                token = %e.token,
                pool_id = %e.pool_id,
                is_currency0 = %e.token_is_currency0,
                "LiquidityAllocated processed, pool mapping stored"
            );
            Ok(())
        }
        OnChainEvent::FeesCollected(e) => {
            let now = current_unix_timestamp();

            // Look up pool mapping to determine decimals for amount0/amount1
            let mappings = lp_ctrl::load_pool_mappings(reader_pool)
                .await
                .unwrap_or_default();
            let is_token0 = mappings
                .iter()
                .find(|m| m.token_id == e.token)
                .map(|m| m.is_token0);

            let (display_amount0, display_amount1) = match is_token0 {
                Some(true) => {
                    // currency0 = token (18 dec), currency1 = USDC (6 dec)
                    let a0 = wei_to_display(&e.amount0, TOKEN_DECIMALS)
                        .unwrap_or_else(|_| e.amount0.clone());
                    let a1 = wei_to_display(&e.amount1, USDC_DECIMALS)
                        .unwrap_or_else(|_| e.amount1.clone());
                    (a0, a1)
                }
                Some(false) => {
                    // currency0 = USDC (6 dec), currency1 = token (18 dec)
                    let a0 = wei_to_display(&e.amount0, USDC_DECIMALS)
                        .unwrap_or_else(|_| e.amount0.clone());
                    let a1 = wei_to_display(&e.amount1, TOKEN_DECIMALS)
                        .unwrap_or_else(|_| e.amount1.clone());
                    (a0, a1)
                }
                None => {
                    tracing::warn!(token = %e.token, "No pool mapping found for fee normalization, storing raw");
                    (e.amount0.clone(), e.amount1.clone())
                }
            };

            lp_ctrl::insert_fee_collection(
                pool,
                &e.token,
                &display_amount0,
                &display_amount1,
                &e.tx_hash,
                e.block_number as i64,
                now,
            )
            .await
            .map_err(|err| ObserverError::retriable(err))?;

            tracing::info!(
                token = %e.token,
                amount0 = %display_amount0,
                amount1 = %display_amount1,
                "FeesCollected processed"
            );
            Ok(())
        }
        _ => Ok(()),
    }
}
