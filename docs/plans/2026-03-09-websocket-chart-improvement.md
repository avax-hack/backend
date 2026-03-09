# WebSocket Chart Improvement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade WebSocket chart to provide full OHLCV candles across 6 timeframes with in-memory candle management, resolution-based subscriptions, and DEX swap event support.

**Architecture:** In-memory CandleManager (DashMap) holds active candles per (token_id, interval). IDO and DEX streams feed price+volume updates into CandleManager, which updates all 6 timeframes and broadcasts to per-interval chart subscribers. DB connection added for loading pool mappings on startup.

**Tech Stack:** Rust, Tokio, Alloy (blockchain), DashMap (concurrent map), Axum (WebSocket)

---

### Task 1: CandleManager — Core Data Structures and Logic

**Files:**
- Create: `crates/websocket-server/src/candle/mod.rs`

This is the heart of the system. An in-memory store of OHLCV candles keyed by `(token_id, interval)`.

**Step 1: Write the test**

Add to bottom of the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_creates_new_candle() {
        let mgr = CandleManager::new();
        mgr.update("0xabc", 1.5, 100.0, 1700000000);

        let candle = mgr.get("0xabc", "1m").unwrap();
        assert_eq!(candle.open, 1.5);
        assert_eq!(candle.high, 1.5);
        assert_eq!(candle.low, 1.5);
        assert_eq!(candle.close, 1.5);
        assert_eq!(candle.volume, 100.0);
        // 1m bucket: (1700000000 / 60) * 60 = 1700000000
        assert_eq!(candle.time, 1700000000);
    }

    #[test]
    fn test_update_existing_candle_same_bucket() {
        let mgr = CandleManager::new();
        mgr.update("0xabc", 1.5, 100.0, 1700000000);
        mgr.update("0xabc", 2.0, 50.0, 1700000010); // same 1m bucket

        let candle = mgr.get("0xabc", "1m").unwrap();
        assert_eq!(candle.open, 1.5);   // open never changes
        assert_eq!(candle.high, 2.0);   // max
        assert_eq!(candle.low, 1.5);    // min
        assert_eq!(candle.close, 2.0);  // latest
        assert_eq!(candle.volume, 150.0); // accumulated
    }

    #[test]
    fn test_update_new_bucket_resets_candle() {
        let mgr = CandleManager::new();
        mgr.update("0xabc", 1.5, 100.0, 1700000000);
        mgr.update("0xabc", 2.0, 50.0, 1700000060); // next 1m bucket

        let candle = mgr.get("0xabc", "1m").unwrap();
        assert_eq!(candle.open, 1.5);   // previous close becomes open
        assert_eq!(candle.high, 2.0);
        assert_eq!(candle.low, 1.5);
        assert_eq!(candle.close, 2.0);
        assert_eq!(candle.volume, 50.0); // reset
        assert_eq!(candle.time, 1700000060);
    }

    #[test]
    fn test_all_six_intervals_updated() {
        let mgr = CandleManager::new();
        mgr.update("0xabc", 1.5, 100.0, 1700000000);

        assert!(mgr.get("0xabc", "1m").is_some());
        assert!(mgr.get("0xabc", "5m").is_some());
        assert!(mgr.get("0xabc", "15m").is_some());
        assert!(mgr.get("0xabc", "1h").is_some());
        assert!(mgr.get("0xabc", "4h").is_some());
        assert!(mgr.get("0xabc", "1d").is_some());
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = CandleManager::new();
        assert!(mgr.get("0xabc", "1m").is_none());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p openlaunch-websocket-server candle`
Expected: FAIL — module doesn't exist

**Step 3: Write minimal implementation**

```rust
use dashmap::DashMap;

/// OHLCV candle data.
#[derive(Debug, Clone)]
pub struct Candle {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Supported chart intervals with their duration in seconds.
const INTERVALS: &[(&str, i64)] = &[
    ("1m", 60),
    ("5m", 300),
    ("15m", 900),
    ("1h", 3600),
    ("4h", 14400),
    ("1d", 86400),
];

/// In-memory OHLCV candle store keyed by (token_id, interval).
pub struct CandleManager {
    candles: DashMap<(String, String), Candle>,
}

impl CandleManager {
    pub fn new() -> Self {
        Self {
            candles: DashMap::new(),
        }
    }

    /// Update all 6 timeframes for a given token with a new trade.
    pub fn update(&self, token_id: &str, price: f64, volume: f64, timestamp: i64) {
        for &(interval, secs) in INTERVALS {
            let bucket_time = (timestamp / secs) * secs;
            let key = (token_id.to_lowercase(), interval.to_string());

            self.candles
                .entry(key)
                .and_modify(|c| {
                    if c.time == bucket_time {
                        // Same bucket: update H/L/C, accumulate volume
                        if price > c.high {
                            c.high = price;
                        }
                        if price < c.low {
                            c.low = price;
                        }
                        c.close = price;
                        c.volume += volume;
                    } else {
                        // New bucket: reset candle, open = previous close
                        let prev_close = c.close;
                        *c = Candle {
                            time: bucket_time,
                            open: prev_close,
                            high: if price > prev_close { price } else { prev_close },
                            low: if price < prev_close { price } else { prev_close },
                            close: price,
                            volume,
                        };
                    }
                })
                .or_insert(Candle {
                    time: bucket_time,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                });
        }
    }

    /// Get the current candle for a token and interval.
    pub fn get(&self, token_id: &str, interval: &str) -> Option<Candle> {
        let key = (token_id.to_lowercase(), interval.to_string());
        self.candles.get(&key).map(|c| c.clone())
    }

    /// Returns all supported interval names.
    pub fn intervals() -> &'static [(&'static str, i64)] {
        INTERVALS
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p openlaunch-websocket-server candle`
Expected: PASS — all 5 tests

**Step 5: Register the module in main.rs**

Add `mod candle;` to `crates/websocket-server/src/main.rs` after line 11 (`mod stream;`).

**Step 6: Commit**

```bash
git add crates/websocket-server/src/candle/mod.rs crates/websocket-server/src/main.rs
git commit -m "feat(ws): add CandleManager for in-memory OHLCV candle state"
```

---

### Task 2: Update SubscriptionKey to Include Interval

**Files:**
- Modify: `crates/websocket-server/src/event/core.rs:22,35`

Change `Chart(String)` to `Chart(String, String)` to hold `(token_id, interval)`.

**Step 1: Update the enum and channel key**

In `crates/websocket-server/src/event/core.rs`:

Change line 22:
```rust
    Chart(String),
```
to:
```rust
    /// Chart bar updates for a specific token address and interval.
    Chart(String, String),
```

Change line 35:
```rust
            Self::Chart(id) => format!("chart:{id}"),
```
to:
```rust
            Self::Chart(id, interval) => format!("chart:{id}:{interval}"),
```

**Step 2: Add test for new key format**

Add to the existing test:
```rust
    #[test]
    fn test_chart_subscription_key_includes_interval() {
        let key = SubscriptionKey::Chart("0xabc".to_string(), "5m".to_string());
        assert_eq!(key.to_channel_key(), "chart:0xabc:5m");
    }
```

**Step 3: Run tests**

Run: `cargo test -p openlaunch-websocket-server`
Expected: Compile errors in files still using `Chart(String)` — fix them in next steps.

**Step 4: Fix all usages of `SubscriptionKey::Chart`**

Files to update:

1. `crates/websocket-server/src/event/chart.rs:18,28` — `ChartEventProducer` uses `SubscriptionKey::Chart(...)`. Update to accept interval parameter:

```rust
    pub fn publish_chart(&self, token_address: &str, interval: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Chart(token_address.to_lowercase(), interval.to_string());
        let event = WsEvent {
            method: "chart_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    pub fn subscribe(&self, token_address: &str, interval: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Chart(token_address.to_lowercase(), interval.to_string());
        self.inner.subscribe(&key.to_channel_key())
    }
```

Update test in same file:
```rust
    #[test]
    fn test_chart_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = ChartEventProducer::new(inner);

        let mut rx = producer.subscribe("0xABC", "1m");
        producer.publish_chart("0xabc", "1m", serde_json::json!({"time": 100, "close": "1.5"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "chart_subscribe");
    }
```

2. `crates/websocket-server/src/server/socket/rpc.rs:147-156` — `chart_subscribe` dispatch. Update to extract both `token_id` and `resolution`:

Replace the `"chart_subscribe"` match arm (lines 147-156) with:

```rust
        "chart_subscribe" => {
            handle_chart_subscribe(request, &producers.chart, conn, outbound_tx)
        }
```

Add a new function `handle_chart_subscribe` after `handle_global_subscribe`:

```rust
/// Handle chart subscription with token_id and resolution parameters.
fn handle_chart_subscribe(
    request: &JsonRpcRequest,
    producer: &Arc<dyn crate::event::EventProducer>,
    conn: &mut ConnectionState,
    outbound_tx: &mpsc::Sender<String>,
) -> JsonRpcResponse {
    let token_id = request.params.get("token_id").and_then(|v| v.as_str());
    let resolution = request.params.get("resolution").and_then(|v| v.as_str());

    let Some(raw_id) = token_id else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Missing required param: token_id".to_string(),
        );
    };

    let interval = resolve_interval(resolution.unwrap_or("1"));

    let Some(interval) = interval else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Invalid resolution. Supported: 1, 5, 15, 60, 240, 1D".to_string(),
        );
    };

    let normalized_id = raw_id.to_lowercase();
    let sub_key = SubscriptionKey::Chart(normalized_id, interval.to_string());
    let channel_key = sub_key.to_channel_key();
    let method = request.method.clone();

    let mut rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let push = JsonRpcPush::new(method.clone(), channel_key_for_task.clone(), event.data);
                    if let Ok(json) = serde_json::to_string(&push) {
                        if tx.send(json).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(channel = %channel_key_for_task, lagged = n, "Chart subscriber lagged");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    conn.subscribe(channel_key, handle);

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"subscribed": true}),
    )
}

