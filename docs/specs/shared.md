# OpenLaunch Shared Crate Specification

> **Crate:** `openlaunch-shared` v0.1.0 (Rust 2024 edition)
> **Path:** `crates/shared/src/`

---

## 1. Overview

The `shared` crate is the foundation library for the OpenLaunch backend -- an IDO (Initial DEX Offering) platform on Avalanche C-Chain. It provides types, database access, RPC client infrastructure, contract bindings, configuration, utilities, metrics, and error handling shared across all workspace crates (API server, observer/indexer, WebSocket server, etc.).

### Key Dependencies

| Dependency | Purpose |
|---|---|
| `sqlx` | PostgreSQL async queries (runtime, `PgPool`) |
| `redis` | Redis async commands (`ConnectionManager`) |
| `alloy` | Ethereum/Avalanche RPC, ABI bindings (`sol!` macro) |
| `bigdecimal` | Arbitrary-precision arithmetic for token amounts/prices |
| `moka` | In-process async cache (SingleFlight pattern) |
| `dashmap` | Concurrent hash map for provider state |
| `axum` | HTTP error response types |
| `serde` / `serde_json` | Serialization |
| `thiserror` / `anyhow` | Error types |
| `chrono` | Timestamps |
| `sha2` / `base64` / `uuid` | Session ID generation |
| `lazy_static` | Configuration constants |

---

## 2. Architecture

### Module Organization

```
shared/src/
  lib.rs              -- Re-exports all modules
  config.rs           -- Environment variable constants (lazy_static)
  error.rs            -- AppError enum, AppResult type alias
  types/
    mod.rs            -- Re-exports submodules
    common.rs         -- PaginationParams, PaginatedResponse, validate_address, current_unix_timestamp
    account.rs        -- IAccountInfo, UpdateAccountRequest
    auth.rs           -- NonceRequest/Response, SessionRequest/Response, SessionInfo
    project.rs        -- ProjectStatus, IProjectInfo, IProjectMarketInfo, IProjectData, CreateProjectRequest
    milestone.rs      -- MilestoneStatus, IMilestoneInfo, MilestoneSubmitRequest
    token.rs          -- ITokenInfo, IMarketInfo, MarketType, ITokenData, ITokenMetricsData
    trading.rs        -- TradeType, ISwapInfo, ChartBar, TradeQuote, ChartRequest
    event.rs          -- OnChainEvent enum and all event structs
  db/
    mod.rs            -- Re-exports postgres + redis
    postgres/
      mod.rs          -- PostgresDatabase (write_pool + read_pool)
      pool.rs         -- PoolConfig, create_pool
      controller/
        mod.rs        -- Re-exports all controllers
        account.rs    -- find_by_id, upsert, update
        project.rs    -- find_by_id, find_list, validate_symbol, update_status
        milestone.rs  -- find_by_project, insert_batch, update_status
        investment.rs -- insert, find_by_project
        refund.rs     -- insert, find_by_account
        swap.rs       -- insert, find_by_token
        balance.rs    -- upsert, find_by_account, find_holders
        market.rs     -- upsert, find_by_token
        chart.rs      -- upsert_bar, find_bars
        block.rs      -- get_last_block, set_last_block
    redis/
      mod.rs          -- RedisDatabase (ConnectionManager)
      session.rs      -- set_session, get_session, delete_session, set_nonce, get_and_delete_nonce
      cache.rs        -- cache_get, cache_set, cache_delete
      rate_limit.rs   -- check_rate_limit, RateLimitResult
  client/
    mod.rs            -- RpcClient struct, provider map, init, best_provider
    provider.rs       -- ProviderId enum, ProviderState (scoring)
    fallback.rs       -- execute_with_fallback, HttpProvider type alias
    api.rs            -- get_block_number, get_logs, get_block_by_number, get_block_timestamp, get_transaction_receipt, get_balance
    health.rs         -- run_health_check background loop
  contracts/
    mod.rs            -- Re-exports
    ido.rs            -- IIDO sol! interface
    lp_manager.rs     -- ILpManager sol! interface
    project_token.rs  -- IProjectToken sol! interface
  utils/
    mod.rs            -- Re-exports
    price.rs          -- calculate_price_change_percent, wei_to_display
    address.rs        -- normalize_address, generate_session_id
    single_flight.rs  -- SingleFlightCache
  metrics/
    mod.rs            -- Metrics struct (atomic counters)
```

### Dependency Graph (internal)

```
config.rs  <----  db/postgres/pool.rs
                  client/fallback.rs

types/common.rs  <----  types/account.rs
                        types/auth.rs
                        types/token.rs
                        types/trading.rs
                        types/project.rs
                        db/postgres/controller/*

types/account.rs  <----  types/auth.rs
                          types/token.rs
                          types/trading.rs
                          types/project.rs

types/milestone.rs  <----  types/project.rs
                            db/postgres/controller/milestone.rs

types/trading.rs  <----  db/postgres/controller/chart.rs

client/provider.rs  <----  client/mod.rs
                            client/fallback.rs
                            client/health.rs

client/mod.rs  <----  client/api.rs
                       client/fallback.rs
                       client/health.rs
```

