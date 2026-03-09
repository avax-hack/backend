# WebSocket Server Specification

## 1. Overview

The `openlaunch-websocket-server` crate provides real-time data streaming for the OpenLaunch platform -- an IDO (Initial DEX Offering) launchpad on Avalanche C-Chain. It connects to the blockchain via WebSocket RPC, listens for on-chain contract events, and pushes structured updates to subscribed WebSocket clients using the JSON-RPC 2.0 protocol.

### Tech Stack

| Component | Technology |
|-----------|------------|
| HTTP / WebSocket server | Axum 0.8 (`axum::extract::ws`) |
| Async runtime | Tokio |
| Blockchain connectivity | Alloy (WebSocket transport, log subscriptions) |
| In-process pub/sub | `tokio::sync::broadcast` channels via `DashMap` |
| Serialization | serde / serde_json |
| Logging | tracing + tracing-subscriber (JSON format) |
| Configuration | Environment variables via `dotenvy`, `std::sync::LazyLock` |
| Precision arithmetic | `bigdecimal` for precision-safe price computation |
| Database | `sqlx` for PostgreSQL (candle persistence, pool mappings) |
| Shared types / config | `openlaunch-shared` workspace crate |

### Crate Identity

```toml
[package]
name = "openlaunch-websocket-server"
version = "0.1.0"
edition = "2024"
```

---

## 2. Architecture

### Module Organization

```
src/
  main.rs                        # Entry point: boots RPC client, event producers, streams, HTTP server
  config_local.rs                # Local configuration constants (channel size, cleanup interval)
  cache/
    mod.rs                       # PriceCache -- in-memory latest-price store per token
  candle/
    mod.rs                       # CandleManager -- in-memory OHLCV candle store with DB persistence
  event/
    mod.rs                       # EventProducer trait, BroadcastEventProducer, EventProducers aggregate
    core.rs                      # WsEvent, SubscriptionKey enum
    trade.rs                     # TradeEventProducer (typed wrapper)
    price.rs                     # PriceEventProducer (typed wrapper)
    project.rs                   # ProjectEventProducer (typed wrapper)
    milestone.rs                 # MilestoneEventProducer (typed wrapper)
    new_content.rs               # NewContentEventProducer (typed wrapper, global channel)
    chart.rs                     # ChartEventProducer (typed wrapper)
  server/
    mod.rs                       # AppState, Axum router, WebSocket upgrade + connection handler
    socket/
      mod.rs                     # Re-exports connection and rpc modules
      connection.rs              # ConnectionState -- per-client subscription tracking
      rpc.rs                     # JSON-RPC request/response types, parse + dispatch logic
  stream/
    mod.rs                       # rpc_url_to_ws(), update_and_broadcast_candles() shared helpers
    ido/
      mod.rs                     # Re-exports stream and receive
      stream.rs                  # start_ido_stream -- blockchain log subscription with auto-reconnect
      receive.rs                 # handle_ido_log -- decodes IDO contract events and publishes
    pool/
      mod.rs                     # Re-exports stream and receive
      stream.rs                  # start_pool_stream -- blockchain log subscription with auto-reconnect
      receive.rs                 # handle_pool_log -- decodes LpManager contract events and publishes
    dex/
      mod.rs                     # Re-exports stream and receive
      stream.rs                  # start_dex_stream -- Uniswap V4 Swap log subscription with pool mappings
      receive.rs                 # handle_swap_log -- decodes Swap events, computes price, broadcasts
```

### Connection Management Overview

Each WebSocket client connection is handled in its own Tokio task. The lifecycle is:

