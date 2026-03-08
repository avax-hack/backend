use std::sync::Arc;

use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::stream::BlockRange;

// Define the V4 Pool Swap event signature for log filtering.
sol! {
    event Swap(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );
}

/// Intermediate swap data extracted from logs.
#[derive(Debug, Clone)]
pub struct RawSwapEvent {
    pub pool: String,
    pub sender: String,
    pub recipient: String,
    pub amount0: String,
    pub amount1: String,
    pub sqrt_price_x96: String,
    pub liquidity: String,
    pub tick: i32,
    pub block_number: u64,
    pub tx_hash: String,
}

/// Fetch Swap events from V4 pool contracts for known pool addresses.
pub async fn poll_swap_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    pool_addresses: &[String],
    tx: &mpsc::Sender<EventBatch<RawSwapEvent>>,
) -> Result<(), ObserverError> {
    if pool_addresses.is_empty() {
        return Ok(());
    }

    let addresses: Vec<alloy::primitives::Address> = pool_addresses
        .iter()
        .filter_map(|addr| addr.parse().ok())
        .collect();

    if addresses.is_empty() {
        return Ok(());
    }

    let filter = Filter::new()
        .address(addresses)
        .event_signature(Swap::SIGNATURE_HASH)
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

        if let Ok(decoded) = log.log_decode::<Swap>() {
            let e = &decoded.inner;
            events.push(RawSwapEvent {
                pool: format!("{:#x}", decoded.inner.address),
                sender: format!("{:#x}", e.sender),
                recipient: format!("{:#x}", e.recipient),
                amount0: e.amount0.to_string(),
                amount1: e.amount1.to_string(),
                sqrt_price_x96: e.sqrtPriceX96.to_string(),
                liquidity: e.liquidity.to_string(),
                tick: e.tick.as_i32(),
                block_number,
                tx_hash,
            });
        }
    }

    tracing::debug!(
        from = range.from_block,
        to = range.to_block,
        count = events.len(),
        "Polled Swap events"
    );

    if !events.is_empty() {
        let batch = EventBatch::new(events, range.from_block, range.to_block);
        tx.send(batch)
            .await
            .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Channel send failed: {e}")))?;
    }

    Ok(())
}

// No pure-function unit tests for this module. The core logic
// (poll_swap_events) requires an RPC provider and is covered
// by integration tests.
