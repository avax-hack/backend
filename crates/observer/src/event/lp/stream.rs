use std::sync::Arc;

use alloy::primitives::Address;
use alloy::rpc::types::Filter;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::contracts::lp_manager::ILpManager;
use openlaunch_shared::types::event::{
    FeesCollectedEvent, LiquidityAllocatedEvent, OnChainEvent,
};

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::stream::BlockRange;

/// Fetch LpManager contract events (LiquidityAllocated, FeesCollected).
pub async fn poll_lp_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError> {
    let contract_addr: Address = config::LP_MANAGER_CONTRACT
        .parse()
        .map_err(|e| {
            ObserverError::fatal(anyhow::anyhow!("Invalid LP_MANAGER contract address: {e}"))
        })?;

    let filter = Filter::new()
        .address(contract_addr)
        .from_block(range.from_block)
        .to_block(range.to_block);

    let logs = rpc
        .get_logs(&filter)
        .await
        .map_err(|e| ObserverError::retriable(anyhow::anyhow!("{e}")))?;

    let mut events = Vec::new();

    for log in &logs {
        if log.inner.data.topics().is_empty() {
            continue;
        }
        let block_number = log.block_number.unwrap_or(range.from_block);
        let tx_hash = log
            .transaction_hash
            .map(|h| format!("{h:#x}"))
            .unwrap_or_default();

        if let Some(event) = try_decode_lp_log(log, block_number, &tx_hash) {
            events.push(event);
        }
    }

    tracing::debug!(
        from = range.from_block,
        to = range.to_block,
        count = events.len(),
        "Polled LP events"
    );

    // Always send the batch (even if empty) so the receive side can
    // call mark_completed and advance its block progress cursor.
    let batch = EventBatch::new(events, range.from_block, range.to_block);
    tx.send(batch)
        .await
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Channel send failed: {e}")))?;

    Ok(())
}

fn try_decode_lp_log(
    log: &alloy::rpc::types::Log,
    block_number: u64,
    tx_hash: &str,
) -> Option<OnChainEvent> {
    // Try LiquidityAllocated
    if let Ok(decoded) = log.log_decode::<ILpManager::LiquidityAllocated>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::LiquidityAllocated(LiquidityAllocatedEvent {
            token: format!("{:#x}", e.token),
            pool_id: format!("{:#x}", e.poolId),
            token_is_currency0: e.tokenIsCurrency0,
            token_amount: e.tokenAmount.to_string(),
            tick_lower: e.tickLower.as_i32(),
            tick_upper: e.tickUpper.as_i32(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try FeesCollected
    if let Ok(decoded) = log.log_decode::<ILpManager::FeesCollected>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::FeesCollected(FeesCollectedEvent {
            token: format!("{:#x}", e.token),
            amount0: e.amount0.to_string(),
            amount1: e.amount1.to_string(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    None
}