1. HTTP upgrade at `/ws` via `ws_upgrade_handler`.
2. Connection count atomically incremented via `compare_exchange` loop (CAS) to prevent exceeding `max_connections`. If the limit is reached, the upgrade is rejected.
3. `handle_ws_connection` splits the socket into a read half and a write half.
4. A dedicated sink task reads from an `mpsc::channel<String>(256)` and forwards messages to the WebSocket write half.
5. The read loop parses incoming JSON-RPC requests, dispatches them via `rpc::dispatch`, and sends responses through the same outbound channel. Incoming messages are subject to per-connection rate limiting (60 messages per 10-second window).
6. On disconnect (client close frame or read error), `ConnectionState::cleanup_all()` aborts all subscription forwarding tasks, the outbound channel is dropped, and the sink task exits. Connection counter decremented atomically on disconnect.

The server sends a ping JSON-RPC push every 30 seconds for dead connection detection (see Heartbeat section).

### Message Flow

```
Avalanche C-Chain
       |
       | (WebSocket RPC / alloy Provider::subscribe_logs)
       v
  stream::ido::stream / stream::pool::stream / stream::dex::stream
       |
       | (decode log -> serde_json::Value)
       v
  stream::ido::receive / stream::pool::receive / stream::dex::receive
       |
       | (EventProducers.{trade,price,project,milestone,new_content,chart}.publish())
       v
  BroadcastEventProducer (DashMap<String, broadcast::Sender<WsEvent>>)
       |
       | (broadcast::Receiver per subscription)
       v
  Per-client forwarding task (spawned in rpc::handle_keyed_subscribe / handle_global_subscribe)
       |
       | (mpsc::Sender<String> -> sink task)
       v
  WebSocket client
```

---

## 3. WebSocket Protocol

### Transport

- Endpoint: `GET /ws` (HTTP upgrade to WebSocket)
- Health check: `GET /health` returns `{"status": "ok"}`
- Message format: UTF-8 JSON text frames
- Protocol: JSON-RPC 2.0

### JSON-RPC Request Format

```typescript
interface JsonRpcRequest {
  jsonrpc: "2.0";
  method: string;
  params: object;   // defaults to {} if omitted
  id: string | number | null;
}
```

Rust type (`rpc.rs`):

```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,   // #[serde(default)]
    pub id: serde_json::Value,
}
```

### JSON-RPC Response Format

```typescript
interface JsonRpcResponse {
  jsonrpc: "2.0";
  result?: any;       // present on success
  error?: {           // present on failure
    code: number;
    message: string;
  };
  id: string | number | null;
}
```

Rust type (`rpc.rs`):

```rust
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,    // skip_serializing_if None
    pub error: Option<JsonRpcError>,          // skip_serializing_if None
    pub id: serde_json::Value,
}

pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}
```

### JSON-RPC Push Notification Format

Push notifications are server-initiated messages with no `id` field. They are sent when a subscribed event fires.

```typescript
interface JsonRpcPush {
  jsonrpc: "2.0";
  method: string;     // matches the subscribe method name
  result: any;        // event data payload
}
```

Rust type (`rpc.rs`):

```rust
pub struct JsonRpcPush {
    pub jsonrpc: String,
    pub method: String,
    pub result: serde_json::Value,
}
```

### Available Methods

| Method | Params | Description |
|--------|--------|-------------|
| `trade_subscribe` | `{ "token_id": "<address>" }` | Subscribe to trade events for a token |
| `price_subscribe` | `{ "token_id": "<address>" }` | Subscribe to price updates for a token |
| `project_subscribe` | `{ "project_id": "<address>" }` | Subscribe to project lifecycle events |
| `milestone_subscribe` | `{ "project_id": "<address>" }` | Subscribe to milestone events for a project |
| `new_content_subscribe` | `{}` (no params) | Subscribe to global new content events |
| `chart_subscribe` | `{ "token_id": "<address>", "resolution": "1" }` | Subscribe to OHLCV chart updates |
| `trade_unsubscribe` | `{ "token_id": "<address>" }` | Unsubscribe from trade events |
| `price_unsubscribe` | `{ "token_id": "<address>" }` | Unsubscribe from price events |
| `project_unsubscribe` | `{ "project_id": "<address>" }` | Unsubscribe from project events |
| `milestone_unsubscribe` | `{ "project_id": "<address>" }` | Unsubscribe from milestone events |
| `chart_unsubscribe` | `{ "token_id": "<address>", "resolution": "1" }` | Unsubscribe from chart updates |
| `new_content_unsubscribe` | `{}` | Unsubscribe from new content events |

