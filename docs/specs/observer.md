# Observer Crate Specification

## 1. Overview

The `observer` crate is the on-chain event indexer for the OpenLaunch backend. It continuously monitors the Avalanche C-Chain for smart contract events (IDO lifecycle, liquidity provisioning, token transfers, DEX swaps, and price updates), decodes them, and persists the resulting data into a PostgreSQL database.

The observer runs as a standalone Tokio async binary. It connects to multiple RPC providers (with health-check failover), polls for logs in batched block ranges, and feeds decoded events through typed `mpsc` channels to dedicated receiver tasks that write to the database.

## 2. Architecture

### Module Organization

```
src/
  main.rs              # Entry point, task orchestration
  config_local.rs      # Observer-specific configuration (lazy_static)
  controller/
    mod.rs             # Re-exports project, lp
    project.rs         # DB operations for projects table
    lp.rs              # DB operations for liquidity_positions, fee_collections tables
  event/
    mod.rs             # Re-exports all event sub-modules
    core.rs            # EventBatch<T>, EventType enum
    error.rs           # ObserverError (Skippable | Retriable | Fatal)
    handler.rs         # RetryConfig, run_event_handler_with_retry
    provider.rs        # Alloy HTTP provider construction from RpcClient
    ido/
      stream.rs        # poll_ido_events — fetches IDO contract logs
      receive.rs       # process_ido_events — handles decoded IDO events
    lp/
      stream.rs        # poll_lp_events — fetches LpManager contract logs
      receive.rs       # process_lp_events — handles decoded LP events
    token/
      stream.rs        # poll_token_events — fetches ERC20 Transfer logs
      receive.rs       # process_token_events — updates balances table
    swap/
      stream.rs        # poll_swap_events — fetches Uniswap V4 Swap logs
      receive.rs       # process_swap_events — inserts swaps, chart bars, market data
    price/
      stream.rs        # derive_price_updates — derives prices from swap data
      receive.rs       # process_price_updates — upserts market_data table
  sync/
    mod.rs             # Re-exports stream, receive
    stream.rs          # StreamManager, BlockRange — block progress for polling side
    receive.rs         # ReceiveManager — dependency-aware completion tracking
```

### Event Pipeline

```
  RPC Node
    |
    v
  Stream task (poll logs in BlockRange batches)
    |  alloy Filter -> get_logs -> decode -> Vec<Event>
    v
  mpsc::channel<EventBatch<T>>   (buffer = 128)
    |
    v
  Receive task (iterate batch, write to DB, mark_completed)
```

Each event domain (IDO, LP, Token, Swap, Price) runs its own independent stream/receive pair as separate Tokio tasks within a `JoinSet`.

## 3. Event Types

### 3.1 IDO Events (source: IDO contract, `config::IDO_CONTRACT`)

| Event | Solidity Signature | Decoded Fields |
|---|---|---|
| **ProjectCreated** | `ProjectCreated(address token, address creator, string name, string symbol, string tokenURI, uint256 idoTokenAmount, uint256 tokenPrice, uint256 deadline)` | `token`, `creator`, `name`, `symbol`, `token_uri`, `ido_token_amount`, `token_price`, `deadline`, `block_number`, `tx_hash` |
| **TokensPurchased** | `TokensPurchased(address token, address buyer, uint256 usdcAmount, uint256 tokenAmount)` | `token`, `buyer`, `usdc_amount`, `token_amount`, `block_number`, `tx_hash` |
| **Graduated** | `Graduated(address token)` | `token`, `block_number`, `tx_hash` |
| **MilestoneApproved** | `MilestoneApproved(address token, uint256 milestoneIndex, uint256 usdcReleased)` | `token`, `milestone_index`, `usdc_released`, `block_number`, `tx_hash` |
| **ProjectFailed** | `ProjectFailed(address token)` | `token`, `block_number`, `tx_hash` |
| **Refunded** | `Refunded(address token, address buyer, uint256 tokensBurned, uint256 usdcReturned)` | `token`, `buyer`, `tokens_burned`, `usdc_returned`, `block_number`, `tx_hash` |

