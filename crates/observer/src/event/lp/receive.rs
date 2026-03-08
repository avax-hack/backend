use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::event::OnChainEvent;

use crate::controller::lp as lp_ctrl;
use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::sync::receive::ReceiveManager;

/// Process LP event batches received from the stream.
pub async fn process_lp_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing LP event batch"
        );

        for event in &batch.events {
            if let Err(e) = handle_lp_event(pool, event).await {
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

async fn handle_lp_event(pool: &PgPool, event: &OnChainEvent) -> Result<(), ObserverError> {
    match event {
        OnChainEvent::LiquidityAllocated(e) => {
            let now = current_unix_timestamp();
            lp_ctrl::insert_liquidity_position(
                pool,
                &e.token,
                &e.pool,
                e.tick_lower,
                e.tick_upper,
                &e.token_amount,
                now,
            )
            .await
            .map_err(|err| ObserverError::retriable(err))?;

            tracing::info!(
                token = %e.token,
                pool = %e.pool,
                "LiquidityAllocated processed"
            );
            Ok(())
        }
        OnChainEvent::FeesCollected(e) => {
            let now = current_unix_timestamp();
            lp_ctrl::insert_fee_collection(
                pool,
                &e.token,
                &e.amount0,
                &e.amount1,
                &e.tx_hash,
                e.block_number as i64,
                now,
            )
            .await
            .map_err(|err| ObserverError::retriable(err))?;

            tracing::info!(
                token = %e.token,
                amount0 = %e.amount0,
                amount1 = %e.amount1,
                "FeesCollected processed"
            );
            Ok(())
        }
        _ => Ok(()),
    }
}