/// Map TradingView-style resolution strings to interval names.
fn resolve_interval(resolution: &str) -> Option<&'static str> {
    match resolution {
        "1" | "1m" => Some("1m"),
        "5" | "5m" => Some("5m"),
        "15" | "15m" => Some("15m"),
        "60" | "1h" => Some("1h"),
        "240" | "4h" => Some("4h"),
        "1D" | "1d" => Some("1d"),
        _ => None,
    }
}
```

3. `crates/websocket-server/src/stream/ido/receive.rs:143-154` — chart publish in `handle_tokens_purchased`. Remove the old chart publish block (lines 137-154). Chart publishing will be handled by CandleManager in Task 4.

**Step 5: Run tests**

Run: `cargo test -p openlaunch-websocket-server`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/websocket-server/src/event/core.rs crates/websocket-server/src/event/chart.rs \
  crates/websocket-server/src/server/socket/rpc.rs crates/websocket-server/src/stream/ido/receive.rs
git commit -m "feat(ws): add resolution parameter to chart_subscribe"
```

---

### Task 3: Integrate CandleManager into IDO Stream

**Files:**
- Modify: `crates/websocket-server/src/stream/ido/receive.rs`
- Modify: `crates/websocket-server/src/main.rs`
- Modify: `crates/websocket-server/src/server/mod.rs`