ABI decoding uses `openlaunch_shared::contracts::ido::IIDO`.

### 3.2 LP Events (source: LpManager contract, `config::LP_MANAGER_CONTRACT`)

| Event | Solidity Signature | Decoded Fields |
|---|---|---|
| **LiquidityAllocated** | `LiquidityAllocated(address token, address pool, uint256 tokenAmount, int24 tickLower, int24 tickUpper)` | `token`, `pool`, `token_amount`, `tick_lower`, `tick_upper`, `block_number`, `tx_hash` |
| **FeesCollected** | `FeesCollected(address token, uint256 amount0, uint256 amount1)` | `token`, `amount0`, `amount1`, `block_number`, `tx_hash` |

ABI decoding uses `openlaunch_shared::contracts::lp_manager::ILpManager`.

### 3.3 Token Transfer Events (source: dynamic project token addresses)

| Event | Solidity Signature | Decoded Fields |
|---|---|---|
| **Transfer** | `Transfer(address indexed from, address indexed to, uint256 value)` | `token` (contract address), `from`, `to`, `amount`, `block_number`, `tx_hash` |

Filtered by `IProjectToken::Transfer::SIGNATURE_HASH`. Addresses are dynamically provided as `token_addresses: &[String]`.

### 3.4 Swap Events (source: dynamic Uniswap V4 pool addresses)

| Event | Solidity Signature | Decoded Fields |
|---|---|---|
| **Swap** | `Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)` | `pool`, `sender`, `recipient`, `amount0`, `amount1`, `sqrt_price_x96`, `liquidity`, `tick`, `block_number`, `tx_hash` |

Defined inline via `alloy::sol!` macro. Decoded into `RawSwapEvent` (not `OnChainEvent`).

### 3.5 Price Updates (derived, not directly from chain)

Price updates are not fetched from RPC. They are derived from `RawSwapEvent` batches via `derive_price_updates()`.

```rust
pub struct PriceUpdate {
    pub token_id: String,
    pub price: String,      // native / token ratio, 18 decimal places
    pub volume: String,
    pub block_number: u64,
    pub tx_hash: String,
}
```

## 4. Block Tracking

### StreamManager (`sync/stream.rs`)

Manages the polling cursor for each `EventType`. Uses `DashMap<EventType, u64>` for lock-free concurrent access.

```rust
pub struct BlockRange {
    pub from_block: u64,
    pub to_block: u64,
}

impl StreamManager {
    pub fn new() -> Self;
    pub fn set_progress(&self, event_type: EventType, block: u64);
    pub fn get_range(&self, event_type: EventType, latest_block: u64) -> Option<BlockRange>;
    pub fn advance(&self, event_type: EventType, new_block: u64);
    pub fn current_block(&self, event_type: EventType) -> Option<u64>;
}
```

**Block range calculation** (`get_range`):
- `from_block = last_processed + 1`
- `to_block = min(from_block + BATCH_SIZE - 1, latest_block)`
- Returns `None` if already caught up (`from_block > latest_block`)

**Monotonic progress** (`advance`): Only moves the cursor forward. Attempts to set a lower block number are ignored.

### ReceiveManager (`sync/receive.rs`)

Tracks completion on the receiver side and enforces event-type dependencies.

```rust
impl ReceiveManager {
    pub fn new() -> Self;
    pub fn set_completed(&self, event_type: EventType, block: u64);
    pub fn can_process(&self, event_type: EventType, block: u64) -> bool;
    pub fn mark_completed(&self, event_type: EventType, block: u64);
    pub fn completed_block(&self, event_type: EventType) -> Option<u64>;
}
```

**Dependency graph** (defined on `EventType::dependencies()`):

```
Ido  (no dependencies)
  |
  +---> Token  (depends on Ido)
  +---> Swap   (depends on Ido)
  +---> Lp     (depends on Ido)
            |
            +---> Price  (depends on Swap)
```