### Subscription Flow

1. Client sends a JSON-RPC request with a subscribe method.
2. Server validates the request (jsonrpc version, required params).
3. Server spawns a forwarding task that listens on the corresponding `broadcast::Receiver`.
4. Server returns `{"jsonrpc": "2.0", "result": {"subscribed": true}, "id": <request_id>}`.
5. Subsequent events are pushed as `JsonRpcPush` messages (no `id`, method matches the subscription method).

### Unsubscription

All subscribe methods have corresponding `*_unsubscribe` methods (e.g., `trade_subscribe` / `trade_unsubscribe`). When an unsubscribe request is received, the server aborts the forwarding task for the matching channel key and returns:

```json
{"jsonrpc": "2.0", "result": {"unsubscribed": true}, "id": <request_id>}
```

If no active subscription exists for the given key, the response contains `{"unsubscribed": false}`.

Subscriptions are also cleaned up automatically when:

- The client disconnects (close frame or transport error).
- The client re-subscribes to the same channel key (the old forwarding task is aborted and replaced).

`ConnectionState::cleanup_all()` is called on disconnect, which aborts all spawned forwarding tasks. The `Drop` implementation on `ConnectionState` also calls `cleanup_all()` as a safety net.

---

## 4. Subscription Channels

### Core Types

```rust
// event/core.rs

pub struct WsEvent {
    pub method: String,
    pub data: serde_json::Value,
}

pub enum SubscriptionKey {
    Trade(String),       // channel key: "trade:{token_address}"
    Price(String),       // channel key: "price:{token_address}"
    Project(String),     // channel key: "project:{project_id}"
    Milestone(String),   // channel key: "milestone:{project_id}"
    NewContent,          // channel key: "new_content"
    Chart(String, String),  // channel key: "chart:{token_address}:{interval}"
}
```

All address parameters are **lowercased** before use as channel keys (case-insensitive matching).

---

### 4.1 Trade Channel (`trade:{token_address}`)

**Subscribe method:** `trade_subscribe`
**Param:** `token_id` (token contract address)

Events published to this channel:

#### LIQUIDITY_ALLOCATED

Fired when liquidity is allocated to a pool for a graduated token.

```json
{
  "type": "LIQUIDITY_ALLOCATED",
  "token": "0x...",
  "pool": "0x...",
  "token_amount": "1000000000000000000",
  "tick_lower": -60000,
  "tick_upper": 60000
}
```

#### FEES_COLLECTED

Fired when LP fees are collected for a token's pool.

```json
{
  "type": "FEES_COLLECTED",
  "token": "0x...",
  "amount0": "50000",
  "amount1": "25000"
}
```

**Update frequency:** Real-time, driven by on-chain LpManager contract events.

---

### 4.2 Price Channel (`price:{token_address}`)

**Subscribe method:** `price_subscribe`
**Param:** `token_id` (token contract address)

The price channel publishes real-time price updates for a token. Price events are published by both the IDO stream (on token purchases) and the DEX stream (on Uniswap V4 swaps). The `PriceCache` is updated on each price-affecting event and the new price is broadcast to subscribers.

**Update frequency:** Real-time, driven by on-chain IDO purchases and DEX swap events.

---

### 4.3 Project Channel (`project:{project_id}`)

**Subscribe method:** `project_subscribe`
**Param:** `project_id` (token contract address)

Events published to this channel:

#### PROJECT_CREATED

```json
{
  "type": "PROJECT_CREATED",
  "token": "0x...",
  "creator": "0x...",
  "name": "TokenName",
  "symbol": "TKN",
  "token_uri": "ipfs://...",
  "ido_token_amount": "1000000000000000000000",
  "token_price": "100000",
  "deadline": "1700000000"
}
```

#### TOKENS_PURCHASED

