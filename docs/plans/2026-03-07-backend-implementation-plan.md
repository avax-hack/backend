# OpenLaunch Backend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build 4 Rust backend services (api-server, websocket-server, observer, txbot) with a shared crate, using Cargo workspace.

**Architecture:** Cargo workspace with 5 crates. `shared` provides types, DB, RPC client. Each service is an independent binary that connects directly to Avalanche C-Chain and/or PostgreSQL+Redis. Services don't communicate with each other.

**Tech Stack:** Rust 2024, Tokio, Axum 0.8, Alloy 1.0, SQLx 0.8, Redis 0.29, Moka, DashMap, Serde, Tracing

**Reference:** nadfun codebase at `/Users/gyu/project/nadfun/` (api-server, websocket-server, observer, txbot) — same architecture in Rust, adapt patterns but write from scratch.

**Design Doc:** `docs/plans/2026-03-07-backend-architecture-design.md`

**Contract ABIs:** `/Users/gyu/project/openlaunch/contract/out/` (Foundry build output)

---

## Implementation Phases

| Phase | Crate | Description | Dependencies |
|-------|-------|-------------|-------------|
| 1 | workspace + shared (scaffolding) | Workspace setup, types, config, error | None |
| 2 | shared (DB) | PostgreSQL pool, Redis client, migrations | Phase 1 |
| 3 | shared (RPC) | Multi-provider RPC client with health scoring | Phase 1 |
| 4 | observer | On-chain event indexing → PostgreSQL | Phase 2, 3 |
| 5 | api-server | REST API serving indexed data | Phase 2 |
| 6 | websocket-server | Real-time event streaming to clients | Phase 3 |
| 7 | txbot | Automated graduate + collectFees transactions | Phase 2, 3 |

---

## Phase 1: Workspace + Shared Scaffolding

### Task 1.1: Initialize Cargo Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/shared/Cargo.toml`
- Create: `crates/shared/src/lib.rs`
- Create: `crates/api-server/Cargo.toml`
- Create: `crates/api-server/src/main.rs`
- Create: `crates/websocket-server/Cargo.toml`
- Create: `crates/websocket-server/src/main.rs`
- Create: `crates/observer/Cargo.toml`
- Create: `crates/observer/src/main.rs`
- Create: `crates/txbot/Cargo.toml`
- Create: `crates/txbot/src/main.rs`

**Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/shared",
    "crates/api-server",
    "crates/websocket-server",
    "crates/observer",
    "crates/txbot",
]

[workspace.dependencies]
# Async
tokio = { version = "1.40", features = ["full"] }
futures-util = "0.3"

# Web
axum = { version = "0.8", features = ["ws"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "timeout", "limit"] }
tower-cookies = "0.10"