`can_process(event_type, block)` returns `true` only when all dependencies have `completed_block >= block`.

### Block Progress Persistence

A dedicated background task persists `StreamManager` progress to the database every 10 seconds via `block_ctrl::set_last_block`. Additionally, `handle_stream_result` persists immediately after each successful batch.

## 5. Event Streams

### 5.1 IDO Stream

```rust
pub async fn poll_ido_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError>;
```

- **Filter**: `address = config::IDO_CONTRACT`, block range from `BlockRange`
- **Decoding**: Tries each of 6 IDO event types sequentially via `log_decode::<IIDO::EventName>()`
- **Output**: `EventBatch<OnChainEvent>` sent on channel; empty batches are not sent

### 5.2 LP Stream

```rust
pub async fn poll_lp_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError>;
```

- **Filter**: `address = config::LP_MANAGER_CONTRACT`, block range
- **Decoding**: `LiquidityAllocated`, `FeesCollected` via `ILpManager`

### 5.3 Token Stream

```rust
pub async fn poll_token_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    token_addresses: &[String],
    tx: &mpsc::Sender<EventBatch<OnChainEvent>>,
) -> Result<(), ObserverError>;
```

- **Filter**: Multiple addresses (dynamic token list), event signature `Transfer::SIGNATURE_HASH`
- **Early return**: Returns `Ok(())` immediately if `token_addresses` is empty

### 5.4 Swap Stream

```rust
pub async fn poll_swap_events(
    rpc: &Arc<RpcClient>,
    range: &BlockRange,
    pool_addresses: &[String],
    tx: &mpsc::Sender<EventBatch<RawSwapEvent>>,
) -> Result<(), ObserverError>;
```

- **Filter**: Multiple pool addresses (dynamic), event signature `Swap::SIGNATURE_HASH`
- **Output type**: `EventBatch<RawSwapEvent>` (not `OnChainEvent`)
- **Early return**: Returns `Ok(())` immediately if `pool_addresses` is empty

### 5.5 Price Stream

```rust
pub async fn derive_price_updates(
    swap_batch: &EventBatch<RawSwapEvent>,
    pool_token_map: &HashMap<String, (String, bool)>,
    tx: &mpsc::Sender<EventBatch<PriceUpdate>>,
) -> Result<(), ObserverError>;
```

- Does **not** poll RPC; derives from swap data
- `pool_token_map`: maps pool address to `(token_id, is_token0)`
- Computes price as `native_amount / token_amount` with 18 decimal precision

## 6. Event Processing

### 6.1 IDO Receiver (`event/ido/receive.rs`)

```rust
pub async fn process_ido_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError>;
```

| Event | DB Operations |
|---|---|
| `ProjectCreated` | `account_ctrl::upsert` (creator), `project_ctrl::insert_from_event` (inserts into `projects` with status `"funding"`, total supply `1e27`) |
| `TokensPurchased` | `account_ctrl::upsert` (buyer), `investment_ctrl::insert`, `project_ctrl::add_usdc_raised` |
| `Graduated` | `project::update_status(token, "active")` |
| `MilestoneApproved` | `milestone_ctrl::update_status(token, index, "completed", tx_hash, usdc_released)` |
| `ProjectFailed` | `project::update_status(token, "failed")` |
| `Refunded` | `refund_ctrl::insert(token, buyer, tokens_burned, usdc_returned, tx_hash, block, timestamp)` |

### 6.2 LP Receiver (`event/lp/receive.rs`)

```rust
pub async fn process_lp_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError>;
```

| Event | DB Operations |
|---|---|
| `LiquidityAllocated` | `lp_ctrl::insert_liquidity_position` (upserts `liquidity_positions` by `token_id`) |
| `FeesCollected` | `lp_ctrl::insert_fee_collection` (inserts into `fee_collections`, idempotent on `tx_hash`) |

