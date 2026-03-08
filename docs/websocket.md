# OpenLaunch WebSocket API Reference

> **URL:** `ws://localhost:8001/ws`
> **Protocol:** JSON-RPC 2.0 over WebSocket
> **Health Check:** `GET http://localhost:8001/health` → `{"status": "ok"}`

## Table of Contents

- [Connection](#connection)
- [JSON-RPC Format](#json-rpc-format)
- [Subscription Methods](#subscription-methods)
  - [trade_subscribe](#trade_subscribe)
  - [price_subscribe](#price_subscribe)
  - [project_subscribe](#project_subscribe)
  - [milestone_subscribe](#milestone_subscribe)
  - [new_content_subscribe](#new_content_subscribe)
- [Event Payloads](#event-payloads)
  - [Trade Events](#trade-events)
  - [Price Events](#price-events)
  - [Project Events](#project-events)
  - [Milestone Events](#milestone-events)
  - [New Content Events](#new-content-events)
- [Error Codes](#error-codes)
- [Configuration](#configuration)

---

## Connection

```
ws://127.0.0.1:8001/ws
```

| 제한 사항 | 값 |
|---|---|
| 최대 동시 연결 수 | 1,000 (환경변수로 변경 가능) |
| 최대 메시지 크기 | 16 KB |
| 아웃바운드 버퍼 | 연결당 256 메시지 |
| 브로드캐스트 채널 버퍼 | 1,024 이벤트 |

- 연결 시 자동으로 Ping/Pong 처리
- 연결 해제 시 모든 구독 자동 정리
- 동일 채널 재구독 시 기존 구독 교체
- 브로드캐스트 버퍼 초과(lag) 시 구독 자동 해제 — 재구독 필요

---

## JSON-RPC Format

### Request

```json
{
  "jsonrpc": "2.0",
  "method": "trade_subscribe",
  "params": { "token_id": "0xabc..." },
  "id": 1
}
```

### Response (성공)

```json
{
  "jsonrpc": "2.0",
  "result": { "subscribed": true },
  "id": 1
}
```

### Response (에러)

```json
{
  "jsonrpc": "2.0",
  "error": { "code": -32602, "message": "Missing required param: token_id" },
  "id": 1
}
```

### Push Notification (구독 이벤트)

```json
{
  "jsonrpc": "2.0",
  "method": "trade_subscribe",
  "params": {
    "subscription": "trade:0xabc...",
    "result": { ... }
  }
}
```

---

## Subscription Methods

### trade_subscribe

토큰별 거래 이벤트 구독 (IDO 매수, LP 할당, 수수료 수집)

```json
{
  "jsonrpc": "2.0",
  "method": "trade_subscribe",
  "params": { "token_id": "0xabc..." },
  "id": 1
}
```

| 파라미터 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `token_id` | string | Yes | 토큰 컨트랙트 주소 |

채널 키: `trade:{token_id}`
수신 이벤트: `TRADE`, `LIQUIDITY_ALLOCATED`, `FEES_COLLECTED`

---

### price_subscribe

토큰별 가격 업데이트 구독

```json
{
  "jsonrpc": "2.0",
  "method": "price_subscribe",
  "params": { "token_id": "0xabc..." },
  "id": 2
}
```

| 파라미터 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `token_id` | string | Yes | 토큰 컨트랙트 주소 |

채널 키: `price:{token_id}`
수신 이벤트: `PRICE_UPDATE`

---

### project_subscribe

프로젝트별 상태 변경 이벤트 구독

```json
{
  "jsonrpc": "2.0",
  "method": "project_subscribe",
  "params": { "project_id": "0xabc..." },
  "id": 3
}
```

| 파라미터 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `project_id` | string | Yes | 프로젝트(토큰) 주소 |

채널 키: `project:{project_id}`
수신 이벤트: `PROJECT_CREATED`, `TOKENS_PURCHASED`, `GRADUATED`, `PROJECT_FAILED`, `REFUNDED`, `MILESTONE_APPROVED`

---

### milestone_subscribe

프로젝트별 마일스톤 이벤트 구독

```json
{
  "jsonrpc": "2.0",
  "method": "milestone_subscribe",
  "params": { "project_id": "0xabc..." },
  "id": 4
}
```

| 파라미터 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `project_id` | string | Yes | 프로젝트(토큰) 주소 |

채널 키: `milestone:{project_id}`
수신 이벤트: `MILESTONE_APPROVED`

---

### new_content_subscribe

글로벌 신규 콘텐츠 이벤트 구독 (파라미터 없음)

```json
{
  "jsonrpc": "2.0",
  "method": "new_content_subscribe",
  "params": {},
  "id": 5
}
```

채널 키: `new_content`
수신 이벤트: `PROJECT_CREATED`, `GRADUATED`, `PROJECT_FAILED`, `LIQUIDITY_ALLOCATED`

---

## Event Payloads

### Trade Events

#### TRADE

IDO 토큰 매수 시 발생

```json
{
  "type": "TRADE",
  "token": "0xtoken...",
  "buyer": "0xbuyer...",
  "event_type": "BUY",
  "usdc_amount": "1000000000",
  "token_amount": "50000000000000000000"
}
```

| 필드 | 타입 | 설명 |
|---|---|---|
| `type` | string | `"TRADE"` |
| `token` | string | 토큰 주소 |
| `buyer` | string | 구매자 주소 |
| `event_type` | string | `"BUY"` |
| `usdc_amount` | string | USDC 수량 (6 decimals) |
| `token_amount` | string | 토큰 수량 (18 decimals) |

#### LIQUIDITY_ALLOCATED

LP 유동성 할당 시 발생

```json
{
  "type": "LIQUIDITY_ALLOCATED",
  "token": "0xtoken...",
  "pool_id": "0xpool...",
  "token_is_currency0": true,
  "token_amount": "1000000000000000000000",
  "tick_lower": -887220,
  "tick_upper": 887220
}
```

| 필드 | 타입 | 설명 |
|---|---|---|
| `type` | string | `"LIQUIDITY_ALLOCATED"` |
| `token` | string | 토큰 주소 |
| `pool_id` | string | Uniswap V4 pool ID |
| `token_is_currency0` | boolean | 토큰이 currency0인지 여부 |
| `token_amount` | string | 토큰 수량 (18 decimals) |
| `tick_lower` | integer | 하한 틱 |
| `tick_upper` | integer | 상한 틱 |

#### FEES_COLLECTED

LP 수수료 수집 시 발생

```json
{
  "type": "FEES_COLLECTED",
  "token": "0xtoken...",
  "amount0": "500000",
  "amount1": "1000000000000000000"
}
```

| 필드 | 타입 | 설명 |
|---|---|---|
| `type` | string | `"FEES_COLLECTED"` |
| `token` | string | 토큰 주소 |
| `amount0` | string | currency0 수수료 수량 |
| `amount1` | string | currency1 수수료 수량 |

---

### Price Events

#### PRICE_UPDATE

토큰 매수 시 가격 업데이트

```json
{
  "type": "PRICE_UPDATE",
  "token_id": "0xtoken...",
  "usdc_amount": "1000000000",
  "token_amount": "50000000000000000000",
  "price": "20000000000000.000000000000000000"
}
```

| 필드 | 타입 | 설명 |
|---|---|---|
| `type` | string | `"PRICE_UPDATE"` |
| `token_id` | string | 토큰 주소 |
| `usdc_amount` | string | USDC 수량 (6 decimals) |
| `token_amount` | string | 토큰 수량 (18 decimals) |
| `price` | string | 계산된 가격 (`usdc * 1e12 / tokens`) |

> 가격 계산: USDC(6 decimals)와 토큰(18 decimals)의 소수점 차이를 보정하기 위해 `1e12`를 곱한 후 나눕니다.

---

### Project Events

#### PROJECT_CREATED

새 IDO 프로젝트 생성 시 발생

```json
{
  "type": "PROJECT_CREATED",
  "token": "0xtoken...",
  "creator": "0xcreator...",
  "name": "MyToken",
  "symbol": "MTK",
  "token_uri": "ipfs://...",
  "ido_token_amount": "1000000000000000000000000",
  "token_price": "100000",
  "deadline": "1709856000"
}
```

#### TOKENS_PURCHASED

투자자가 IDO 토큰 매수 시 발생

```json
{
  "type": "TOKENS_PURCHASED",
  "token": "0xtoken...",
  "buyer": "0xbuyer...",
  "usdc_amount": "1000000000",
  "token_amount": "50000000000000000000"
}
```

#### GRADUATED

프로젝트가 졸업(DEX 상장) 시 발생

```json
{
  "type": "GRADUATED",
  "token": "0xtoken..."
}
```

#### PROJECT_FAILED

프로젝트 실패(데드라인 초과 등) 시 발생

```json
{
  "type": "PROJECT_FAILED",
  "token": "0xtoken..."
}
```

#### REFUNDED

투자자 환불 시 발생

```json
{
  "type": "REFUNDED",
  "token": "0xtoken...",
  "buyer": "0xbuyer...",
  "tokens_burned": "50000000000000000000",
  "usdc_returned": "1000000000"
}
```

---

### Milestone Events

#### MILESTONE_APPROVED

마일스톤 승인 시 발생

```json
{
  "type": "MILESTONE_APPROVED",
  "token": "0xtoken...",
  "milestone_index": "0",
  "usdc_released": "500000000"
}
```

| 필드 | 타입 | 설명 |
|---|---|---|
| `type` | string | `"MILESTONE_APPROVED"` |
| `token` | string | 프로젝트 토큰 주소 |
| `milestone_index` | string | 마일스톤 인덱스 (0부터 시작) |
| `usdc_released` | string | 해제된 USDC 수량 (6 decimals) |

---

### New Content Events

글로벌 채널로 주요 이벤트 브로드캐스트:

| 이벤트 | 설명 |
|---|---|
| `PROJECT_CREATED` | 새 프로젝트 생성 |
| `GRADUATED` | 프로젝트 졸업 |
| `PROJECT_FAILED` | 프로젝트 실패 |
| `LIQUIDITY_ALLOCATED` | LP 유동성 할당 |

페이로드 형식은 위 각 이벤트 섹션과 동일합니다.

---

## Error Codes

| 코드 | 메시지 | 설명 |
|---|---|---|
| `-32700` | Parse error | 잘못된 JSON |
| `-32600` | Invalid Request | 요청 형식 오류 (`jsonrpc: "2.0"` 누락, 메시지 크기 초과 >16KB 등) |
| `-32601` | Method not found | 존재하지 않는 메서드 |
| `-32602` | Invalid params | 필수 파라미터 누락 (`token_id`, `project_id`) |

---

## Channel Summary

| 채널 | 메서드 | 키 형식 | 이벤트 |
|---|---|---|---|
| Trade | `trade_subscribe` | `trade:{token_id}` | TRADE, LIQUIDITY_ALLOCATED, FEES_COLLECTED |
| Price | `price_subscribe` | `price:{token_id}` | PRICE_UPDATE |
| Project | `project_subscribe` | `project:{project_id}` | PROJECT_CREATED, TOKENS_PURCHASED, GRADUATED, PROJECT_FAILED, REFUNDED, MILESTONE_APPROVED |
| Milestone | `milestone_subscribe` | `milestone:{project_id}` | MILESTONE_APPROVED |
| New Content | `new_content_subscribe` | `new_content` | PROJECT_CREATED, GRADUATED, PROJECT_FAILED, LIQUIDITY_ALLOCATED |

---

## Event Sources

온체인 이벤트 → WebSocket 채널 매핑:

| 컨트랙트 이벤트 | 발행 채널 |
|---|---|
| IDO.ProjectCreated | `project:{token}`, `new_content` |
| IDO.TokensPurchased | `project:{token}`, `trade:{token}`, `price:{token}` |
| IDO.Graduated | `project:{token}`, `new_content` |
| IDO.MilestoneApproved | `milestone:{token}`, `project:{token}` |
| IDO.ProjectFailed | `project:{token}`, `new_content` |
| IDO.Refunded | `project:{token}` |
| LpManager.LiquidityAllocated | `trade:{token}`, `new_content` |
| LpManager.FeesCollected | `trade:{token}` |

---

## Configuration

| 환경변수 | 기본값 | 설명 |
|---|---|---|
| `WS_IP` | `127.0.0.1` | 바인드 IP |
| `WS_PORT` | `8001` | 바인드 포트 |
| `WS_MAX_CONNECTIONS` | `1000` | 최대 동시 연결 수 |
| `WS_CHANNEL_SIZE` | `1024` | 브로드캐스트 채널 버퍼 크기 |
| `WS_CLEANUP_INTERVAL_SECS` | `300` | 비활성 구독 정리 주기 (초) |

---

## Example: Full Client Interaction

```javascript
const ws = new WebSocket("ws://127.0.0.1:8001/ws");

ws.onopen = () => {
  // 1. 거래 구독
  ws.send(JSON.stringify({
    jsonrpc: "2.0",
    method: "trade_subscribe",
    params: { token_id: "0xabc123" },
    id: 1
  }));

  // 2. 가격 구독
  ws.send(JSON.stringify({
    jsonrpc: "2.0",
    method: "price_subscribe",
    params: { token_id: "0xabc123" },
    id: 2
  }));

  // 3. 글로벌 신규 콘텐츠 구독
  ws.send(JSON.stringify({
    jsonrpc: "2.0",
    method: "new_content_subscribe",
    params: {},
    id: 3
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.id) {
    // 구독 확인 응답
    console.log("Subscribed:", msg.result);
  } else if (msg.params) {
    // 푸시 이벤트
    const { subscription, result } = msg.params;
    console.log(`[${subscription}]`, result.type, result);
  }
};
```
