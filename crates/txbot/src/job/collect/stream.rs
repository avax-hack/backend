use std::sync::Arc;
use std::time::Duration;

use alloy::primitives::{Address, U256};
use alloy::providers::ProviderBuilder;
use tokio::sync::mpsc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::config;
use openlaunch_shared::contracts::ido::IIDO;
use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::project;

use crate::config_local::{COLLECT_POLL_SECS, MIN_COLLECT_AMOUNT};

use super::CollectTask;

/// Run the collect-fees stream.
///
/// Polls the database every `COLLECT_POLL_SECS` seconds for graduated projects.
/// For each graduated project, checks accumulated LP fees on-chain.
/// If fees >= `MIN_COLLECT_AMOUNT`, sends a `CollectTask` through the channel.
pub async fn run(
    rpc: Arc<RpcClient>,
    db: Arc<PostgresDatabase>,
    tx: mpsc::Sender<CollectTask>,
) -> anyhow::Result<()> {
    let provider_url = get_rpc_url(&rpc)?;
    let provider = ProviderBuilder::new().connect_http(provider_url.parse()?);

    let ido_address: Address = config::IDO_CONTRACT.parse()?;

    let poll_interval = Duration::from_secs(*COLLECT_POLL_SECS);
    let min_amount = U256::from_str_radix(*MIN_COLLECT_AMOUNT, 10)
        .unwrap_or(U256::from(1_000_000u64));

    tracing::info!(
        ido_address = %ido_address,
        poll_secs = *COLLECT_POLL_SECS,
        min_collect_amount = *MIN_COLLECT_AMOUNT,
        "Collect stream started"
    );

    loop {
        tokio::time::sleep(poll_interval).await;

        // Query DB for graduated projects (status = "completed" maps to on-chain Graduated)
        let graduated_projects = match fetch_graduated_projects(&db).await {
            Ok(projects) => projects,
            Err(err) => {
                tracing::warn!(%err, "Failed to fetch graduated projects from DB");
                continue;
            }
        };

        if graduated_projects.is_empty() {
            tracing::debug!("No graduated projects found");
            continue;
        }

        tracing::debug!(
            count = graduated_projects.len(),
            "Checking fees for graduated projects"
        );

        for token_addr_str in &graduated_projects {
            let token_address: Address = match token_addr_str.parse() {
                Ok(addr) => addr,
                Err(err) => {
                    tracing::warn!(
                        token = %token_addr_str,
                        %err,
                        "Invalid token address in DB, skipping"
                    );
                    continue;
                }
            };

            // Check on-chain fees for this token
            let ido = IIDO::new(ido_address, &provider);
            let fees_result = ido.projects(token_address).call().await;

            match fees_result {
                Ok(project) => {
                    // Use usdcRaised - usdcReleased as proxy for collectible fees.
                    // In production, query actual LP position accumulated trading fees.
                    let unreleased = project
                        .usdcRaised
                        .checked_sub(project.usdcReleased)
                        .unwrap_or(U256::ZERO);

                    if unreleased >= min_amount {
                        let task = CollectTask {
                            token_address: token_addr_str.clone(),
                        };
                        tracing::info!(
                            token = %token_addr_str,
                            unreleased = %unreleased,
                            "Fees above threshold, sending collect task"
                        );
                        if tx.send(task).await.is_err() {
                            tracing::error!("Collect task channel closed");
                            return Ok(());
                        }
                    } else {
                        tracing::debug!(
                            token = %token_addr_str,
                            unreleased = %unreleased,
                            "Fees below threshold, skipping"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        token = %token_addr_str,
                        %err,
                        "Failed to check fees for token"
                    );
                }
            }
        }
    }
}

/// Fetch the token addresses of all graduated projects from the database.
///
/// In the DB, graduated projects have status = "completed".
/// The `project_id` column stores the token contract address.
/// Paginates through results since the DB limits to 100 per page.
async fn fetch_graduated_projects(db: &PostgresDatabase) -> anyhow::Result<Vec<String>> {
    let mut all_addresses = Vec::new();
    let mut page: i64 = 1;

    loop {
        let pagination = openlaunch_shared::types::common::PaginationParams {
            page,
            limit: 100,
        };

        let (rows, total) =
            project::find_list(db.reader(), "recent", &pagination, Some("completed")).await?;

        let row_count = rows.len() as i64;
        for row in rows {
            all_addresses.push(row.project_id);
        }

        if all_addresses.len() as i64 >= total || row_count < 100 {
            break;
        }

        page += 1;
    }

    Ok(all_addresses)
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
