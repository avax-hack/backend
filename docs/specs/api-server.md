# API Server Specification

## 1. Overview

The `api-server` crate is the REST API server for the OpenLaunch backend -- an IDO (Initial DEX Offering) platform built on the Avalanche C-Chain. It provides HTTP endpoints consumed by the frontend for authentication, project management, token trading, milestone tracking, and user profiles.

### Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (async, tokio runtime) |
| HTTP framework | Axum 0.8 |
| Middleware | tower / tower-http (CORS, rate limiting, auth) |
| Database | PostgreSQL (primary + read replica via `sqlx`) |
| Cache | Redis (sessions, rate limiting, nonce storage) |
| In-memory cache | `SingleFlightCache` (request deduplication, 1s TTL, 20k entries) |
| Logging | `tracing` + `tracing-subscriber` (JSON format, env filter) |
| Config | Environment variables via `dotenvy` |

### Entry Point

`main.rs` initializes logging, connects to PostgreSQL (primary + replica) and Redis, constructs `AppState`, builds the router, and binds a TCP listener.

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()>
```

---

## 2. Architecture

### Module Organization

```
src/
  main.rs              -- Entry point, server bootstrap
  state.rs             -- AppState definition
  cors.rs              -- CORS layer configuration
  middleware/
    mod.rs             -- Module declarations (auth, rate_limit)
    auth.rs            -- Session middleware + AuthUser extractor
    rate_limit.rs      -- Redis-based per-IP rate limiting
  router/
    mod.rs             -- build_router(), route nesting
    auth/mod.rs        -- /auth endpoints
    project/mod.rs     -- /project endpoints
    milestone/mod.rs   -- /milestone endpoints
    token/mod.rs       -- /token, /order, /trend endpoints
    trade/mod.rs       -- /trade endpoints
    profile/mod.rs     -- /profile, /account endpoints
    builder/mod.rs     -- /builder endpoints
    metadata/mod.rs    -- /metadata endpoints
    health/mod.rs      -- /health endpoint
  services/
    mod.rs             -- Module declarations
    auth.rs            -- Nonce generation, session verification
    project.rs         -- Project CRUD, validation, investors
    milestone.rs       -- Evidence submission, verification status
    token.rs           -- Token data, market info, trending
    trade.rs           -- Charts, swap history, holders, quotes
    profile.rs         -- User profile, portfolio, trade history
    builder.rs         -- Builder dashboard overview/stats
    upload.rs          -- Image/evidence file upload (placeholder)
```

### AppState

Shared application state cloned into every request handler via Axum's `State` extractor.

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PostgresDatabase>,      // PostgreSQL (primary + replica)
    pub redis: Arc<RedisDatabase>,       // Redis connection
    pub r2: Arc<R2Client>,              // Cloudflare R2 storage
    pub cache: Arc<SingleFlightCache>,   // In-memory dedup cache (20k entries, 1s TTL)
}
```

### Middleware Stack

Applied in this order (outermost listed first):

1. **CORS** (`tower_http::cors::CorsLayer`) -- outermost
2. **Rate Limiting** (`rate_limit_middleware`) -- Redis-based, per-IP
3. **Session Injection** (`session_middleware`) -- extracts session cookie, injects `SessionInfo` into request extensions

### Router Structure

```rust
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/project", project::router())
        .nest("/milestone", milestone::router())
        .nest("/token", token::router())
        .nest("/order", token::order_router())
        .nest("/trend", token::trend_router())
        .nest("/trade", trade::router())
        .nest("/profile", profile::router())
        .nest("/account", profile::account_router())
        .nest("/builder", builder::router())
        .nest("/metadata", metadata::router())
        .merge(health::router())
        .layer(session_middleware)
        .layer(rate_limit_middleware)
        .layer(cors_layer())
        .with_state(state)
}
```

---

## 3. Endpoints

### Legend

- **Auth**: `None` = public, `Session` = requires valid session cookie (uses `AuthUser` extractor, returns 401 if absent)
- **Cache**: endpoints using `SingleFlightCache` are marked with TTL

