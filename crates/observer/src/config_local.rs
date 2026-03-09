use lazy_static::lazy_static;

lazy_static! {
    pub static ref POLL_INTERVAL_MS: u64 = std::env::var("OBSERVER_POLL_INTERVAL_MS")
        .unwrap_or_else(|_| "1000".to_string())
        .parse()
        .unwrap_or(1000);

    pub static ref BATCH_SIZE: u64 = std::env::var("OBSERVER_BATCH_SIZE")
        .unwrap_or_else(|_| "100".to_string())
        .parse()
        .unwrap_or(100);

    pub static ref START_BLOCK: u64 = std::env::var("OBSERVER_START_BLOCK")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .unwrap_or(0);
}
