use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::client::provider::ProviderId;
use openlaunch_shared::config;
use tower_http::cors::CorsLayer;

mod cache;
mod config_local;
mod event;
mod server;
mod stream;
mod candle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Install default rustls CryptoProvider for WebSocket TLS connections.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!("Starting OpenLaunch WebSocket Server");

    let _rpc = RpcClient::init(vec![
        (ProviderId::Main, config::MAIN_RPC_URL.clone()),
        (ProviderId::Sub1, config::SUB_RPC_URL_1.clone()),
        (ProviderId::Sub2, config::SUB_RPC_URL_2.clone()),
    ])
    .await?;

    // Initialize event producers.
    let producers = event::EventProducers::new();

    // Initialize price cache.
    let price_cache = Arc::new(cache::PriceCache::new());

    // Initialize in-memory candle manager.
    let candle_mgr = Arc::new(candle::CandleManager::new());

    // Spawn IDO event stream.
    let ido_producers = Arc::clone(&producers);
    let ido_cache = Arc::clone(&price_cache);
    let ido_candle = Arc::clone(&candle_mgr);
    tokio::spawn(async move {
        if let Err(e) = stream::ido::stream::start_ido_stream(ido_producers, ido_cache, ido_candle).await {
            tracing::error!(error = %e, "IDO stream terminated with error");
        }
    });

    // Spawn Pool event stream.
    let pool_producers = Arc::clone(&producers);
    let pool_cache = Arc::clone(&price_cache);
    tokio::spawn(async move {
        if let Err(e) = stream::pool::stream::start_pool_stream(pool_producers, pool_cache).await {
            tracing::error!(error = %e, "Pool stream terminated with error");
        }
    });

    // Initialize database pool for DEX stream pool mappings.
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL required for DEX stream");
    let db_pool = sqlx::PgPool::connect(&database_url).await?;

    // Spawn DEX Swap event stream.
    let dex_producers = Arc::clone(&producers);
    let dex_cache = Arc::clone(&price_cache);
    let dex_candle = Arc::clone(&candle_mgr);
    let dex_db = db_pool.clone();
    tokio::spawn(async move {
        if let Err(e) = stream::dex::stream::start_dex_stream(dex_producers, dex_cache, dex_candle, dex_db).await {
            tracing::error!(error = %e, "DEX stream terminated with error");
        }
    });

    // Build HTTP + WebSocket router.
    let max_connections: usize = std::env::var("WS_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "1000".to_string())
        .parse()
        .unwrap_or(1000);

    let app_state = server::AppState {
        producers: Arc::clone(&producers),
        connection_count: Arc::new(AtomicUsize::new(0)),
        max_connections,
    };

    let app = server::build_router(app_state).layer(CorsLayer::permissive());

    let ip = std::env::var("WS_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("WS_PORT").unwrap_or_else(|_| "8001".to_string());
    let addr = format!("{ip}:{port}");

    tracing::info!("WebSocket server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("WebSocket server shut down");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    tracing::info!("Shutdown signal received");
}
