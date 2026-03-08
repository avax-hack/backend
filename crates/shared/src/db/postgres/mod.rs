pub mod pool;
pub mod controller;

use sqlx::PgPool;

pub struct PostgresDatabase {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresDatabase {
    pub async fn new(primary_url: &str, replica_url: &str) -> anyhow::Result<Self> {
        let write_pool = pool::create_pool(primary_url, pool::PoolConfig::writer()).await?;
        let read_pool = pool::create_pool(replica_url, pool::PoolConfig::reader()).await?;
        tracing::info!("PostgreSQL connection pools initialized");
        Ok(Self { write_pool, read_pool })
    }

    pub fn writer(&self) -> &PgPool {
        &self.write_pool
    }

    pub fn reader(&self) -> &PgPool {
        &self.read_pool
    }
}