---

### 3.1 Health

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/health` | None | Health check |

**Response:** `200 OK` -- plain text `"OK"`

---

### 3.2 Auth (`/auth`)

#### `POST /auth/nonce`

Request a SIWE (Sign-In with Ethereum) nonce message for wallet authentication.

**Auth:** None

**Request Body:**
```json
{
  "address": "0x..."  // 42-char hex Ethereum address
}
```

**Validation:** Address must be 0x-prefixed, 42 chars, valid hex.

**Response (200):**
```json
{
  "nonce": "openlaunch.io wants you to sign in with your wallet.\n\nAddress: 0x...\nNonce: ...\nIssued At: ..."
}
```

**Nonce TTL:** 5 minutes (stored in Redis).

---

#### `POST /auth/session`

Create a session by verifying a signed nonce message.

**Auth:** None

**Request Body:**
```json
{
  "nonce": "<full nonce message string>",
  "signature": "0x<130 hex chars>",  // 65-byte secp256k1 signature (132 chars total)
  "chain_id": 43114                   // Avalanche C-Chain
}
```

**Validation:**
- `signature`: must be `0x`-prefixed, exactly 132 chars, valid hex
- `nonce`: 1-256 characters, non-empty
- Nonce must match stored value (atomically consumed for replay protection)

**Response (200):**

Sets `Set-Cookie` header:
```
session=<uuid>; HttpOnly; Secure; Path=/; Max-Age=604800; SameSite=Lax
```

Body:
```json
{
  "account_info": {
    "account_id": "0x...",
    "nickname": "",
    "bio": "",
    "image_uri": ""
  }
}
```

**Session TTL:** 7 days.

**NOTE:** Full secp256k1 signature recovery is TODO; currently validates format only.

---

#### `DELETE /auth/delete_session`

Log out the current user by deleting the session.

**Auth:** Session (required)

**Response (200):**

Sets `Set-Cookie` with `Max-Age=0` to expire the cookie.

```json
{ "success": true }
```

---

### 3.3 Project (`/project`)

#### `GET /project/featured`

Get featured projects (top funded projects currently in "funding" status).

**Auth:** None
**Cache:** `"project:featured"` (1s TTL)

**Response (200):**
```json
[
  {
    "project_info": { <IProjectInfo> },
    "market_info": { <IProjectMarketInfo> },
    "milestone_completed": 2,
    "milestone_total": 5
  }
]
```

---

#### `GET /project/:projectId`

Get full project details including milestones.

**Auth:** None
**Cache:** `"project:<projectId>"` (1s TTL)

**Path Params:** `projectId` (string)

**Response (200):**
```json
{
  "project_info": {
    "project_id": "0x...",
    "name": "string",
    "symbol": "string",
    "image_uri": "string",
    "description": "string | null",
    "tagline": "string",
    "category": "string",
    "creator": { <IAccountInfo> },
    "website": "string | null",
    "twitter": "string | null",
    "github": "string | null",
    "telegram": "string | null",
    "created_at": 1700000000
  },
  "market_info": {
    "project_id": "0x...",
    "status": "funding | active | completed | failed",
    "target_raise": "1000000",
    "total_committed": "500000",
    "funded_percent": 50.0,
    "investor_count": 42
  },
  "milestones": [ { <IMilestoneInfo> } ]
}
```

---

#### `POST /project/create`

Create a new project.

**Auth:** Session (required)

**Request Body:**
```json
{
  "name": "string (2-50 chars)",
  "symbol": "string (2-10 chars, uppercase + digits only)",
  "tagline": "string (5-120 chars)",
  "description": "string (min 20 chars)",
  "image_uri": "string (required)",
  "website": "string | null",
  "twitter": "string | null",
  "github": "string | null",
  "target_raise": "string (positive number)",
  "token_supply": "string (positive number)",
  "milestones": [
    {
      "order": 1,
      "title": "string (required)",
      "description": "string (required)",
      "fund_allocation_percent": 50  // 1-100, all must sum to 100
    }
  ]  // 2-6 milestones required
}
```

**Response (200):**
```json
{ "project_id": "0x<uuid>" }
```

---

#### `GET /project/validate-symbol`

Check if a token symbol is available.

**Auth:** None

**Query Params:** `symbol` (string, required)

**Response (200):**
```json
{ "available": true }
```

---

#### `GET /project/investor/:projectId`

Get paginated list of investors for a project.

**Auth:** None

**Path Params:** `projectId` (string)
**Query Params:** `page` (default 1), `limit` (default 20, max 100)

**Response (200):**
```json
{
  "data": [
    {
      "account_info": { <IAccountInfo> },
      "usdc_amount": "1000",
      "created_at": 1700000000
    }
  ],
  "total_count": 42
}
```

---

### 3.4 Milestone (`/milestone`)

#### `POST /milestone/submit/:milestoneId`

Submit evidence for a milestone. Milestone must be in "pending" status.

**Auth:** Session (required)

**Path Params:** `milestoneId` (string, format `ms_<number>`)

**Request Body:**
```json
{
  "evidence_text": "string (required, non-empty)",
  "evidence_uri": "string | null"
}
```

**Response (200):**
```json
{ "success": true }
```

**NOTE:** Creator ownership verification is TODO.

---

#### `GET /milestone/verification/:milestoneId`

Get verification status for a milestone.

**Auth:** None

**Path Params:** `milestoneId` (string, format `ms_<number>`)

**Response (200):**
```json
{
  "milestone_id": "ms_001",
  "status": "pending | submitted | in_verification | completed | failed",
  "submitted_at": 1700000000,
  "estimated_completion": 1700604800,
  "dispute_info": null
}
```

---

### 3.5 Token (`/token`, `/order`, `/trend`)

#### `GET /token/:tokenId`

Get token data (token info + market info).

**Auth:** None
**Cache:** `"token:<tokenId>"` (1s TTL)

**Path Params:** `tokenId` (string)

**Response (200):**
```json
{
  "token_info": {
    "token_id": "string",
    "name": "string",
    "symbol": "string",
    "image_uri": "string",
    "banner_uri": "string | null",
    "description": "string | null",
    "category": "string",
    "is_graduated": false,
    "creator": { <IAccountInfo> },
    "website": "string | null",
    "twitter": "string | null",
    "telegram": "string | null",
    "created_at": 1700000000,
    "project_id": "string | null"
  },
  "market_info": {
    "market_type": "IDO | CURVE | DEX",
    "token_id": "string",
    "token_price": "string",
    "native_price": "string",
    "price": "string",
    "ath_price": "string",
    "total_supply": "string",
    "volume": "string",
    "holder_count": 0,
    "bonding_percent": 0.0,
    "milestone_completed": 0,
    "milestone_total": 0
  }
}
```

---

#### `GET /order/:sortType`

Get paginated token list sorted by `sortType`.

**Auth:** None
**Cache:** `"token_list:<sortType>:<page>:<limit>"` (1s TTL)

**Path Params:** `sortType` (string, e.g. `"recent"`)
**Query Params:** `page` (default 1), `limit` (default 20, max 100), `category`, `verified_only`, `search`, `is_ido` (bool: `true`=funding, `false`=graduated)

**Response (200):**
```json
{
  "data": [
    {
      "token_info": { <ITokenInfo> },
      "market_info": { <IMarketInfo> }
    }
  ],
  "total_count": 100
}
```

---

#### `GET /order/project/:sortType`

Get paginated project list sorted by `sortType`.

**Auth:** None

**Path Params:** `sortType` (string, e.g. `"funded"`)
**Query Params:** `page` (default 1), `limit` (default 20, max 100)

**Response (200):**
```json
{
  "data": [ { <IProjectListItem> } ],
  "total_count": 50
}
```

---

#### `GET /trend/`

Get trending tokens (top 10 by recent activity).

**Auth:** None
**Cache:** `"token:trending"` (1s TTL)

**Response (200):**
```json
[
  {
    "token_info": { <ITokenInfo> },
    "market_info": { <IMarketInfo> }
  }
]
```

---

### 3.6 Trade (`/trade`)

#### `GET /trade/chart/:tokenAddress`

Get OHLCV chart bars for a token.

**Auth:** None

**Path Params:** `tokenAddress` (string)
**Query Params:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `resolution` | string | (required) | `1`, `5`, `15`, `60`, `240`, `D`, `W` or variants (`1m`, `5m`, `1h`, `4h`, `1d`, `1w`) |
| `from` | i64 | (required) | Start timestamp (unix) |
| `to` | i64 | (required) | End timestamp (unix) |
| `countback` | i64 | 300 | Max number of bars |
| `chart_type` | string | `"price"` | Chart type |

**Response (200):**
```json
{
  "bars": [
    {
      "time": 1700000000,
      "open": "0.025",
      "high": "0.026",
      "low": "0.024",
      "close": "0.0255",
      "volume": "15000"
    }
  ]
}
```

---

#### `GET /trade/swap-history/:tokenId`

Get paginated swap history for a token.

**Auth:** None

**Path Params:** `tokenId` (string)
**Query Params:** `page` (default 1), `limit` (default 20, max 100)

**Response (200):**
```json
{
  "data": [
    {
      "event_type": "BUY | SELL",
      "native_amount": "string",
      "token_amount": "string",
      "native_price": "string",
      "transaction_hash": "0x...",
      "value": "string",
      "account_info": { <IAccountInfo> },
      "created_at": 1700000000
    }
  ],
  "total_count": 200
}
```

---

#### `GET /trade/holder/:tokenId`

Get paginated token holders.

**Auth:** None

**Path Params:** `tokenId` (string)
**Query Params:** `page` (default 1), `limit` (default 20, max 100)

**Response (200):**
```json
{
  "data": [
    {
      "account_info": { <IAccountInfo> },
      "balance": "string"
    }
  ],
  "total_count": 150
}
```

---

#### `GET /trade/market/:tokenId`

Get market data for a token.

**Auth:** None
**Cache:** `"market:<tokenId>"` (1s TTL)

**Path Params:** `tokenId` (string)

**Response (200):**
```json
{ <IMarketInfo> }
```

---

#### `GET /trade/metrics/:tokenId`

Get token metrics (price changes, volume, trades) across timeframes.

**Auth:** None
**Cache:** `"metrics:<tokenId>"` (1s TTL)

**Path Params:** `tokenId` (string)

**Response (200):**
```json
{
  "metrics": {
    "1h": { "price_change": "0", "volume": "50000", "trades": 0 },
    "6h": { "price_change": "0", "volume": "50000", "trades": 0 },
    "24h": { "price_change": "0", "volume": "50000", "trades": 0 }
  }
}
```

**NOTE:** Currently returns placeholder data; real per-timeframe computation is TODO.

---

#### `GET /trade/quote/:tokenId`

Get a swap quote for a token.

**Auth:** None

**Path Params:** `tokenId` (string)
**Query Params:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `amount` | string | `""` | Amount to quote |
| `is_buy` | bool | `false` | Buy or sell direction |

**Response (200):**
```json
{
  "expected_output": "0",
  "price_impact_percent": "0",
  "minimum_received": "0",
  "fee": "0"
}
```

**NOTE:** Returns placeholder data; actual AMM math is TODO.

---

### 3.7 Profile (`/profile`)

#### `GET /profile/:address`

Get public profile for a wallet address.

**Auth:** None

**Path Params:** `address` (string)

**Response (200):**
```json
{
  "account_id": "0x...",
  "nickname": "string",
  "bio": "string",
  "image_uri": "string"
}
```

---

#### `GET /profile/hold-token/:accountId`

Get tokens held by an account.

**Auth:** None

**Path Params:** `accountId` (string)
**Query Params:** `page`, `limit`

**Response (200):**
```json
{
  "data": [
    { "token_id": "0x...", "balance": "string" }
  ],
  "total_count": 5
}
```

---

#### `GET /profile/swap-history/:accountId`

Get swap history for an account.

**Auth:** None

**Path Params:** `accountId` (string)
**Query Params:** `page`, `limit`

**Response (200):** `PaginatedResponse<ISwapInfo>`

---

#### `GET /profile/ido-history/:accountId`

Get IDO investment history for an account.

**Auth:** None

**Path Params:** `accountId` (string)
**Query Params:** `page`, `limit`

**Response (200):**
```json
{
  "data": [
    { "project_id": "0x...", "usdc_amount": "1000", "created_at": 1700000000 }
  ],
  "total_count": 3
}
```

---

#### `GET /profile/refund-history/:accountId`

Get refund history for an account.

**Auth:** None

**Path Params:** `accountId` (string)
**Query Params:** `page`, `limit`

**Response (200):**
```json
{
  "data": [
    {
      "project_id": "0x...",
      "tokens_burned": "string",
      "usdc_returned": "string",
      "tx_hash": "0x...",
      "created_at": 1700000000
    }
  ],
  "total_count": 1
}
```

---

#### `GET /profile/portfolio/:accountId`

Get portfolio summary for an account.

**Auth:** None

**Path Params:** `accountId` (string)

**Response (200):**
```json
{
  "hold_tokens_count": 5,
  "total_invested": "10000",
  "total_refunded": "500"
}
```

---

#### `GET /profile/tokens/created/:accountId`

Get tokens/projects created by an account.

**Auth:** None

**Path Params:** `accountId` (string)
**Query Params:** `page`, `limit`

**Response (200):**
```json
{
  "data": [
    {
      "project_id": "0x...",
      "name": "string",
      "symbol": "string",
      "image_uri": "string",
      "status": "funding",
      "created_at": 1700000000
    }
  ],
  "total_count": 2
}
```

---

### 3.8 Account (`/account`)

#### `GET /account/get_account`

Get the authenticated user's account info.

**Auth:** Session (required)

**Response (200):**
```json
{
  "account_id": "0x...",
  "nickname": "string",
  "bio": "string",
  "image_uri": "string"
}
```

---

### 3.9 Builder (`/builder`)

#### `GET /builder/overview/:projectId`

Get builder dashboard overview for a project.

**Auth:** None

**Path Params:** `projectId` (string)

**Response (200):**
```json
{
  "project_id": "string",
  "name": "string",
  "symbol": "string",
  "image_uri": "string",
  "status": "string",
  "target_raise": "string",
  "usdc_raised": "string",
  "investor_count": 10,
  "milestones": [ { <IMilestoneInfo> } ],
  "created_at": 1700000000
}
```

---

#### `GET /builder/stats/:projectId`

Get builder dashboard stats for a project.

**Auth:** None

**Path Params:** `projectId` (string)

**Response (200):**
```json
{
  "total_raised": "75000",
  "total_investors": 25,
  "milestones_completed": 3,
  "milestones_total": 5,
  "funds_released": "30000"
}
```

---

### 3.10 Metadata (`/metadata`)

#### `POST /metadata/image`

Upload an image to Cloudflare R2.

**Auth:** Session (required)

**Request Body:** `multipart/form-data` with a `file` field.

**Validation:**
- Content-Type: `image/png`, `image/jpeg`, `image/webp`, `image/gif` only
- File size: 5MB max
- Magic bytes verification

**Response (200):**
```json
{ "image_uri": "https://<R2_IMAGE_PUBLIC_URL>/<uuid>.<ext>" }
```

---

#### `POST /metadata/create`

Create metadata JSON and upload to R2 metadata bucket.

**Auth:** Session (required)

**Request Body:**
```json
{
  "name": "string (2-50 chars)",
  "symbol": "string (2-10 chars, uppercase + digits only)",
  "image_uri": "string (must be valid R2 image URL)",
  "category": "string (1-50 chars)",
  "homepage": "string | null (https:// only)",
  "twitter": "string | null (https:// only)",
  "telegram": "string | null (https:// only)",
  "discord": "string | null (https:// only)",
  "milestones": [
    { "order": 1, "title": "string", "description": "string", "fund_allocation_percent": 50 }
  ]
}
```

**Validation:** `image_uri` must start with `R2_IMAGE_PUBLIC_URL` prefix. Milestones: 2-6 items, allocations sum to 100%.

**Response (200):**
```json
{ "metadata_uri": "https://<R2_METADATA_PUBLIC_URL>/<uuid>.json" }
```

---

#### `POST /metadata/evidence`

Upload an evidence file.

**Auth:** Session (required)

**Request Body:** `multipart/form-data` with a `file` field.

**Validation:** File must not be empty.

**Response (200):**
```json
{ "uri": "/uploads/evidence/<filename>" }
```

---

## 4. Services

### 4.1 `services::auth`

Handles wallet-based authentication using EIP-4361 (SIWE) nonce flow.

| Function | Signature | Description |
|----------|-----------|-------------|
| `generate_nonce` | `(redis, address) -> Result<String>` | Creates a nonce message with address, timestamp, UUID. Stores in Redis with 5min TTL. |
| `verify_session` | `(redis, nonce, signature, chain_id) -> Result<(String, SessionInfo)>` | Atomically consumes nonce (replay protection), validates signature format, creates session (7-day TTL). |
| `delete_session` | `(redis, session_id) -> Result<()>` | Deletes session from Redis. |

### 4.2 `services::project`

Project lifecycle management.

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_project` | `(db, project_id) -> AppResult<IProjectData>` | Full project data with milestones, market info, investor count. |
| `get_featured` | `(db) -> AppResult<Vec<IProjectListItem>>` | Top 10 projects sorted by funded amount, filtered to "funding" status. |
| `get_project_list` | `(db, sort_type, pagination, status) -> AppResult<PaginatedResponse<IProjectListItem>>` | Paginated project list with optional status filter. |
| `create_project` | `(db, creator, request) -> AppResult<String>` | Validates request, checks symbol uniqueness, upserts account, creates project + milestones. Returns project_id. |
| `validate_symbol` | `(db, symbol) -> AppResult<bool>` | Checks if symbol is available. |
| `get_investors` | `(db, project_id, pagination) -> AppResult<PaginatedResponse<InvestorInfo>>` | Paginated investor list for a project. |

