use std::sync::Arc;

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, TxHash};
use alloy::providers::Provider;
use alloy::rpc::types::{Block, Filter, Log, TransactionReceipt};

use super::RpcClient;

impl RpcClient {
    /// Get the latest block number from the chain.
    pub async fn get_block_number(self: &Arc<Self>) -> anyhow::Result<u64> {
        self.execute_with_fallback(|provider| async move {
            provider
                .get_block_number()
                .await
                .map_err(|e| anyhow::anyhow!("get_block_number failed: {e}"))
        })
        .await
    }

    /// Get logs matching the given filter.
    pub async fn get_logs(self: &Arc<Self>, filter: &Filter) -> anyhow::Result<Vec<Log>> {
        let filter = filter.clone();
        self.execute_with_fallback(|provider| {
            let f = filter.clone();
            async move {
                provider
                    .get_logs(&f)
                    .await
                    .map_err(|e| anyhow::anyhow!("get_logs failed: {e}"))
            }
        })
        .await
    }

    /// Get block by number.
    pub async fn get_block_by_number(
        self: &Arc<Self>,
        block_number: u64,
    ) -> anyhow::Result<Option<Block>> {
        self.execute_with_fallback(|provider| async move {
            provider
                .get_block_by_number(BlockNumberOrTag::Number(block_number))
                .await
                .map_err(|e| anyhow::anyhow!("get_block_by_number failed: {e}"))
        })
        .await
    }

    /// Get block timestamp for a given block number.
    pub async fn get_block_timestamp(
        self: &Arc<Self>,
        block_number: u64,
    ) -> anyhow::Result<u64> {
        let block = self
            .get_block_by_number(block_number)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block {block_number} not found"))?;
        Ok(block.header.timestamp)
    }

    /// Get a transaction receipt by hash.
    pub async fn get_transaction_receipt(
        self: &Arc<Self>,
        tx_hash: TxHash,
    ) -> anyhow::Result<Option<TransactionReceipt>> {
        self.execute_with_fallback(|provider| async move {
            provider
                .get_transaction_receipt(tx_hash)
                .await
                .map_err(|e| anyhow::anyhow!("get_transaction_receipt failed: {e}"))
        })
        .await
    }

    /// Get native token balance for an address.
    pub async fn get_balance(
        self: &Arc<Self>,
        address: Address,
    ) -> anyhow::Result<alloy::primitives::U256> {
        self.execute_with_fallback(|provider| async move {
            provider
                .get_balance(address)
                .await
                .map_err(|e| anyhow::anyhow!("get_balance failed: {e}"))
        })
        .await
    }
}
