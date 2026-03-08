# TxBot Specification

## 1. Overview

TxBot (`openlaunch-txbot`) is the automated transaction executor for the OpenLaunch backend. It runs as a standalone Rust binary that monitors on-chain events and database state, then submits transactions to the Avalanche C-Chain on behalf of the platform.

**Primary responsibilities:**

- **Graduating IDO projects** -- detecting when an IDO has sold out or reached its deadline, then calling the `graduate()` function on the IDO contract to migrate liquidity.
- **Collecting LP fees** -- periodically scanning graduated projects for accumulated trading fees above a threshold, then calling `collectFees()` on the IDO contract.

TxBot operates independently of the API server. It connects to the same PostgreSQL database (read replica) and the same set of RPC endpoints, but uses its own dedicated wallet keys.

**Crate:** `openlaunch-txbot` (edition 2024)
**Entry point:** `crates/txbot/src/main.rs`

---

## 2. Architecture

### Module Organization

```
src/
  main.rs              -- Binary entry point; wires up DB, RPC, wallets, metrics, jobs
  config_local.rs      -- Environment-driven configuration (polling intervals, thresholds, retries)
  keystore.rs          -- Wallet loading and private key parsing
  metrics/
    mod.rs             -- Atomic counters and periodic metrics reporter
  job/
    mod.rs             -- Re-exports job sub-modules
    handler.rs         -- Generic retry-with-exponential-backoff executor
    graduate/
      mod.rs           -- GraduateTask definition; spawn() wiring
      stream.rs        -- Event polling loop (watches TokensPurchased logs)
      execute.rs       -- Transaction sender (calls IDO.graduate())
    collect/
      mod.rs           -- CollectTask definition; spawn() wiring
      stream.rs        -- DB polling loop (queries graduated projects, checks fees)
      execute.rs       -- Transaction sender (calls IDO.collectFees())
```

### Job Queue Model

Each job type follows a **stream/execute** split connected by a bounded `tokio::sync::mpsc::channel` (capacity 256):

```
[Stream task] --mpsc::channel(256)--> [Execute task]
```

- **Stream** -- A polling loop that detects work to be done and produces task messages.
- **Execute** -- A loop that receives task messages and submits on-chain transactions with retry logic.

This decouples detection from execution, allowing the stream to continue scanning while the executor handles retries and confirmations.

### Startup Sequence

1. Load environment variables via `dotenvy`.
2. Initialize structured JSON logging via `tracing_subscriber`.
3. Connect to PostgreSQL (primary + replica) via `PostgresDatabase`.
4. Initialize `RpcClient` with three provider endpoints (`Main`, `Sub1`, `Sub2`).
5. Load wallet signers from environment (graceful `None` if keys are missing).
6. Initialize `TxBotMetrics` and spawn the 60-second metrics reporter.
7. Conditionally spawn the graduate job (if `GRADUATE_PRIVATE_KEY` is set).
8. Conditionally spawn the collect job (if `COLLECTOR_PRIVATE_KEY` is set).
9. Block on `Ctrl+C` (`tokio::signal::ctrl_c`).

---

## 3. Job Types

### 3.1 Graduate Job

**Task struct:**

```rust
pub struct GraduateTask {
    pub token_address: String,
}
```

**Trigger conditions (stream):**
- Polls for `TokensPurchased` events on the IDO contract via log filtering.
- For each unique token address in the events, reads the on-chain project record (`ido.projects(token)`).
- Graduation is triggered when:
  - `project.status == Active` (status value `0`), AND
  - `project.idoSold >= project.idoSupply` (sold out), OR
  - `block.timestamp >= project.deadline` (deadline reached).

**Contract interaction (execute):**
- Calls `IIDO::graduate(token_address)` on the IDO contract.

**Expected outcome:**
- The IDO contract transitions the project from `Active` to `Graduated`.
- Liquidity is migrated from the IDO pool to a DEX LP position.

### 3.2 Collect Fees Job

**Task struct:**

```rust
pub struct CollectTask {
    pub token_address: String,
}
```

**Trigger conditions (stream):**
- Polls the PostgreSQL database for projects with status `"completed"` (maps to on-chain `Graduated`).
- For each graduated project, reads the on-chain project record and computes unreleased fees: `usdcRaised - usdcReleased`.
- Collection is triggered when `unreleased >= MIN_COLLECT_AMOUNT` (default 1 USDC = 1,000,000 units at 6 decimals).

**Contract interaction (execute):**
- Calls `IIDO::collectFees(token_address)` on the IDO contract.

**Expected outcome:**
- Accumulated LP trading fees are transferred from the contract to the platform treasury.

---

## 4. Graduate Flow

The full graduation process from detection to on-chain confirmation:

```
1. Stream polls RPC for new blocks
   |
2. Queries TokensPurchased event logs from (last_block+1) to current_block
   |
3. Extracts unique token addresses from events (HashSet deduplication)
   |
4. For each token, calls ido.projects(token) to read on-chain state
   |
5. Checks eligibility:
   - status must be Active (0)
   - idoSold >= idoSupply  OR  block.timestamp >= deadline
   |
6. Sends GraduateTask { token_address } through mpsc channel
   |
7. Executor receives task, re-verifies project is still Active
   |
8. Builds graduate(token) transaction via alloy contract bindings
   |
9. Submits TX with run_with_retry (up to GRADUATE_MAX_RETRIES attempts)
   |
10. Waits for receipt (pending_tx.get_receipt())
    |
11. Logs tx_hash, block_number, gas_used on success
    |
12. Records metrics (attempt/success/failure)
```

**Key detail:** The executor performs a second on-chain status check before submitting the transaction. If the project is no longer `Active` (e.g., another bot instance already graduated it), the task is silently skipped. If the status check RPC call fails, the executor proceeds with the graduation attempt anyway (fail-open).

---

## 5. Fee Collection

The fee collection process:

```
1. Stream sleeps for COLLECT_POLL_SECS (default 30s)
   |
2. Queries PostgreSQL for all projects with status = "completed"
   - Paginates through results (100 per page via project::find_list)
   - Collects all project_id values (token contract addresses)
   |
3. For each graduated token, calls ido.projects(token) on-chain
   |
4. Computes unreleased = usdcRaised - usdcReleased
   |
5. If unreleased >= MIN_COLLECT_AMOUNT, sends CollectTask through channel
   |
6. Executor receives task
   |
7. Builds collectFees(token) transaction via alloy contract bindings
   |
8. Submits TX with run_with_retry (up to COLLECT_MAX_RETRIES attempts)
   |
9. Waits for receipt, logs confirmation details
   |
10. Records metrics
```

**Database query details:**

```rust
async fn fetch_graduated_projects(db: &PostgresDatabase) -> anyhow::Result<Vec<String>>
```

Paginates through `project::find_list(db.reader(), "recent", &pagination, Some("completed"))` using pages of 100 rows until all graduated projects are collected.

---

## 6. Wallet Management

### Private Key Handling

Wallets are loaded at startup from environment variables. Two separate keys are used for role isolation:

| Environment Variable     | Role      | Purpose                         |
|--------------------------|-----------|----------------------------------|
| `GRADUATE_PRIVATE_KEY`   | Graduate  | Signs `graduate()` transactions  |
| `COLLECTOR_PRIVATE_KEY`  | Collector | Signs `collectFees()` transactions |

**Key parsing** (`keystore.rs`):

```rust
fn parse_private_key(hex_key: &str) -> anyhow::Result<PrivateKeySigner>
```

- Accepts hex-encoded private keys with or without `0x` prefix.
- Trims whitespace before parsing.
- Uses `alloy::signers::local::PrivateKeySigner`.

**Wallet struct:**

```rust
pub struct Wallets {
    pub graduate: Option<PrivateKeySigner>,
    pub collector: Option<PrivateKeySigner>,
}
```

Both fields are `Option` to allow partial operation. If a key is missing or invalid, the corresponding job is disabled at startup with a warning log. The bot continues running with whichever jobs have valid keys.

**Accessor methods:**

```rust
pub fn graduate_signer(&self) -> anyhow::Result<&PrivateKeySigner>
pub fn collector_signer(&self) -> anyhow::Result<&PrivateKeySigner>
```

Return `Err` with a descriptive message if the key was not configured.

### Nonce and Gas Management

TxBot delegates nonce management and gas estimation to alloy's `ProviderBuilder` with an `EthereumWallet`. The provider handles:

- Automatic nonce tracking per sender address.
- Gas price / priority fee estimation from the RPC node.
- Transaction signing via the configured `PrivateKeySigner`.

No manual nonce or gas override logic exists in the crate -- this is handled entirely by the alloy provider layer.

### RPC Provider Selection

```rust
fn get_rpc_url(rpc: &RpcClient) -> anyhow::Result<String>
```

Each stream and executor calls `rpc.best_provider()` at initialization to select the healthiest RPC endpoint from the pool of three (`Main`, `Sub1`, `Sub2`). The URL is resolved once and used for the lifetime of the task. Provider selection is handled by the shared `RpcClient` from `openlaunch-shared`.

---

## 7. Transaction Lifecycle

All transactions follow the same lifecycle, managed by the `run_with_retry` wrapper:

```
Build (alloy contract call builder)
  -> Sign (EthereumWallet via ProviderBuilder)
    -> Send (tx_builder.send().await)
      -> Confirm (pending_tx.get_receipt().await)
        -> Log (tx_hash, block_number, gas_used)
```

**Build:** Contract method calls are constructed via alloy's generated bindings (`IIDO::graduate(token)` or `IIDO::collectFees(token)`).

**Sign:** Handled automatically by the alloy provider configured with an `EthereumWallet`.

**Send:** `tx_builder.send().await` returns a `PendingTransactionBuilder`.

**Confirm:** `pending_tx.get_receipt().await` blocks until the transaction is mined and returns the receipt.

**Retry:** If any step fails (RPC error, revert, timeout), the entire build-sign-send-confirm cycle is retried after an exponential backoff delay.

---

## 8. Configuration

All configuration is loaded from environment variables with sensible defaults.

### Local Config (`config_local.rs`)

