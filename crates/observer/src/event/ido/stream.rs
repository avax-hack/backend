use std::sync::Arc;

use alloy::primitives::Address;
use alloy::rpc::types::Filter;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::types::event::{
    GraduatedEvent, MilestoneApprovedEvent, OnChainEvent, ProjectCreatedEvent,
    ProjectFailedEvent, RefundedEvent, TokensPurchasedEvent,
};

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::stream::BlockRange;

// Re-export the IDO contract ABI types for log decoding.
use openlaunch_shared::contracts::ido::IIDO;

/// Fetch IDO contract events for the given block range and send them through the channel.
pub async fn poll_ido_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError> {
    let contract_addr: Address = config::IDO_CONTRACT
        .parse()
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Invalid IDO contract address: {e}")))?;

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

        if let Some(event) = try_decode_ido_log(log, block_number, &tx_hash) {
            events.push(event);
        }
    }

    tracing::debug!(
        from = range.from_block,
        to = range.to_block,
        count = events.len(),
        "Polled IDO events"
    );

    if !events.is_empty() {
        let batch = EventBatch::new(events, range.from_block, range.to_block);
        tx.send(batch)
            .await
            .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Channel send failed: {e}")))?;
    }

    Ok(())
}

fn try_decode_ido_log(
    log: &alloy::rpc::types::Log,
    block_number: u64,
    tx_hash: &str,
) -> Option<OnChainEvent> {
    // Try ProjectCreated
    if let Ok(decoded) = log.log_decode::<IIDO::ProjectCreated>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::ProjectCreated(ProjectCreatedEvent {
            token: format!("{:#x}", e.token),
            creator: format!("{:#x}", e.creator),
            name: e.name.clone(),
            symbol: e.symbol.clone(),
            token_uri: e.tokenURI.clone(),
            ido_token_amount: e.idoTokenAmount.to_string(),
            token_price: e.tokenPrice.to_string(),
            deadline: e.deadline.to::<i64>(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try TokensPurchased
    if let Ok(decoded) = log.log_decode::<IIDO::TokensPurchased>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::TokensPurchased(TokensPurchasedEvent {
            token: format!("{:#x}", e.token),
            buyer: format!("{:#x}", e.buyer),
            usdc_amount: e.usdcAmount.to_string(),
            token_amount: e.tokenAmount.to_string(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try Graduated
    if let Ok(decoded) = log.log_decode::<IIDO::Graduated>() {
        return Some(OnChainEvent::Graduated(GraduatedEvent {
            token: format!("{:#x}", decoded.inner.token),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try MilestoneApproved
    if let Ok(decoded) = log.log_decode::<IIDO::MilestoneApproved>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::MilestoneApproved(MilestoneApprovedEvent {
            token: format!("{:#x}", e.token),
            milestone_index: e.milestoneIndex.to::<u64>(),
            usdc_released: e.usdcReleased.to_string(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try ProjectFailed
    if let Ok(decoded) = log.log_decode::<IIDO::ProjectFailed>() {
        return Some(OnChainEvent::ProjectFailed(ProjectFailedEvent {
            token: format!("{:#x}", decoded.inner.token),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    // Try Refunded
    if let Ok(decoded) = log.log_decode::<IIDO::Refunded>() {
        let e = &decoded.inner;
        return Some(OnChainEvent::Refunded(RefundedEvent {
            token: format!("{:#x}", e.token),
            buyer: format!("{:#x}", e.buyer),
            tokens_burned: e.tokensBurned.to_string(),
            usdc_returned: e.usdcReturned.to_string(),
            block_number,
            tx_hash: tx_hash.to_string(),
        }));
    }

    None
}