---

## 3. Types

### 3.1 `types::common`

#### `PaginationParams`

```rust
pub struct PaginationParams {
    pub page: i64,   // default: 1
    pub limit: i64,  // default: 20
}
```

- `validated()` -- Returns a new instance with `page >= 1` and `limit` clamped to `[1, 100]`.
- `offset()` -- Computes `(validated_page - 1) * validated_limit`.

#### `PaginatedResponse<T>`

```rust
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: i64,
}
```

#### Free Functions

| Function | Signature | Description |
|---|---|---|
| `validate_address` | `fn validate_address(address: &str) -> anyhow::Result<String>` | Validates `0x`-prefixed, 42-char hex address; returns lowercased. |
| `current_unix_timestamp` | `fn current_unix_timestamp() -> i64` | Returns `chrono::Utc::now().timestamp()`. |

### 3.2 `types::account`

#### `IAccountInfo`

```rust
pub struct IAccountInfo {
    pub account_id: String,   // Ethereum address (lowercased)
    pub nickname: String,
    pub bio: String,
    pub image_uri: String,
}
```

- `new(account_id)` -- Creates instance with empty nickname/bio/image_uri.

#### `UpdateAccountRequest`

```rust
pub struct UpdateAccountRequest {
    pub nickname: Option<String>,
    pub bio: Option<String>,
    pub image_uri: Option<String>,
}
```

### 3.3 `types::auth`

#### `NonceRequest` / `NonceResponse`

```rust
pub struct NonceRequest { pub address: String }
pub struct NonceResponse { pub nonce: String }
```

#### `SessionRequest`

```rust
pub struct SessionRequest {
    pub nonce: String,       // 1-256 chars
    pub signature: String,   // 0x-prefixed, 132-char hex (65 bytes)
    pub chain_id: u64,
}
```

- `validate()` -- Validates signature format (0x prefix, 132 chars, hex-only) and nonce length (1-256).

#### `SessionResponse`

```rust
pub struct SessionResponse {
    pub account_info: IAccountInfo,
}
```

#### `SessionInfo`

```rust
pub struct SessionInfo {
    pub session_id: String,
    pub account_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}
```

- `is_expired()` -- Returns `true` if `now >= expires_at`.

### 3.4 `types::project`

#### `ProjectStatus`

```rust
pub enum ProjectStatus { Funding, Active, Completed, Failed }
```

Serialized as `snake_case` (`"funding"`, `"active"`, `"completed"`, `"failed"`).

#### `IProjectInfo`

```rust
pub struct IProjectInfo {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: Option<String>,
    pub tagline: String,
    pub category: String,
    pub creator: IAccountInfo,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,
}
```

#### `IProjectMarketInfo`

```rust
pub struct IProjectMarketInfo {
    pub project_id: String,
    pub status: ProjectStatus,
    pub target_raise: String,       // USDC amount (string for precision)
    pub total_committed: String,
    pub funded_percent: f64,
    pub investor_count: i64,
}
```

#### `IProjectData`

Combines `IProjectInfo`, `IProjectMarketInfo`, and `Vec<IMilestoneInfo>`.

#### `IProjectListItem`

Combines `IProjectInfo`, `IProjectMarketInfo`, `milestone_completed: i32`, `milestone_total: i32`.

#### `CreateProjectRequest`

```rust
pub struct CreateProjectRequest {
    pub name: String,           // 2-50 chars
    pub symbol: String,         // 2-10 chars, uppercase+digits only
    pub tagline: String,        // 5-120 chars
    pub description: String,    // >= 20 chars
    pub image_uri: String,      // non-empty
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub target_raise: String,   // positive BigDecimal
    pub token_supply: String,   // positive BigDecimal
    pub milestones: Vec<CreateMilestoneRequest>,  // 2-6 items, allocations sum to 100
}
```

#### `CreateMilestoneRequest`

```rust
pub struct CreateMilestoneRequest {
    pub order: i32,
    pub title: String,                  // non-empty
    pub description: String,            // non-empty
    pub fund_allocation_percent: i32,   // 1-100
}
```

**Validation rules** (`CreateProjectRequest::validate()`):
- Name: 2-50 characters
- Symbol: 2-10 characters, uppercase ASCII letters and digits only
- Tagline: 5-120 characters
- Description: >= 20 characters
- Image URI: non-empty
- `target_raise`: must parse as positive `BigDecimal`
- `token_supply`: must parse as positive `BigDecimal`
- Milestones: 2-6 items, each with non-empty title and description, each allocation 1-100, all allocations must sum to exactly 100

