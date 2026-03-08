use std::sync::Arc;
use std::time::Duration;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::client::health::run_health_check;
use openlaunch_shared::client::provider::ProviderId;
use openlaunch_shared::config;
use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::block as block_ctrl;
use openlaunch_shared::types::event::OnChainEvent;
use tokio::task::JoinSet;

mod config_local;
mod controller;
mod event;
mod sync;

use event::core::{EventBatch, EventType};
use event::handler::{RetryConfig, run_event_handler_with_retry};
use event::swap::stream::RawSwapEvent;
use event::price::stream::PriceUpdate;
use sync::receive::ReceiveManager;
use sync::stream::StreamManager;

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

    tracing::info!("Starting OpenLaunch Observer");

    let db = Arc::new(
        PostgresDatabase::new(&config::PRIMARY_DATABASE_URL, &config::REPLICA_DATABASE_URL).await?,
    );

    let rpc = RpcClient::init(vec![
        (ProviderId::Main, config::MAIN_RPC_URL.clone()),
        (ProviderId::Sub1, config::SUB_RPC_URL_1.clone()),
        (ProviderId::Sub2, config::SUB_RPC_URL_2.clone()),
    ])
    .await?;

    // Load block progress from DB for each EventType
    let stream_mgr = Arc::new(StreamManager::new());
    let receive_mgr = Arc::new(ReceiveManager::new());

    for event_type in EventType::all() {
        let last_block = block_ctrl::get_last_block(db.writer(), event_type.as_str())
            .await?
            .map(|b| b as u64)
            .unwrap_or(*config_local::START_BLOCK);

        stream_mgr.set_progress(*event_type, last_block);
        receive_mgr.set_completed(*event_type, last_block);

        tracing::info!(
            event_type = event_type.as_str(),
            last_block,
            "Loaded block progress"
        );
    }

    // Create mpsc channels
    let (ido_tx, mut ido_rx) = tokio::sync::mpsc::channel::<EventBatch<OnChainEvent>>(128);
    let (_token_tx, mut token_rx) = tokio::sync::mpsc::channel::<EventBatch<OnChainEvent>>(128);
    let (_swap_tx, mut swap_rx) = tokio::sync::mpsc::channel::<EventBatch<RawSwapEvent>>(128);
    let (lp_tx, mut lp_rx) = tokio::sync::mpsc::channel::<EventBatch<OnChainEvent>>(128);
    let (_price_tx, mut price_rx) = tokio::sync::mpsc::channel::<EventBatch<PriceUpdate>>(128);

    let mut join_set = JoinSet::new();

    // Spawn health check
    {
        let rpc = Arc::clone(&rpc);
        join_set.spawn(async move {
            run_health_check(rpc, Duration::from_secs(30)).await;
            Ok::<(), anyhow::Error>(())
        });
    }

    // Spawn IDO stream
    {
        let rpc = Arc::clone(&rpc);
        let stream_mgr = Arc::clone(&stream_mgr);
        let db = Arc::clone(&db);
        let tx = ido_tx.clone();
        join_set.spawn(async move {
            run_ido_stream(&rpc, &stream_mgr, &db, &tx).await
        });
    }

    // Spawn IDO receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            event::ido::receive::process_ido_events(
                db.writer(),
                &mut ido_rx,
                &receive_mgr,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn LP stream
    {
        let rpc = Arc::clone(&rpc);
        let stream_mgr = Arc::clone(&stream_mgr);
        let db = Arc::clone(&db);
        let tx = lp_tx.clone();
        join_set.spawn(async move {
            run_lp_stream(&rpc, &stream_mgr, &db, &tx).await
        });
    }

    // Spawn LP receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            event::lp::receive::process_lp_events(
                db.writer(),
                &mut lp_rx,
                &receive_mgr,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn Token receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            event::token::receive::process_token_events(
                db.writer(),
                &mut token_rx,
                &receive_mgr,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn Swap receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            let mappings = vec![]; // Populated from DB once pools are created
            event::swap::receive::process_swap_events(
                db.writer(),
                &mut swap_rx,
                &receive_mgr,
                &mappings,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn Price receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            event::price::receive::process_price_updates(
                db.writer(),
                &mut price_rx,
                &receive_mgr,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn block progress persister
    {
        let db = Arc::clone(&db);
        let stream_mgr = Arc::clone(&stream_mgr);
        join_set.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                for event_type in EventType::all() {
                    if let Some(block) = stream_mgr.current_block(*event_type) {
                        if let Err(e) = block_ctrl::set_last_block(
                            db.writer(),
                            event_type.as_str(),
                            block as i64,
                        )
                        .await
                        {
                            tracing::error!(
                                event_type = event_type.as_str(),
                                error = %e,
                                "Failed to persist block progress"
                            );
                        }
                    }
                }
            }
        });
    }

    tracing::info!("Observer running. Press Ctrl+C to stop.");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutdown signal received");
        }
        Some(result) = join_set.join_next() => {
            match result {
                Ok(Ok(())) => tracing::info!("A task completed normally"),
                Ok(Err(e)) => tracing::error!(error = %e, "A task failed"),
                Err(e) => tracing::error!(error = %e, "A task panicked"),
            }
        }
    }

    Ok(())
}

