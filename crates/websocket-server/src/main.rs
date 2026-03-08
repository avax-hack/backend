use std::sync::Arc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::client::provider::ProviderId;
use openlaunch_shared::config;
use tower_http::cors::CorsLayer;

mod cache;
mod config_local;
mod event;
mod server;
mod stream;

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

    // Spawn IDO event stream.
    let ido_producers = Arc::clone(&producers);
    let ido_cache = Arc::clone(&price_cache);
    tokio::spawn(async move {
        if let Err(e) = stream::ido::stream::start_ido_stream(ido_producers, ido_cache).await {
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

    // Build HTTP + WebSocket router.
    let app_state = server::AppState {
        producers: Arc::clone(&producers),
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