### 3.5 `types::milestone`

#### `MilestoneStatus`

```rust
pub enum MilestoneStatus { Completed, InVerification, Submitted, Pending, Failed }
```

Serialized as `snake_case`.

#### `IMilestoneInfo`

```rust
pub struct IMilestoneInfo {
    pub milestone_id: String,             // e.g. "ms_001"
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32,     // Stored as bps/100
    pub fund_release_amount: String,      // Wei string, default "0"
    pub status: MilestoneStatus,
    pub funds_released: bool,
    pub evidence_uri: Option<String>,
    pub submitted_at: Option<i64>,
    pub verified_at: Option<i64>,
}
```

#### `MilestoneSubmitRequest`

```rust
pub struct MilestoneSubmitRequest {
    pub evidence_text: String,
    pub evidence_uri: Option<String>,
}
```

#### `IMilestoneVerificationData`

```rust
pub struct IMilestoneVerificationData {
    pub milestone_id: String,
    pub status: MilestoneStatus,
    pub submitted_at: Option<i64>,
    pub estimated_completion: Option<i64>,
    pub dispute_info: Option<String>,
}
```

### 3.6 `types::token`

#### `MarketType`

```rust
pub enum MarketType { Curve, Dex, Ido }
```

Serialized as `UPPERCASE` (`"CURVE"`, `"DEX"`, `"IDO"`).

#### `ITokenInfo`

```rust
pub struct ITokenInfo {
    pub token_id: String,           // Token contract address
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub banner_uri: Option<String>,
    pub description: Option<String>,
    pub category: String,
    pub is_graduated: bool,         // true if token graduated from bonding curve to DEX
    pub creator: IAccountInfo,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,
    pub project_id: Option<String>,
}
```

#### `IMarketInfo`

```rust
pub struct IMarketInfo {
    pub market_type: MarketType,
    pub token_id: String,
    pub token_price: String,
    pub native_price: String,
    pub price: String,
    pub ath_price: String,
    pub total_supply: String,
    pub volume: String,
    pub holder_count: i64,
    pub bonding_percent: f64,
    pub milestone_completed: i32,
    pub milestone_total: i32,
}
```

#### `ITokenData`

Combines `ITokenInfo` and `IMarketInfo`.

#### `ITokenMetricsData`

```rust
pub struct ITokenMetricsData {
    pub metrics: HashMap<String, TimeframeMetrics>,  // key: timeframe e.g. "1h", "24h"
}

pub struct TimeframeMetrics {
    pub price_change: String,
    pub volume: String,
    pub trades: i64,
}
```

### 3.7 `types::trading`

#### `TradeType`

```rust
pub enum TradeType { Buy, Sell }
```

Serialized as `UPPERCASE` (`"BUY"`, `"SELL"`).

#### `ISwapInfo`

```rust
pub struct ISwapInfo {
    pub event_type: TradeType,
    pub native_amount: String,
    pub token_amount: String,
    pub native_price: String,
    pub transaction_hash: String,
    pub value: String,
    pub account_info: IAccountInfo,
    pub created_at: i64,
}
```

#### `ISwapWithTokenInfo`

Same as `ISwapInfo` but replaces `account_info` with `token_info: ITokenInfo` (no `account_info`).

#### `ChartBar`

```rust
pub struct ChartBar {
    pub time: i64,     // Unix timestamp (interval start)
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}
```

#### `TradeQuote`

```rust
pub struct TradeQuote {
    pub expected_output: String,
    pub price_impact_percent: String,
    pub minimum_received: String,
    pub fee: String,
}
```

#### `ChartRequest`

```rust
pub struct ChartRequest {
    pub resolution: String,          // e.g. "1h", "5m"
    pub from: i64,
    pub to: i64,
    pub countback: i64,              // default: 300
    pub chart_type: String,          // default: "price"
}
```

### 3.8 `types::event`

#### `OnChainEvent`

Tagged enum representing all on-chain events emitted by OpenLaunch contracts:

```rust
pub enum OnChainEvent {
    ProjectCreated(ProjectCreatedEvent),
    TokensPurchased(TokensPurchasedEvent),
    Graduated(GraduatedEvent),
    MilestoneApproved(MilestoneApprovedEvent),
    ProjectFailed(ProjectFailedEvent),
    Refunded(RefundedEvent),
    LiquidityAllocated(LiquidityAllocatedEvent),
    FeesCollected(FeesCollectedEvent),
    Transfer(TransferEvent),
}
```

#### Event Structs

All event structs share `block_number: u64` and `tx_hash: String`.

