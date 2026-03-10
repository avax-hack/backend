use std::sync::Arc;
use std::time::Duration;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::client::health::run_health_check;
use openlaunch_shared::client::provider::ProviderId;
use openlaunch_shared::config;
use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::block as block_ctrl;
use openlaunch_shared::db::redis::RedisDatabase;
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

    let redis = Arc::new(RedisDatabase::new(&config::REDIS_URL).await?);

    // Seed Redis whitelist from existing on-chain projects.
    match controller::project::get_token_addresses(db.reader()).await {
        Ok(addrs) => {
            for addr in &addrs {
                let _ = redis.whitelist_add_token(addr).await;
            }
            tracing::info!(count = addrs.len(), "Seeded Redis token whitelist from DB");
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to seed token whitelist from DB");
        }
    }

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
    let (token_tx, mut token_rx) = tokio::sync::mpsc::channel::<EventBatch<OnChainEvent>>(128);
    let (swap_tx, mut swap_rx) = tokio::sync::mpsc::channel::<EventBatch<RawSwapEvent>>(128);
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
        let tx = ido_tx.clone();
        join_set.spawn(async move {
            run_ido_stream(&rpc, &stream_mgr, &tx).await
        });
    }

    // Spawn IDO receive
    {
        let db = Arc::clone(&db);
        let receive_mgr = Arc::clone(&receive_mgr);
        let redis = Arc::clone(&redis);
        join_set.spawn(async move {
            event::ido::receive::process_ido_events(
                db.writer(),
                &mut ido_rx,
                &receive_mgr,
                &redis,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn LP stream
    {
        let rpc = Arc::clone(&rpc);
        let stream_mgr = Arc::clone(&stream_mgr);
        let tx = lp_tx.clone();
        join_set.spawn(async move {
            run_lp_stream(&rpc, &stream_mgr, &tx).await
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
                db.reader(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
        });
    }

    // Spawn Swap stream (from PoolManager)
    {
        let rpc = Arc::clone(&rpc);
        let stream_mgr = Arc::clone(&stream_mgr);
        let tx = swap_tx.clone();
        join_set.spawn(async move {
            run_swap_stream(&rpc, &stream_mgr, &tx).await
        });
    }

    // Spawn Token Transfer stream
    {
        let rpc = Arc::clone(&rpc);
        let redis = Arc::clone(&redis);
        let stream_mgr = Arc::clone(&stream_mgr);
        let tx = token_tx.clone();
        join_set.spawn(async move {
            run_token_stream(&rpc, &redis, &stream_mgr, &tx).await
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
            event::swap::receive::process_swap_events(
                db.writer(),
                &mut swap_rx,
                &receive_mgr,
                db.reader(),
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
    // Persists the MINIMUM of stream and receive progress for each event type,
    // ensuring we only record blocks that have been fully processed on both sides.
    {
        let db = Arc::clone(&db);
        let stream_mgr = Arc::clone(&stream_mgr);
        let receive_mgr_persist = Arc::clone(&receive_mgr);
        join_set.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                for event_type in EventType::all() {
                    let stream_block = stream_mgr.current_block(*event_type);
                    let receive_block = receive_mgr_persist.completed_block(*event_type);

                    let safe_block = match (stream_block, receive_block) {
                        (Some(s), Some(r)) => s.min(r),
                        _ => continue,
                    };

                    if let Err(e) = block_ctrl::set_last_block(
                        db.writer(),
                        event_type.as_str(),
                        safe_block as i64,
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
        });
    }

    // Spawn periodic volume_24h refresh (every 60 seconds)
    // Ensures volumes decay as swaps age out of the 24h window
    {
        let db = Arc::clone(&db);
        join_set.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                if let Err(e) = openlaunch_shared::db::postgres::controller::market::refresh_all_volumes_24h(db.writer()).await {
                    tracing::error!(error = %e, "Failed to refresh volume_24h");
                }
            }
        });
    }

    tracing::info!("Observer running. Press Ctrl+C to stop.");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutdown signal received");
            Ok(())
        }
        Some(result) = join_set.join_next() => {
            match result {
                Ok(Ok(())) => {
                    tracing::info!("A task completed unexpectedly");
                    Err(anyhow::anyhow!("A task exited unexpectedly without error"))
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "A task failed with error");
                    Err(anyhow::anyhow!("Task failed: {e}"))
                }
                Err(e) => {
                    tracing::error!(error = %e, "A task panicked");
                    Err(anyhow::anyhow!("Task panicked: {e}"))
                }
            }
        }
    }
}

async fn run_ido_stream(
    rpc: &Arc<RpcClient>,
    stream_mgr: &Arc<StreamManager>,
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

        handle_stream_result("ido_stream", EventType::Ido, &result, &range, stream_mgr).await;
    }
}

async fn run_lp_stream(
    rpc: &Arc<RpcClient>,
    stream_mgr: &Arc<StreamManager>,
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

        handle_stream_result("lp_stream", EventType::Lp, &result, &range, stream_mgr).await;
    }
}

