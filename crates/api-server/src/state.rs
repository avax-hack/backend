use std::sync::Arc;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::redis::RedisDatabase;
use openlaunch_shared::storage::r2::R2Client;
use openlaunch_shared::utils::single_flight::SingleFlightCache;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PostgresDatabase>,
    pub redis: Arc<RedisDatabase>,
    pub cache: Arc<SingleFlightCache>,
    pub r2: Arc<R2Client>,
}

impl AppState {
    pub fn new(db: Arc<PostgresDatabase>, redis: Arc<RedisDatabase>, r2: Arc<R2Client>) -> Self {
        let cache = Arc::new(SingleFlightCache::new(
            20_000,
            std::time::Duration::from_secs(1),
        ));
        Self { db, redis, cache, r2 }
    }
}