| Event | Key Fields |
|---|---|
| `ProjectCreatedEvent` | `token`, `creator`, `name`, `symbol`, `token_uri`, `ido_token_amount`, `token_price`, `deadline` |
| `TokensPurchasedEvent` | `token`, `buyer`, `usdc_amount`, `token_amount` |
| `GraduatedEvent` | `token` |
| `MilestoneApprovedEvent` | `token`, `milestone_index: u64`, `usdc_released` |
| `ProjectFailedEvent` | `token` |
| `RefundedEvent` | `token`, `buyer`, `tokens_burned`, `usdc_returned` |
| `LiquidityAllocatedEvent` | `token`, `pool`, `token_amount`, `tick_lower: i32`, `tick_upper: i32` |
| `FeesCollectedEvent` | `token`, `amount0`, `amount1` |
| `TransferEvent` | `token`, `from`, `to`, `amount` |

---

## 4. Database Layer

### 4.1 PostgreSQL

#### Connection Architecture

`PostgresDatabase` maintains **two connection pools**:
- **write_pool** (primary) -- For INSERT/UPDATE/DELETE operations
- **read_pool** (replica) -- For SELECT operations

```rust
pub struct PostgresDatabase {
    write_pool: PgPool,
    read_pool: PgPool,
}
```

Constructed via `PostgresDatabase::new(primary_url, replica_url)`.

#### Pool Configuration

| Parameter | Writer Default | Reader Default | Env Var |
|---|---|---|---|
| max_connections | 50 | 200 | `PG_PRIMARY_MAX_CONNECTIONS` / `PG_REPLICA_MAX_CONNECTIONS` |
| min_connections | 5 | 10 | `PG_PRIMARY_MIN_CONNECTIONS` / `PG_REPLICA_MIN_CONNECTIONS` |
| max_lifetime | 1800s | 1800s | -- |
| acquire_timeout | 10s | 10s | -- |
| idle_timeout | 300s | 300s | -- |

#### Schema (Inferred from Queries)

##### `accounts`

| Column | Type | Notes |
|---|---|---|
| `account_id` | TEXT (PK) | Ethereum address |
| `nickname` | TEXT | |
| `bio` | TEXT | |
| `image_uri` | TEXT | |
| `created_at` | BIGINT | Unix timestamp |
| `updated_at` | BIGINT | Unix timestamp |

##### `projects`

| Column | Type | Notes |
|---|---|---|
| `project_id` | TEXT (PK) | Token contract address |
| `name` | TEXT | |
| `symbol` | TEXT | Unique |
| `image_uri` | TEXT | |
| `description` | TEXT | Nullable |
| `tagline` | TEXT | |
| `category` | TEXT | |
| `creator` | TEXT | Account address |
| `status` | TEXT | `funding`, `active`, `completed`, `failed` |
| `target_raise` | NUMERIC | USDC target |
| `token_price` | NUMERIC | Price per token in USDC |
| `ido_supply` | NUMERIC | |
| `ido_sold` | NUMERIC | |
| `total_supply` | NUMERIC | |
| `usdc_raised` | NUMERIC | |
| `usdc_released` | NUMERIC | |
| `tokens_refunded` | NUMERIC | |
| `deadline` | BIGINT | Unix timestamp |
| `website` | TEXT | Nullable |
| `twitter` | TEXT | Nullable |
| `github` | TEXT | Nullable |
| `telegram` | TEXT | Nullable |
| `created_at` | BIGINT | |
| `tx_hash` | TEXT | |

##### `milestones`

| Column | Type | Notes |
|---|---|---|
| `id` | SERIAL (PK) | Auto-increment |
| `project_id` | TEXT (FK) | |
| `milestone_index` | INT | 0-based order |
| `title` | TEXT | |
| `description` | TEXT | |
| `allocation_bps` | INT | Basis points (e.g. 5000 = 50%) |
| `status` | TEXT | `pending`, `submitted`, `in_verification`, `completed`, `failed` |
| `funds_released` | BOOLEAN | |
| `release_amount` | NUMERIC | Nullable, USDC released |
| `evidence_uri` | TEXT | Nullable |
| `evidence_text` | TEXT | Nullable |
| `submitted_at` | BIGINT | Nullable |
| `verified_at` | BIGINT | Nullable |
| `tx_hash` | TEXT | Nullable |

##### `investments`

| Column | Type | Notes |
|---|---|---|
| `project_id` | TEXT | |
| `account_id` | TEXT | |
| `usdc_amount` | NUMERIC | |
| `token_amount` | NUMERIC | |
| `tx_hash` | TEXT (unique) | Conflict: DO NOTHING |
| `block_number` | BIGINT | |
| `created_at` | BIGINT | |

##### `refunds`

| Column | Type | Notes |
|---|---|---|
| `project_id` | TEXT | |
| `account_id` | TEXT | |
| `tokens_burned` | NUMERIC | |
| `usdc_returned` | NUMERIC | |
| `tx_hash` | TEXT (unique) | Conflict: DO NOTHING |
| `block_number` | BIGINT | |
| `created_at` | BIGINT | |

