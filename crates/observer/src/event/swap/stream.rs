use std::sync::Arc;

use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::sync::stream::BlockRange;

// Uniswap V4 PoolManager Swap event.
// All swaps go through the singleton PoolManager contract.
sol! {
    event Swap(
        bytes32 indexed id,
        address indexed sender,
        int128 amount0,
        int128 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick,
        uint24 fee
    );
}

/// Intermediate swap data extracted from PoolManager logs.
#[derive(Debug, Clone)]
pub struct RawSwapEvent {
    pub pool_id: String,
    pub sender: String,
    pub amount0: String,
    pub amount1: String,
    pub sqrt_price_x96: String,
    pub liquidity: String,
    pub tick: i32,
    pub fee: u32,
    pub block_number: u64,
    pub tx_hash: String,
}

/// Fetch Swap events from the V4 PoolManager contract.
pub async fn poll_swap_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    tx: &mpsc::Sender<EventBatch<RawSwapEvent>>,
) -> Result<(), ObserverError> {
    let pool_manager_addr: alloy::primitives::Address = config::POOL_MANAGER_CONTRACT
        .parse()
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Invalid POOL_MANAGER_CONTRACT: {e}")))?;

    let filter = Filter::new()
        .address(pool_manager_addr)
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
                pool_id: format!("{:#x}", e.id),
                sender: format!("{:#x}", e.sender),
                amount0: e.amount0.to_string(),
                amount1: e.amount1.to_string(),
                sqrt_price_x96: e.sqrtPriceX96.to_string(),
                liquidity: e.liquidity.to_string(),
                tick: e.tick.as_i32(),
                fee: e.fee.to::<u32>(),
                block_number,
                tx_hash,
            });
        }
    }

    tracing::debug!(
        from = range.from_block,
        to = range.to_block,
        count = events.len(),
        "Polled Swap events from PoolManager"
    );

    let batch = EventBatch::new(events, range.from_block, range.to_block);
    tx.send(batch)
        .await
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Channel send failed: {e}")))?;

    Ok(())
}
