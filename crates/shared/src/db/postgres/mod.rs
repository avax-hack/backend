pub mod pool;
pub mod controller;

use sqlx::PgPool;

pub struct PostgresDatabase {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresDatabase {
    pub async fn new(primary_url: &str, replica_url: &str) -> anyhow::Result<Self> {
        let mut attempts = 0u32;
        let max_attempts = 10u32;

        loop {
            attempts += 1;
            match Self::try_connect(primary_url, replica_url).await {
                Ok(db) => {
                    tracing::info!("PostgreSQL connection pools initialized");
                    return Ok(db);
                }
                Err(e) if attempts < max_attempts => {
                    tracing::warn!(
                        attempt = attempts,
                        max = max_attempts,
                        error = %e,
                        "Failed to connect to PostgreSQL, retrying in 3s..."
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn try_connect(primary_url: &str, replica_url: &str) -> anyhow::Result<Self> {
        let write_pool = pool::create_pool(primary_url, pool::PoolConfig::writer()).await?;
        let read_pool = pool::create_pool(replica_url, pool::PoolConfig::reader()).await?;
        Ok(Self { write_pool, read_pool })
    }

    pub fn writer(&self) -> &PgPool {
        &self.write_pool
    }

    pub fn reader(&self) -> &PgPool {
        &self.read_pool
    }
}