On `TokensPurchased`, compute price and feed into CandleManager. CandleManager broadcasts to all 6 intervals.

**Step 1: Add CandleManager to the IDO receive handler signature**

In `crates/websocket-server/src/stream/ido/receive.rs`, update `handle_ido_log`:

```rust
use crate::candle::CandleManager;

pub fn handle_ido_log(
    log: &Log,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
) -> anyhow::Result<()> {
```

Update the `TokensPurchased` call:
```rust
    } else if signature == IIDO::TokensPurchased::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::TokensPurchased>()?;
        handle_tokens_purchased(&decoded.inner.data, producers, price_cache, candle_mgr);
    }
```

**Step 2: Update `handle_tokens_purchased` to use CandleManager**

Replace the old chart publish block (lines 137-154) with CandleManager integration:

```rust
fn handle_tokens_purchased(
    event: &IIDO::TokensPurchased,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
) {
    // ... existing project, trade, price publish code stays ...

    // Update in-memory candles and broadcast to all intervals
    let price_f64: f64 = price_str.parse().unwrap_or(0.0);
    let volume_f64: f64 = usdc_amount.parse().unwrap_or(0.0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    candle_mgr.update(&token, price_f64, volume_f64, now);

    // Broadcast updated candle for each interval
    for &(interval, _) in CandleManager::intervals() {
        if let Some(candle) = candle_mgr.get(&token, interval) {
            let chart_data = serde_json::json!({
                "type": "CHART_UPDATE",
                "token_id": token,
                "interval": interval,
                "o": candle.open,
                "h": candle.high,
                "l": candle.low,
                "c": candle.close,
                "v": candle.volume,
                "t": candle.time,
            });
            let chart_key = SubscriptionKey::Chart(token.clone(), interval.to_string()).to_channel_key();
            producers.chart.publish(&chart_key, WsEvent {
                method: "chart_subscribe".to_string(),
                data: chart_data,
            });
        }
    }

    tracing::info!(token = %token, buyer = %buyer, "TokensPurchased event forwarded to project, trade, price, and chart channels");
}
```

