pub mod session;
pub mod cache;
pub mod rate_limit;

use redis::aio::ConnectionManager;

pub struct RedisDatabase {
    conn: ConnectionManager,
}

impl RedisDatabase {
    pub async fn new(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;
        tracing::info!("Redis connection established");
        Ok(Self { conn })
    }

    pub fn conn(&self) -> ConnectionManager {
        self.conn.clone()
    }
}
