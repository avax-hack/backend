# OpenLaunch Backend — Implementation Checklist

> **Plan:** `docs/plans/2026-03-07-backend-implementation-plan.md`
> **Design:** `docs/plans/2026-03-07-backend-architecture-design.md`

---

## Phase 1: Workspace + Shared Scaffolding

- [x] **1.1** Initialize Cargo workspace (root Cargo.toml + 5 crate scaffolds)
- [x] **1.2** Shared types — Common (PaginationParams, validate_address)
- [x] **1.3** Shared types — Account (IAccountInfo)
- [x] **1.4** Shared types — Project (IProjectInfo, IProjectMarketInfo, ProjectStatus, CreateProjectRequest)
- [x] **1.5** Shared types — Milestone (IMilestoneInfo, MilestoneStatus)
- [x] **1.6** Shared types — Token & Trading (ITokenInfo, IMarketInfo, ISwapInfo, ChartBar)
- [x] **1.7** Shared types — Auth & Event (NonceRequest, SessionRequest, OnChainEvent)
- [x] **1.8** Shared Config & Error (lazy_static config, AppError enum)

## Phase 2: Shared — Database Layer

- [x] **2.1** PostgreSQL connection pool (read/write split)
- [ ] **2.2** PostgreSQL migrations (15 migration files) — IN PROGRESS
- [x] **2.3** Controller — Account (find_by_id, upsert, update)
- [x] **2.4** Controller — Project (find_by_id, find_list, update_status, validate_symbol)
- [x] **2.5** Controller — Milestone, Investment, Refund
- [x] **2.6** Controller — Swap, Balance, Chart, Market
- [x] **2.7** Controller — Block Progress (get/set last_block)
- [x] **2.8** Redis client (session, cache, rate_limit)
- [x] **2.9** Shared utils (address, price, single_flight)

## Phase 3: Shared — RPC Client

- [x] **3.1** RPC client — Multi-provider with health scoring
- [ ] **3.2** Contract ABI bindings (Alloy sol! macro) — IN PROGRESS

## Phase 4: Observer

- [ ] **4.1** Observer scaffolding (main, config, handler framework, sync managers)
- [ ] **4.2** IDO event handler (ProjectCreated, TokensPurchased, Graduated, MilestoneApproved, ProjectFailed, Refunded)
- [ ] **4.3** Token event handler (ERC20 Transfer → balance tracking)
- [ ] **4.4** Swap event handler (V4 Pool → swaps, charts, market_data)
- [ ] **4.5** LP event handler (LiquidityAllocated, FeesCollected)
- [ ] **4.6** Price handler (swap-based price calculation)
- [ ] **4.7** Observer integration test (full pipeline)

## Phase 5: API Server

- [x] **5.1** API server scaffolding (main, state, cors, router assembly)
- [ ] **5.2** Auth middleware & routes (nonce, session, delete_session)
- [ ] **5.3** Project routes (detail, featured, list, create, validate-symbol, investors)
- [ ] **5.4** Milestone routes (submit, verification status)
- [ ] **5.5** Token & Trade routes (token detail, trend, order, chart, swap-history, holder, market, metrics, quote)
- [ ] **5.6** Profile & Portfolio routes (profile, hold-token, swap-history, ido-history, refund-history, portfolio)
- [ ] **5.7** Builder routes (created tokens, overview, stats)
- [ ] **5.8** Metadata upload routes (image, evidence → S3/R2)
- [ ] **5.9** Rate limiting middleware (Redis-based, per-IP)
- [ ] **5.10** API server integration test

## Phase 6: WebSocket Server

- [ ] **6.1** WebSocket server scaffolding (main, config, HTTP+WS router)
- [ ] **6.2** Event producer framework (generic publish/subscribe with DashMap)
- [ ] **6.3** Stream handlers (IDO + Pool event streaming from RPC)
- [ ] **6.4** Trade & Price event producers
- [ ] **6.5** Project & Milestone event producers
- [ ] **6.6** NewContent event producer (global ticker feed)
- [ ] **6.7** WebSocket handler & JSON-RPC dispatcher
- [ ] **6.8** Cache manager (Redis L1 + PostgreSQL L2)

## Phase 7: TxBot

- [ ] **7.1** TxBot scaffolding (main, config, keystore, job framework)
- [ ] **7.2** Graduate job (watch IDO events → call IDO.graduate())
- [ ] **7.3** Collect fees job (poll DB → call IDO.collectFees())
- [ ] **7.4** Metrics & health monitoring

## Phase 8: Documentation & Cleanup

- [ ] **8.1** Update PRODUCT.md with backend architecture
- [ ] **8.2** Create README.md (setup, env vars, run instructions)
- [ ] **8.3** Create TEST.md (testing strategy, coverage targets)
- [ ] **8.4** Create .env.example

---

## Parallel Agent Assignment (Phase 4-7)

After Phase 1-3 complete, dispatch 4 parallel agents:

| Agent | Phase | Crate | Tasks |
|-------|-------|-------|-------|
| A | 4 | observer | 4.1 ~ 4.7 |
| B | 5 | api-server | 5.1 ~ 5.10 |
| C | 6 | websocket-server | 6.1 ~ 6.8 |
| D | 7 | txbot | 7.1 ~ 7.4 |

---

## Review Section

_(To be filled after implementation)_