async fn run_ido_stream(
    rpc: &Arc<RpcClient>,
    stream_mgr: &Arc<StreamManager>,
    db: &Arc<PostgresDatabase>,
    tx: &tokio::sync::mpsc::Sender<EventBatch<OnChainEvent>>,
) -> anyhow::Result<()> {
    let poll_interval = Duration::from_millis(*config_local::POLL_INTERVAL_MS);
    let retry_config = RetryConfig::default();

    loop {
        tokio::time::sleep(poll_interval).await;

        let latest_block = rpc.latest_block();
        if latest_block == 0 {
            continue;
        }

        let range = match stream_mgr.get_range(EventType::Ido, latest_block) {
            Some(r) => r,
            None => continue,
        };

        let result = run_event_handler_with_retry("ido_stream", &retry_config, || {
            event::ido::stream::poll_ido_events(rpc, &range, tx)
        })
        .await;

        handle_stream_result("ido_stream", EventType::Ido, &result, &range, stream_mgr, db).await;
    }
}

async fn run_lp_stream(
    rpc: &Arc<RpcClient>,
    stream_mgr: &Arc<StreamManager>,
    db: &Arc<PostgresDatabase>,
    tx: &tokio::sync::mpsc::Sender<EventBatch<OnChainEvent>>,
) -> anyhow::Result<()> {
    let poll_interval = Duration::from_millis(*config_local::POLL_INTERVAL_MS);
    let retry_config = RetryConfig::default();

    loop {
        tokio::time::sleep(poll_interval).await;

        let latest_block = rpc.latest_block();
        if latest_block == 0 {
            continue;
        }

        let range = match stream_mgr.get_range(EventType::Lp, latest_block) {
            Some(r) => r,
            None => continue,
        };

        let result = run_event_handler_with_retry("lp_stream", &retry_config, || {
            event::lp::stream::poll_lp_events(rpc, &range, tx)
        })
        .await;

        handle_stream_result("lp_stream", EventType::Lp, &result, &range, stream_mgr, db).await;
    }
}

async fn handle_stream_result(
    name: &str,
    event_type: EventType,
    result: &Result<(), event::error::ObserverError>,
    range: &sync::stream::BlockRange,
    stream_mgr: &Arc<StreamManager>,
    db: &Arc<PostgresDatabase>,
) {
    match result {
        Ok(()) => {
            stream_mgr.advance(event_type, range.to_block);
            if let Err(e) = block_ctrl::set_last_block(
                db.writer(),
                event_type.as_str(),
                range.to_block as i64,
            )
            .await
            {
                tracing::error!(
                    handler = name,
                    error = %e,
                    "Failed to persist block progress"
                );
            }
        }
        Err(e) => {
            tracing::error!(handler = name, error = %e, "Stream handler error");
        }
    }
}
