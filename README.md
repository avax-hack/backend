# OpenLaunch Backend

> Milestone-based Decentralized Launchpad on Avalanche C-Chain

## Overview

OpenLaunch is a decentralized launchpad that enables milestone-based fundraising through IDOs (Initial DEX Offerings) on the Avalanche C-Chain. The backend is a Rust workspace comprising four independent server binaries and a shared library. It handles on-chain event indexing, REST/WebSocket APIs, and automated transaction execution for token graduation and fee collection.

## Architecture

```
                                  +-----------------------+
                                  |   Avalanche C-Chain   |
                                  |    (RPC Providers)    |
                                  +----+-------+-----+---+
                                       |       |     |
                          +------------+   +---+     +----------+
                          |                |                    |
                +---------v---+   +--------v------+   +---------v---+
                |  observer   |   |  websocket-   |   |   txbot     |
                |             |   |  server :8001 |   |             |
                | Index chain |   | Stream events |   | Graduate &  |
                | events into |   | to clients    |   | collect fee |
                | PostgreSQL  |   | via WS        |   | txs         |
                +------+------+   +-------+-------+   +------+------+
                       |                  |                   |
          +------------+------------------+-------------------+
          |                                                   |
  +-------v--------+                              +-----------v---+
  |   PostgreSQL   |                              |     Redis     |
  | (primary +     |                              | sessions,     |
  |  replica)      |                              | cache, rate   |
  +-------+--------+                              |  limiting     |
          |                                       +---------------+
  +-------v-----------+
  |  api-server :8000 |
  |  REST API +       |
  |  Swagger UI       |
  +-------------------+
          |
    [ Frontend ]
```

## Project Structure

```
backend/
├── Cargo.toml                # Workspace root (resolver v2)
├── crates/
│   ├── shared/               # Common library shared by all binaries
│   ├── api-server/           # REST API server (port 8000)
│   ├── websocket-server/     # WebSocket server (port 8001)
│   ├── observer/             # On-chain event indexer
│   └── txbot/                # Automated transaction bot
├── migrations/               # SQLx PostgreSQL migrations
├── abi/                      # Smart contract ABI JSON files
└── docs/
    ├── api.md                # API reference
    └── specs/                # Feature specifications
        ├── api-server.md
        ├── observer.md
        ├── shared.md
        ├── txbot.md
        └── websocket-server.md
```

## Crates

### shared (`openlaunch-shared`)

The core library depended on by all four binaries. Contains:

- **types** -- Domain models (`account`, `auth`, `project`, `milestone`, `token`, `trading`, `profile`, `event`, `common`)
- **db/postgres** -- PostgreSQL connection pool (primary + replica split), controllers for every table (`account`, `project`, `milestone`, `investment`, `refund`, `swap`, `balance`, `chart`, `market`, `block`)
- **db/redis** -- Session management (`session:{id}`), response caching (`cache:{key}`), rate limiting (`rate:{id}:{window}`)
- **client** -- Multi-provider RPC client with automatic health checking and fallback (`Main`, `Sub1`, `Sub2`)
- **contracts** -- Type-safe bindings for `IDO`, `LpManager`, and `ProjectToken` contracts (via `alloy`)
- **utils** -- Address utilities, price helpers, `SingleFlight` cache (coalesces concurrent identical requests)
- **config** -- Centralized environment variable loading (see [Environment Variables](#environment-variables))
- **metrics** -- Shared metrics primitives
- **error** -- Unified error types with `thiserror`

### api-server (`openlaunch-api-server`)

REST API serving the frontend application.

- **Default address**: `127.0.0.1:8000` (`API_IP`, `API_PORT`)
- **Swagger UI**: `http://localhost:8000/swagger-ui` (via `utoipa` + `utoipa-swagger-ui`)
- **Route groups**: `/auth`, `/project`, `/milestone`, `/token`, `/order`, `/trend`, `/trade`, `/profile`, `/account`, `/builder`, `/metadata`, `/health`
- **Middleware stack**: CORS layer, Redis-based rate limiting (60s sliding window), cookie-based session authentication
- **Services**: `auth`, `project`, `milestone`, `token`, `trade`, `profile`, `upload`, `builder`

### websocket-server (`openlaunch-websocket-server`)

Real-time event streaming to connected clients.

- **Default address**: `127.0.0.1:8001` (`WS_IP`, `WS_PORT`)
- **Protocol**: JSON-RPC 2.0 over WebSocket
- **Subscription channels**:
  - `trade` -- Swap/trade execution events
  - `price` -- Token price updates
  - `project` -- IDO project state changes
  - `milestone` -- Milestone approval/rejection events
  - `new_content` -- New project listings and content updates
- **Event streams**: Subscribes to on-chain IDO and Pool contract events via RPC, transforms them into `WsEvent` payloads, and broadcasts through `DashMap<String, broadcast::Sender>` channels
- **Graceful shutdown**: Listens for `SIGINT` (Ctrl+C)

### observer (`openlaunch-observer`)

Blockchain event indexer that polls on-chain logs and persists them to PostgreSQL.

- **Event types**: `Ido`, `Token`, `Swap`, `Lp`, `Price` (with dependency ordering: Token/Swap/Lp depend on Ido, Price depends on Swap)
- **Block tracking**: `StreamManager` tracks the latest polled block per event type; `ReceiveManager` tracks the latest processed block. Progress is persisted to the `block_progress` table every 10 seconds.
- **Architecture**: Each event type has a stream (polls logs, sends batches over mpsc channel) and a receive (consumes batches, writes to DB). Retry with configurable `RetryConfig`.
- **Health check**: Periodic RPC health check (every 30s) across all providers

### txbot (`openlaunch-txbot`)

Automated transaction execution bot.

- **Job types**:
  - `Graduate` -- Monitors IDO contracts for graduation eligibility and sends graduation transactions. Polls every 5s by default.
  - `CollectFees` -- Collects accumulated protocol fees from LP positions. Polls every 30s by default. Minimum threshold: 1 USDC.
- **Wallet management**: Loads private keys from environment (`GRADUATE_PRIVATE_KEY`, `COLLECTOR_PRIVATE_KEY`). Jobs are gracefully disabled when keys are not configured.
- **Metrics**: Built-in metrics tracking via `TxBotMetrics` with periodic reporting
- **Retry**: Configurable max retry attempts per job type

## Tech Stack

| Category | Technology | Version |
|---|---|---|
| Language | Rust | Edition 2024 |
| Async Runtime | Tokio | 1.40 |
| HTTP Framework | Axum | 0.8 |
| Blockchain | Alloy | 1.0 |
| PostgreSQL Driver | SQLx | 0.8 |
| Redis Driver | redis-rs | 0.29 |
| Serialization | Serde / serde_json | 1.0 |
| In-memory Cache | Moka | 0.12 |
| Concurrent Map | DashMap | 6.1 |
| API Docs | utoipa + utoipa-swagger-ui | 5 / 9 |
| HTTP Middleware | tower / tower-http | 0.5 / 0.6 |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 |
| Error Handling | anyhow + thiserror | 1.0 / 2.0 |

## Environment Variables

### Database

| Variable | Required | Default | Description |
|---|---|---|---|
| `PRIMARY_DATABASE_URL` | Yes* | -- | PostgreSQL primary connection URL |
| `DATABASE_URL` | Yes* | -- | Fallback if `PRIMARY_DATABASE_URL` is not set |
| `REPLICA_DATABASE_URL` | No | Same as primary | PostgreSQL read-replica connection URL |
| `PG_PRIMARY_MAX_CONNECTIONS` | No | `50` | Primary pool max connections |
| `PG_PRIMARY_MIN_CONNECTIONS` | No | `5` | Primary pool min connections |
| `PG_REPLICA_MAX_CONNECTIONS` | No | `200` | Replica pool max connections |
| `PG_REPLICA_MIN_CONNECTIONS` | No | `10` | Replica pool min connections |

### Redis

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_URL` | Yes | -- | Redis connection URL |

### RPC

| Variable | Required | Default | Description |
|---|---|---|---|
| `MAIN_RPC_URL` | Yes | -- | Primary Avalanche C-Chain RPC endpoint |
| `SUB_RPC_URL_1` | No | `""` | Fallback RPC endpoint 1 |
| `SUB_RPC_URL_2` | No | `""` | Fallback RPC endpoint 2 |
| `RPC_TIMEOUT_MS` | No | `30000` | RPC request timeout (milliseconds) |
| `CHAIN_ID` | No | `43114` | EVM chain ID (43114 = Avalanche mainnet) |

### Contracts

| Variable | Required | Default | Description |
|---|---|---|---|
| `IDO_CONTRACT` | Yes | -- | IDO contract address |
| `LP_MANAGER_CONTRACT` | Yes | -- | LP Manager contract address |
| `USDC_ADDRESS` | Yes | -- | USDC token address |

### API Server

| Variable | Required | Default | Description |
|---|---|---|---|
| `API_IP` | No | `127.0.0.1` | API server bind address |
| `API_PORT` | No | `8000` | API server port |

### WebSocket Server

| Variable | Required | Default | Description |
|---|---|---|---|
| `WS_IP` | No | `127.0.0.1` | WebSocket server bind address |
| `WS_PORT` | No | `8001` | WebSocket server port |
| `WS_CHANNEL_SIZE` | No | `1024` | Broadcast channel buffer size |
| `WS_CLEANUP_INTERVAL_SECS` | No | `300` | Inactive connection cleanup interval (seconds) |

### Observer

| Variable | Required | Default | Description |
|---|---|---|---|
| `OBSERVER_POLL_INTERVAL_MS` | No | `2000` | Block polling interval (milliseconds) |
| `OBSERVER_BATCH_SIZE` | No | `100` | Max blocks per poll batch |
| `OBSERVER_START_BLOCK` | No | `0` | Starting block (used when no progress exists in DB) |

### TxBot

| Variable | Required | Default | Description |
|---|---|---|---|
| `GRADUATE_PRIVATE_KEY` | No | -- | Private key for graduation transactions (job disabled if unset) |
| `COLLECTOR_PRIVATE_KEY` | No | -- | Private key for fee collection transactions (job disabled if unset) |
| `GRADUATE_POLL_MS` | No | `5000` | Graduate eligibility polling interval (milliseconds) |
| `COLLECT_POLL_SECS` | No | `30` | Fee collection polling interval (seconds) |
| `GRADUATE_MAX_RETRIES` | No | `20` | Max retry attempts for graduate transactions |
| `COLLECT_MAX_RETRIES` | No | `5` | Max retry attempts for collect-fees transactions |

### Logging

| Variable | Required | Default | Description |
|---|---|---|---|
| `RUST_LOG` | No | `info` | tracing log level filter (e.g., `debug`, `openlaunch_observer=debug,info`) |

## Database

### PostgreSQL Tables

Migrations are in `migrations/`, applied via SQLx:

| Migration | Table | Description |
|---|---|---|
| `20260307000001_accounts.sql` | `accounts` | Wallet addresses and user accounts |
| `20260307000002_sessions.sql` | `sessions` | Authentication sessions (backed by Redis) |
| `20260307000003_projects.sql` | `projects` | IDO project definitions and state |
| `20260307000004_milestones.sql` | `milestones` | Project milestones with approval tracking |
| `20260307000005_investments.sql` | `investments` | User investment records per project |
| `20260307000006_refunds.sql` | `refunds` | Refund records for failed/cancelled projects |
| `20260307000007_swaps.sql` | `swaps` | Token swap transaction history |
| `20260307000008_balances.sql` | `balances` | Token balance snapshots per user |
| `20260307000009_charts.sql` | `charts` | OHLCV candlestick data for price charts |
| `20260307000010_market_data.sql` | `market_data` | Aggregated market statistics |
| `20260307000011_holders.sql` | `holders` | Token holder tracking |
| `20260307000012_liquidity_positions.sql` | `liquidity_positions` | LP position records |
| `20260307000013_fee_collections.sql` | `fee_collections` | Protocol fee collection history |
| `20260307000014_block_progress.sql` | `block_progress` | Observer block tracking per event type |
| `20260307000015_funding_snapshots.sql` | `funding_snapshots` | Periodic funding status snapshots |

### Redis Key Patterns

| Pattern | Purpose | TTL |
|---|---|---|
| `session:{session_id}` | User session data | Configurable |
| `nonce:{address}` | Auth nonce for wallet signature (GETDEL for replay protection) | Configurable |
| `cache:{key}` | General-purpose response cache | Per-key |
| `rate:{identifier}:{window}` | Rate limit counter (60s sliding window) | 120s |

## Getting Started

### Prerequisites

- **Rust** (edition 2024, stable toolchain)
- **PostgreSQL** 14+
- **Redis** 7+
- **SQLx CLI** (`cargo install sqlx-cli --features postgres`)

### Setup

```bash
# Clone the repository
git clone <repo-url>
cd openlaunch/backend

# Copy and configure environment variables
cp .env.example .env
# Edit .env with your database, Redis, RPC, and contract addresses

# Run database migrations
sqlx database create
sqlx migrate run

# Build all crates
cargo build --workspace
```

### Running

Start each service in a separate terminal (or use a process manager):

```bash
# API Server (port 8000)
cargo run -p openlaunch-api-server

# WebSocket Server (port 8001)
cargo run -p openlaunch-websocket-server

# Observer (blockchain indexer)
cargo run -p openlaunch-observer

# TxBot (automated transactions)
cargo run -p openlaunch-txbot
```

For production builds:

```bash
cargo build --workspace --release

# Binaries are in target/release/
./target/release/openlaunch-api-server
./target/release/openlaunch-websocket-server
./target/release/openlaunch-observer
./target/release/openlaunch-txbot
```

## API Documentation

- **Swagger UI**: [http://localhost:8000/swagger-ui](http://localhost:8000/swagger-ui)
- **OpenAPI Spec**: [http://localhost:8000/api-docs/openapi.json](http://localhost:8000/api-docs/openapi.json)
- **API Reference**: [docs/api.md](docs/api.md)
- **Spec Documents**: [docs/specs/](docs/specs/)

## Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p openlaunch-shared
cargo test -p openlaunch-observer
```

## Project Status

### Implemented (MVP)

- IDO project creation and lifecycle management
- Milestone-based funding with approval tracking
- On-chain event indexing (IDO, LP events with stream + receive pipeline)
- Real-time WebSocket streaming (trade, price, project, milestone, new_content)
- REST API with session auth, rate limiting, CORS, and Swagger UI
- Multi-provider RPC client with health checks and automatic fallback
- Automated graduation and fee collection (TxBot)
- OHLCV chart data and market statistics
- Token holder tracking and balance snapshots
- Primary/replica PostgreSQL connection pool split

### TODO / Placeholder

- Wallet signature verification (auth flow)
- AMM math calculations
- S3 file upload (upload service)
- `trading_pnl` computation
- Token event stream polling (channel created, receive handler exists, but stream task not yet spawned)
- Swap event stream polling (channel created, receive handler exists, but stream task not yet spawned)
- Price event stream polling (channel created, receive handler exists, but stream task not yet spawned)