### 4.3 `services::milestone`

Milestone evidence submission and verification tracking.

| Function | Signature | Description |
|----------|-----------|-------------|
| `submit_evidence` | `(db, milestone_id, request) -> AppResult<()>` | Updates milestone status from "pending" to "submitted" with evidence text/URI. |
| `get_verification` | `(db, milestone_id) -> AppResult<IMilestoneVerificationData>` | Returns verification status with estimated completion (~7 days for submitted/in_verification). |

Milestone IDs use format `ms_<number>` (e.g., `ms_001`).

### 4.4 `services::token`

Token data aggregation from projects and market data.

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_token` | `(db, token_id) -> AppResult<ITokenData>` | Token info + market info. Token ID == project ID. |
| `get_token_list` | `(db, sort_type, pagination) -> AppResult<PaginatedResponse<TokenListItem>>` | Paginated token list with market data. Supports `is_ido` filter (true=funding, false=graduated). |
| `get_trending` | `(db) -> AppResult<Vec<TokenListItem>>` | Top 10 tokens sorted by recent activity. |

Market types: `CURVE` (bonding curve), `DEX` (graduated to DEX), `IDO` (default/initial).

### 4.5 `services::trade`

Trading-related data access.

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_chart` | `(db, token_address, params) -> AppResult<Vec<ChartBar>>` | OHLCV chart bars. Supports resolutions: 1m, 5m, 15m, 1h, 4h, 1d, 1w. |
| `get_swap_history` | `(db, token_id, pagination) -> AppResult<PaginatedResponse<ISwapInfo>>` | Paginated swap history for a token. |
| `get_holders` | `(db, token_id, pagination) -> AppResult<PaginatedResponse<HolderInfo>>` | Paginated holder list. |
| `get_market` | `(db, token_id) -> AppResult<IMarketInfo>` | Current market data for a token. |
| `get_metrics` | `(db, token_id) -> AppResult<ITokenMetricsData>` | Price changes/volume/trades per timeframe (1h, 6h, 24h). Currently placeholder. |
| `get_quote` | `(db, token_id, amount, is_buy) -> AppResult<TradeQuote>` | Swap quote. Currently placeholder. |