async fn run_swap_stream(
    rpc: &Arc<RpcClient>,
    stream_mgr: &Arc<StreamManager>,
    tx: &tokio::sync::mpsc::Sender<EventBatch<RawSwapEvent>>,
) -> anyhow::Result<()> {
    let poll_interval = Duration::from_millis(*config_local::POLL_INTERVAL_MS);
    let retry_config = RetryConfig::default();

    loop {
        tokio::time::sleep(poll_interval).await;

        let latest_block = rpc.latest_block();
        if latest_block == 0 {
            continue;
        }

        let range = match stream_mgr.get_range(EventType::Swap, latest_block) {
            Some(r) => r,
            None => continue,
        };

        let result = run_event_handler_with_retry("swap_stream", &retry_config, || {
            event::swap::stream::poll_swap_events(rpc, &range, tx)
        })
        .await;

        handle_stream_result("swap_stream", EventType::Swap, &result, &range, stream_mgr).await;
    }
}

async fn run_token_stream(
    rpc: &Arc<RpcClient>,
    redis: &Arc<RedisDatabase>,
    stream_mgr: &Arc<StreamManager>,
    tx: &tokio::sync::mpsc::Sender<EventBatch<OnChainEvent>>,
) -> anyhow::Result<()> {
    let poll_interval = Duration::from_millis(*config_local::POLL_INTERVAL_MS);
    let retry_config = RetryConfig::default();

    loop {
        tokio::time::sleep(poll_interval).await;

        // Read token whitelist from Redis (updated instantly by IDO receive).
        let token_addresses = redis.whitelist_get_tokens().await.unwrap_or_default();
        if token_addresses.is_empty() {
            continue;
        }

        let latest_block = rpc.latest_block();
        if latest_block == 0 {
            continue;
        }

        let range = match stream_mgr.get_range(EventType::Token, latest_block) {
            Some(r) => r,
            None => continue,
        };

        let result = run_event_handler_with_retry("token_stream", &retry_config, || {
            event::token::stream::poll_token_events(rpc, &range, &token_addresses, tx)
        })
        .await;

        handle_stream_result("token_stream", EventType::Token, &result, &range, stream_mgr).await;
    }
}

async fn handle_stream_result(
    name: &str,
    event_type: EventType,
    result: &Result<(), event::error::ObserverError>,
    range: &sync::stream::BlockRange,
    stream_mgr: &Arc<StreamManager>,
) {
    match result {
        Ok(()) => {
            stream_mgr.advance(event_type, range.to_block);
            tracing::info!(
                handler = name,
                from = range.from_block,
                to = range.to_block,
                "Polled blocks"
            );
        }
        Err(e) => {
            tracing::error!(handler = name, error = %e, "Stream handler error");
        }
    }
}