```json
{
  "type": "TOKENS_PURCHASED",
  "token": "0x...",
  "buyer": "0x...",
  "usdc_amount": "100000000",
  "token_amount": "500000000000000000000"
}
```

#### GRADUATED

```json
{
  "type": "GRADUATED",
  "token": "0x..."
}
```

#### MILESTONE_APPROVED

Also published to the project channel (in addition to the milestone channel).

```json
{
  "type": "MILESTONE_APPROVED",
  "token": "0x...",
  "milestone_index": "1",
  "usdc_released": "50000000000"
}
```

#### PROJECT_FAILED

```json
{
  "type": "PROJECT_FAILED",
  "token": "0x..."
}
```

#### REFUNDED

```json
{
  "type": "REFUNDED",
  "token": "0x...",
  "buyer": "0x...",
  "tokens_burned": "500000000000000000000",
  "usdc_returned": "100000000"
}
```

**Update frequency:** Real-time, driven by on-chain IDO contract events.

---

### 4.4 Milestone Channel (`milestone:{project_id}`)

**Subscribe method:** `milestone_subscribe`
**Param:** `project_id` (token contract address)

Events published to this channel:

#### MILESTONE_APPROVED

```json
{
  "type": "MILESTONE_APPROVED",
  "token": "0x...",
  "milestone_index": "1",
  "usdc_released": "50000000000"
}
```

**Update frequency:** Real-time, driven by on-chain IDO contract events.

---

### 4.5 New Content Channel (`new_content`)

**Subscribe method:** `new_content_subscribe`
**Params:** None (global broadcast channel)

This is a global channel that receives significant platform-wide events. Events published here:

| Event Type | Source |
|------------|--------|
| `PROJECT_CREATED` | IDO contract |
| `GRADUATED` | IDO contract |
| `PROJECT_FAILED` | IDO contract |
| `LIQUIDITY_ALLOCATED` | LpManager contract |

All payloads match the schemas defined in their respective channel sections above.

**Update frequency:** Real-time, driven by on-chain events from both IDO and LpManager contracts.

---

### 4.6 Chart Channel (`chart:{token_address}:{interval}`)

**Subscribe method:** `chart_subscribe`
**Params:**
- `token_id` (token contract address)
- `resolution` (optional, default `"1"`)

The resolution parameter maps to candle intervals as follows:

| Resolution value | Interval |
|-----------------|----------|
| `"1"` | 1m |
| `"5"` | 5m |
| `"15"` | 15m |
| `"60"` | 1h |
| `"240"` | 4h |
| `"1D"` | 1d |

Events published to this channel:

#### CHART_UPDATE

Fired on every IDO token purchase or DEX swap that affects the token price. Contains OHLCV candle data for the subscribed interval.

```json
{
  "type": "CHART_UPDATE",
  "token": "0x...",
  "interval": "1m",
  "open": "1.23",
  "high": "1.25",
  "low": "1.20",
  "close": "1.24",
  "volume": "5000.00"
}
```

**Update frequency:** Real-time, on every IDO purchase or DEX swap.

---

### Cross-Channel Event Routing Summary

| On-Chain Event | Channels Published To |
|----------------|----------------------|
| `ProjectCreated` | `project:{token}`, `new_content` |
| `TokensPurchased` | `project:{token}`, `trade:{token}`, `price:{token}`, `chart:{token}:*` |
| `Graduated` | `project:{token}`, `new_content` |
| `MilestoneApproved` | `milestone:{token}`, `project:{token}` |
| `ProjectFailed` | `project:{token}`, `new_content` |
| `Refunded` | `project:{token}` |
| `LiquidityAllocated` | `trade:{token}`, `new_content` |
| `FeesCollected` | `trade:{token}` |
| `PoolManager.Swap` | `trade:{token}`, `price:{token}`, `chart:{token}:*` |

---

## 5. Event Pipeline

### On-Chain Event Sources

Three smart contracts are monitored via WebSocket RPC log subscriptions:

