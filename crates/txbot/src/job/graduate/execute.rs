use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::network::EthereumWallet;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::contracts::ido::IIDO;

use crate::config_local::GRADUATE_MAX_RETRIES;
use crate::job::handler::{RetryConfig, run_with_retry};
use crate::keystore::Wallets;
use crate::metrics::TxBotMetrics;

use super::GraduateTask;

/// Run the graduate executor loop.
///
/// Receives `GraduateTask` messages from the channel, verifies the project
/// is still Active on-chain, then sends the `graduate(token)` transaction.
pub async fn run(
    rpc: Arc<RpcClient>,
    wallets: Wallets,
    mut rx: mpsc::Receiver<GraduateTask>,
    metrics: Arc<TxBotMetrics>,
) -> anyhow::Result<()> {
    let signer = match wallets.graduate_signer() {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(%err, "Failed to load graduate wallet signer at startup");
            return Err(anyhow::anyhow!(
                "Graduate executor cannot start: wallet initialization failed: {err}"
            ));
        }
    };
    let wallet = EthereumWallet::from(signer.clone());

    let provider_url = get_rpc_url(&rpc)?;
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(provider_url.parse()?);

    let ido_address: Address = config::IDO_CONTRACT.parse()?;

    tracing::info!(
        ido_address = %ido_address,
        signer_address = %signer.address(),
        "Graduate executor started"
    );

    while let Some(task) = rx.recv().await {
        tracing::info!(token = %task.token_address, "Received graduate task");
        metrics.record_graduate_attempt();

        let token_address: Address = match task.token_address.parse() {
            Ok(addr) => addr,
            Err(err) => {
                tracing::error!(
                    token = %task.token_address,
                    %err,
                    "Invalid token address in graduate task"
                );
                metrics.record_graduate_failure();
                continue;
            }
        };

        // Verify project is still Active before sending TX
        let ido_read = IIDO::new(ido_address, &provider);
        match ido_read.projects(token_address).call().await {
            Ok(project) => {
                let status_val: u8 = project.status.into();
                if status_val != 0 {
                    tracing::info!(
                        token = %task.token_address,
                        status = status_val,
                        "Project is no longer Active, skipping graduation"
                    );
                    continue;
                }
            }
            Err(err) => {
                tracing::warn!(
                    token = %task.token_address,
                    %err,
                    "Failed to verify project status, attempting graduation anyway"
                );
            }
        }

        let retry_config = RetryConfig::new(*GRADUATE_MAX_RETRIES)
            .with_backoff(2000, 30_000, 2.0);

        let ido_contract = IIDO::new(ido_address, &provider);
        let token_addr = token_address;
        let task_name = format!("graduate:{}", task.token_address);

        let result = run_with_retry(&retry_config, &task_name, || {
            let contract = ido_contract.clone();
            async move {
                let tx_builder = contract.graduate(token_addr).gas(500_000);
                let pending_tx = tx_builder.send().await?;

                tracing::info!(
                    tx_hash = %pending_tx.tx_hash(),
                    "Graduate transaction sent, waiting for receipt"
                );

                let receipt = pending_tx.get_receipt().await?;

                // Bug 38 fix: Check receipt status. A status of false/0 means
                // the transaction reverted on-chain.
                if !receipt.status() {
                    return Err(anyhow::anyhow!(
                        "Graduate transaction reverted on-chain (tx_hash={})",
                        receipt.transaction_hash
                    ));
                }

                tracing::info!(
                    tx_hash = %receipt.transaction_hash,
                    block = ?receipt.block_number,
                    gas_used = ?receipt.gas_used,
                    "Graduate transaction confirmed"
                );

                Ok(receipt)
            }
        })
        .await;

        match result {
            Ok(_receipt) => {
                tracing::info!(
                    token = %task.token_address,
                    "Successfully graduated token"
                );
                metrics.record_graduate_success();
            }
            Err(err) => {
                tracing::error!(
                    token = %task.token_address,
                    %err,
                    "Failed to graduate token after all retries"
                );
                metrics.record_graduate_failure();
            }
        }
    }

    tracing::info!("Graduate executor channel closed, shutting down");
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
