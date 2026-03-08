use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::contracts::ido::IIDO;

use crate::config_local::GRADUATE_POLL_MS;

use super::GraduateTask;

/// Run the graduate event stream.
///
/// Polls the IDO contract for `TokensPurchased` events via log filtering.
/// For each event, checks whether the project should graduate:
///   - idoSold >= idoSupply (sold out), or
///   - current block timestamp >= deadline
/// If graduation conditions are met, sends a `GraduateTask` through the channel.
pub async fn run(
    rpc: Arc<RpcClient>,
    tx: mpsc::Sender<GraduateTask>,
) -> anyhow::Result<()> {
    let provider_url = get_rpc_url(&rpc)?;
    let provider = ProviderBuilder::new().connect_http(provider_url.parse()?);

    let ido_address: Address = config::IDO_CONTRACT.parse()?;

    let poll_interval = Duration::from_millis(*GRADUATE_POLL_MS);

    tracing::info!(
        ido_address = %ido_address,
        poll_ms = *GRADUATE_POLL_MS,
        "Graduate stream started"
    );

    let mut last_block = provider.get_block_number().await?;

    // Bug 41 fix: Track tokens already sent to the channel to avoid
    // sending duplicate graduate tasks for the same token.
    let mut graduated_tokens: HashSet<Address> = HashSet::new();

    loop {
        // Bug 6 fix: Cap the graduated_tokens set to prevent unbounded growth.
        // The executor already checks on-chain status before sending TX,
        // so false re-sends after clearing are caught downstream.
        if graduated_tokens.len() > 10_000 {
            tracing::info!(
                size = graduated_tokens.len(),
                "Clearing graduated_tokens set to prevent unbounded growth"
            );
            graduated_tokens.clear();
        }

        tokio::time::sleep(poll_interval).await;

        let current_block = match provider.get_block_number().await {
            Ok(block) => block,
            Err(err) => {
                tracing::warn!(%err, "Failed to get current block number");
                continue;
            }
        };

        if current_block <= last_block {
            continue;
        }

        let ido = IIDO::new(ido_address, &provider);
        let filter = ido
            .TokensPurchased_filter()
            .from_block(last_block + 1)
            .to_block(current_block);

        let events = match filter.query().await {
            Ok(events) => events,
            Err(err) => {
                // Bug 16 fix: Do NOT advance last_block on query failure.
                // The next poll will re-query from the same block range so events
                // are not permanently missed.
                tracing::warn!(
                    %err,
                    from_block = last_block + 1,
                    to_block = current_block,
                    "Failed to query TokensPurchased events"
                );
                continue;
            }
        };

        if events.is_empty() {
            // No events but query succeeded — safe to advance.
            last_block = current_block;
            continue;
        }

        tracing::info!(
            event_count = events.len(),
            "Processing TokensPurchased events"
        );

        // Collect unique token addresses from the events
        let mut seen_tokens = std::collections::HashSet::new();
        for (event, _log) in &events {
            seen_tokens.insert(event.token);
        }

        // Check each token for graduation eligibility
        for token in seen_tokens {
            // Bug 41 fix: Skip tokens we've already sent to the channel.
            if graduated_tokens.contains(&token) {
                tracing::debug!(
                    token = %token,
                    "Token already sent for graduation, skipping duplicate"
                );
                continue;
            }

            let ido_check = IIDO::new(ido_address, &provider);
            let project_result = ido_check.projects(token).call().await;

            match project_result {
                Ok(project) => {
                    // Status::Active == 0
                    let status_val: u8 = project.status.into();
                    if status_val != 0 {
                        tracing::debug!(
                            token = %token,
                            status = status_val,
                            "Token not Active, skipping"
                        );
                        continue;
                    }

                    let should_graduate = if project.idoSold >= project.idoSupply {
                        tracing::info!(token = %token, "Token sold out, eligible for graduation");
                        true
                    } else {
                        // Check deadline
                        match provider
                            .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
                            .await
                        {
                            Ok(Some(block)) => {
                                let current_ts = U256::from(block.header.timestamp);
                                current_ts >= project.deadline
                            }
                            Ok(None) => {
                                tracing::warn!("Latest block returned None");
                                false
                            }
                            Err(err) => {
                                tracing::warn!(%err, "Failed to get latest block for deadline check");
                                false
                            }
                        }
                    };

                    if should_graduate {
                        let task = GraduateTask {
                            token_address: format!("{token:#x}"),
                        };
                        tracing::info!(
                            token = %token,
                            "Token eligible for graduation, sending task"
                        );
                        if tx.send(task).await.is_err() {
                            tracing::error!("Graduate task channel closed");
                            return Ok(());
                        }
                        graduated_tokens.insert(token);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        token = %token,
                        %err,
                        "Failed to check graduation eligibility"
                    );
                }
            }
        }

        // Bug 16 fix: Only advance last_block AFTER successful event processing.
        last_block = current_block;
    }
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