# Blockchain
alloy = { version = "1.0", features = ["full"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "bigdecimal", "chrono", "json", "uuid"] }
redis = { version = "0.29", features = ["tokio-comp", "connection-manager"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bigdecimal = { version = "0.4", features = ["serde"] }

# Caching
moka = { version = "0.12", features = ["future"] }
dashmap = "6.1"

# Error & Logging
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

# Utilities
uuid = { version = "1.11", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
rand = "0.8"
sha2 = "0.10"
base64 = "0.22"
dotenvy = "0.15"
lazy_static = "1.5"
once_cell = "1.21"
```

**Step 2: Create shared crate Cargo.toml**

```toml
[package]
name = "openlaunch-shared"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
bigdecimal = { workspace = true }
sqlx = { workspace = true }
redis = { workspace = true }
alloy = { workspace = true }
moka = { workspace = true }
dashmap = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
dotenvy = { workspace = true }
lazy_static = { workspace = true }
once_cell = { workspace = true }
```

**Step 3: Create each binary crate with minimal Cargo.toml and main.rs**

Each binary crate depends on `openlaunch-shared` and has a simple `main.rs` that prints the service name.

**Step 4: Verify workspace builds**

Run: `cargo build --workspace`
Expected: All 5 crates compile successfully.

**Step 5: Commit**

```bash
git init
git add -A
git commit -m "feat: initialize cargo workspace with 5 crates"
```

---

### Task 1.2: Shared Types — Common

**Files:**
- Create: `crates/shared/src/types/mod.rs`
- Create: `crates/shared/src/types/common.rs`
- Test: `crates/shared/tests/types_common_test.rs`

**Step 1: Write tests for common types**

```rust
// crates/shared/tests/types_common_test.rs
use openlaunch_shared::types::common::{PaginationParams, validate_address};

#[test]
fn test_pagination_defaults() {
    let params = PaginationParams::default();
    assert_eq!(params.page, 1);
    assert_eq!(params.limit, 20);
}

#[test]
fn test_pagination_clamp() {
    let params = PaginationParams { page: 0, limit: 200 };
    let clamped = params.validated();
    assert_eq!(clamped.page, 1);
    assert_eq!(clamped.limit, 100);
}

#[test]
fn test_validate_address_valid() {
    assert!(validate_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B").is_ok());
}

#[test]
fn test_validate_address_invalid() {
    assert!(validate_address("not_an_address").is_err());
    assert!(validate_address("0x123").is_err());
    assert!(validate_address("").is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p openlaunch-shared --test types_common_test`
Expected: FAIL — modules don't exist yet.

**Step 3: Implement common types**

```rust
// crates/shared/src/types/common.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: i64,
    pub limit: i64,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self { page: 1, limit: 20 }
    }
}

impl PaginationParams {
    pub fn validated(&self) -> Self {
        Self {
            page: self.page.max(1),
            limit: self.limit.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> i64 {
        (self.validated().page - 1) * self.validated().limit
    }
}

pub fn validate_address(address: &str) -> anyhow::Result<String> {
    if !address.starts_with("0x") {
        anyhow::bail!("Address must start with 0x");
    }
    if address.len() != 42 {
        anyhow::bail!("Address must be 42 characters");
    }
    // Validate hex characters after 0x
    if !address[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Address contains invalid hex characters");
    }
    Ok(address.to_string())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p openlaunch-shared --test types_common_test`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add crates/shared/src/types/ crates/shared/tests/
git commit -m "feat(shared): add common types with pagination and address validation"
```

---

### Task 1.3: Shared Types — Account

**Files:**
- Create: `crates/shared/src/types/account.rs`
- Test: `crates/shared/tests/types_account_test.rs`

**Step 1: Write tests for account types**

Test serialization/deserialization of `IAccountInfo`.

**Step 2: Run test — FAIL**

**Step 3: Implement**

```rust
// crates/shared/src/types/account.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IAccountInfo {
    pub account_id: String,
    pub nickname: String,
    pub bio: String,
    pub image_uri: String,
}

impl Default for IAccountInfo {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            nickname: String::new(),
            bio: String::new(),
            image_uri: String::new(),
        }
    }
}
```

**Step 4: Run test — PASS**

**Step 5: Commit**

---

### Task 1.4: Shared Types — Project

**Files:**
- Create: `crates/shared/src/types/project.rs`
- Test: `crates/shared/tests/types_project_test.rs`

Implement: `IProjectInfo`, `IProjectMarketInfo`, `ProjectStatus` enum, `CreateProjectRequest`.

Test: Serialization, status enum string mapping, validation of create request fields.

---

### Task 1.5: Shared Types — Milestone

**Files:**
- Create: `crates/shared/src/types/milestone.rs`
- Test: `crates/shared/tests/types_milestone_test.rs`

Implement: `IMilestoneInfo`, `MilestoneStatus` enum, `MilestoneSubmitRequest`.

Test: Serialization, status enum mapping, allocation validation (sum to 10000 bps).

---

### Task 1.6: Shared Types — Token & Trading

**Files:**
- Create: `crates/shared/src/types/token.rs`
- Create: `crates/shared/src/types/trading.rs`
- Test: `crates/shared/tests/types_token_test.rs`
- Test: `crates/shared/tests/types_trading_test.rs`

Implement:
- `ITokenInfo`, `IMarketInfo` (token.rs)
- `ISwapInfo`, `ChartBar`, `TradeQuote` (trading.rs)

Test: Serialization, BigDecimal price fields, chart bar OHLCV validation.

---

### Task 1.7: Shared Types — Auth & Event

**Files:**
- Create: `crates/shared/src/types/auth.rs`
- Create: `crates/shared/src/types/event.rs`
- Test: `crates/shared/tests/types_auth_test.rs`

Implement:
- `NonceRequest`, `SessionRequest`, `SessionInfo` (auth.rs)
- `OnChainEvent` enum variants matching contract events (event.rs)

Test: Request validation, session info serialization.

---

### Task 1.8: Shared Config & Error

**Files:**
- Create: `crates/shared/src/config.rs`
- Create: `crates/shared/src/error.rs`
- Test: `crates/shared/tests/config_test.rs`

**Config:**
```rust
// crates/shared/src/config.rs
use lazy_static::lazy_static;

lazy_static! {
    // Database
    pub static ref PRIMARY_DATABASE_URL: String =
        std::env::var("PRIMARY_DATABASE_URL").expect("PRIMARY_DATABASE_URL required");
    pub static ref REPLICA_DATABASE_URL: String =
        std::env::var("REPLICA_DATABASE_URL").unwrap_or_else(|_| PRIMARY_DATABASE_URL.clone());
    pub static ref REDIS_URL: String =
        std::env::var("REDIS_URL").expect("REDIS_URL required");

    // RPC
    pub static ref MAIN_RPC_URL: String =
        std::env::var("MAIN_RPC_URL").expect("MAIN_RPC_URL required");
    pub static ref SUB_RPC_URL_1: String =
        std::env::var("SUB_RPC_URL_1").unwrap_or_default();
    pub static ref SUB_RPC_URL_2: String =
        std::env::var("SUB_RPC_URL_2").unwrap_or_default();

    // Contract Addresses
    pub static ref IDO_CONTRACT: String =
        std::env::var("IDO_CONTRACT").expect("IDO_CONTRACT required");
    pub static ref LP_MANAGER_CONTRACT: String =
        std::env::var("LP_MANAGER_CONTRACT").expect("LP_MANAGER_CONTRACT required");
    pub static ref USDC_ADDRESS: String =
        std::env::var("USDC_ADDRESS").expect("USDC_ADDRESS required");

    // Chain
    pub static ref CHAIN_ID: u64 =
        std::env::var("CHAIN_ID").unwrap_or_else(|_| "43114".to_string())
            .parse().expect("CHAIN_ID must be a number");

    // Connection pool
    pub static ref PG_PRIMARY_MAX_CONNECTIONS: u32 =
        std::env::var("PG_PRIMARY_MAX_CONNECTIONS").unwrap_or_else(|_| "50".to_string())
            .parse().unwrap_or(50);
    pub static ref PG_REPLICA_MAX_CONNECTIONS: u32 =
        std::env::var("PG_REPLICA_MAX_CONNECTIONS").unwrap_or_else(|_| "200".to_string())
            .parse().unwrap_or(200);
}
```

**Error:**
```rust
// crates/shared/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Too many requests")]
    TooManyRequests { retry_after: u64 },
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
```

**Step 5: Commit**

---

## Phase 2: Shared — Database Layer

### Task 2.1: PostgreSQL Connection Pool

**Files:**
- Create: `crates/shared/src/db/mod.rs`
- Create: `crates/shared/src/db/postgres/mod.rs`
- Create: `crates/shared/src/db/postgres/pool.rs`
- Test: `crates/shared/tests/db_postgres_test.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/db/postgres/mod.rs`

Implement `PostgresDatabase` with read/write pool separation:

```rust
pub struct PostgresDatabase {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresDatabase {
    pub async fn new() -> anyhow::Result<Self>;
    pub fn writer(&self) -> &PgPool;
    pub fn reader(&self) -> &PgPool;
}
```

Test: Connection pool creation with test database (use `#[sqlx::test]`).

---

### Task 2.2: PostgreSQL Migrations

**Files:**
- Create: `migrations/001_create_accounts.sql`
- Create: `migrations/002_create_sessions.sql`
- Create: `migrations/003_create_projects.sql`
- Create: `migrations/004_create_milestones.sql`
- Create: `migrations/005_create_investments.sql`
- Create: `migrations/006_create_refunds.sql`
- Create: `migrations/007_create_swaps.sql`
- Create: `migrations/008_create_balances.sql`
- Create: `migrations/009_create_charts.sql`
- Create: `migrations/010_create_market_data.sql`
- Create: `migrations/011_create_liquidity_positions.sql`
- Create: `migrations/012_create_fee_collections.sql`
- Create: `migrations/013_create_block_progress.sql`
- Create: `migrations/014_create_funding_snapshots.sql`

**Reference:** Design doc Section 8 (Database Schema).

Full SQL in each migration file. Use `sqlx migrate run` to apply.

**Step 1: Write all migration SQL files** (from design doc schema)

**Step 2: Run migrations**

```bash
DATABASE_URL=postgres://... sqlx migrate run
```

**Step 3: Verify tables exist**

```bash
DATABASE_URL=postgres://... sqlx migrate info
```

**Step 4: Commit**

```bash
git add migrations/
git commit -m "feat(db): add PostgreSQL schema migrations"
```

---

### Task 2.3: PostgreSQL Controllers — Account

**Files:**
- Create: `crates/shared/src/db/postgres/controller/mod.rs`
- Create: `crates/shared/src/db/postgres/controller/account.rs`
- Test: `crates/shared/tests/db_account_controller_test.rs`

Implement:
- `find_by_id(pool, account_id) -> Option<IAccountInfo>`
- `upsert(pool, account_id) -> IAccountInfo`
- `update(pool, account_id, nickname, bio, image_uri) -> IAccountInfo`

TDD: Write test first with `#[sqlx::test]` macro (auto-creates test DB, runs migrations).

---

### Task 2.4: PostgreSQL Controllers — Project

**Files:**
- Create: `crates/shared/src/db/postgres/controller/project.rs`
- Test: `crates/shared/tests/db_project_controller_test.rs`

Implement:
- `find_by_id(pool, project_id) -> Option<ProjectData>` (project + milestones joined)
- `find_featured(pool) -> Vec<FeaturedProject>`
- `find_list(pool, sort, pagination, status) -> (Vec<ProjectListItem>, i64)`
- `insert(pool, project) -> ProjectData`
- `update_status(pool, project_id, status)`
- `update_market_data(pool, project_id, ido_sold, usdc_raised)`
- `validate_symbol(pool, symbol) -> bool`

---

### Task 2.5: PostgreSQL Controllers — Milestone, Investment, Refund

**Files:**
- Create: `crates/shared/src/db/postgres/controller/milestone.rs`
- Create: `crates/shared/src/db/postgres/controller/investment.rs`
- Create: `crates/shared/src/db/postgres/controller/refund.rs`
- Test: `crates/shared/tests/db_milestone_controller_test.rs`

Milestone:
- `find_by_project(pool, project_id) -> Vec<IMilestoneInfo>`
- `insert_batch(pool, project_id, milestones)`
- `update_status(pool, milestone_id, status, evidence_uri, tx_hash)`

Investment:
- `insert(pool, investment)`
- `find_by_project(pool, project_id, pagination) -> (Vec<InvestmentRecord>, i64)`
- `find_by_account(pool, account_id, pagination) -> (Vec<IdoHistory>, i64)`

Refund:
- `insert(pool, refund)`
- `find_by_account(pool, account_id, pagination) -> (Vec<RefundRecord>, i64)`

---

### Task 2.6: PostgreSQL Controllers — Swap, Balance, Chart, Market

**Files:**
- Create: `crates/shared/src/db/postgres/controller/swap.rs`
- Create: `crates/shared/src/db/postgres/controller/balance.rs`
- Create: `crates/shared/src/db/postgres/controller/chart.rs`
- Create: `crates/shared/src/db/postgres/controller/market.rs`
- Test: `crates/shared/tests/db_swap_controller_test.rs`
- Test: `crates/shared/tests/db_chart_controller_test.rs`

Swap:
- `insert(pool, swap)`
- `find_by_token(pool, token_id, pagination, trade_type) -> (Vec<ISwapInfo>, i64)`
- `find_by_account(pool, account_id, pagination) -> (Vec<SwapHistory>, i64)`

Balance:
- `upsert(pool, account_id, token_id, balance)`
- `find_by_account(pool, account_id, pagination) -> (Vec<HoldToken>, i64)`
- `find_holders(pool, token_id, pagination) -> (Vec<Holder>, i64)`

Chart:
- `upsert_bar(pool, token_id, interval, bar: ChartBar)`
- `find_bars(pool, token_id, interval, from, to, limit) -> Vec<ChartBar>`

Market:
- `upsert(pool, market_data)`
- `find_by_token(pool, token_id) -> Option<IMarketInfo>`
- `find_trending(pool) -> Vec<TokenWithMarket>`

---

### Task 2.7: PostgreSQL Controllers — Block Progress

**Files:**
- Create: `crates/shared/src/db/postgres/controller/block.rs`
- Test: `crates/shared/tests/db_block_controller_test.rs`

Implement:
- `get_last_block(pool, event_type) -> Option<u64>`
- `set_last_block(pool, event_type, block_number)`

---

### Task 2.8: Redis Client

**Files:**
- Create: `crates/shared/src/db/redis/mod.rs`
- Create: `crates/shared/src/db/redis/session.rs`
- Create: `crates/shared/src/db/redis/cache.rs`
- Create: `crates/shared/src/db/redis/rate_limit.rs`
- Test: `crates/shared/tests/db_redis_test.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/db/redis/mod.rs`

```rust
pub struct RedisDatabase {
    conn: redis::aio::ConnectionManager,
}

impl RedisDatabase {
    pub async fn new() -> anyhow::Result<Self>;
}
```

Session: set/get/delete session, set/getdel nonce
Cache: generic get/set with TTL
Rate limit: increment counter with TTL, check limit

---

### Task 2.9: Shared Utils — Price, SingleFlight

**Files:**
- Create: `crates/shared/src/utils/mod.rs`
- Create: `crates/shared/src/utils/address.rs`
- Create: `crates/shared/src/utils/price.rs`
- Create: `crates/shared/src/utils/single_flight.rs`
- Test: `crates/shared/tests/utils_test.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/utils/single_flight.rs`

SingleFlight: Moka cache-based deduplication for concurrent identical requests.

---

## Phase 3: Shared — RPC Client

### Task 3.1: RPC Client — Multi-Provider with Health Scoring

**Files:**
- Create: `crates/shared/src/client/mod.rs`
- Create: `crates/shared/src/client/provider.rs`
- Create: `crates/shared/src/client/health.rs`
- Test: `crates/shared/tests/rpc_client_test.rs`

**Reference:** `/Users/gyu/project/nadfun/websocket-server/src/client/mod.rs`

```rust
pub struct RpcClient {
    providers: DashMap<ProviderId, ProviderState>,
    latest_block: AtomicU64,
}

impl RpcClient {
    pub async fn new() -> anyhow::Result<Arc<Self>>;
    pub async fn get_logs(&self, filter: &Filter) -> anyhow::Result<Vec<Log>>;
    pub async fn subscribe_blocks(&self) -> anyhow::Result<impl Stream<Item = u64>>;
    pub async fn call_contract<T: SolCall>(&self, address: Address, call: T) -> anyhow::Result<T::Return>;
    pub async fn send_tx(&self, tx: TransactionRequest, wallet: &EthereumWallet) -> anyhow::Result<TxHash>;
    pub fn latest_block(&self) -> u64;
}
```

Provider scoring: 0-100, penalize on failure, recover on success.
Health check: background task polling block numbers, detect stale providers.

Test: Unit test scoring logic, provider selection, failover.

---

### Task 3.2: Contract ABI Bindings

**Files:**
- Create: `crates/shared/src/contracts/mod.rs`
- Create: `crates/shared/src/contracts/ido.rs`
- Create: `crates/shared/src/contracts/project_token.rs`
- Create: `crates/shared/src/contracts/lp_manager.rs`

Use Alloy `sol!` macro to generate Rust bindings from contract ABIs.

```rust
// crates/shared/src/contracts/ido.rs
use alloy::sol;

sol!(
    #[sol(rpc)]
    IIDO,
    "../../abi/IIDO.json"
);
```

**Step 1: Copy ABI JSON files from contract build output**

```bash
cp contract/out/IDO.sol/IDO.json abi/IIDO.json
cp contract/out/ProjectToken.sol/ProjectToken.json abi/IProjectToken.json
cp contract/out/LpManager.sol/LpManager.json abi/ILpManager.json
```

**Step 2: Create Alloy sol! bindings**

**Step 3: Verify compilation**

Run: `cargo build -p openlaunch-shared`

**Step 4: Commit**

---

## Phase 4: Observer

### Task 4.1: Observer Scaffolding

**Files:**
- Create: `crates/observer/src/main.rs` (full implementation)
- Create: `crates/observer/src/config.rs`
- Create: `crates/observer/src/event/mod.rs`
- Create: `crates/observer/src/event/core.rs`
- Create: `crates/observer/src/event/handler.rs`
- Create: `crates/observer/src/event/error.rs`
- Create: `crates/observer/src/sync/mod.rs`
- Create: `crates/observer/src/sync/stream.rs`
- Create: `crates/observer/src/sync/receive.rs`

**Reference:** `/Users/gyu/project/nadfun/observer/src/`

**main.rs pattern:**
1. Load env (dotenvy)
2. Init tracing
3. Init PostgresDatabase
4. Init RpcClient
5. Load block progress from DB
6. Spawn event handlers (JoinSet)
7. Spawn metrics server
8. Await all

**handler.rs:** `run_event_handler_with_retry` with exponential backoff.

**sync/stream.rs:** StreamManager tracking from_block → to_block per event type.

**sync/receive.rs:** ReceiveManager with dependency ordering.

---

### Task 4.2: Observer — IDO Event Handler

**Files:**
- Create: `crates/observer/src/event/ido/mod.rs`
- Create: `crates/observer/src/event/ido/stream.rs`
- Create: `crates/observer/src/event/ido/receive.rs`
- Test: `crates/observer/tests/ido_event_test.rs`

**Events indexed:**
- `ProjectCreated` → INSERT into projects + milestones + accounts (creator)
- `TokensPurchased` → INSERT into investments, UPDATE projects (ido_sold, usdc_raised), UPSERT accounts (buyer), UPSERT balances
- `Graduated` → UPDATE projects.status = 'active'
- `MilestoneApproved` → UPDATE milestones.status = 'completed', funds_released = true
- `ProjectFailed` → UPDATE projects.status = 'failed'
- `Refunded` → INSERT into refunds, UPDATE projects (usdc_released), UPDATE balances

**stream.rs:**
```rust
pub async fn stream_ido_events(
    rpc: Arc<RpcClient>,
    ido_address: Address,
    sender: mpsc::Sender<EventBatch<IdoEvent>>,
    stream_manager: Arc<StreamManager>,
) {
    loop {
        let range = stream_manager.get_range(EventType::Ido);
        let filter = Filter::new()
            .address(ido_address)
            .from_block(range.from_block)
            .to_block(range.to_block);

        match rpc.get_logs(&filter).await {
            Ok(logs) => {
                let events = parse_ido_logs(logs);
                sender.send(EventBatch { events, block: range.to_block }).await;
                stream_manager.advance(EventType::Ido, range.to_block);
            }
            Err(e) => {
                tracing::error!("IDO stream error: {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(BLOCK_INTERVAL)).await;
    }
}
```

**receive.rs:**
```rust
pub async fn receive_ido_events(
    db: Arc<PostgresDatabase>,
    mut receiver: mpsc::Receiver<EventBatch<IdoEvent>>,
    receive_manager: Arc<ReceiveManager>,
) {
    while let Some(batch) = receiver.recv().await {
        for event in batch.events {
            match event {
                IdoEvent::ProjectCreated(e) => handle_project_created(&db, e).await,
                IdoEvent::TokensPurchased(e) => handle_tokens_purchased(&db, e).await,
                IdoEvent::Graduated(e) => handle_graduated(&db, e).await,
                IdoEvent::MilestoneApproved(e) => handle_milestone_approved(&db, e).await,
                IdoEvent::ProjectFailed(e) => handle_project_failed(&db, e).await,
                IdoEvent::Refunded(e) => handle_refunded(&db, e).await,
            }
        }
        receive_manager.mark_completed(EventType::Ido, batch.block);
    }
}
```

Test: Parse sample log data, verify DB inserts.

---

### Task 4.3: Observer — Token (Transfer) Event Handler

**Files:**
- Create: `crates/observer/src/event/token/mod.rs`
- Create: `crates/observer/src/event/token/stream.rs`
- Create: `crates/observer/src/event/token/receive.rs`
- Test: `crates/observer/tests/token_event_test.rs`

**Events indexed:**
- ERC20 `Transfer(from, to, amount)` → UPDATE balances for both accounts, track holder count

Depends on: IDO handler (project must exist for token to be tracked).

---

### Task 4.4: Observer — Swap Event Handler (V4 Pool)

**Files:**
- Create: `crates/observer/src/event/swap/mod.rs`
- Create: `crates/observer/src/event/swap/stream.rs`
- Create: `crates/observer/src/event/swap/receive.rs`
- Test: `crates/observer/tests/swap_event_test.rs`

**Events indexed:**
- V4 Pool Swap events → INSERT into swaps, UPSERT charts (all intervals), UPDATE market_data

Chart OHLCV calculation: group swaps by interval, maintain open/high/low/close/volume.

Depends on: IDO handler (project must be graduated).

---

### Task 4.5: Observer — LP Event Handler

**Files:**
- Create: `crates/observer/src/event/lp/mod.rs`
- Create: `crates/observer/src/event/lp/stream.rs`
- Create: `crates/observer/src/event/lp/receive.rs`
- Test: `crates/observer/tests/lp_event_test.rs`

**Events indexed:**
- `LiquidityAllocated` → INSERT into liquidity_positions
- `FeesCollected` → INSERT into fee_collections

---

### Task 4.6: Observer — Price Handler

**Files:**
- Create: `crates/observer/src/event/price/mod.rs`
- Create: `crates/observer/src/event/price/stream.rs`
- Create: `crates/observer/src/event/price/receive.rs`

Calculate token prices from swap events. Update `market_data.token_price`, `ath_price`.
Track AVAX/USD price from external oracle or Chainlink.

---

### Task 4.7: Observer — Integration Test (Full Pipeline)

**Files:**
- Create: `crates/observer/tests/integration_test.rs`

Test full pipeline: mock RPC logs → stream → receive → verify DB state.

---

## Phase 5: API Server

### Task 5.1: API Server Scaffolding

**Files:**
- Modify: `crates/api-server/src/main.rs`
- Create: `crates/api-server/src/state.rs`
- Create: `crates/api-server/src/cors.rs`
- Create: `crates/api-server/src/router/mod.rs`
- Create: `crates/api-server/src/middleware/mod.rs`
- Create: `crates/api-server/src/services/mod.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/main.rs`

```rust
// main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter("info").json().init();

    let db = Arc::new(PostgresDatabase::new().await?);
    let redis = Arc::new(RedisDatabase::new().await?);
    let cache = Arc::new(MokaCache::new());

    let state = AppState { db, redis, cache };
    let router = build_router(state);

    let addr = format!("{}:{}", *API_IP, *API_PORT);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {addr}");
    axum::serve(listener, router).await?;
    Ok(())
}
```

---

### Task 5.2: Auth Middleware & Routes

**Files:**
- Create: `crates/api-server/src/middleware/auth.rs`
- Create: `crates/api-server/src/router/auth/mod.rs`
- Create: `crates/api-server/src/services/auth.rs`
- Test: `crates/api-server/tests/auth_test.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/services/auth/`

Endpoints:
- `POST /auth/nonce` — Generate EIP-4361 nonce
- `POST /auth/session` — Verify signature, create session cookie
- `DELETE /auth/delete_session` — Remove session

Middleware: Extract session cookie → Redis lookup → fallback PostgreSQL → Extension<SessionInfo>.

Test: Full auth flow (nonce → sign → session → authenticated request).

---

### Task 5.3: Project Routes

**Files:**
- Create: `crates/api-server/src/router/project/mod.rs`
- Create: `crates/api-server/src/services/project.rs`
- Test: `crates/api-server/tests/project_test.rs`

Endpoints:
- `GET /project/:projectId` — Project detail with milestones
- `GET /project/featured` — Featured projects (top funded)
- `GET /order/project/:sortType` — Project list with sorting/pagination
- `POST /project/create` — Create new project (auth required)
- `GET /project/validate-symbol` — Ticker availability check
- `GET /project/investor/:projectId` — Investor list

Test: Each endpoint with mock DB data.

---

### Task 5.4: Milestone Routes

**Files:**
- Create: `crates/api-server/src/router/milestone/mod.rs`
- Create: `crates/api-server/src/services/milestone.rs`
- Test: `crates/api-server/tests/milestone_test.rs`

Endpoints:
- `POST /milestone/submit/:milestoneId` — Submit evidence (auth, creator only)
- `GET /milestone/verification/:milestoneId` — Verification status

---

### Task 5.5: Token & Trade Routes

**Files:**
- Create: `crates/api-server/src/router/token/mod.rs`
- Create: `crates/api-server/src/router/trade/mod.rs`
- Create: `crates/api-server/src/services/token.rs`
- Create: `crates/api-server/src/services/trade.rs`
- Test: `crates/api-server/tests/token_test.rs`
- Test: `crates/api-server/tests/trade_test.rs`

Token endpoints:
- `GET /token/:tokenId`
- `GET /trend`
- `GET /order/:sortType`

Trade endpoints:
- `GET /trade/chart/:tokenAddress`
- `GET /trade/swap-history/:tokenId`
- `GET /trade/holder/:tokenId`
- `GET /trade/market/:tokenId`
- `GET /trade/metrics/:tokenId`
- `GET /trade/quote/:tokenId`

---

### Task 5.6: Profile & Portfolio Routes

**Files:**
- Create: `crates/api-server/src/router/profile/mod.rs`
- Create: `crates/api-server/src/services/profile.rs`
- Test: `crates/api-server/tests/profile_test.rs`

Endpoints:
- `GET /profile/:address`
- `GET /account/get_account` (auth)
- `GET /profile/hold-token/:accountId`
- `GET /profile/swap-history/:accountId`
- `GET /profile/ido-history/:accountId`
- `GET /profile/refund-history/:accountId`
- `GET /profile/portfolio/:accountId`

---

### Task 5.7: Builder Routes

**Files:**
- Create: `crates/api-server/src/router/builder/mod.rs`
- Create: `crates/api-server/src/services/builder.rs`
- Test: `crates/api-server/tests/builder_test.rs`

Endpoints:
- `GET /profile/tokens/created/:accountId`
- `GET /builder/overview/:projectId` (auth, creator only)
- `GET /builder/stats/:projectId` (auth, creator only)

---

### Task 5.8: Metadata Upload Routes

**Files:**
- Create: `crates/api-server/src/router/metadata/mod.rs`
- Create: `crates/api-server/src/services/upload.rs`
- Test: `crates/api-server/tests/upload_test.rs`

Endpoints:
- `POST /metadata/image` — Image upload (5MB, PNG/JPG)
- `POST /metadata/evidence` — Evidence file (10MB, PDF/ZIP)

Uses S3/R2 client for storage.

---

### Task 5.9: Rate Limiting Middleware

**Files:**
- Create: `crates/api-server/src/middleware/rate_limit.rs`
- Test: `crates/api-server/tests/rate_limit_test.rs`

**Reference:** `/Users/gyu/project/nadfun/api-server/src/services/rate_limiter.rs`

Redis-based per-IP rate limiting: 60 req/min default.
Returns 429 with Retry-After header.

---

### Task 5.10: API Server Integration Test

**Files:**
- Create: `crates/api-server/tests/integration_test.rs`

Full E2E: Start server → create project → buy → check portfolio → verify responses.

---

## Phase 6: WebSocket Server

### Task 6.1: WebSocket Server Scaffolding

**Files:**
- Modify: `crates/websocket-server/src/main.rs`
- Create: `crates/websocket-server/src/config.rs`
- Create: `crates/websocket-server/src/server/mod.rs`
- Create: `crates/websocket-server/src/server/socket/mod.rs`
- Create: `crates/websocket-server/src/server/socket/connection.rs`
- Create: `crates/websocket-server/src/server/socket/rpc.rs`

**Reference:** `/Users/gyu/project/nadfun/websocket-server/src/`

main.rs:
1. Init RpcClient
2. Init CacheManager (Redis + PostgreSQL)
3. Init Event Producers (Trade, Price, Project, Milestone, NewContent)
4. Start IDO stream + Pool stream
5. Start HTTP/WS server on port 8001

---

### Task 6.2: Event Producer Framework

**Files:**
- Create: `crates/websocket-server/src/event/mod.rs`
- Create: `crates/websocket-server/src/event/core.rs`
- Test: `crates/websocket-server/tests/event_producer_test.rs`

```rust
pub struct EventProducer<K: Hash + Eq, V: Clone> {
    channels: DashMap<K, broadcast::Sender<V>>,
    channel_capacity: usize,
}

impl<K, V> EventProducer<K, V> {
    pub fn publish(&self, key: &K, value: V);
    pub fn subscribe(&self, key: &K) -> broadcast::Receiver<V>;
    pub fn cleanup_idle(&self, max_idle: Duration);
}
```

Test: publish/subscribe, multiple subscribers, idle cleanup.

---

### Task 6.3: Stream Handlers (IDO + Pool)

**Files:**
- Create: `crates/websocket-server/src/stream/mod.rs`
- Create: `crates/websocket-server/src/stream/ido/stream.rs`
- Create: `crates/websocket-server/src/stream/ido/receive.rs`
- Create: `crates/websocket-server/src/stream/pool/stream.rs`
- Create: `crates/websocket-server/src/stream/pool/receive.rs`

Same pattern as observer, but instead of DB writes → publish to Event Producers.

---

### Task 6.4: Trade & Price Event Producers

**Files:**
- Create: `crates/websocket-server/src/event/trade.rs`
- Create: `crates/websocket-server/src/event/price.rs`
- Test: `crates/websocket-server/tests/trade_event_test.rs`

TradeEventProducer: Keyed by token_id, broadcasts `ISwapInfo` on each swap.
PriceEventProducer: Keyed by token_id, broadcasts price updates derived from swaps.

---

### Task 6.5: Project & Milestone Event Producers

**Files:**
- Create: `crates/websocket-server/src/event/project.rs`
- Create: `crates/websocket-server/src/event/milestone.rs`
- Test: `crates/websocket-server/tests/project_event_test.rs`

ProjectEventProducer: Keyed by project_id, broadcasts investment/graduation/failure events.
MilestoneEventProducer: Keyed by project_id, broadcasts milestone approval events.

---

### Task 6.6: NewContent Event Producer

**Files:**
- Create: `crates/websocket-server/src/event/new_content.rs`

Global broadcast (no key): all trades, new projects, graduations → ticker feed.

---

### Task 6.7: WebSocket Handler & JSON-RPC

**Files:**
- Modify: `crates/websocket-server/src/server/socket/mod.rs`
- Modify: `crates/websocket-server/src/server/socket/rpc.rs`
- Modify: `crates/websocket-server/src/server/socket/connection.rs`
- Test: `crates/websocket-server/tests/ws_handler_test.rs`

JSON-RPC dispatch:
- `trade_subscribe` → TradeEventProducer.subscribe(token_id)
- `price_subscribe` → PriceEventProducer.subscribe(token_id)
- `project_subscribe` → ProjectEventProducer.subscribe(project_id)
- `milestone_subscribe` → MilestoneEventProducer.subscribe(project_id)
- `new_content_subscribe` → NewContentEventProducer.subscribe()
- `ping` → `pong`

Test: Connect WS → subscribe → receive event → unsubscribe.

---

### Task 6.8: Cache Manager

**Files:**
- Create: `crates/websocket-server/src/cache/mod.rs`

L1 Redis + L2 PostgreSQL for account_info, token_info lookups.
Used by event producers to enrich raw events with metadata.

---

## Phase 7: TxBot

### Task 7.1: TxBot Scaffolding

**Files:**
- Modify: `crates/txbot/src/main.rs`
- Create: `crates/txbot/src/config.rs`
- Create: `crates/txbot/src/keystore.rs`
- Create: `crates/txbot/src/job/mod.rs`
- Create: `crates/txbot/src/job/handler.rs`

**Reference:** `/Users/gyu/project/nadfun/txbot/src/`

main.rs:
1. Init RpcClient
2. Init PostgresDatabase
3. Load wallets from env (keystore)
4. Spawn graduate job
5. Spawn collect job
6. Spawn health monitor
7. Spawn metrics server

keystore.rs:
- Load `GRADUATE_PRIVATE_KEY`, `COLLECTOR_PRIVATE_KEY`
- hex → SigningKey → EthereumWallet
- zeroize sensitive memory

---

### Task 7.2: Graduate Job

**Files:**
- Create: `crates/txbot/src/job/graduate/mod.rs`
- Create: `crates/txbot/src/job/graduate/stream.rs`
- Create: `crates/txbot/src/job/graduate/execute.rs`
- Test: `crates/txbot/tests/graduate_test.rs`

**stream.rs:**
- Watch blocks for `TokensPurchased` events
- For each: check if `idoSold >= idoSupply` (sold out) or `block.timestamp >= deadline`
- If either: send `GraduateTask { token }` to executor

**execute.rs:**
- Verify on-chain: `project.status == Active` and not already graduated
- Call `IDO.graduate(token)`
- Wait for receipt
- Log success/failure

**Retry:** 20 attempts, 500ms → 30s exponential backoff.

Test: Mock contract calls, verify graduation logic.

---

### Task 7.3: Collect Fees Job

**Files:**
- Create: `crates/txbot/src/job/collect/mod.rs`
- Create: `crates/txbot/src/job/collect/stream.rs`
- Create: `crates/txbot/src/job/collect/execute.rs`
- Test: `crates/txbot/tests/collect_test.rs`

**stream.rs:**
- Poll DB every 30s for graduated projects
- For each: check V4 position accumulated fees on-chain
- If fees >= MIN_COLLECT_AMOUNT: send `CollectTask { token }`

**execute.rs:**
- Call `IDO.collectFees(token)`
- Wait for receipt
- Log success/failure

**Retry:** 5 attempts, 1s → 60s exponential backoff.

Test: Mock DB + contract calls, verify collection logic.

---

### Task 7.4: TxBot Metrics & Health

**Files:**
- Create: `crates/txbot/src/metrics/mod.rs`
- Create: `crates/txbot/src/metrics/wallet_metrics.rs`
- Create: `crates/txbot/src/metrics/job_metrics.rs`

Monitor:
- Wallet balances (AVAX for gas)
- Job success/failure counts
- RPC provider health
- Prometheus `/metrics` endpoint on port 9091

---

## Phase 8: Documentation & Cleanup

### Task 8.1: Update PRODUCT.md

Add backend architecture section.

### Task 8.2: Create README.md

Setup instructions, env vars, how to run each service.

### Task 8.3: Create TEST.md

Testing strategy, how to run tests, coverage targets.

### Task 8.4: Create .env.example

All required environment variables with descriptions.

---

## Task Dependency Graph

```
Phase 1 (workspace + types)
    ├── Phase 2 (DB layer)
    │     ├── Phase 4 (observer) ← Phase 3 (RPC)
    │     ├── Phase 5 (api-server)
    │     └── Phase 7 (txbot) ← Phase 3 (RPC)
    └── Phase 3 (RPC client)
          └── Phase 6 (websocket-server)

Phase 8 (docs) — after all others
```

## Parallelization Opportunities

These phases can run in parallel once their dependencies are met:

- **After Phase 2+3 complete:** Observer, API Server, TxBot can be built in parallel
- **After Phase 3 complete:** WebSocket Server can start
- **Within each phase:** Tests can run in parallel

Recommended parallel agent assignment:
1. Agent A: Observer (Phase 4)
2. Agent B: API Server (Phase 5)
3. Agent C: WebSocket Server (Phase 6)
4. Agent D: TxBot (Phase 7)