### 4.6 `services::profile`

User profile and portfolio data.

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_profile` | `(db, address) -> AppResult<IAccountInfo>` | Public profile lookup. |
| `get_hold_tokens` | `(db, account_id, pagination) -> AppResult<PaginatedResponse<HoldTokenInfo>>` | Tokens held by account. |
| `get_swap_history` | `(db, account_id, pagination) -> AppResult<PaginatedResponse<ISwapInfo>>` | Account's swap history. |
| `get_ido_history` | `(db, account_id, pagination) -> AppResult<PaginatedResponse<IdoHistoryItem>>` | IDO investment history. |
| `get_refund_history` | `(db, account_id, pagination) -> AppResult<PaginatedResponse<RefundHistoryItem>>` | Refund history. |
| `get_portfolio` | `(db, account_id) -> AppResult<PortfolioData>` | Aggregate portfolio stats. |
| `get_created_tokens` | `(db, account_id, pagination) -> AppResult<PaginatedResponse<CreatedTokenInfo>>` | Projects created by account. |
| `get_account` | `(db, account_id) -> AppResult<IAccountInfo>` | Authenticated user's account info. |

### 4.7 `services::builder`

Builder dashboard data.

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_overview` | `(db, project_id) -> AppResult<BuilderOverview>` | Project summary with milestones, investor count, raise progress. |
| `get_stats` | `(db, project_id) -> AppResult<BuilderStats>` | Aggregate stats: total raised, investors, milestone progress, funds released. |