**Step 3: Thread CandleManager through main.rs**

In `crates/websocket-server/src/main.rs`, after `let price_cache = ...` (line 45), add:

```rust
    // Initialize in-memory candle manager.
    let candle_mgr = Arc::new(candle::CandleManager::new());
```

Update the IDO stream spawn to pass `candle_mgr`:

```rust
    let ido_candle = Arc::clone(&candle_mgr);
    tokio::spawn(async move {
        if let Err(e) = stream::ido::stream::start_ido_stream(ido_producers, ido_cache, ido_candle).await {
            tracing::error!(error = %e, "IDO stream terminated with error");
        }
    });
```

**Step 4: Update IDO stream.rs to pass CandleManager**

In `crates/websocket-server/src/stream/ido/stream.rs`, update all function signatures to accept `Arc<CandleManager>` and pass it to `receive::handle_ido_log`.

`start_ido_stream`:
```rust
pub async fn start_ido_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
    candle_mgr: Arc<CandleManager>,
) -> anyhow::Result<()> {
```

`run_ido_subscription`:
```rust
async fn run_ido_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
) -> anyhow::Result<()> {
```

Update the log handler call:
```rust
        if let Err(e) = receive::handle_ido_log(&log, producers, price_cache, candle_mgr) {
```

**Step 5: Run tests**

Run: `cargo test -p openlaunch-websocket-server`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/websocket-server/src/stream/ido/receive.rs \
  crates/websocket-server/src/stream/ido/stream.rs \
  crates/websocket-server/src/main.rs
