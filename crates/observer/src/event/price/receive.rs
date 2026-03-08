use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::db::postgres::controller::market as market_ctrl;

use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::event::price::stream::PriceUpdate;
use crate::sync::receive::ReceiveManager;

/// Process price update batches derived from swap events.
pub async fn process_price_updates(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<PriceUpdate>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        // Wait until dependencies are met before processing
        while !receive_mgr.can_process(EventType::Price, batch.to_block) {
            tracing::warn!(
                event_type = "Price",
                to_block = batch.to_block,
                "Dependencies not met, waiting before processing"
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing Price update batch"
        );

        for update in &batch.events {
            if let Err(e) = handle_price_update(pool, update).await {
                if e.is_skippable() {
                    tracing::warn!(error = %e, "Skipping price update");
                    continue;
                }
                return Err(e);
            }
        }

        receive_mgr.mark_completed(EventType::Price, batch.to_block);
    }

    Ok(())
}

/// Compare two numeric strings and return the greater value.
/// Falls back to `new_val` if either string cannot be parsed.
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

async fn handle_price_update(
    pool: &PgPool,
    update: &PriceUpdate,
) -> Result<(), ObserverError> {
    let existing = market_ctrl::find_by_token(pool, &update.token_id)
        .await
        .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Market data read failed: {e}")))?;

    let data = match existing {
        Some(existing) => {
            let ath_price = max_numeric_str(&existing.ath_price, &update.price);
            market_ctrl::MarketDataRow {
                token_price: update.price.clone(),
                ath_price,
                ..existing
            }
        }
        None => market_ctrl::MarketDataRow {
            token_id: update.token_id.clone(),
            market_type: "DEX".to_string(),
            token_price: update.price.clone(),
            ath_price: update.price.clone(),
            total_supply: "0".to_string(),
            volume_24h: update.volume.clone(),
            holder_count: 0,
            bonding_percent: "0".to_string(),
            milestone_completed: 0,
            milestone_total: 0,
            is_graduated: true,
        },
    };

    market_ctrl::upsert(pool, &data)
        .await
        .map_err(|e| {
            ObserverError::retriable(anyhow::anyhow!("Market data upsert failed: {e}"))
        })?;

    tracing::debug!(
        token = %update.token_id,
        price = %update.price,
        "Price update processed"
    );

    Ok(())
}