1. **IDO Contract** (`config::IDO_CONTRACT`) -- Handles project creation, token purchases, graduation, milestones, failures, and refunds.
2. **LpManager Contract** (`config::LP_MANAGER_CONTRACT`) -- Handles post-graduation liquidity allocation and fee collection.
3. **PoolManager Contract** (`config::POOL_MANAGER_CONTRACT`) -- Monitors DEX swap events (Uniswap V4).

### Stream Lifecycle

Each stream module (`stream::ido::stream`, `stream::pool::stream`) follows the same pattern:

```rust
pub async fn start_ido_stream(
    producers: Arc<EventProducers>,
    price_cache: Arc<PriceCache>,
) -> anyhow::Result<()>
```

1. Convert the HTTP RPC URL to a WebSocket URL (`https://` -> `wss://`, `http://` -> `ws://`) via the shared `rpc_url_to_ws()` helper.
2. Connect to the blockchain node via `alloy::transports::ws::WsConnect`.
3. Create a `Filter` scoped to the contract address.
4. Call `provider.subscribe_logs(&filter)` to get a live log stream.
5. For each incoming log, call the corresponding `receive::handle_*_log()` function.
6. If the stream ends or errors, **automatically reconnect** after a 5-second delay.

The reconnection loop runs indefinitely (`loop { ... }`) ensuring the stream is always active.

The DEX stream (`stream::dex::stream`) follows a similar pattern with an additional step: pool mappings are reloaded from the database every 300 seconds. The `load_mappings()` function queries the `pool_mappings` table and returns `PoolMapping` structs that map Uniswap V4 pool IDs to token addresses. When a `Swap` event is decoded, the buy/sell direction is determined based on Uniswap V4 semantics (sign of `amount0` / `amount1`), and the resulting trade and price events are broadcast to the appropriate channels.

### Log Decoding

Logs are decoded using Alloy's `SolEvent` derive macros. Each log's first topic (event signature hash) is matched against known event signatures:

**IDO events** (`stream::ido::receive::handle_ido_log`):

```rust
fn handle_ido_log(log: &Log, producers: &Arc<EventProducers>, _price_cache: &Arc<PriceCache>) -> anyhow::Result<()>
```

- `IIDO::ProjectCreated`
- `IIDO::TokensPurchased`
- `IIDO::Graduated`
- `IIDO::MilestoneApproved`
- `IIDO::ProjectFailed`
- `IIDO::Refunded`

**Pool events** (`stream::pool::receive::handle_pool_log`):

```rust
fn handle_pool_log(log: &Log, producers: &Arc<EventProducers>, _price_cache: &Arc<PriceCache>) -> anyhow::Result<()>
```

- `ILpManager::LiquidityAllocated`
- `ILpManager::FeesCollected`

**DEX events** (`stream::dex::receive::handle_swap_log`):

Decodes Uniswap V4 `Swap` events from the PoolManager contract, computes the token price using `BigDecimal` for precision, updates the `PriceCache`, and broadcasts trade, price, and chart events.

### Broadcast Mechanism

`BroadcastEventProducer` uses a `DashMap<String, broadcast::Sender<WsEvent>>` for lock-free concurrent access. Channels are lazily created on first publish or subscribe. The broadcast channel capacity is configurable via `WS_CHANNEL_SIZE` (default: 1024).

```rust
pub trait EventProducer: Send + Sync {
    fn publish(&self, key: &str, event: WsEvent);
    fn subscribe(&self, key: &str) -> broadcast::Receiver<WsEvent>;
}
```

When a subscriber lags behind and misses messages (buffer overflow), the forwarding task sends a `SUBSCRIPTION_ERROR` push to the client and then terminates the subscription task. The client must re-subscribe to resume receiving events.

---

## 6. Connection Management

### Per-Connection State

Each client connection has a `ConnectionState` that tracks active subscription forwarding tasks:

```rust
pub struct ConnectionState {
    subscriptions: HashMap<String, JoinHandle<()>>,
}
```

Key methods:

| Method | Signature | Behavior |
|--------|-----------|----------|
| `new` | `fn new() -> Self` | Creates empty state |
| `subscribe` | `fn subscribe(&mut self, key: String, handle: JoinHandle<()>) -> bool` | Registers a task; aborts any existing task for the same key. Returns `false` if the subscription limit (`WS_MAX_SUBSCRIPTIONS_PER_CONN`, default 100) is reached |
| `unsubscribe` | `fn unsubscribe(&mut self, key: &str) -> bool` | Aborts and removes a task; returns whether it existed |
| `cleanup_all` | `fn cleanup_all(&mut self)` | Aborts all tasks; called on disconnect |
| `subscription_count` | `fn subscription_count(&self) -> usize` | Returns active subscription count |
| `has_subscription` | `fn has_subscription(&self, key: &str) -> bool` | Checks if a key is subscribed |
| `prune_finished` | `fn prune_finished(&mut self)` | Removes tasks that have already completed |

### Heartbeat / Ping-Pong

The server sends a ping JSON-RPC push to each connected client every 30 seconds:

```json
{"jsonrpc": "2.0", "method": "ping", "params": {}}
```

This server-initiated ping is used for dead connection detection. If the write fails, the connection is considered dead and is cleaned up.

### Duplicate Subscription Handling

If a client subscribes to the same channel key twice, the previous forwarding task is aborted and replaced with a new one. This prevents resource leaks from duplicate subscriptions.

### Disconnection Cleanup

On disconnect:
1. The read loop exits (client close frame or stream end).
2. `conn.cleanup_all()` aborts all spawned forwarding tasks.
3. The outbound `mpsc::Sender` is dropped, causing the sink task to exit.
4. The `Drop` implementation on `ConnectionState` calls `cleanup_all()` as a safety net.

### Outbound Channel

Each connection has an `mpsc::channel::<String>(256)` for outbound messages. Both subscription push notifications and RPC responses flow through this channel to the sink task, which writes them to the WebSocket.

---

## 7. Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WS_IP` | `127.0.0.1` | IP address to bind the server |
| `WS_PORT` | `8001` | Port to bind the server |
| `WS_CHANNEL_SIZE` | `1024` | Capacity of each `broadcast` channel |
| `WS_CLEANUP_INTERVAL_SECS` | `300` | Cleanup interval in seconds (defined but not yet used) |
| `WS_MAX_CONNECTIONS` | `1000` | Maximum concurrent WebSocket connections |
| `WS_CORS_ORIGIN` | permissive | Comma-separated CORS origins |
| `WS_MAX_SUBSCRIPTIONS_PER_CONN` | `100` | Maximum subscriptions per connection |
| `DATABASE_URL` | (required) | PostgreSQL connection URL |
| `RUST_LOG` | `info` | Log level filter (standard `tracing` env filter) |

### Shared Configuration (from `openlaunch-shared`)

| Constant | Description |
|----------|-------------|
| `config::MAIN_RPC_URL` | Primary Avalanche C-Chain RPC endpoint |
| `config::SUB_RPC_URL_1` | Secondary RPC endpoint |
| `config::SUB_RPC_URL_2` | Tertiary RPC endpoint |
| `config::IDO_CONTRACT` | IDO contract address on Avalanche C-Chain |
| `config::LP_MANAGER_CONTRACT` | LpManager contract address on Avalanche C-Chain |
| `config::POOL_MANAGER_CONTRACT` | PoolManager contract address on Avalanche C-Chain |

### RPC Client Initialization

The server initializes an `RpcClient` with three provider slots:

```rust
RpcClient::init(vec![
    (ProviderId::Main, config::MAIN_RPC_URL.clone()),
    (ProviderId::Sub1, config::SUB_RPC_URL_1.clone()),
    (ProviderId::Sub2, config::SUB_RPC_URL_2.clone()),
])
```

The stream modules use `MAIN_RPC_URL` for their WebSocket connections to the blockchain node.

---

## 8. Error Handling

### JSON-RPC Error Codes

