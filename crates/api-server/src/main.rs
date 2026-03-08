use std::sync::Arc;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::redis::RedisDatabase;
use openlaunch_shared::storage::r2::R2Client;
use openlaunch_shared::config;

mod state;
mod cors;
mod router;
mod middleware;
mod services;
mod openapi;

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

    tracing::info!("Starting OpenLaunch API Server");

    let db = Arc::new(
        PostgresDatabase::new(&config::PRIMARY_DATABASE_URL, &config::REPLICA_DATABASE_URL).await?,
    );
    let redis = Arc::new(RedisDatabase::new(&config::REDIS_URL).await?);
    let r2 = Arc::new(R2Client::new().await?);

    let state = state::AppState::new(db, redis, r2);
    let app = router::build_router(state);

    let ip = std::env::var("API_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("API_PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("{ip}:{port}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