##### `swaps`

| Column | Type | Notes |
|---|---|---|
| `token_id` | TEXT | |
| `account_id` | TEXT | |
| `event_type` | TEXT | `BUY` or `SELL` |
| `native_amount` | NUMERIC | |
| `token_amount` | NUMERIC | |
| `price` | NUMERIC | |
| `value` | NUMERIC | |
| `tx_hash` | TEXT (unique) | Conflict: DO NOTHING |
| `block_number` | BIGINT | |
| `created_at` | BIGINT | |

##### `balances`

| Column | Type | Notes |
|---|---|---|
| `account_id` | TEXT | Composite PK: (account_id, token_id) |
| `token_id` | TEXT | |
| `balance` | NUMERIC | |
| `updated_at` | BIGINT | |

##### `market_data`

| Column | Type | Notes |
|---|---|---|
| `token_id` | TEXT (PK) | |
| `market_type` | TEXT | `CURVE`, `DEX`, `IDO` |
| `token_price` | NUMERIC | |
| `native_price` | NUMERIC | |
| `ath_price` | NUMERIC | Upsert uses `GREATEST` to preserve ATH |
| `total_supply` | NUMERIC | |
| `volume_24h` | NUMERIC | |
| `holder_count` | INT | |
| `bonding_percent` | NUMERIC | |
| `milestone_completed` | INT | |
| `milestone_total` | INT | |
| `is_graduated` | BOOLEAN | |
| `updated_at` | BIGINT | |

##### `charts`

| Column | Type | Notes |
|---|---|---|
| `token_id` | TEXT | Composite PK: (token_id, interval, time) |
| `interval` | TEXT | e.g. "1m", "5m", "1h" |
| `time` | BIGINT | Interval start timestamp |
| `open` | NUMERIC | |
| `high` | NUMERIC | Upsert: `GREATEST(existing, new)` |
| `low` | NUMERIC | Upsert: `LEAST(existing, new)` |
| `close` | NUMERIC | Upsert: overwritten |
| `volume` | NUMERIC | Upsert: accumulated (`+= new`) |

##### `block_progress`

| Column | Type | Notes |
|---|---|---|
| `event_type` | TEXT (PK) | e.g. event category name |
| `last_block` | BIGINT | Last processed block number |
| `updated_at` | BIGINT | |

#### Query Patterns

- **Idempotent inserts**: `ON CONFLICT (tx_hash) DO NOTHING` for swaps, investments, refunds.
- **Upserts**: `ON CONFLICT ... DO UPDATE` for accounts, balances, market_data, charts, block_progress.
- **Pagination**: All list queries accept `PaginationParams`, return `(Vec<Row>, total_count)`.
- **Dynamic sorting**: `find_list` for projects supports `recent`, `funded`, `target`, `investors` sort types (string interpolated into ORDER BY clause).
- **NUMERIC casts**: All monetary/amount values are cast via `$N::NUMERIC` in queries to ensure PostgreSQL stores them as precise NUMERIC.

### 4.2 Redis

#### Connection

`RedisDatabase` wraps a `redis::aio::ConnectionManager` (auto-reconnecting multiplexed connection).

```rust
pub struct RedisDatabase { conn: ConnectionManager }
```

#### Key Patterns

| Feature | Key Pattern | TTL | Operations |
|---|---|---|---|
| Sessions | `session:{session_id}` | Configurable (caller sets `ttl_secs`) | `SET_EX`, `GET`, `DEL` |
| Nonces | `nonce:{address}` | Configurable | `SET_EX`, `GETDEL` (atomic get+delete) |
| Cache | `cache:{key}` | Configurable | `SET_EX`, `GET`, `DEL` |
| Rate limit | `rate:{identifier}:{window}` | `WINDOW_SECS * 2` (120s) | `INCR`, `EXPIRE` |

#### Session Management

| Method | Signature |
|---|---|
| `set_session` | `async fn set_session(&self, session_id: &str, info: &SessionInfo, ttl_secs: u64) -> Result<()>` |
| `get_session` | `async fn get_session(&self, session_id: &str) -> Result<Option<SessionInfo>>` |
| `delete_session` | `async fn delete_session(&self, session_id: &str) -> Result<()>` |
| `set_nonce` | `async fn set_nonce(&self, address: &str, nonce: &str, ttl_secs: u64) -> Result<()>` |
| `get_and_delete_nonce` | `async fn get_and_delete_nonce(&self, address: &str) -> Result<Option<String>>` |

The nonce is consumed atomically via Redis `GETDEL` to prevent replay attacks.

#### Generic Cache

| Method | Signature |
|---|---|
| `cache_get` | `async fn cache_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>` |
| `cache_set` | `async fn cache_set<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> Result<()>` |
| `cache_delete` | `async fn cache_delete(&self, key: &str) -> Result<()>` |

