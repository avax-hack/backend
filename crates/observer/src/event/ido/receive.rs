use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::mpsc;

use openlaunch_shared::db::postgres::controller::{
    account as account_ctrl, investment as investment_ctrl, milestone as milestone_ctrl,
    refund as refund_ctrl,
};
use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::event::OnChainEvent;

use crate::controller::project as project_ctrl;
use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::receive::ReceiveManager;
use crate::event::core::EventType;

/// Total supply constant for project tokens (1 billion with 18 decimals).
const TOTAL_SUPPLY: &str = "1000000000000000000000000000";

/// Process IDO event batches received from the stream.
pub async fn process_ido_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError> {
    while let Some(batch) = rx.recv().await {
        tracing::info!(
            from = batch.from_block,
            to = batch.to_block,
            count = batch.len(),
            "Processing IDO event batch"
        );

        for event in &batch.events {
            if let Err(e) = handle_single_event(pool, event).await {
                if e.is_skippable() {
                    tracing::warn!(error = %e, "Skipping IDO event");
                    continue;
                }
                return Err(e);
            }
        }

        receive_mgr.mark_completed(EventType::Ido, batch.to_block);
    }

    Ok(())
}

async fn handle_single_event(pool: &PgPool, event: &OnChainEvent) -> Result<(), ObserverError> {
    match event {
        OnChainEvent::ProjectCreated(e) => handle_project_created(pool, e).await,
        OnChainEvent::TokensPurchased(e) => handle_tokens_purchased(pool, e).await,
        OnChainEvent::Graduated(e) => handle_graduated(pool, e).await,
        OnChainEvent::MilestoneApproved(e) => handle_milestone_approved(pool, e).await,
        OnChainEvent::ProjectFailed(e) => handle_project_failed(pool, e).await,
        OnChainEvent::Refunded(e) => handle_refunded(pool, e).await,
        _ => Ok(()),
    }
}

async fn handle_project_created(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::ProjectCreatedEvent,
) -> Result<(), ObserverError> {
    let now = current_unix_timestamp();

    // Upsert creator account
    account_ctrl::upsert(pool, &e.creator)
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    // Insert project (token address = project_id)
    project_ctrl::insert_from_event(
        pool,
        &e.token,
        &e.name,
        &e.symbol,
        &e.token_uri,
        &e.creator,
        &e.ido_token_amount,
        &e.token_price,
        e.deadline,
        TOTAL_SUPPLY,
        &e.tx_hash,
        now,
    )
    .await
    .map_err(|err| ObserverError::retriable(err))?;

    // Insert default milestones (the on-chain event doesn't carry milestone details,
    // so we create placeholder milestones that will be enriched via the API later).
    tracing::info!(
        token = %e.token,
        creator = %e.creator,
        name = %e.name,
        "ProjectCreated processed"
    );

    Ok(())
}

async fn handle_tokens_purchased(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::TokensPurchasedEvent,
) -> Result<(), ObserverError> {
    use openlaunch_shared::utils::price::wei_to_display;

    let now = current_unix_timestamp();

    // Normalize raw on-chain values to human-readable strings
    let usdc_display = wei_to_display(&e.usdc_amount, 6)
        .map_err(|err| ObserverError::retriable(err))?;
    let token_display = wei_to_display(&e.token_amount, 18)
        .map_err(|err| ObserverError::retriable(err))?;

    // Upsert buyer account
    account_ctrl::upsert(pool, &e.buyer)
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    // Insert investment record (normalized values)
    investment_ctrl::insert(
        pool,
        &e.token,
        &e.buyer,
        &usdc_display,
        &token_display,
        &e.tx_hash,
        e.block_number as i64,
        now,
    )
    .await
    .map_err(|err| ObserverError::retriable(err))?;

    // Update project usdc_raised (raw value, add_usdc_raised normalizes internally)
    project_ctrl::add_usdc_raised(pool, &e.token, &e.usdc_amount)
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    tracing::info!(
        token = %e.token,
        buyer = %e.buyer,
        usdc = %e.usdc_amount,
        "TokensPurchased processed"
    );

    Ok(())
}

async fn handle_graduated(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::GraduatedEvent,
) -> Result<(), ObserverError> {
    openlaunch_shared::db::postgres::controller::project::update_status(pool, &e.token, "active")
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    openlaunch_shared::db::postgres::controller::market::set_graduated(pool, &e.token)
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    tracing::info!(token = %e.token, "Graduated processed");
    Ok(())
}

async fn handle_milestone_approved(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::MilestoneApprovedEvent,
) -> Result<(), ObserverError> {
    milestone_ctrl::update_status(
        pool,
        &e.token,
        e.milestone_index as i32,
        "completed",
        Some(&e.tx_hash),
        Some(&e.usdc_released),
    )
    .await
    .map_err(|err| ObserverError::retriable(err))?;

    tracing::info!(
        token = %e.token,
        index = e.milestone_index,
        "MilestoneApproved processed"
    );
    Ok(())
}

async fn handle_project_failed(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::ProjectFailedEvent,
) -> Result<(), ObserverError> {
    openlaunch_shared::db::postgres::controller::project::update_status(pool, &e.token, "failed")
        .await
        .map_err(|err| ObserverError::retriable(err))?;

    tracing::info!(token = %e.token, "ProjectFailed processed");
    Ok(())
}

async fn handle_refunded(
    pool: &PgPool,
    e: &openlaunch_shared::types::event::RefundedEvent,
) -> Result<(), ObserverError> {
    use openlaunch_shared::utils::price::wei_to_display;

    let now = current_unix_timestamp();

    let tokens_display = wei_to_display(&e.tokens_burned, 18)
        .map_err(|err| ObserverError::retriable(err))?;
    let usdc_display = wei_to_display(&e.usdc_returned, 6)
        .map_err(|err| ObserverError::retriable(err))?;

    refund_ctrl::insert(
        pool,
        &e.token,
        &e.buyer,
        &tokens_display,
        &usdc_display,
        &e.tx_hash,
        e.block_number as i64,
        now,
    )
    .await
    .map_err(|err| ObserverError::retriable(err))?;

    tracing::info!(
        token = %e.token,
        buyer = %e.buyer,
        "Refunded processed"
    );
    Ok(())
}