git commit -m "feat(ws): integrate CandleManager into IDO stream"
```

---

### Task 4: Add DEX Swap Stream

**Files:**
- Create: `crates/websocket-server/src/stream/dex/mod.rs`
- Create: `crates/websocket-server/src/stream/dex/stream.rs`
- Create: `crates/websocket-server/src/stream/dex/receive.rs`
- Modify: `crates/websocket-server/src/stream/mod.rs`
- Modify: `crates/websocket-server/src/main.rs`
- Modify: `crates/websocket-server/Cargo.toml` (add `sqlx` for DB pool mappings)

The DEX stream subscribes to PoolManager Swap events, resolves pool_id → token_id via in-memory mappings (loaded from DB), computes price, and feeds CandleManager.

**Step 1: Add sqlx dependency**

In `crates/websocket-server/Cargo.toml`, add:
```toml
sqlx = { workspace = true }
```

**Step 2: Create `stream/dex/mod.rs`**

```rust
pub mod stream;
pub mod receive;
```

**Step 3: Create `stream/dex/receive.rs`**

```rust
use std::sync::Arc;

use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;

use crate::candle::CandleManager;
use crate::cache::PriceCache;
use crate::event::EventProducers;
use crate::event::core::{SubscriptionKey, WsEvent};

use super::stream::PoolMapping;

// Uniswap V4 PoolManager Swap event.
sol! {
    event Swap(
        bytes32 indexed id,
        address indexed sender,
        int128 amount0,
        int128 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick,
        uint24 fee
    );
}

/// Handle a raw Swap log from the PoolManager contract.
pub fn handle_swap_log(
    log: &Log,
    mappings: &[PoolMapping],
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
) -> anyhow::Result<()> {
    let topics = log.topics();
    if topics.is_empty() {
        return Ok(());
    }

    let signature = topics[0];
    if signature != Swap::SIGNATURE_HASH {
        return Ok(());
    }

    let decoded = log.log_decode::<Swap>()?;
    let event = &decoded.inner;

    let pool_id = format!("{:#x}", event.id);
    let mapping = match mappings.iter().find(|m| m.pool_id == pool_id) {
        Some(m) => m,
        None => return Ok(()), // Unknown pool, skip
    };

    let amount0: i128 = event.amount0.as_i128();
    let amount1: i128 = event.amount1.as_i128();

    let (native_amount, token_amount, event_type) = if mapping.is_token0 {
        let token_amt = amount0.unsigned_abs();
        let native_amt = amount1.unsigned_abs();
        let evt = if amount0 > 0 { "BUY" } else { "SELL" };
        (native_amt, token_amt, evt)
    } else {
        let token_amt = amount1.unsigned_abs();
        let native_amt = amount0.unsigned_abs();
        let evt = if amount1 > 0 { "BUY" } else { "SELL" };
        (native_amt, token_amt, evt)
    };

    if token_amount == 0 {
        return Ok(());
    }

    // price = (native / 1e6) / (token / 1e18) = native * 1e12 / token
    let price = (native_amount as f64 * 1e12) / token_amount as f64;
    let volume = native_amount as f64;
    let token_id = &mapping.token_id;

    let price_str = format!("{price:.18}");
    price_cache.set_price(token_id, price_str.clone());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Update candles
    candle_mgr.update(token_id, price, volume, now);

    // Broadcast chart updates for all intervals
    let token_lower = token_id.to_lowercase();
    for &(interval, _) in CandleManager::intervals() {
        if let Some(candle) = candle_mgr.get(&token_lower, interval) {
            let chart_data = serde_json::json!({
                "type": "CHART_UPDATE",
                "token_id": token_lower,
                "interval": interval,
                "o": candle.open,
                "h": candle.high,
                "l": candle.low,
                "c": candle.close,
                "v": candle.volume,
                "t": candle.time,
            });
            let chart_key = SubscriptionKey::Chart(token_lower.clone(), interval.to_string()).to_channel_key();
            producers.chart.publish(&chart_key, WsEvent {
                method: "chart_subscribe".to_string(),
                data: chart_data,
            });
        }
    }

    // Broadcast trade event
    let sender = format!("{:#x}", event.sender);
    let trade_data = serde_json::json!({
        "type": "TRADE",
        "token": token_lower,
        "sender": sender,
        "event_type": event_type,
        "native_amount": native_amount.to_string(),
        "token_amount": token_amount.to_string(),
    });
    let trade_key = SubscriptionKey::Trade(token_lower.clone()).to_channel_key();
    producers.trade.publish(&trade_key, WsEvent {
        method: "trade_subscribe".to_string(),
        data: trade_data,
    });

    // Broadcast price update
    let price_data = serde_json::json!({
        "type": "PRICE_UPDATE",
        "token_id": token_lower,
        "price": price_str,
    });
    let price_key = SubscriptionKey::Price(token_lower.clone()).to_channel_key();
    producers.price.publish(&price_key, WsEvent {
        method: "price_subscribe".to_string(),
        data: price_data,
    });

    tracing::info!(
        token = %token_lower,
        event_type = %event_type,
        price = %price_str,
        "DEX Swap event forwarded to chart, trade, and price channels"
    );

    Ok(())
}
```

**Step 4: Create `stream/dex/stream.rs`**

```rust
use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::primitives::Address;
use alloy::transports::ws::WsConnect;
use alloy::sol_types::SolEvent;
use futures_util::StreamExt;
use sqlx::PgPool;