Values are JSON-serialized before storage.

#### Rate Limiting

- **Algorithm**: Fixed-window counter per `WINDOW_SECS` (60 seconds).
- **Key**: `rate:{identifier}:{unix_time / 60}`
- **TTL**: 120 seconds (2x window to handle boundary).
- **Method**: `check_rate_limit(identifier, max_requests) -> RateLimitResult`

```rust
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u64,
    pub retry_after: u64,   // seconds until window resets (0 if allowed)
}
```

---

## 5. RPC Client

### 5.1 Architecture

`RpcClient` is a multi-provider Ethereum/Avalanche RPC client with health-score-based provider selection and automatic failover.

```rust
pub struct RpcClient {
    pub providers: DashMap<ProviderId, ProviderState>,
    latest_block: AtomicU64,
}
```

### 5.2 Provider Identity

```rust
pub enum ProviderId { Main, Sub1, Sub2 }
```

Priority scores (used as initial score bonus):
- `Main`: +30
- `Sub1`: +20
- `Sub2`: +10

### 5.3 Scoring Algorithm

Each `ProviderState` maintains an atomic health score:

- **Initial score**: `50 + priority_score` (Main=80, Sub1=70, Sub2=60)
- **Score range**: `[0, 100]`

**On failure** (`record_failure`):
- Failure count increments
- Penalty is based on cumulative failure count:

| Failures | Penalty |
|---|---|
| 1-2 | 15 |
| 3-5 | 30 |
| 6-10 | 50 |
| 11+ | 70 |

- New score = `max(base - penalty, 0)` where `base = 50 + priority_score`

**On success** (`record_success`):
- Score increases by +2 (capped at 100)
- Failure count resets to 0 (next failure restarts the penalty ladder)

### 5.4 Fallback Logic

`execute_with_fallback` is the core execution method:

```rust
pub async fn execute_with_fallback<F, Fut, T>(
    self: &Arc<Self>,
    operation: F,
) -> anyhow::Result<T>
where
    F: Fn(HttpProvider) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
```

**Algorithm**:
1. Sort all providers by score (highest first).
2. For each provider:
   a. Build an `HttpProvider` from the URL via `ProviderBuilder::new().connect_http(url)`.
   b. Execute `operation(provider)` wrapped in `tokio::time::timeout(RPC_TIMEOUT_MS)`.
   c. On success: reward provider, return result.
   d. On failure or timeout: penalize provider, log warning, try next.
3. If all providers fail, return the last error.

**Timeout**: Configured via `RPC_TIMEOUT_MS` env var (default: 30000ms).

### 5.5 Public API Methods

All methods use `execute_with_fallback` internally and take `self: &Arc<Self>`:

| Method | Return Type | Description |
|---|---|---|
| `get_block_number` | `u64` | Latest block number |
| `get_logs(filter)` | `Vec<Log>` | Event logs matching filter |
| `get_block_by_number(n)` | `Option<Block>` | Block by number |
| `get_block_timestamp(n)` | `u64` | Block header timestamp |
| `get_transaction_receipt(hash)` | `Option<TransactionReceipt>` | Tx receipt by hash |
| `get_balance(address)` | `U256` | Native token balance |

### 5.6 Health Check Loop

`run_health_check(client, interval)` -- Runs in the background as a `tokio::spawn` task.

**Each iteration**:
1. Sleep for `interval`.
2. Call `get_block_number()` to update `latest_block`.
3. For each provider:
   - Query `get_block_number()` with a 5-second timeout.
   - If provider block is more than **10 blocks behind** `latest_block`: penalize (stale).
   - If query fails or times out: penalize.

### 5.7 Initialization

```rust
pub async fn init(rpc_urls: Vec<(ProviderId, String)>) -> anyhow::Result<Arc<Self>>
```

- Skips empty URLs.
- Fails if no providers remain after filtering.
- Returns `Arc<RpcClient>`.

---

## 6. Configuration

All configuration is loaded from environment variables via `lazy_static!` at first access.

### Required Environment Variables

| Variable | Type | Description |
|---|---|---|
| `PRIMARY_DATABASE_URL` (or `DATABASE_URL`) | String | PostgreSQL primary connection URL |
| `REDIS_URL` | String | Redis connection URL |
| `MAIN_RPC_URL` | String | Primary Avalanche C-Chain RPC URL |
| `IDO_CONTRACT` | String | IDO contract address |
| `LP_MANAGER_CONTRACT` | String | LP Manager contract address |
| `USDC_ADDRESS` | String | USDC token address on C-Chain |

### Optional Environment Variables

