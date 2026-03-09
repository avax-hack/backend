use lazy_static::lazy_static;

lazy_static! {
    pub static ref WS_CHANNEL_SIZE: usize = std::env::var("WS_CHANNEL_SIZE")
        .unwrap_or_else(|_| "1024".to_string())
        .parse()
        .unwrap_or(1024);

    pub static ref WS_CLEANUP_INTERVAL_SECS: u64 = std::env::var("WS_CLEANUP_INTERVAL_SECS")
        .unwrap_or_else(|_| "300".to_string())
        .parse()
        .unwrap_or(300);

    pub static ref WS_MAX_SUBSCRIPTIONS_PER_CONN: usize = std::env::var("WS_MAX_SUBSCRIPTIONS_PER_CONN")
        .unwrap_or_else(|_| "100".to_string())
        .parse()
        .unwrap_or(100);
}