| Variable               | Type  | Default   | Description                                    |
|------------------------|-------|-----------|------------------------------------------------|
| `GRADUATE_POLL_MS`     | `u64` | `5000`    | Polling interval for graduate event checks (ms)|
| `COLLECT_POLL_SECS`    | `u64` | `30`      | Polling interval for fee collection checks (s) |
| `MIN_COLLECT_AMOUNT`   | `str` | `1000000` | Minimum fee to trigger collection (6 decimals) |
| `GRADUATE_MAX_RETRIES` | `u32` | `20`      | Max retry attempts for graduate transactions   |
| `COLLECT_MAX_RETRIES`  | `u32` | `5`       | Max retry attempts for collect transactions    |

### Wallet Config

| Variable               | Type     | Required | Description                    |
|------------------------|----------|----------|--------------------------------|
| `GRADUATE_PRIVATE_KEY` | `String` | No       | Hex private key for graduating |
| `COLLECTOR_PRIVATE_KEY`| `String` | No       | Hex private key for collecting |

### Shared Config (from `openlaunch_shared::config`)

| Variable               | Description                        |
|------------------------|------------------------------------|
| `PRIMARY_DATABASE_URL` | PostgreSQL primary connection URL  |
| `REPLICA_DATABASE_URL` | PostgreSQL replica connection URL  |
| `MAIN_RPC_URL`         | Primary Avalanche C-Chain RPC URL  |
| `SUB_RPC_URL_1`        | Secondary RPC URL                  |
| `SUB_RPC_URL_2`        | Tertiary RPC URL                   |
| `IDO_CONTRACT`         | Address of the IDO contract        |

### Retry Parameters

Graduate executor:
- Max attempts: `GRADUATE_MAX_RETRIES` (default 20)
- Initial backoff: 2,000 ms
- Max backoff: 30,000 ms
- Backoff factor: 2.0x

Collect executor:
- Max attempts: `COLLECT_MAX_RETRIES` (default 5)
- Initial backoff: 3,000 ms
- Max backoff: 30,000 ms
- Backoff factor: 2.0x

---

## 9. Error Handling

### Retry Logic (`handler.rs`)

```rust
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_factor: f64,
}

pub async fn run_with_retry<F, Fut, T>(
    config: &RetryConfig,
    task_name: &str,
    f: F,
) -> anyhow::Result<T>
```

Exponential backoff formula: `delay = initial_backoff_ms * backoff_factor ^ (attempt - 1)`, capped at `max_backoff_ms`.

Each attempt is logged at `info` level with `task_name`, attempt number, and max attempts. Failures are logged at `warn` level with the error. Final exhaustion is logged at `error` level by the caller.

### Stream Error Recovery

Both stream loops use `continue` on transient errors:
- Failed `get_block_number()` calls: logged at `warn`, loop continues.
- Failed event queries: logged at `warn`, `last_block` is NOT updated (events will be re-scanned on the next iteration).
- Failed project status reads: logged at `warn`, individual token is skipped.
- Failed DB queries (collect stream): logged at `warn`, iteration skipped.

### Executor Error Recovery

- Invalid token address parsing: logged at `error`, task skipped, failure metric recorded.
- Project no longer Active (graduate): task silently skipped (not counted as failure).
- Project status check RPC failure: logged at `warn`, graduation attempted anyway.
- All retries exhausted: logged at `error`, failure metric recorded.

### Channel Closure

If the mpsc channel is closed (receiver dropped), the stream logs an error and returns `Ok(())` to exit gracefully. If the sender side is dropped, the executor exits its `while let Some(task) = rx.recv().await` loop and shuts down.

---

## 10. Metrics

### TxBotMetrics Struct

```rust
pub struct TxBotMetrics {
    pub graduate_attempts: AtomicU64,
    pub graduate_successes: AtomicU64,
    pub graduate_failures: AtomicU64,
    pub collect_attempts: AtomicU64,
    pub collect_successes: AtomicU64,
    pub collect_failures: AtomicU64,
}
```

All counters use `AtomicU64` with `Ordering::Relaxed` for lock-free concurrent access from multiple executor tasks.

### Recording Methods

| Method                      | Called When                                    |
|-----------------------------|------------------------------------------------|
| `record_graduate_attempt()` | Graduate task received by executor              |
| `record_graduate_success()` | Graduate transaction confirmed on-chain         |
| `record_graduate_failure()` | Graduate task fails (bad address or all retries)|
| `record_collect_attempt()`  | Collect task received by executor               |
| `record_collect_success()`  | CollectFees transaction confirmed on-chain      |
| `record_collect_failure()`  | Collect task fails (bad address or all retries) |

### Metrics Reporter

```rust
pub fn spawn_reporter(metrics: Arc<TxBotMetrics>) -> tokio::task::JoinHandle<()>
```

Spawns a background task that logs a `MetricsSnapshot` every 60 seconds via `tracing::info!` in structured JSON format. The snapshot includes all six counters as cumulative totals.

**Sample log output fields:**
- `graduate_attempts`, `graduate_successes`, `graduate_failures`
- `collect_attempts`, `collect_successes`, `collect_failures`