| Variable | Type | Default | Description |
|---|---|---|---|
| `REPLICA_DATABASE_URL` | String | same as primary | PostgreSQL read-replica URL |
| `SUB_RPC_URL_1` | String | `""` | Secondary RPC endpoint |
| `SUB_RPC_URL_2` | String | `""` | Tertiary RPC endpoint |
| `RPC_TIMEOUT_MS` | u64 | 30000 | RPC call timeout in milliseconds |
| `CHAIN_ID` | u64 | 43114 | Avalanche C-Chain mainnet |
| `PG_PRIMARY_MAX_CONNECTIONS` | u32 | 50 | Writer pool max connections |
| `PG_PRIMARY_MIN_CONNECTIONS` | u32 | 5 | Writer pool min connections |
| `PG_REPLICA_MAX_CONNECTIONS` | u32 | 200 | Reader pool max connections |
| `PG_REPLICA_MIN_CONNECTIONS` | u32 | 10 | Reader pool min connections |

---

## 7. Contracts

Solidity interface bindings generated via the `alloy::sol!` macro with `#[sol(rpc)]` for type-safe contract interaction.

### 7.1 `IIDO` (IDO Pool)

**Source**: `contracts/ido.rs`

#### Structs

```solidity
enum Status { Active, Graduated, Failed }

struct Project {
    address creator;
    Status status;
    uint256 tokenPrice;
    uint256 idoSupply;
    uint256 idoSold;
    uint256 deadline;
    uint256 usdcRaised;
    uint256 usdcReleased;
    uint256 tokensRefunded;
}

struct Milestone {
    uint256 percentage;
    bool isApproved;
}

struct CreateParams {
    string name;
    string symbol;
    string tokenURI;
    uint256 idoTokenAmount;
    uint256 tokenPrice;
    uint256 deadline;
    uint256[] milestonePercentages;
    bytes32 salt;
}
```

#### Events

| Event | Indexed Fields | Data Fields |
|---|---|---|
| `ProjectCreated` | `token`, `creator` | `name`, `symbol`, `tokenURI`, `idoTokenAmount`, `tokenPrice`, `deadline` |
| `TokensPurchased` | `token`, `buyer` | `usdcAmount`, `tokenAmount` |
| `Graduated` | `token` | -- |
| `MilestoneApproved` | `token`, `milestoneIndex` | `usdcReleased` |
| `ProjectFailed` | `token` | -- |
| `Refunded` | `token`, `buyer` | `tokensBurned`, `usdcReturned` |
| `FeeManagerUpdated` | `newFeeManager` | -- |
| `LpManagerUpdated` | `newLpManager` | -- |
| `ProtocolTreasuryUpdated` | `newProtocolTreasury` | -- |

#### Functions

| Function | Mutability | Description |
|---|---|---|
| `create(CreateParams)` | write | Creates a new IDO project, deploys token, returns token address |
| `buy(token, usdcAmount)` | write | Purchase IDO tokens with USDC |
| `graduate(token)` | write | Graduate project (IDO complete, deploy to DEX) |
| `approveMilestone(token, index)` | write | Approve milestone, release funds |
| `failProject(token)` | write | Mark project as failed |
| `refund(token, tokenAmount)` | write | Burn tokens, receive USDC refund |
| `collectFees(token)` | write | Collect LP fees |
| `projects(token)` | view | Get project state |
| `getMilestones(token)` | view | Get milestone array |
| `USDC()` | view | USDC token address |
| `TOKEN_IMPLEMENTATION()` | view | Token implementation address |
| `feeManager()` | view | Fee manager address |
| `lpManager()` | view | LP manager address |
| `protocolTreasury()` | view | Treasury address |
| `TOTAL_SUPPLY()` | view | Token total supply constant |
| `MAX_MILESTONES()` | view | Max milestones per project |

#### Errors

`InvalidName`, `InvalidSymbol`, `InvalidTokenPrice`, `InvalidDeadline`, `InvalidMilestonePercentages`, `InvalidIdoTokenAmount`, `IDONotActive`, `IDOExceedsSupply`, `AlreadyGraduated`, `IDONotFinished`, `MilestoneAlreadyApproved`, `InvalidMilestoneIndex`, `ProjectAlreadyFailed`, `ProjectNotFailed`, `ZeroAmount`, `ProjectNotFound`, `CreatorCannotRefund`, `InsufficientPurchaseAmount`, `ZeroAddress`, `TooManyMilestones`, `ExceedsRefundableAmount`

### 7.2 `ILpManager` (Liquidity Manager)

**Source**: `contracts/lp_manager.rs`

```solidity
struct AllocateParams {
    address token;
    address usdc;
    uint256 tokenAmount;
    uint256 usdcAmount;
    uint256 tokenPrice;
}
```

| Function | Description |
|---|---|
| `allocate(AllocateParams)` | Allocate liquidity to a Uniswap V3 pool |
| `collectFees(token, recipient)` | Collect accumulated LP fees |

| Event | Key Fields |
|---|---|
| `LiquidityAllocated` | `token` (indexed), `pool` (indexed), `tokenAmount`, `tickLower`, `tickUpper` |
| `FeesCollected` | `token` (indexed), `amount0`, `amount1` |

