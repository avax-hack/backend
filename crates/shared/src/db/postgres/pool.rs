use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub max_lifetime: Duration,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
}

impl PoolConfig {
    pub fn writer() -> Self {
        Self {
            max_connections: *crate::config::PG_PRIMARY_MAX_CONNECTIONS,
            min_connections: *crate::config::PG_PRIMARY_MIN_CONNECTIONS,
            max_lifetime: Duration::from_secs(1800),
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
        }
    }

    pub fn reader() -> Self {
        Self {
            max_connections: *crate::config::PG_REPLICA_MAX_CONNECTIONS,
            min_connections: *crate::config::PG_REPLICA_MIN_CONNECTIONS,
            max_lifetime: Duration::from_secs(1800),
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
        }
    }
}

pub async fn create_pool(url: &str, config: PoolConfig) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .max_lifetime(config.max_lifetime)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .connect(url)
        .await?;
    Ok(pool)
}
