use std::sync::Arc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::client::provider::ProviderId;
use openlaunch_shared::config;
use openlaunch_shared::db::postgres::PostgresDatabase;

mod config_local;
mod job;
mod keystore;
mod metrics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!("Starting OpenLaunch TxBot");

    let db = Arc::new(
        PostgresDatabase::new(&config::PRIMARY_DATABASE_URL, &config::REPLICA_DATABASE_URL).await?,
    );

    let rpc = RpcClient::init(vec![
        (ProviderId::Main, config::MAIN_RPC_URL.clone()),
        (ProviderId::Sub1, config::SUB_RPC_URL_1.clone()),
        (ProviderId::Sub2, config::SUB_RPC_URL_2.clone()),
    ])
    .await?;

    // Load wallets from environment (graceful fallback if keys missing)
    let wallets = keystore::load_wallets_from_env();

    // Initialize metrics
    let bot_metrics = Arc::new(metrics::TxBotMetrics::new());

    // Spawn metrics reporter
    let _metrics_handle = metrics::spawn_reporter(Arc::clone(&bot_metrics));

    // Spawn graduate job if wallet is available
    let _graduate_handles = if wallets.graduate.is_some() {
        tracing::info!("Spawning graduate job");
        let (stream_h, exec_h) = job::graduate::spawn(
            Arc::clone(&rpc),
            wallets.clone(),
            Arc::clone(&bot_metrics),
        );
        Some((stream_h, exec_h))
    } else {
        tracing::warn!("Graduate job disabled: no GRADUATE_PRIVATE_KEY configured");
        None
    };

    // Spawn collect fees job if wallet is available
    let _collect_handles = if wallets.collector.is_some() {
        tracing::info!("Spawning collect fees job");
        let (stream_h, exec_h) = job::collect::spawn(
            Arc::clone(&rpc),
            Arc::clone(&db),
            wallets.clone(),
            Arc::clone(&bot_metrics),
        );
        Some((stream_h, exec_h))
    } else {
        tracing::warn!("Collect fees job disabled: no COLLECTOR_PRIVATE_KEY configured");
        None
    };

    tracing::info!("TxBot running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutdown signal received, exiting");
    Ok(())
}
