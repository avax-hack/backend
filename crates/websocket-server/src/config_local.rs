use std::sync::LazyLock;

pub static WS_CHANNEL_SIZE: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("WS_CHANNEL_SIZE")
        .unwrap_or_else(|_| "1024".to_string())
        .parse()
        .unwrap_or(1024)
});

pub static WS_CLEANUP_INTERVAL_SECS: LazyLock<u64> = LazyLock::new(|| {
    std::env::var("WS_CLEANUP_INTERVAL_SECS")
        .unwrap_or_else(|_| "300".to_string())
        .parse()
        .unwrap_or(300)
});

pub static WS_MAX_SUBSCRIPTIONS_PER_CONN: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("WS_MAX_SUBSCRIPTIONS_PER_CONN")
        .unwrap_or_else(|_| "100".to_string())
        .parse()
        .unwrap_or(100)
});
