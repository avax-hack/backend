use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::network::EthereumWallet;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::contracts::ido::IIDO;

use crate::config_local::COLLECT_MAX_RETRIES;
use crate::job::handler::{RetryConfig, run_with_retry};
use crate::keystore::Wallets;
use crate::metrics::TxBotMetrics;

use super::CollectTask;

/// Run the collect-fees executor loop.
///
/// Receives `CollectTask` messages from the channel, then sends
/// the `collectFees(token)` transaction to the IDO contract.
pub async fn run(
    rpc: Arc<RpcClient>,
    wallets: Wallets,
    mut rx: mpsc::Receiver<CollectTask>,
    metrics: Arc<TxBotMetrics>,
) -> anyhow::Result<()> {
    let signer = wallets.collector_signer()?;
    let wallet = EthereumWallet::from(signer.clone());

    let provider_url = get_rpc_url(&rpc)?;
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(provider_url.parse()?);

    let ido_address: Address = config::IDO_CONTRACT.parse()?;

    tracing::info!(
        ido_address = %ido_address,
        signer_address = %signer.address(),
        "Collect executor started"
    );

    while let Some(task) = rx.recv().await {
        tracing::info!(token = %task.token_address, "Received collect task");
        metrics.record_collect_attempt();

        let token_address: Address = match task.token_address.parse() {
            Ok(addr) => addr,
            Err(err) => {
                tracing::error!(
                    token = %task.token_address,
                    %err,
                    "Invalid token address in collect task"
                );
                metrics.record_collect_failure();
                continue;
            }
        };

        let retry_config = RetryConfig::new(*COLLECT_MAX_RETRIES)
            .with_backoff(3000, 30_000, 2.0);

        let ido_contract = IIDO::new(ido_address, &provider);
        let token_addr = token_address;
        let task_name = format!("collect:{}", task.token_address);

        let result = run_with_retry(&retry_config, &task_name, || {
            let contract = ido_contract.clone();
            async move {
                let tx_builder = contract.collectFees(token_addr);
                let pending_tx = tx_builder.send().await?;

                tracing::info!(
                    tx_hash = %pending_tx.tx_hash(),
                    "CollectFees transaction sent, waiting for receipt"
                );

                let receipt = pending_tx.get_receipt().await?;

                tracing::info!(
                    tx_hash = %receipt.transaction_hash,
                    block = ?receipt.block_number,
                    gas_used = ?receipt.gas_used,
                    "CollectFees transaction confirmed"
                );

                Ok(receipt)
            }
        })
        .await;

        match result {
            Ok(_receipt) => {
                tracing::info!(
                    token = %task.token_address,
                    "Successfully collected fees for token"
                );
                metrics.record_collect_success();
            }
            Err(err) => {
                tracing::error!(
                    token = %task.token_address,
                    %err,
                    "Failed to collect fees after all retries"
                );
                metrics.record_collect_failure();
            }
        }
    }

    tracing::info!("Collect executor channel closed, shutting down");
    Ok(())
}

/// Extract the best available RPC URL from the client.
fn get_rpc_url(rpc: &RpcClient) -> anyhow::Result<String> {
    let provider_id = rpc
        .best_provider()
        .ok_or_else(|| anyhow::anyhow!("No RPC providers available"))?;
    let provider_ref = rpc
        .get_provider(&provider_id)
        .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;
    Ok(provider_ref.url.clone())
}