### 4.8 `services::upload`

Image upload to Cloudflare R2.

| Function | Signature | Description |
|----------|-----------|-------------|
| `upload_image` | `(r2, bytes, content_type) -> AppResult<String>` | Validates file type (magic bytes), uploads to R2 image bucket, returns public URL. |
| `validate_image_uri` | `(uri) -> bool` | Checks URI starts with `R2_IMAGE_PUBLIC_URL` prefix. |

### 4.9 `services::metadata`

Metadata JSON creation and upload to Cloudflare R2.

| Function | Signature | Description |
|----------|-----------|-------------|
| `create_metadata` | `(r2, request) -> AppResult<String>` | Validates all fields (name, symbol, category, image_uri, milestones), builds JSON, uploads to R2 metadata bucket, returns public URL. |

---

## 5. Middleware

### 5.1 CORS (`cors.rs`)

Configured via `tower_http::cors::CorsLayer`.

| Setting | Value |
|---------|-------|
| **Allowed Origins** | `CORS_ALLOWED_ORIGINS` env var (comma-separated) or defaults: `http://localhost:3000`, `http://localhost:5173` |
| **Allowed Methods** | GET, POST, PUT, PATCH, DELETE, OPTIONS |
| **Allowed Headers** | Any |
| **Credentials** | Allowed |

