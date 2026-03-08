# OpenLaunch Backend Architecture Design

> **Date**: 2026-03-07
> **Status**: Approved
> **Language**: Rust (Edition 2024)
> **Chain**: Avalanche C-Chain
> **Reference**: nadfun backend (api-server, websocket-server, observer, txbot)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Cargo Workspace Structure](#2-cargo-workspace-structure)
3. [Shared Crate (`shared`)](#3-shared-crate)
4. [API Server (`api-server`)](#4-api-server)
5. [WebSocket Server (`websocket-server`)](#5-websocket-server)
6. [Observer (`observer`)](#6-observer)
7. [TxBot (`txbot`)](#7-txbot)
8. [Database Schema](#8-database-schema)
9. [Configuration & Environment](#9-configuration--environment)

---

## 1. Overview

### System Architecture

```
                        Avalanche C-Chain (RPC / WSS)
                    ┌──────────┼──────────┬──────────────┐
                    │          │          │              │
                    v          v          v              v
              ┌──────────┐ ┌────────┐ ┌──────┐    ┌──────────┐
              │ observer │ │  ws    │ │txbot │    │api-server│
              │          │ │ server │ │      │    │          │
              └────┬─────┘ └───┬────┘ └──┬───┘    └────┬─────┘
                   │           │         │             │
                   v           │         │             v
              ┌─────────┐     │         │        ┌─────────┐
              │PostgreSQL│◄────┘─────────┘────────│  Redis  │
              └─────────┘                         └─────────┘
                                                       │
              ┌─────────┐                              │
              │  S3/R2  │◄─────────────────────────────┘
              └─────────┘

              ┌─────────────────────────────────┐
              │         shared crate            │
              │  types / db / rpc / config      │
              └─────────────────────────────────┘
```

### Data Flow Summary

| Service | Input | Output |
|---------|-------|--------|
| **observer** | Blockchain RPC (이벤트 로그) | PostgreSQL (영구 저장) |
| **websocket-server** | Blockchain RPC (이벤트 스트림) | WebSocket 클라이언트 (실시간 push) |
| **txbot** | Blockchain RPC (이벤트 감지) + PostgreSQL | Blockchain TX (graduate, collectFees) |
| **api-server** | PostgreSQL + Redis | REST API 응답 |

### Key Design Decisions

- 서비스 간 직접 통신 없음 (각각 독립적으로 블록체인/DB 접근)
- shared crate로 타입, DB, RPC 클라이언트 공유
- nadfun 패턴 참고하되 처음부터 작성
- UMA Oracle 미연동 (owner 수동 milestone approve)

---

## 2. Cargo Workspace Structure

```
openlaunch/
├── Cargo.toml                    # [workspace] members
├── crates/
│   ├── shared/
│   │   └── Cargo.toml
│   ├── api-server/
│   │   └── Cargo.toml
│   ├── websocket-server/
│   │   └── Cargo.toml
│   ├── observer/
│   │   └── Cargo.toml
│   └── txbot/
│       └── Cargo.toml
├── abi/                          # 컨트랙트 ABI JSON
│   ├── IIDO.json
│   ├── IProjectToken.json
│   ├── ILpManager.json
│   └── ISwapFeeHook.json
├── migrations/                   # sqlx 마이그레이션
├── contract/                     # Foundry 컨트랙트 (기존)
└── docs/
    └── plans/
```

### Workspace Cargo.toml

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
tower-http = { version = "0.6", features = ["cors", "timeout"] }
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
dotenv = "0.15"
lazy_static = "1.5"
once_cell = "1.21"
```

---

## 3. Shared Crate

공통 타입, DB 접근, RPC 클라이언트, 설정을 모든 서비스가 공유.

### 3.1 디렉토리 구조

```
crates/shared/src/
├── lib.rs
├── config.rs                 # 환경변수 (lazy_static)
├── error.rs                  # 공통 에러 타입
│
├── types/
│   ├── mod.rs
│   ├── common.rs             # PaginationParams, Address alias
│   ├── account.rs            # IAccountInfo
│   ├── project.rs            # IProjectInfo, IProjectMarketInfo, Status enum
│   ├── milestone.rs          # IMilestoneInfo, MilestoneStatus enum
│   ├── token.rs              # ITokenInfo, IMarketInfo
│   ├── trading.rs            # ISwapInfo, ChartBar, TradeQuote
│   ├── auth.rs               # NonceRequest, SessionRequest, SessionInfo
│   └── event.rs              # 온체인 이벤트 타입 (ProjectCreated, TokensPurchased 등)
│
├── db/
│   ├── mod.rs
│   ├── postgres/
│   │   ├── mod.rs            # PostgresDatabase (read/write pool)
│   │   ├── pool.rs           # 커넥션 풀 설정
│   │   └── controller/
│   │       ├── mod.rs
│   │       ├── account.rs    # 계정 CRUD
│   │       ├── project.rs    # 프로젝트 CRUD
│   │       ├── milestone.rs  # 마일스톤 CRUD
│   │       ├── investment.rs # 투자 기록
│   │       ├── token.rs      # 토큰 메타데이터
│   │       ├── swap.rs       # 스왑 기록
│   │       ├── balance.rs    # 잔액 관리
│   │       ├── chart.rs      # OHLCV 차트 데이터
│   │       ├── market.rs     # 마켓 데이터
│   │       └── block.rs      # 블록 진행 추적
│   └── redis/
│       ├── mod.rs            # RedisDatabase (connection manager)
│       ├── session.rs        # 세션 저장/조회/삭제
│       ├── cache.rs          # 범용 캐시 (get/set with TTL)
│       └── rate_limit.rs     # Rate limiting
│
├── client/
│   ├── mod.rs                # RpcClient (멀티 프로바이더)
│   ├── provider.rs           # 개별 프로바이더 (health score, failover)
│   └── health.rs             # 헬스 체크 루프
│
├── metrics/
│   ├── mod.rs                # Metrics 구조체
│   ├── db_metrics.rs         # DB 작업 측정
│   └── provider_metrics.rs   # RPC 프로바이더 측정
│
└── utils/
    ├── mod.rs
    ├── address.rs            # 주소 검증/체크섬
    ├── price.rs              # BigDecimal 가격 계산
    └── single_flight.rs      # Moka 기반 캐시 dedup
```

### 3.2 핵심 타입 정의

```rust
// types/project.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IProjectInfo {
    pub project_id: String,           // 컨트랙트 주소 (= token address)
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: Option<String>,
    pub tagline: String,
    pub category: String,             // "defi"|"infra"|"ai"|"gaming"|"social"|"meme"
    pub creator: IAccountInfo,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,              // unix seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IProjectMarketInfo {
    pub project_id: String,
    pub status: ProjectStatus,        // "funding"|"active"|"completed"|"failed"
    pub target_raise: String,         // wei string
    pub total_committed: String,
    pub funded_percent: f64,
    pub investor_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectStatus {
    Funding,    // IDO 진행 중
    Active,     // 졸업 완료, 마일스톤 진행 중
    Completed,  // 모든 마일스톤 완료
    Failed,     // 프로젝트 실패
}

// types/milestone.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IMilestoneInfo {
    pub milestone_id: String,
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32, // basis points / 100
    pub fund_release_amount: String,  // wei string
    pub status: MilestoneStatus,
    pub funds_released: bool,
    pub evidence_uri: Option<String>,
    pub submitted_at: Option<i64>,
    pub verified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MilestoneStatus {
    Completed,
    InVerification,
    Submitted,
    Pending,
    Failed,
}
```

### 3.3 RPC Client 아키텍처

```rust
// client/mod.rs
pub struct RpcClient {
    providers: DashMap<ProviderId, ProviderState>,
    latest_block: AtomicU64,
}

pub struct ProviderState {
    provider: RootProvider<Http<Client>>,
    score: AtomicI32,          // 0-100 점수
    failure_count: AtomicU32,
    priority: ProviderPriority, // Main=30, Sub1=20, Sub2=10
}

impl RpcClient {
    /// 가장 높은 점수의 프로바이더로 요청
    pub async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>>;

    /// 블록 스트림 구독 (WSS)
    pub async fn subscribe_blocks(&self) -> Result<BlockStream>;

    /// 컨트랙트 호출
    pub async fn call<T: SolCall>(&self, address: Address, call: T) -> Result<T::Return>;

    /// TX 전송 (txbot 전용)
    pub async fn send_transaction(&self, tx: TransactionRequest, wallet: &EthereumWallet) -> Result<TxHash>;
}
```

### 3.4 PostgreSQL 커넥션 풀

```rust
// db/postgres/mod.rs
pub struct PostgresDatabase {
    write_pool: PgPool,    // Primary (max 50 connections)
    read_pool: PgPool,     // Replica (max 200 connections)
}

impl PostgresDatabase {
    pub fn writer(&self) -> &PgPool;
    pub fn reader(&self) -> &PgPool;
}
```

### 3.5 Redis 구조

```rust
// db/redis/mod.rs
pub struct RedisDatabase {
    conn: ConnectionManager,
}

// db/redis/session.rs
impl RedisDatabase {
    pub async fn set_session(&self, session_id: &str, info: &SessionInfo, ttl: u64) -> Result<()>;
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionInfo>>;
    pub async fn delete_session(&self, session_id: &str) -> Result<()>;
    pub async fn set_nonce(&self, address: &str, nonce: &str, ttl: u64) -> Result<()>;
    pub async fn get_and_delete_nonce(&self, address: &str) -> Result<Option<String>>;
}

// db/redis/cache.rs
impl RedisDatabase {
    pub async fn cache_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    pub async fn cache_set<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> Result<()>;
}
```

---

## 4. API Server

REST API를 제공하는 HTTP 서버. 클라이언트(프론트엔드)의 모든 데이터 요청을 처리.

### 4.1 디렉토리 구조

```
crates/api-server/src/
├── main.rs                       # 서버 시작, 라우터 조립, state 초기화
├── state.rs                      # AppState (DB, Redis, S3 클라이언트)
├── cors.rs                       # CORS 설정
│
├── middleware/
│   ├── mod.rs
│   ├── auth.rs                   # 세션 인증 미들웨어
│   └── rate_limit.rs             # IP/API key 기반 rate limiting
│
├── router/
│   ├── mod.rs                    # 전체 라우터 조립
│   ├── auth/
│   │   └── mod.rs                # POST /auth/nonce, /auth/session, DELETE /auth/delete_session
│   ├── project/
│   │   └── mod.rs                # GET /project/:id, /project/featured, POST /project/create 등
│   ├── milestone/
│   │   └── mod.rs                # POST /milestone/submit/:id, GET /milestone/verification/:id
│   ├── token/
│   │   └── mod.rs                # GET /token/:id, /trend, /order/:sortType
│   ├── trade/
│   │   └── mod.rs                # GET /trade/chart, /swap-history, /holder, /market, /metrics, /quote
│   ├── profile/
│   │   └── mod.rs                # GET /profile/:address, /hold-token, /swap-history 등
│   ├── builder/
│   │   └── mod.rs                # GET /builder/overview/:id, /builder/stats/:id
│   ├── metadata/
│   │   └── mod.rs                # POST /metadata/image, /metadata/evidence
│   └── health/
│       └── mod.rs                # GET /health
│
└── services/
    ├── mod.rs
    ├── auth.rs                   # SIWE nonce 생성, 서명 검증, 세션 관리
    ├── project.rs                # 프로젝트 생성, 조회, 리스트 로직
    ├── milestone.rs              # 마일스톤 제출, 검증 상태 조회
    ├── token.rs                  # 토큰 조회, 트렌드, 정렬
    ├── trade.rs                  # 차트, 스왑 히스토리, 홀더, 마켓, 견적
    ├── profile.rs                # 포트폴리오, 보유토큰, 활동내역
    ├── builder.rs                # 빌더 대시보드 데이터
    └── upload.rs                 # S3/R2 이미지/파일 업로드
```

### 4.2 AppState

```rust
pub struct AppState {
    pub db: Arc<PostgresDatabase>,
    pub redis: Arc<RedisDatabase>,
    pub s3: Arc<S3Client>,
    pub cache: Arc<MokaCache>,       // Single Flight 캐시
}
```

### 4.3 라우터 조립

```rust
// router/mod.rs
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Public (인증 불필요)
        .nest("/auth", auth::router())
        .nest("/project", project::public_router())
        .nest("/order", order::router())
        .nest("/token", token::router())
        .nest("/trend", trend::router())
        .nest("/trade", trade::router())
        .nest("/profile", profile::router())
        .route("/health", get(health::check))

        // Authenticated (세션 필요)
        .nest("/project", project::auth_router())
        .nest("/milestone", milestone::router())
        .nest("/builder", builder::router())
        .nest("/metadata", metadata::router())
        .nest("/account", account::router())

        // Middleware stack
        .layer(CookieManagerLayer::new())
        .layer(cors_layer())
        .layer(timeout_layer())
        .with_state(state)
}
```

### 4.4 인증 플로우

```
1. POST /auth/nonce { address }
   → 서버: nonce 생성 (EIP-4361 메시지 포맷)
   → Redis에 nonce 저장 (TTL 5분)
   → 응답: { nonce: "Sign this message..." }

2. POST /auth/session { nonce, signature, chain_id }
   → 서버: Redis에서 nonce 조회 (GETDEL - atomic)
   → Alloy secp256k1 서명 검증
   → 주소 복구 → nonce 메시지의 주소와 일치 확인
   → session_id 생성: base64(address-timestamp-uuid)[0:32]
   → Redis + PostgreSQL에 세션 저장
   → Set-Cookie: session=...; HttpOnly; Secure; SameSite=None; Max-Age=86400
   → 응답: { account_info }

3. 인증이 필요한 요청
   → 미들웨어: Cookie에서 session 추출
   → Redis에서 세션 조회 (miss 시 PostgreSQL fallback)
   → request extensions에 SessionInfo 저장
   → 핸들러에서 Extension<SessionInfo> 사용
```

### 4.5 엔드포인트 상세

#### Auth (3개)

| Method | Path | Auth | 설명 |
|--------|------|------|------|
| POST | `/auth/nonce` | No | nonce 발급 |
| POST | `/auth/session` | No | 세션 생성 (서명 검증) |
| DELETE | `/auth/delete_session` | Yes | 세션 삭제 |

#### Project (6개)

| Method | Path | Auth | 설명 | 캐시 |
|--------|------|------|------|------|
| GET | `/project/:projectId` | No | 프로젝트 상세 (milestones 포함) | Redis 10s |
| GET | `/project/featured` | No | 피쳐드 프로젝트 (2~5개) | Redis 30s |
| GET | `/order/project/:sortType` | No | 프로젝트 리스트 (recent/funded/target/investors) | Redis 5s |
| POST | `/project/create` | Yes | 프로젝트 생성 | - |
| GET | `/project/validate-symbol` | No | 티커 중복 확인 | - |
| GET | `/project/investor/:projectId` | No | 투자자 리스트 | Redis 10s |

#### Milestone (2개)

| Method | Path | Auth | 설명 |
|--------|------|------|------|
| POST | `/milestone/submit/:milestoneId` | Yes | 마일스톤 검증 제출 (evidence 저장) |
| GET | `/milestone/verification/:milestoneId` | No | 검증 상태 조회 |

#### Token (3개)

| Method | Path | Auth | 설명 | 캐시 |
|--------|------|------|------|------|
| GET | `/token/:tokenId` | No | 토큰 상세 | Redis 10s |
| GET | `/trend` | No | Trending 토큰 | Redis 30s |
| GET | `/order/:sortType` | No | 토큰 리스트 (mcap/creation_time/trending/most_funded) | Redis 5s |

#### Trade (6개)

| Method | Path | Auth | 설명 | 캐시 |
|--------|------|------|------|------|
| GET | `/trade/chart/:tokenAddress` | No | OHLCV 차트 데이터 | Moka 1s |
| GET | `/trade/swap-history/:tokenId` | No | 거래 내역 | Redis 5s |
| GET | `/trade/holder/:tokenId` | No | 홀더 리스트 | Redis 10s |
| GET | `/trade/market/:tokenId` | No | 마켓 데이터 | Redis 5s |
| GET | `/trade/metrics/:tokenId` | No | 토큰 지표 (5m/1h/6h/24h) | Redis 10s |
| GET | `/trade/quote/:tokenId` | No | 스왑 견적 | - |

#### Profile (7개)

| Method | Path | Auth | 설명 |
|--------|------|------|------|
| GET | `/profile/:address` | No | 유저 프로필 (공개) |
| GET | `/account/get_account` | Yes | 내 계정 정보 |
| GET | `/profile/hold-token/:accountId` | No | 보유 토큰 리스트 |
| GET | `/profile/swap-history/:accountId` | No | 트레이딩 히스토리 |
| GET | `/profile/ido-history/:accountId` | No | IDO 참여 내역 |
| GET | `/profile/refund-history/:accountId` | No | 환불 내역 |
| GET | `/profile/portfolio/:accountId` | No | 포트폴리오 요약 |

#### Builder (3개)

| Method | Path | Auth | 설명 |
|--------|------|------|------|
| GET | `/profile/tokens/created/:accountId` | No | 내가 만든 프로젝트 |
| GET | `/builder/overview/:projectId` | Yes | 빌더 대시보드 오버뷰 |
| GET | `/builder/stats/:projectId` | Yes | 펀딩 추이 차트 데이터 |

#### Upload (2개)

| Method | Path | Auth | 설명 | 제한 |
|--------|------|------|------|------|
| POST | `/metadata/image` | Yes | 이미지 업로드 (로고/배너) | 5MB, PNG/JPG |
| POST | `/metadata/evidence` | Yes | 마일스톤 증거 파일 | 10MB, PDF/ZIP |

### 4.6 에러 응답 형식

```rust
pub enum AppError {
    BadRequest(String),        // 400
    Unauthorized(String),      // 401
    Forbidden(String),         // 403
    NotFound(String),          // 404
    TooManyRequests { retry_after: u64 },  // 429
    Internal(anyhow::Error),   // 500
}

// JSON 응답
{
    "error": "Human readable error message",
    "code": "ERROR_CODE"
}
```

### 4.7 캐싱 전략

| 데이터 | 캐시 레이어 | TTL | 패턴 |
|--------|------------|-----|------|
| 세션 | Redis | 24h | Read-through (miss → PostgreSQL) |
| Nonce | Redis | 5min | GETDEL (atomic, replay 방지) |
| 프로젝트 상세 | Redis | 10s | Cache-aside |
| 토큰 리스트 | Redis | 5s | Cache-aside |
| 차트 데이터 | Moka (in-memory) | 1s | Single Flight |
| 검색 결과 | Redis | 30s | Cache-aside |

---

## 5. WebSocket Server

블록체인에서 직접 이벤트를 수신하여 클라이언트에 실시간으로 push.

### 5.1 디렉토리 구조

```
crates/websocket-server/src/
├── main.rs                           # 서버 시작, 이벤트 프로듀서 초기화
├── config.rs                         # WS 서버 전용 설정 (포트, 채널 크기 등)
│
├── server/
│   ├── mod.rs                        # HTTP + WS 라우터
│   └── socket/
│       ├── mod.rs                    # WebSocket 업그레이드 핸들러
│       ├── connection.rs             # ConnectionState (구독 관리)
│       └── rpc.rs                    # JSON-RPC 2.0 파서 & 디스패처
│
├── stream/
│   ├── mod.rs                        # 스트림 매니저 (블록 범위 추적)
│   ├── ido/
│   │   ├── stream.rs                 # IDO 컨트랙트 이벤트 스트리밍
│   │   └── receive.rs                # 이벤트 파싱 → 프로듀서 전달
│   └── pool/
│       ├── stream.rs                 # V4 Pool 스왑 이벤트 스트리밍
│       └── receive.rs                # 스왑 이벤트 파싱 → 프로듀서 전달
│
├── event/
│   ├── mod.rs                        # 이벤트 프로듀서 trait
│   ├── core.rs                       # MonitoredChannel, EventBatch
│   ├── trade.rs                      # TradeEventProducer (BUY/SELL 실시간)
│   ├── price.rs                      # PriceEventProducer (가격 업데이트)
│   ├── project.rs                    # ProjectEventProducer (투자, 졸업, 실패)
│   ├── milestone.rs                  # MilestoneEventProducer (승인 이벤트)
│   └── new_content.rs                # NewContentEventProducer (틱커용 전체 피드)
│
└── cache/
    └── mod.rs                        # CacheManager (Redis L1 + PostgreSQL L2)
```

### 5.2 이벤트 스트리밍 파이프라인

```
Avalanche RPC (WSS)
    │
    ├─── IDO Stream ──────────────────────────────────────┐
    │    감시: ProjectCreated, TokensPurchased,            │
    │          Graduated, MilestoneApproved,               │
    │          ProjectFailed, Refunded                     │
    │                                                      v
    ├─── Pool Stream ─────────────────────────────┐   Event Producers
    │    감시: V4 Pool Swap 이벤트,                │      │
    │          Transfer 이벤트                     │      ├── TradeEventProducer
    │                                              v      ├── PriceEventProducer
    └──────────────────────────────────────────────┘      ├── ProjectEventProducer
                                                          ├── MilestoneEventProducer
                                                          └── NewContentEventProducer
                                                                │
                                                          DashMap<Key, broadcast::Sender>
                                                                │
                                                          WebSocket Clients
```

### 5.3 구독 채널 상세

#### trade_subscribe
```rust
// 클라이언트 → 서버
{ "jsonrpc": "2.0", "method": "trade_subscribe", "params": { "token_id": "0x..." } }

// 서버 → 클라이언트 (V4 Pool Swap 발생 시)
{
    "jsonrpc": "2.0",
    "method": "trade_subscribe",
    "result": {
        "type": "TRADE",
        "data": {
            "event_type": "BUY",
            "account_info": { "account_id": "0x...", "nickname": "..." },
            "token_amount": "500000000000000000000",
            "native_amount": "5000000000000000000",
            "transaction_hash": "0xabc...",
            "created_at": 1717253570
        }
    }
}
```

#### price_subscribe
```rust
// 클라이언트 → 서버
{ "jsonrpc": "2.0", "method": "price_subscribe", "params": { "token_id": "0x..." } }

// 서버 → 클라이언트
{
    "jsonrpc": "2.0",
    "method": "price_subscribe",
    "result": {
        "type": "PRICE_UPDATE",
        "data": {
            "token_id": "0x...",
            "token_price": "0.0258",
            "native_price": "32.55",
            "volume": "8500000000000000000000",
            "holder_count": 345
        }
    }
}
```

#### project_subscribe
```rust
// 클라이언트 → 서버
{ "jsonrpc": "2.0", "method": "project_subscribe", "params": { "project_id": "0x..." } }

// 서버 → 클라이언트 (투자 발생 시)
{
    "jsonrpc": "2.0",
    "method": "project_subscribe",
    "result": {
        "type": "PROJECT_UPDATE",
        "data": {
            "event": "TOKENS_PURCHASED",
            "project_id": "0x...",
            "buyer": "0x...",
            "usdc_amount": "10000000000",
            "total_committed": "312450000000000000000000",
            "funded_percent": 62
        }
    }
}
```

#### milestone_subscribe
```rust
// 클라이언트 → 서버
{ "jsonrpc": "2.0", "method": "milestone_subscribe", "params": { "project_id": "0x..." } }

// 서버 → 클라이언트
{
    "jsonrpc": "2.0",
    "method": "milestone_subscribe",
    "result": {
        "type": "MILESTONE_UPDATE",
        "data": {
            "project_id": "0x...",
            "milestone_index": 1,
            "status": "verified",
            "usdc_released": "125000000000000000000000"
        }
    }
}
```

#### new_content_subscribe
```rust
// 클라이언트 → 서버 (파라미터 없음)
{ "jsonrpc": "2.0", "method": "new_content_subscribe" }

// 서버 → 클라이언트 (모든 거래/이벤트 브로드캐스트)
{
    "jsonrpc": "2.0",
    "method": "new_content_subscribe",
    "result": {
        "type": "NEW_CONTENT",
        "data": {
            "new_buy": { "account_info": {...}, "token_info": {...}, "amount": "..." },
            "new_project": null,
            "new_graduation": null
        }
    }
}
```

### 5.4 커넥션 상태 관리

```rust
pub struct ConnectionState {
    /// 구독 키 → JoinHandle (구독 태스크)
    subscriptions: HashMap<SubscriptionKey, JoinHandle<()>>,
    /// 클라이언트에 메시지를 보내는 채널
    sender: mpsc::Sender<Message>,
}

#[derive(Hash, Eq, PartialEq)]
pub enum SubscriptionKey {
    Trade(String),              // token_id
    Price(String),              // token_id
    Project(String),            // project_id
    Milestone(String),          // project_id
    NewContent,
}

impl ConnectionState {
    /// 같은 키로 새 구독 시 기존 구독 abort
    pub fn subscribe(&mut self, key: SubscriptionKey, handle: JoinHandle<()>);
    /// 연결 종료 시 모든 구독 cleanup
    pub fn cleanup_all(&mut self);
}
```

### 5.5 이벤트 프로듀서 패턴

```rust
// event/mod.rs
pub trait EventProducer: Send + Sync {
    type Event: Clone + Send;
    type Key: Hash + Eq + Clone;

    /// 새 이벤트 수신 → 해당 키의 broadcast channel로 전송
    fn publish(&self, key: &Self::Key, event: Self::Event);

    /// 클라이언트가 구독 → broadcast::Receiver 반환
    fn subscribe(&self, key: &Self::Key) -> broadcast::Receiver<Self::Event>;
}

// 각 프로듀서는 DashMap<Key, broadcast::Sender<Event>> 보유
// 5분간 구독자 없으면 채널 자동 정리
```

---

## 6. Observer

블록체인 이벤트를 인덱싱하여 PostgreSQL에 영구 저장. API Server가 조회할 데이터를 생성.

### 6.1 디렉토리 구조

```
crates/observer/src/
├── main.rs                           # 이벤트 핸들러 스폰, 메트릭 서버
├── config.rs                         # Observer 전용 설정
│
├── event/
│   ├── mod.rs                        # 이벤트 핸들러 등록
│   ├── core.rs                       # EventBatch, MonitoredChannel
│   ├── handler.rs                    # run_event_handler_with_retry (재시도 로직)
│   ├── error.rs                      # 스킵 가능한 에러 정의
│   │
│   ├── ido/                          # IDO 컨트랙트 이벤트
│   │   ├── mod.rs
│   │   ├── stream.rs                 # ProjectCreated, TokensPurchased, Graduated,
│   │   │                             # MilestoneApproved, ProjectFailed, Refunded
│   │   └── receive.rs                # 파싱 → DB 저장
│   │
│   ├── token/                        # ERC20 Transfer 이벤트
│   │   ├── mod.rs
│   │   ├── stream.rs                 # Transfer 이벤트 (잔액 추적)
│   │   └── receive.rs                # balance, transfer 테이블 업데이트
│   │
│   ├── swap/                         # V4 Pool 스왑 이벤트
│   │   ├── mod.rs
│   │   ├── stream.rs                 # Swap 이벤트
│   │   └── receive.rs                # swaps, chart, market_data 업데이트
│   │
│   ├── lp/                           # LP Manager 이벤트
│   │   ├── mod.rs
│   │   ├── stream.rs                 # LiquidityAllocated, FeesCollected
│   │   └── receive.rs                # liquidity_positions 업데이트
│   │
│   └── price/                        # 가격 데이터
│       ├── mod.rs
│       ├── stream.rs                 # 스왑 기반 가격 계산 또는 외부 Oracle
│       └── receive.rs                # prices, market_data 업데이트
│
├── sync/
│   ├── mod.rs
│   ├── stream.rs                     # StreamManager (from_block → to_block 관리)
│   └── receive.rs                    # ReceiveManager (이벤트 간 의존성 관리)
│
└── controller/                       # DB 저장 로직 (shared controller 호출)
    ├── mod.rs
    ├── project.rs                    # 프로젝트 생성/상태 업데이트
    ├── investment.rs                 # 투자 기록 저장
    ├── milestone.rs                  # 마일스톤 상태 업데이트
    ├── swap.rs                       # 스왑 기록 + 차트 업데이트
    ├── balance.rs                    # 잔액 업데이트
    ├── market.rs                     # 마켓 데이터 업데이트
    └── block.rs                      # 블록 진행 상태 저장
```

### 6.2 이벤트 핸들러 파이프라인

```
┌─────────────────────────────────────────────────┐
│                   main.rs                        │
│                                                  │
│  spawn ido_handler ─────────────────┐            │
│  spawn token_handler ───────────────┤            │
│  spawn swap_handler ────────────────┤  JoinSet   │
│  spawn lp_handler ──────────────────┤            │
│  spawn price_handler ───────────────┘            │
│  spawn metrics_server                            │
│  spawn block_updater                             │
└─────────────────────────────────────────────────┘

각 핸들러:
┌──────────────┐    mpsc     ┌──────────────┐
│  stream.rs   │ ──────────> │  receive.rs  │
│  (RPC 조회)  │  EventBatch │  (DB 저장)   │
└──────────────┘             └──────────────┘
```

### 6.3 인덱싱 대상 이벤트 매핑

| 컨트랙트 | 이벤트 | DB 테이블 | 설명 |
|----------|--------|-----------|------|
| IDO | `ProjectCreated` | projects, milestones | 프로젝트 + 마일스톤 생성 |
| IDO | `TokensPurchased` | investments, projects (total_committed 업데이트) | 투자 기록 |
| IDO | `Graduated` | projects (status→Active) | 졸업 상태 변경 |
| IDO | `MilestoneApproved` | milestones (status→Completed, funds_released) | 마일스톤 승인 |
| IDO | `ProjectFailed` | projects (status→Failed) | 프로젝트 실패 |
| IDO | `Refunded` | refunds, projects (usdcReleased 업데이트) | 환불 기록 |
| ProjectToken | `Transfer` | balances, transfers | 잔액 추적 |
| V4 Pool | Swap | swaps, charts, market_data | 거래 기록 + 차트 + 가격 |
| LpManager | `LiquidityAllocated` | liquidity_positions | LP 포지션 |
| LpManager | `FeesCollected` | fee_collections | 수수료 수거 기록 |

### 6.4 블록 추적 & 의존성

```rust
// sync/stream.rs
pub struct StreamManager {
    /// 이벤트 타입별 현재 처리 블록
    progress: DashMap<EventType, BlockRange>,
}

pub struct BlockRange {
    pub from_block: u64,
    pub to_block: u64,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub enum EventType {
    Ido,
    Token,
    Swap,
    Lp,
    Price,
}

// sync/receive.rs
pub struct ReceiveManager {
    /// 이벤트 간 의존성 (swap은 ido 이후 처리)
    dependencies: HashMap<EventType, Vec<EventType>>,
    /// 이벤트 타입별 마지막 처리 완료 블록
    completed: DashMap<EventType, u64>,
}

// 의존성 그래프:
// Price  → (독립)
// Ido    → (독립)
// Token  → Ido (프로젝트 존재해야 Transfer 의미 있음)
// Swap   → Ido (졸업 후 풀 생성)
// Lp     → Ido (졸업 후 LP 생성)
```

### 6.5 재시도 & 에러 처리

```rust
// event/handler.rs
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_factor: f64,
}

// 기본값
const IDO_RETRY: RetryConfig = RetryConfig {
    max_attempts: 10,
    initial_backoff_ms: 500,
    max_backoff_ms: 30_000,
    backoff_factor: 2.0,
};

// 스킵 가능한 에러 (재시도하지 않음)
const SKIPPABLE_ERRORS: &[&str] = &[
    "Unknown event type",
    "Token not found in registry",
    "Duplicate entry",
];
```

---

## 7. TxBot

블록체인 이벤트를 감지하여 자동으로 트랜잭션을 전송하는 봇.

### 7.1 디렉토리 구조

```
crates/txbot/src/
├── main.rs                           # 잡 핸들러 스폰, 모니터링 태스크
├── config.rs                         # TxBot 전용 설정
│
├── keystore.rs                       # 지갑 로딩 (env → SigningKey → EthereumWallet)
│
├── job/
│   ├── mod.rs
│   ├── handler.rs                    # JobHandler trait + retry 로직
│   │
│   ├── graduate/
│   │   ├── mod.rs                    # GraduateJob 정의
│   │   ├── stream.rs                 # IDO 이벤트 감시 (sold out / deadline 판단)
│   │   └── execute.rs                # IDO.graduate(token) TX 전송
│   │
│   └── collect/
│       ├── mod.rs                    # CollectJob 정의
│       ├── stream.rs                 # DB 폴링 (graduated 토큰 중 수수료 누적)
│       └── execute.rs                # IDO.collectFees(token) TX 전송
│
└── metrics/
    ├── mod.rs
    ├── wallet_metrics.rs             # 지갑 잔액 모니터링
    └── job_metrics.rs                # 잡 성공/실패 카운터
```

### 7.2 Graduate Job

```
감시 대상: IDO 컨트랙트 이벤트

판단 로직:
1. TokensPurchased 이벤트 수신
2. on-chain 조회: project.idoSold >= project.idoSupply ? → sold out → graduate
3. 또는 block.timestamp >= project.deadline ? → deadline passed → graduate

실행:
1. IDO.graduate(token) 호출 확인:
   - project.status == Active (아직 졸업 안 함)
   - idoSold > 0 (최소 1건 투자)
2. TX 전송: graduate(token)
3. TX 확인 (receipt 대기)
4. 실패 시 재시도 (max 20회, exponential backoff)
```

```rust
// job/graduate/stream.rs
pub async fn watch_graduate_events(
    rpc: Arc<RpcClient>,
    ido_address: Address,
    sender: mpsc::Sender<GraduateTask>,
) {
    // 블록 스트림 구독
    // TokensPurchased 이벤트 파싱
    // sold out 또는 deadline 도달 판단
    // GraduateTask { token_address } 전송
}

// job/graduate/execute.rs
pub async fn execute_graduate(
    rpc: Arc<RpcClient>,
    wallet: &EthereumWallet,
    ido_address: Address,
    token: Address,
) -> Result<TxHash> {
    // 1. 온체인 상태 확인 (이미 졸업했는지)
    let project = rpc.call(ido_address, IDO::projectsCall { token }).await?;
    if project.status != Status::Active { return Ok(/* skip */); }

    // 2. graduate TX 전송
    let tx = TransactionRequest::default()
        .to(ido_address)
        .input(IDO::graduateCall { token }.abi_encode());
    rpc.send_transaction(tx, wallet).await
}
```

### 7.3 Collect Fees Job

```
감시 대상: PostgreSQL 폴링 (30초 간격)

판단 로직:
1. DB에서 graduated 프로젝트 목록 조회
2. 각 프로젝트의 V4 position에 누적 수수료 확인 (on-chain 조회)
3. 수수료 >= MIN_COLLECT_AMOUNT 이면 수거 대상

실행:
1. IDO.collectFees(token) TX 전송
2. TX 확인
3. 실패 시 재시도 (max 5회)
```

```rust
// job/collect/stream.rs
pub async fn poll_collectable_fees(
    db: Arc<PostgresDatabase>,
    rpc: Arc<RpcClient>,
    sender: mpsc::Sender<CollectTask>,
    interval: Duration,  // 30s
) {
    loop {
        // 1. DB에서 graduated 프로젝트 조회
        let projects = db.reader().fetch_graduated_projects().await?;

        // 2. 각 프로젝트 수수료 확인 (병렬)
        for project in projects {
            // on-chain: LpManager position의 누적 수수료 확인
            // 임계값 이상이면 CollectTask 전송
        }

        tokio::time::sleep(interval).await;
    }
}
```

### 7.4 지갑 관리

```rust
// keystore.rs
pub struct Wallets {
    pub graduate: EthereumWallet,   // GRADUATE_PRIVATE_KEY
    pub collector: EthereumWallet,  // COLLECTOR_PRIVATE_KEY
}

pub fn load_wallets_from_env() -> Result<Wallets> {
    let graduate_key = std::env::var("GRADUATE_PRIVATE_KEY")?;
    let collector_key = std::env::var("COLLECTOR_PRIVATE_KEY")?;

    // hex → SigningKey → LocalSigner → EthereumWallet
    // zeroize로 메모리 클리어
}
```

### 7.5 재시도 설정

| Job | Max Attempts | Initial Backoff | Max Backoff |
|-----|-------------|-----------------|-------------|
| Graduate | 20 | 500ms | 30s |
| Collect | 5 | 1s | 60s |

---

## 8. Database Schema

### 8.1 Core Tables

```sql
-- 계정
CREATE TABLE accounts (
    account_id      VARCHAR(42) PRIMARY KEY,  -- 0x prefixed address
    nickname        VARCHAR(50) NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    image_uri       TEXT NOT NULL DEFAULT '',
    created_at      BIGINT NOT NULL,           -- unix seconds
    updated_at      BIGINT NOT NULL
);

-- 세션
CREATE TABLE sessions (
    session_id      VARCHAR(64) PRIMARY KEY,
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    created_at      BIGINT NOT NULL,
    expires_at      BIGINT NOT NULL
);
CREATE INDEX idx_sessions_account ON sessions(account_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

-- 프로젝트
CREATE TABLE projects (
    project_id      VARCHAR(42) PRIMARY KEY,   -- token contract address
    name            VARCHAR(50) NOT NULL,
    symbol          VARCHAR(10) NOT NULL UNIQUE,
    image_uri       TEXT NOT NULL,
    description     TEXT,
    tagline         VARCHAR(120) NOT NULL,
    category        VARCHAR(20) NOT NULL,
    creator         VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    status          VARCHAR(20) NOT NULL DEFAULT 'funding',  -- funding/active/completed/failed
    target_raise    NUMERIC NOT NULL,          -- USDC (6 decimals)
    token_price     NUMERIC NOT NULL,          -- USDC per token
    ido_supply      NUMERIC NOT NULL,          -- 18 decimals
    ido_sold        NUMERIC NOT NULL DEFAULT 0,
    total_supply    NUMERIC NOT NULL,          -- 1B * 1e18
    usdc_raised     NUMERIC NOT NULL DEFAULT 0,
    usdc_released   NUMERIC NOT NULL DEFAULT 0,
    tokens_refunded NUMERIC NOT NULL DEFAULT 0,
    deadline        BIGINT NOT NULL,           -- unix seconds
    website         TEXT,
    twitter         TEXT,
    github          TEXT,
    telegram        TEXT,
    created_at      BIGINT NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL
);
CREATE INDEX idx_projects_creator ON projects(creator);
CREATE INDEX idx_projects_status ON projects(status);
CREATE INDEX idx_projects_created ON projects(created_at DESC);
CREATE INDEX idx_projects_symbol ON projects(symbol);

-- 마일스톤
CREATE TABLE milestones (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    milestone_index INT NOT NULL,              -- 0-based (컨트랙트 인덱스)
    title           VARCHAR(200) NOT NULL,
    description     TEXT NOT NULL,
    allocation_bps  INT NOT NULL,              -- basis points (합계 10000)
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
                                               -- pending/submitted/in_verification/completed/failed
    funds_released  BOOLEAN NOT NULL DEFAULT FALSE,
    release_amount  NUMERIC,                   -- 실제 릴리스된 USDC
    evidence_uri    TEXT,
    evidence_text   TEXT,
    submitted_at    BIGINT,
    verified_at     BIGINT,
    tx_hash         VARCHAR(66),
    UNIQUE(project_id, milestone_index)
);
CREATE INDEX idx_milestones_project ON milestones(project_id);

-- 투자 기록
CREATE TABLE investments (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    usdc_amount     NUMERIC NOT NULL,
    token_amount    NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_investments_project ON investments(project_id);
CREATE INDEX idx_investments_account ON investments(account_id);
CREATE INDEX idx_investments_created ON investments(created_at DESC);

-- 환불 기록
CREATE TABLE refunds (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    tokens_burned   NUMERIC NOT NULL,
    usdc_returned   NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_refunds_project ON refunds(project_id);
CREATE INDEX idx_refunds_account ON refunds(account_id);
```

### 8.2 Trading Tables

```sql
-- 스왑 기록 (V4 Pool)
CREATE TABLE swaps (
    id              SERIAL PRIMARY KEY,
    token_id        VARCHAR(42) NOT NULL,      -- project token address
    account_id      VARCHAR(42) NOT NULL,
    event_type      VARCHAR(4) NOT NULL,        -- BUY/SELL
    native_amount   NUMERIC NOT NULL,           -- USDC amount
    token_amount    NUMERIC NOT NULL,
    price           NUMERIC NOT NULL,           -- token price at swap time
    value           NUMERIC NOT NULL,           -- USD value
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_swaps_token ON swaps(token_id, created_at DESC);
CREATE INDEX idx_swaps_account ON swaps(account_id, created_at DESC);
CREATE INDEX idx_swaps_created ON swaps(created_at DESC);

-- 잔액
CREATE TABLE balances (
    account_id      VARCHAR(42) NOT NULL,
    token_id        VARCHAR(42) NOT NULL,
    balance         NUMERIC NOT NULL DEFAULT 0,
    updated_at      BIGINT NOT NULL,
    PRIMARY KEY (account_id, token_id)
);
CREATE INDEX idx_balances_token ON balances(token_id);

-- OHLCV 차트
CREATE TABLE charts (
    token_id        VARCHAR(42) NOT NULL,
    interval        VARCHAR(5) NOT NULL,        -- 1/5/15/60/240/1D
    time            BIGINT NOT NULL,            -- 캔들 시작 시간
    open            NUMERIC NOT NULL,
    high            NUMERIC NOT NULL,
    low             NUMERIC NOT NULL,
    close           NUMERIC NOT NULL,
    volume          NUMERIC NOT NULL,
    PRIMARY KEY (token_id, interval, time)
);

-- 마켓 데이터 (최신 스냅샷)
CREATE TABLE market_data (
    token_id            VARCHAR(42) PRIMARY KEY,
    market_type         VARCHAR(5) NOT NULL DEFAULT 'IDO',  -- IDO/DEX
    token_price         NUMERIC NOT NULL DEFAULT 0,
    native_price        NUMERIC NOT NULL DEFAULT 0,         -- AVAX/USD
    ath_price           NUMERIC NOT NULL DEFAULT 0,
    total_supply        NUMERIC NOT NULL,
    volume_24h          NUMERIC NOT NULL DEFAULT 0,
    holder_count        INT NOT NULL DEFAULT 0,
    bonding_percent     NUMERIC NOT NULL DEFAULT 0,         -- IDO funded %
    milestone_completed INT NOT NULL DEFAULT 0,
    milestone_total     INT NOT NULL DEFAULT 0,
    is_graduated        BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at          BIGINT NOT NULL
);

-- 홀더 스냅샷 (balances에서 집계, 캐시용)
CREATE TABLE holders (
    token_id        VARCHAR(42) NOT NULL,
    account_id      VARCHAR(42) NOT NULL,
    balance         NUMERIC NOT NULL,
    percent         NUMERIC NOT NULL,           -- % of total supply
    rank            INT NOT NULL,
    updated_at      BIGINT NOT NULL,
    PRIMARY KEY (token_id, account_id)
);
CREATE INDEX idx_holders_token_rank ON holders(token_id, rank);
```

### 8.3 Infrastructure Tables

```sql
-- LP 포지션
CREATE TABLE liquidity_positions (
    token_id        VARCHAR(42) PRIMARY KEY,
    pool_id         VARCHAR(66),                -- V4 pool ID
    tick_lower      INT,
    tick_upper      INT,
    liquidity       NUMERIC,
    created_at      BIGINT NOT NULL
);

-- 수수료 수거 기록
CREATE TABLE fee_collections (
    id              SERIAL PRIMARY KEY,
    token_id        VARCHAR(42) NOT NULL,
    amount0         NUMERIC NOT NULL,
    amount1         NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_fee_collections_token ON fee_collections(token_id);

-- 블록 인덱싱 진행 상태
CREATE TABLE block_progress (
    event_type      VARCHAR(20) PRIMARY KEY,    -- ido/token/swap/lp/price
    last_block      BIGINT NOT NULL,
    updated_at      BIGINT NOT NULL
);

-- 펀딩 추이 (빌더 대시보드용)
CREATE TABLE funding_snapshots (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    cumulative_usdc NUMERIC NOT NULL,
    investor_count  INT NOT NULL,
    snapshot_date   BIGINT NOT NULL,            -- daily unix timestamp
    UNIQUE(project_id, snapshot_date)
);
CREATE INDEX idx_funding_snapshots_project ON funding_snapshots(project_id);
```

---

## 9. Configuration & Environment

### 9.1 공통 환경변수 (shared)

```env
# Database
PRIMARY_DATABASE_URL=postgres://user:pass@host:5432/openlaunch
REPLICA_DATABASE_URL=postgres://user:pass@replica:5432/openlaunch
REDIS_URL=redis://host:6379

# PostgreSQL Pool
PG_PRIMARY_MAX_CONNECTIONS=50
PG_PRIMARY_MIN_CONNECTIONS=5
PG_REPLICA_MAX_CONNECTIONS=200
PG_REPLICA_MIN_CONNECTIONS=10

# RPC
MAIN_RPC_URL=wss://api.avax.network/ext/bc/C/ws
SUB_RPC_URL_1=wss://avax-rpc-2.example.com/ws
SUB_RPC_URL_2=wss://avax-rpc-3.example.com/ws
RPC_TIME_OUT=30000

# Contract Addresses
IDO_CONTRACT=0x...
LP_MANAGER_CONTRACT=0x...
USDC_ADDRESS=0x...

# Chain
CHAIN_ID=43114
```

### 9.2 API Server

```env
# Server
API_IP=127.0.0.1
API_PORT=8000
APP_DOMAIN=https://openlaunch.io
COOKIE_NAME=openlaunch_session

# CORS
CORS_ALLOWED_ORIGINS=https://openlaunch.io,https://app.openlaunch.io

# Session
SESSION_TTL_SECS=86400           # 24h
NONCE_TTL_SECS=300               # 5min

# Cache TTLs (seconds)
PROJECT_CACHE_TTL=10
TOKEN_LIST_CACHE_TTL=5
TREND_CACHE_TTL=30
SEARCH_CACHE_TTL=30
CHART_CACHE_TTL=1

# Rate Limiting
RATE_LIMIT_PER_MIN=60            # IP 기반

# S3/R2
S3_BUCKET=openlaunch-assets
S3_REGION=auto
S3_ENDPOINT=https://xxx.r2.cloudflarestorage.com
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
```

### 9.3 WebSocket Server

```env
# Server
WS_IP=127.0.0.1
WS_PORT=8001

# Streaming
BLOCK_BATCH_SIZE=100
BLOCK_INTERVAL=2000              # ms
DEFAULT_DELAY=500                # ms

# Channels
CHANNEL_CAPACITY=1000            # broadcast channel buffer
CHANNEL_CLEANUP_SECS=300         # 5min 무구독 시 정리
```

### 9.4 Observer

```env
# Indexing
START_BLOCK=0                    # 시작 블록 (첫 실행)
BLOCK_BATCH_SIZE=500
BLOCK_INTERVAL=1000              # ms
BLOCK_OFFSET=2                   # safety margin

# Metrics
METRICS_PORT=9090
METRICS_REPORT_INTERVAL=60       # seconds

# Retry
MAX_RETRY_ATTEMPTS=10
INITIAL_BACKOFF_MS=500
MAX_BACKOFF_MS=30000
```

### 9.5 TxBot

```env
# Wallets
GRADUATE_PRIVATE_KEY=0x...
COLLECTOR_PRIVATE_KEY=0x...

# Graduate Job
GRADUATE_MAX_RETRIES=20
GRADUATE_INITIAL_BACKOFF_MS=500
GRADUATE_MAX_BACKOFF_MS=30000

# Collect Job
COLLECT_INTERVAL_SECS=30
COLLECT_MAX_RETRIES=5
COLLECT_MIN_FEE_AMOUNT=1000000   # 최소 수거 금액 (USDC, 6 decimals)

# Metrics
METRICS_PORT=9091
HEALTH_CHECK_INTERVAL=60         # seconds
```

---

## Appendix: Contract ABI Events (Observer Reference)

```solidity
// IDO.sol events
event ProjectCreated(
    address indexed token,
    address indexed creator,
    string name,
    string symbol,
    string tokenURI,
    uint256 idoTokenAmount,
    uint256 tokenPrice,
    uint256 deadline
);

event TokensPurchased(
    address indexed token,
    address indexed buyer,
    uint256 usdcAmount,
    uint256 tokenAmount
);

event Graduated(address indexed token);

event MilestoneApproved(
    address indexed token,
    uint256 indexed milestoneIndex,
    uint256 usdcReleased
);

event ProjectFailed(address indexed token);

event Refunded(
    address indexed token,
    address indexed buyer,
    uint256 tokensBurned,
    uint256 usdcReturned
);

// LpManager.sol events
event LiquidityAllocated(
    address indexed token,
    address indexed pool,
    uint256 tokenAmount,
    int24 tickLower,
    int24 tickUpper
);

event FeesCollected(
    address indexed token,
    uint256 amount0,
    uint256 amount1
);
```