### 6.3 Token Receiver (`event/token/receive.rs`)

```rust
pub async fn process_token_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<OnChainEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError>;
```

Handles `Transfer` events:
- Converts wei amount to display (18 decimals) via `wei_to_display`
- **From non-zero address**: `UPDATE balances SET balance = GREATEST(balance - amount, 0)` (upsert)
- **To non-zero address**: `INSERT INTO balances ... ON CONFLICT DO UPDATE SET balance = balance + amount`
- Skips mint (from = zero address) and burn (to = zero address) directions

### 6.4 Swap Receiver (`event/swap/receive.rs`)

```rust
pub async fn process_swap_events(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<RawSwapEvent>>,
    receive_mgr: &Arc<ReceiveManager>,
    mappings: &[PoolTokenMapping],
) -> Result<(), ObserverError>;
```

`PoolTokenMapping` maps pool address to `(token_id, is_token0)`.

Per swap event:
1. Resolve pool address to token via `PoolTokenMapping` (skip if unknown pool)
2. Parse amounts: determine `native_amount`, `token_amount`, and direction (`"BUY"` or `"SELL"`) based on `is_token0` and sign of `amount0`/`amount1`
3. Compute price: `native / token` (18 decimal places)
4. `swap_ctrl::insert` -- insert swap record
5. `chart_ctrl::upsert_bar` -- upsert 1-minute OHLCV bar (time rounded to minute)
6. `market_ctrl::upsert` -- update `market_data` with latest price

### 6.5 Price Receiver (`event/price/receive.rs`)

```rust
pub async fn process_price_updates(
    pool: &PgPool,
    rx: &mut mpsc::Receiver<EventBatch<PriceUpdate>>,
    receive_mgr: &Arc<ReceiveManager>,
) -> Result<(), ObserverError>;
```

Per price update:
- Reads existing `market_data` row for the token
- Upserts with updated `token_price`, `native_price`, `ath_price`
- If no existing row, creates a new entry with `market_type = "DEX"`, `is_graduated = true`

## 7. Retry Logic

### ObserverError (`event/error.rs`)

```rust
pub enum ObserverError {
    Skippable(String),
    Retriable(#[source] anyhow::Error),
    Fatal(#[source] anyhow::Error),
}
```

| Variant | Behavior | Typical Cause |
|---|---|---|
| `Skippable` | Logged as warning, processing continues (returns `Ok(())`) | Duplicate event, unknown pool, invalid amount |
| `Retriable` | Retried with exponential backoff up to `max_attempts` | RPC timeout, network error, transient DB failure |
| `Fatal` | Stops the handler immediately; also returned when retries exhausted | Invalid contract address, channel closed, DB schema mismatch |

### RetryConfig (`event/handler.rs`)

```rust
pub struct RetryConfig {
    pub max_attempts: u32,       // default: 5
    pub initial_backoff_ms: u64, // default: 500
    pub max_backoff_ms: u64,     // default: 30_000
    pub backoff_factor: f64,     // default: 2.0
}
```

**Backoff formula**: `delay = min(initial_backoff_ms * backoff_factor^attempt, max_backoff_ms)`

```rust
pub async fn run_event_handler_with_retry<F, Fut>(
    name: &str,
    config: &RetryConfig,
    mut f: F,
) -> Result<(), ObserverError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), ObserverError>>;
```

When retries are exhausted, the `Retriable` error is promoted to `Fatal`.

## 8. Configuration

### Observer-Local Config (`config_local.rs`)

| Variable | Env Var | Default | Description |
|---|---|---|---|
| `POLL_INTERVAL_MS` | `OBSERVER_POLL_INTERVAL_MS` | `2000` | Milliseconds between poll cycles per stream |
| `BATCH_SIZE` | `OBSERVER_BATCH_SIZE` | `100` | Maximum number of blocks per `get_logs` call |
| `START_BLOCK` | `OBSERVER_START_BLOCK` | `0` | Fallback start block when no DB progress exists |