use openlaunch_shared::config;

use crate::cache::PriceCache;
use crate::candle::CandleManager;
use crate::event::EventProducers;
use super::receive;

/// Pool ID → Token mapping loaded from database.
#[derive(Debug, Clone)]
pub struct PoolMapping {
    pub pool_id: String,
    pub token_id: String,
    pub is_token0: bool,
}

/// Load pool mappings from the database.
async fn load_mappings(db: &PgPool) -> Vec<PoolMapping> {
    let rows = sqlx::query_as::<_, PoolMappingRow>(
        "SELECT pool_id, token_id, is_token0 FROM pool_mappings",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| PoolMapping {
            pool_id: r.pool_id,
            token_id: r.token_id,
            is_token0: r.is_token0,
        })
        .collect()
}

#[derive(Debug, sqlx::FromRow)]
struct PoolMappingRow {
    pool_id: String,
    token_id: String,
    is_token0: bool,
}

/// Start streaming PoolManager Swap events from the blockchain.
/// Periodically reloads pool mappings from DB.
pub async fn start_dex_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
    candle_mgr: Arc<CandleManager>,
    db: PgPool,
) -> anyhow::Result<()> {
    let rpc_url = config::MAIN_RPC_URL.clone();
    let ws_url = rpc_url_to_ws(&rpc_url);

    tracing::info!(url = %ws_url, "Connecting to DEX Swap event stream");

    loop {
        match run_dex_subscription(&ws_url, &producers, &price_cache, &candle_mgr, &db).await {
            Ok(()) => {
                tracing::warn!("DEX stream ended unexpectedly, reconnecting...");
            }
            Err(e) => {
                tracing::error!(error = %e, "DEX stream error, reconnecting in 5s...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn run_dex_subscription(
    ws_url: &str,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db: &PgPool,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let pool_manager_addr: Address = config::POOL_MANAGER_CONTRACT.parse()?;
    let filter = Filter::new()
        .address(pool_manager_addr)
        .event_signature(receive::Swap::SIGNATURE_HASH);

    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    // Load initial mappings
    let mut mappings = load_mappings(db).await;
    tracing::info!(count = mappings.len(), "DEX stream connected, loaded pool mappings");

    let mut event_count: u64 = 0;
    let reload_interval: u64 = 100;

    while let Some(log) = stream.next().await {
        event_count += 1;

        // Periodically reload mappings to pick up new pools
        if event_count % reload_interval == 0 {
            mappings = load_mappings(db).await;
            tracing::info!(count = mappings.len(), "Reloaded pool mappings (periodic)");
        }

        if let Err(e) = receive::handle_swap_log(&log, &mappings, producers, price_cache, candle_mgr) {
            tracing::error!(error = %e, "Failed to handle DEX Swap log");
        }
    }

    Ok(())
}

fn rpc_url_to_ws(url: &str) -> String {
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return url.to_string();
    }
    let ws = url
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    if ws.ends_with("/rpc") {
        ws[..ws.len() - 4].to_string() + "/ws"
    } else {
        ws
    }
}
```

**Step 5: Update `stream/mod.rs`**

```rust
pub mod ido;
pub mod pool;
pub mod dex;
```

**Step 6: Update `main.rs` to spawn DEX stream**

Add DB pool initialization after the RPC init block. Add DEX stream spawn after Pool stream spawn:

```rust
    // Initialize database pool for DEX stream pool mappings.
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL required for DEX stream");
    let db_pool = sqlx::PgPool::connect(&database_url).await?;

    // ... (existing IDO and Pool spawns) ...

    // Spawn DEX Swap event stream.
    let dex_producers = Arc::clone(&producers);
    let dex_cache = Arc::clone(&price_cache);
    let dex_candle = Arc::clone(&candle_mgr);
    let dex_db = db_pool.clone();
    tokio::spawn(async move {
        if let Err(e) = stream::dex::stream::start_dex_stream(dex_producers, dex_cache, dex_candle, dex_db).await {
            tracing::error!(error = %e, "DEX stream terminated with error");
        }
    });
```

**Step 7: Run tests and build**

Run: `cargo build -p openlaunch-websocket-server`
Expected: Compiles successfully

Run: `cargo test -p openlaunch-websocket-server`
Expected: PASS

**Step 8: Commit**

```bash
git add crates/websocket-server/src/stream/dex/ \
  crates/websocket-server/src/stream/mod.rs \
  crates/websocket-server/src/main.rs \
  crates/websocket-server/Cargo.toml
git commit -m "feat(ws): add DEX swap stream with pool mapping resolution"
```

---

### Task 5: Add Pool Stream → Mapping Updates

**Files:**
- Modify: `crates/websocket-server/src/stream/pool/receive.rs`
- Modify: `crates/websocket-server/src/stream/pool/stream.rs`

When a `LiquidityAllocated` event arrives, the pool stream already processes it. We should also store the mapping so the DEX stream can use it without waiting for a DB reload.

This is optional — the DEX stream reloads from DB periodically. Skip this task if you want to keep it simple. The periodic reload (every 100 events) handles new pools with minimal delay.

**Decision: Skip this task for now.** The DB reload pattern from the observer works well enough.

---

### Task 6: Final Integration Test and Cleanup

**Files:**
- All modified files

**Step 1: Run full test suite**

Run: `cargo test -p openlaunch-websocket-server`
Expected: All tests PASS

**Step 2: Run clippy**

Run: `cargo clippy -p openlaunch-websocket-server -- -D warnings`
Expected: No warnings

**Step 3: Build release**

Run: `cargo build -p openlaunch-websocket-server --release`
Expected: Compiles successfully

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore(ws): fix clippy warnings and cleanup"
```

---

## Summary of Changes

| Component | Change |
|---|---|
| `candle/mod.rs` | NEW — In-memory CandleManager with OHLCV for 6 intervals |
| `event/core.rs` | `Chart(String)` → `Chart(String, String)` for token+interval |
| `event/chart.rs` | Updated to accept interval parameter |
| `server/socket/rpc.rs` | `chart_subscribe` now requires `resolution` param |
| `stream/ido/receive.rs` | Uses CandleManager instead of direct chart publish |
| `stream/ido/stream.rs` | Passes CandleManager through |
| `stream/dex/` | NEW — DEX swap stream with pool mapping resolution |
| `main.rs` | Initializes CandleManager, DB pool, spawns DEX stream |
| `Cargo.toml` | Added `sqlx` dependency |

## Client Usage

```json
{"jsonrpc":"2.0","method":"chart_subscribe","params":{"token_id":"0x5673...","resolution":"5"},"id":1}
```

Supported `resolution` values: `"1"`, `"5"`, `"15"`, `"60"`, `"240"`, `"1D"`