| Error | Description |
|---|---|
| `OnlyIDO` | Caller must be IDO contract |
| `OnlyPoolManager` | Caller must be pool manager |
| `ZeroAddress` | Address parameter is zero |
| `InvalidTokenAmount` | Token amount is invalid |
| `PositionNotFound` | LP position not found |

### 7.3 `IProjectToken` (ERC20 + Extensions)

**Source**: `contracts/project_token.rs`

Standard ERC20 interface (`name`, `symbol`, `decimals`, `totalSupply`, `balanceOf`, `transfer`, `allowance`, `approve`, `transferFrom`) plus:

| Function | Description |
|---|---|
| `TOTAL_SUPPLY()` | Constant total supply |
| `initialize(name, symbol, tokenURI, mintTo)` | Initialize cloned token |
| `burn(amount)` | Burn own tokens |
| `burnFrom(account, amount)` | Burn from approved account |

Events: `Transfer(from, to, value)`, `Approval(owner, spender, value)`.

---

## 8. Utils

### 8.1 Price Conversion (`utils/price`)

```rust
/// Calculate percentage change: ((new - old) / old) * 100, formatted to 2 decimal places.
/// Returns "0" if old_price is zero.
pub fn calculate_price_change_percent(old_price: &str, new_price: &str) -> anyhow::Result<String>

/// Convert a wei-denominated string to human-readable decimal.
/// Handles arbitrary decimals (including > 19) using BigDecimal division.
pub fn wei_to_display(wei: &str, decimals: u32) -> anyhow::Result<String>
```

Both functions use `BigDecimal` for arbitrary-precision arithmetic. `wei_to_display` avoids `u64` overflow for `decimals > 19` by using iterative BigDecimal multiplication instead of `10u64.pow(decimals)`.

### 8.2 Address Utilities (`utils/address`)

```rust
/// Validate and normalize an Ethereum address to lowercase.
/// Delegates to types::common::validate_address.
pub fn normalize_address(address: &str) -> anyhow::Result<String>

/// Generate a unique 32-character URL-safe session ID.
/// Uses SHA-256(address + timestamp + UUIDv4), then URL-safe base64, truncated to 32 chars.
pub fn generate_session_id(address: &str, timestamp: i64) -> String
```

### 8.3 SingleFlight Cache (`utils/single_flight`)

```rust
pub struct SingleFlightCache {
    cache: moka::future::Cache<String, Arc<String>>,
}
```

Deduplicates concurrent identical requests using Moka's `try_get_with`:

```rust
pub fn new(max_capacity: u64, ttl: Duration) -> Self

pub async fn get_or_insert<T, F, Fut>(&self, key: &str, f: F) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<T>> + Send + 'static,
```

- Values are serialized to JSON for storage, deserialized on retrieval.
- Concurrent callers for the same key block and share a single computation result.
- Backed by Moka's TTL-based eviction.

---

## 9. Error Handling

### `AppError` Enum

```rust
#[derive(Error, Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict,
    TooManyRequests { retry_after: u64 },
    Internal(#[from] anyhow::Error),
}
```

### HTTP Response Mapping

Implements `axum::response::IntoResponse`:

| Variant | HTTP Status | Response Code |
|---|---|---|
| `BadRequest(msg)` | 400 | `BAD_REQUEST` |
| `Unauthorized(msg)` | 401 | `UNAUTHORIZED` |
| `Forbidden(msg)` | 403 | `FORBIDDEN` |
| `NotFound(msg)` | 404 | `NOT_FOUND` |
| `Conflict` | 409 | `CONFLICT` |
| `TooManyRequests { retry_after }` | 429 | `TOO_MANY_REQUESTS` |
| `Internal(err)` | 500 | `INTERNAL_ERROR` |

### Response Body Format

```json
{
    "error": "<user-facing message>",
    "code": "<ERROR_CODE>"
}
```

For `TooManyRequests`, the body also includes `"retry_after": <seconds>` and sets the `Retry-After` HTTP header.

**Security**: `Internal` errors log the real error via `tracing::error!` but return a generic `"Internal server error"` message to the client -- sensitive details are never leaked.

### Type Alias

```rust
pub type AppResult<T> = Result<T, AppError>;
```

---

## 10. Metrics

### `Metrics` Struct

Thread-safe atomic counters for observability:

```rust
pub struct Metrics {
    pub db_queries: AtomicU64,
    pub db_errors: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub rpc_requests: AtomicU64,
    pub rpc_errors: AtomicU64,
}
```

Recording methods: `record_db_query()`, `record_db_error()`, `record_cache_hit()`, `record_cache_miss()`, `record_rpc_request()`, `record_rpc_error()`.

All counters use `Ordering::Relaxed` for minimal overhead.