### Shared Config (from `openlaunch_shared::config`)

| Variable | Description |
|---|---|
| `PRIMARY_DATABASE_URL` | PostgreSQL writer connection |
| `REPLICA_DATABASE_URL` | PostgreSQL reader connection |
| `MAIN_RPC_URL` | Primary RPC endpoint (Avalanche C-Chain) |
| `SUB_RPC_URL_1` | Secondary RPC endpoint |
| `SUB_RPC_URL_2` | Tertiary RPC endpoint |
| `IDO_CONTRACT` | Address of the IDO contract |
| `LP_MANAGER_CONTRACT` | Address of the LpManager contract |

### RPC Provider

The observer initializes an `RpcClient` with three prioritized providers (`Main`, `Sub1`, `Sub2`). A health-check task runs every 30 seconds. The best available provider is selected via `rpc.best_provider()` when constructing an alloy `HttpProvider` (see `event/provider.rs`).

```rust
pub fn create_provider(rpc: &Arc<RpcClient>) -> Result<HttpProvider, ObserverError>;
```

## 9. Sync Flow

### Startup

1. Load `.env` via `dotenvy`, initialize `tracing` (JSON format, env filter)
2. Connect to PostgreSQL (primary + replica)
3. Initialize `RpcClient` with 3 provider URLs
4. Create `StreamManager` and `ReceiveManager`
5. For each `EventType`, load last processed block from DB (`block_ctrl::get_last_block`); fall back to `START_BLOCK` if none exists
6. Create `mpsc` channels (buffer size 128) for each event domain
7. Spawn all tasks into a `JoinSet`:
   - Health check (30s interval)
   - Stream tasks: IDO, LP (Token, Swap, Price streams exist but are not currently spawned in `main.rs`)
   - Receive tasks: IDO, LP, Token, Swap, Price
   - Block progress persister (10s interval)

### Steady-State Polling Loop (per stream)

```
loop {
    sleep(POLL_INTERVAL_MS)
    latest_block = rpc.latest_block()
    if latest_block == 0: continue

    range = stream_mgr.get_range(event_type, latest_block)
    if range is None: continue  // caught up

    result = run_event_handler_with_retry(poll_fn)

    if Ok:
        stream_mgr.advance(event_type, range.to_block)
        block_ctrl::set_last_block(event_type, range.to_block)  // persist immediately
    else:
        log error, do NOT advance cursor (will retry same range next cycle)
}
```

### Receive Loop (per event type)

```
while batch = rx.recv():
    for event in batch.events:
        match handle(event):
            Ok       -> continue
            Skippable -> log warning, continue
            Retriable/Fatal -> return Err (kills the receive task)

    receive_mgr.mark_completed(event_type, batch.to_block)
```

### Shutdown

The observer listens for `Ctrl+C` via `tokio::signal::ctrl_c()`. It also terminates if any task in the `JoinSet` completes (successfully or with error). On termination, the last persisted block progress in the DB ensures the observer resumes from where it left off on next startup.

## 10. Database Tables Affected

| Table | Written By | Operations |
|---|---|---|
| `projects` | IDO receive | INSERT (on ProjectCreated), UPDATE status/usdc_raised |
| `accounts` | IDO receive | UPSERT (creator, buyer) |
| `investments` | IDO receive | INSERT (on TokensPurchased) |
| `milestones` | IDO receive | UPDATE status (on MilestoneApproved) |
| `refunds` | IDO receive | INSERT (on Refunded) |
| `liquidity_positions` | LP receive | UPSERT by token_id |
| `fee_collections` | LP receive | INSERT (idempotent on tx_hash) |
| `balances` | Token receive | UPSERT (add/subtract per Transfer) |
| `swaps` | Swap receive | INSERT |
| `chart_bars` | Swap receive | UPSERT 1-minute bars |
| `market_data` | Swap receive, Price receive | UPSERT (latest price, ATH) |
| `block_progress` | Main (persister task) | SET last_block per event_type |
