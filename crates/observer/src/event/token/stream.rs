use std::sync::Arc;

use alloy::rpc::types::Filter;
use alloy::sol_types::SolEvent;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::contracts::project_token::IProjectToken;
use openlaunch_shared::types::event::{OnChainEvent, TransferEvent};

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::stream::BlockRange;

/// Fetch ERC20 Transfer events for tracked project tokens.
pub async fn poll_token_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    token_addresses: &[String],
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError> {
    if token_addresses.is_empty() {
        return Ok(());
    }

    let addresses: Vec<alloy::primitives::Address> = token_addresses
        .iter()
        .filter_map(|addr| addr.parse().ok())
        .collect();

    if addresses.is_empty() {
        return Ok(());
    }

    let filter = Filter::new()
        .address(addresses)
        .event_signature(IProjectToken::Transfer::SIGNATURE_HASH)
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

        if let Ok(decoded) = log.log_decode::<IProjectToken::Transfer>() {
            let e = &decoded.inner;
            events.push(OnChainEvent::Transfer(TransferEvent {
                token: format!("{:#x}", decoded.inner.address),
                from: format!("{:#x}", e.from),
                to: format!("{:#x}", e.to),
                amount: e.value.to_string(),
                block_number,
                tx_hash,
            }));
        }
    }

    tracing::debug!(
        from = range.from_block,
        to = range.to_block,
        count = events.len(),
        "Polled Token Transfer events"
    );

    if !events.is_empty() {
        let batch = EventBatch::new(events, range.from_block, range.to_block);
        tx.send(batch)
            .await
            .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Channel send failed: {e}")))?;
    }

    Ok(())
}