### 5.2 Rate Limiting (`middleware/rate_limit.rs`)

Redis-based per-IP rate limiting.

| Setting | Value |
|---------|-------|
| **Limit** | 60 requests per minute per IP |
| **IP Extraction** | `X-Forwarded-For` (first IP) > `X-Real-IP` > `"unknown"` |
| **On Exceed** | 429 Too Many Requests with `Retry-After` header |
| **On Redis Failure** | Request is allowed (fail-open) |

### 5.3 Session Auth (`middleware/auth.rs`)

Two components:

**`session_middleware`** (applied to all routes):
- Extracts `session` cookie from request headers
- Looks up session in Redis
- If valid and not expired, injects `SessionInfo` into request extensions
- Does NOT reject unauthenticated requests

**`AuthUser` extractor** (used per-handler):
- Pulls `SessionInfo` from request extensions
- Returns `401 Unauthorized` if absent

```rust
pub struct AuthUser(pub SessionInfo);

pub struct SessionInfo {
    pub session_id: String,
    pub account_id: String,   // Ethereum address (lowercase)
    pub created_at: i64,       // Unix timestamp
    pub expires_at: i64,       // Unix timestamp
}
```

---

## 6. Configuration

All configuration is via environment variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `API_IP` | `127.0.0.1` | Bind IP address |
| `API_PORT` | `8000` | Bind port |
| `PRIMARY_DATABASE_URL` | (required) | PostgreSQL primary connection string |
| `REPLICA_DATABASE_URL` | (required) | PostgreSQL read replica connection string |
| `REDIS_URL` | (required) | Redis connection string |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:3000,http://localhost:5173` | Comma-separated allowed origins |
| `R2_ACCOUNT_ID` | (required) | Cloudflare account ID |
| `R2_ACCESS_KEY_ID` | (required) | R2 API access key |
| `R2_SECRET_ACCESS_KEY` | (required) | R2 API secret key |
| `R2_IMAGE_BUCKET` | `openlaunch-image` | R2 image bucket name |
| `R2_METADATA_BUCKET` | `openlaunch-metadata` | R2 metadata bucket name |
| `R2_IMAGE_PUBLIC_URL` | (required) | R2 image bucket public URL |
| `R2_METADATA_PUBLIC_URL` | (required) | R2 metadata bucket public URL |
| `RUST_LOG` | `info` | Log level filter (tracing env filter) |

