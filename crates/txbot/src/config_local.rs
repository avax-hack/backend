use lazy_static::lazy_static;

lazy_static! {
    /// Polling interval for graduate event checks (milliseconds).
    pub static ref GRADUATE_POLL_MS: u64 = std::env::var("GRADUATE_POLL_MS")
        .unwrap_or_else(|_| "5000".to_string())
        .parse()
        .unwrap_or(5000);

    /// Polling interval for fee collection checks (seconds).
    pub static ref COLLECT_POLL_SECS: u64 = std::env::var("COLLECT_POLL_SECS")
        .unwrap_or_else(|_| "30".to_string())
        .parse()
        .unwrap_or(30);

    /// Minimum fee amount to trigger collection (1 USDC = 1_000_000, 6 decimals).
    pub static ref MIN_COLLECT_AMOUNT: &'static str = "1000000";

    /// Maximum retry attempts for graduate transactions.
    pub static ref GRADUATE_MAX_RETRIES: u32 = std::env::var("GRADUATE_MAX_RETRIES")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap_or(20);

    /// Maximum retry attempts for collect-fees transactions.
    pub static ref COLLECT_MAX_RETRIES: u32 = std::env::var("COLLECT_MAX_RETRIES")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .unwrap_or(5);
}
