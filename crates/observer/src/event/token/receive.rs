use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::types::event::{OnChainEvent, TransferEvent};
use openlaunch_shared::utils::price::wei_to_display;

use crate::event::core::{EventBatch, EventType};
use crate::event::error::ObserverError;
use crate::sync::receive::ReceiveManager;

const TOKEN_DECIMALS: u32 = 18;
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Process Token Transfer event batches received from the stream.
pub async fn process_token_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing Token Transfer batch"
        );

        for event in &batch.events {
            if let OnChainEvent::Transfer(transfer) = event {
                if let Err(e) = handle_transfer(pool, transfer).await {
                    if e.is_skippable() {
                        tracing::warn!(error = %e, "Skipping Transfer event");
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        receive_mgr.mark_completed(EventType::Token, batch.to_block);
    }

    Ok(())
}

async fn handle_transfer(pool: &PgPool, e: &TransferEvent) -> Result<(), ObserverError> {
    let display_amount = wei_to_display(&e.amount, TOKEN_DECIMALS)
        .map_err(|err| ObserverError::skippable(format!("Invalid amount: {err}")))?;

    // Update sender balance (decrease) - skip if mint (from zero address)
    if e.from != ZERO_ADDRESS {
        update_balance_subtract(pool, &e.from, &e.token, &display_amount).await?;
    }

    // Update receiver balance (increase) - skip if burn (to zero address)
    if e.to != ZERO_ADDRESS {
        update_balance_add(pool, &e.to, &e.token, &display_amount).await?;
    }

    tracing::debug!(
        token = %e.token,
        from = %e.from,
        to = %e.to,
        amount = %display_amount,
        "Transfer processed"
    );

    Ok(())
}

async fn update_balance_add(
    pool: &PgPool,
    account_id: &str,
    token_id: &str,
    amount: &str,
) -> Result<(), ObserverError> {
    // Read current balance, compute new, upsert.
    // We use a SQL expression for atomicity.
    sqlx::query(
        r#"
        INSERT INTO balances (account_id, token_id, balance, updated_at)
        VALUES ($1, $2, $3::NUMERIC, EXTRACT(EPOCH FROM NOW())::BIGINT)
        ON CONFLICT (account_id, token_id) DO UPDATE SET
            balance = balances.balance + $3::NUMERIC,
            updated_at = EXTRACT(EPOCH FROM NOW())::BIGINT
        "#,
    )
    .bind(account_id)
    .bind(token_id)
    .bind(amount)
    .execute(pool)
    .await
    .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Balance add failed: {e}")))?;

    Ok(())
}

async fn update_balance_subtract(
    pool: &PgPool,
    account_id: &str,
    token_id: &str,
    amount: &str,
) -> Result<(), ObserverError> {
    // Use a proper UPSERT: on INSERT (new account) clamp to 0 since we cannot
    // subtract from a balance that does not exist yet; on UPDATE subtract and clamp.
    let result = sqlx::query_scalar::<_, bool>(
        r#"
        WITH upsert AS (
            INSERT INTO balances (account_id, token_id, balance, updated_at)
            VALUES ($1, $2, 0, EXTRACT(EPOCH FROM NOW())::BIGINT)
            ON CONFLICT (account_id, token_id) DO UPDATE SET
                balance = GREATEST(balances.balance - $3::NUMERIC, 0),
                updated_at = EXTRACT(EPOCH FROM NOW())::BIGINT
            RETURNING (xmax = 0) AS is_insert
        )
        SELECT is_insert FROM upsert
        "#,
    )
    .bind(account_id)
    .bind(token_id)
    .bind(amount)
    .fetch_one(pool)
    .await
    .map_err(|e| ObserverError::retriable(anyhow::anyhow!("Balance subtract failed: {e}")))?;

    if result {
        tracing::warn!(
            account_id = %account_id,
            token_id = %token_id,
            amount = %amount,
            "Subtract on unknown account: inserted with 0 balance (no prior balance record)"
        );
    }

    Ok(())
}