---

## 7. Error Handling

All errors use the `AppError` enum, which implements Axum's `IntoResponse`.

### Error Variants and HTTP Status Codes

| Variant | HTTP Status | Code Field | Description |
|---------|-------------|------------|-------------|
| `BadRequest(String)` | 400 | `BAD_REQUEST` | Input validation failures |
| `Unauthorized(String)` | 401 | `UNAUTHORIZED` | Missing/invalid session |
| `Forbidden(String)` | 403 | `FORBIDDEN` | Insufficient permissions |
| `NotFound(String)` | 404 | `NOT_FOUND` | Resource not found |
| `Conflict` | 409 | `CONFLICT` | Resource conflict |
| `TooManyRequests { retry_after }` | 429 | `TOO_MANY_REQUESTS` | Rate limit exceeded |
| `Internal(anyhow::Error)` | 500 | `INTERNAL_ERROR` | Server-side errors |

### Standard Error Response Body

```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE"
}
```

### Rate Limit Error Response Body

```json
{
  "error": "Too many requests",
  "code": "TOO_MANY_REQUESTS",
  "retry_after": 45
}
```

Also includes `Retry-After` HTTP header.

### Security Note

Internal errors (500) always return `"Internal server error"` as the message. The actual error details are logged server-side via `tracing::error!` but never leaked to the client.

### Pagination

All paginated endpoints accept `page` (default 1) and `limit` (default 20, clamped to 1-100) as query parameters and return:

```json
{
  "data": [ ... ],
  "total_count": 100
}
```

### Common Shared Types

**`IAccountInfo`:**
```json
{
  "account_id": "0x...",
  "nickname": "string",
  "bio": "string",
  "image_uri": "string"
}
```

**`IMilestoneInfo`:**
```json
{
  "milestone_id": "ms_001",
  "order": 1,
  "title": "string",
  "description": "string",
  "fund_allocation_percent": 25,
  "fund_release_amount": "string",
  "status": "pending | submitted | in_verification | completed | failed",
  "funds_released": false,
  "evidence_uri": "string | null",
  "submitted_at": 1700000000,
  "verified_at": null
}
```