| Code | Meaning | Trigger |
|------|---------|---------|
| `-32700` | Parse error | Malformed JSON in client message |
| `-32600` | Invalid request | `jsonrpc` field is not `"2.0"` |
| `-32601` | Method not found | Unknown method name |
| `-32602` | Invalid params | Missing required parameter (e.g., `token_id`, `project_id`) |
| `-32000` | Server error | Rate limit exceeded, subscription limit reached |

### Error Response Format

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32700,
    "message": "Parse error: expected value at line 1 column 1"
  },
  "id": null
}
```

For parse errors, `id` is `null` since the request could not be parsed. For other errors, `id` reflects the request's `id` value.

### Stream Error Handling

- **Log decode failure:** Logged as error, the individual log is skipped, stream continues.
- **Stream disconnection:** Logged as warning/error, automatic reconnection after 5 seconds.
- **Empty topics:** Silently skipped (returns `Ok(())`).

### WebSocket Error Handling

- **Outbound channel send failure:** Breaks the read loop, triggering cleanup.
- **Sink write failure:** Breaks the sink task, which eventually causes the read loop to detect a dead channel.
- **Broadcast lag:** `SUBSCRIPTION_ERROR` push sent to client, subscription task terminated. Client must re-subscribe.
- **Broadcast channel closed:** Forwarding task exits gracefully.

### Graceful Shutdown

The server listens for `CTRL+C` (via `tokio::signal::ctrl_c`) and performs a graceful shutdown through `axum::serve(...).with_graceful_shutdown(shutdown_signal())`. After the server stops, `drop(rpc)` is called explicitly to ensure clean shutdown ordering of the RPC client resources.

---

## 9. Price Cache

The `PriceCache` provides an in-memory store for the latest token price, backed by a `DashMap` for concurrent access.

```rust
pub struct PriceCache {
    prices: DashMap<String, PriceSnapshot>,
}

pub struct PriceSnapshot {
    pub token_address: String,
    pub price: String,
    pub updated_at: i64,
}
```

Key methods:

| Method | Signature |
|--------|-----------|
| `new` | `fn new() -> Self` |
| `set_price` | `fn set_price(&self, token_address: &str, price: String) -> PriceSnapshot` |
| `get_price` | `fn get_price(&self, token_address: &str) -> Option<PriceSnapshot>` |
| `remove_price` | `fn remove_price(&self, token_address: &str) -> Option<PriceSnapshot>` |
| `len` | `fn len(&self) -> usize` |
| `is_empty` | `fn is_empty(&self) -> bool` |

All lookups are case-insensitive (addresses are lowercased before storage and retrieval). The cache is actively used by both the IDO stream (on token purchases) and the DEX stream (on Uniswap V4 swaps) to store and broadcast the latest price for each token.

---

## 10. Candle Manager

The `CandleManager` provides a thread-safe in-memory OHLCV (Open-High-Low-Close-Volume) candle store, keyed by `(token_id, interval)`. It supports database persistence via the `charts` table in PostgreSQL.

### Supported Intervals

Six candle intervals are maintained:

| Interval | Duration |
|----------|----------|
| `1m` | 1 minute |
| `5m` | 5 minutes |
| `15m` | 15 minutes |
| `1h` | 1 hour |
| `4h` | 4 hours |
| `1d` | 1 day |

### Key Behavior

- **`update(token_id, price, volume)`** -- Normalizes `token_id` to lowercase. For each interval, determines the current bucket timestamp. If a candle already exists for that bucket, high/low/close and volume are updated in place. If the bucket has advanced, a new candle is created with the given price as open/high/low/close.
- **`get(token_id, interval)`** -- Normalizes `token_id` to lowercase and returns the current candle for the given interval, if any.
- **`load_from_db(pool)`** -- Restores candle state from the `charts` table at startup, ensuring continuity across server restarts.

### Price Precision

All price computation within the candle manager uses `BigDecimal` (not `f64`) to avoid floating-point precision errors that would accumulate over many updates.
