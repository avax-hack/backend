# OpenLaunch API Reference

> **Base URL:** `http://localhost:8000`
> **Auth:** Session-based (httpOnly cookie, 7-day TTL)
> **Chain:** Avalanche C-Chain (43114)
> **Swagger UI:** `http://localhost:8000/swagger-ui`

## Table of Contents

- [Auth](#auth)
  - [POST /auth/nonce](#post-authnonce)
  - [POST /auth/session](#post-authsession)
  - [DELETE /auth/delete_session](#delete-authdelete_session)
- [Project](#project)
  - [GET /project/featured](#get-projectfeatured)
  - [POST /project/create](#post-projectcreate)
  - [GET /project/validate-symbol](#get-projectvalidate-symbol)
  - [GET /project/investor/:projectId](#get-projectinvestorprojectid)
  - [GET /project/:projectId](#get-projectprojectid)
- [Milestone](#milestone)
  - [POST /milestone/submit/:milestoneId](#post-milestonesubmitmilestoneid)
  - [GET /milestone/verification/:milestoneId](#get-milestoneverificationmilestoneid)
- [Token](#token)
  - [GET /token/:tokenId](#get-tokentokenid)
  - [GET /order/:sortType](#get-ordersorttype)
  - [GET /order/project/:sortType](#get-orderprojectsorttype)
  - [GET /trend](#get-trend)
- [Trade](#trade)
  - [GET /trade/chart/:tokenAddress](#get-tradecharttokenaddress)
  - [GET /trade/swap-history/:tokenId](#get-tradeswap-historytokenid)
  - [GET /trade/holder/:tokenId](#get-tradeholdertokenid)
  - [GET /trade/market/:tokenId](#get-trademarkettokenid)
  - [GET /trade/metrics/:tokenId](#get-trademetricstokenid)
  - [GET /trade/quote/:tokenId](#get-tradequotetokenid)
- [Profile](#profile)
  - [GET /profile/:address](#get-profileaddress)
  - [GET /profile/hold-token/:accountId](#get-profilehold-tokenaccountid)
  - [GET /profile/swap-history/:accountId](#get-profileswap-historyaccountid)
  - [GET /profile/ido-history/:accountId](#get-profileido-historyaccountid)
  - [GET /profile/refund-history/:accountId](#get-profilerefund-historyaccountid)
  - [GET /profile/portfolio/:accountId](#get-profileportfolioaccountid)
  - [GET /profile/tokens/created/:accountId](#get-profiletokenscreatedaccountid)
  - [GET /account/get_account](#get-accountget_account)
- [Builder](#builder)
  - [GET /builder/overview/:projectId](#get-builderoverviewprojectid)
  - [GET /builder/stats/:projectId](#get-builderstatsprojectid)
- [Upload](#upload)
  - [POST /metadata/image](#post-metadataimage)
  - [POST /metadata/create](#post-metadatacreate)
  - [POST /metadata/evidence](#post-metadataevidence)
- [Health](#health)
  - [GET /health](#get-health)
- [Common Types](#common-types)
- [Error Response Format](#error-response-format)
- [Pagination](#pagination)
- [WebSocket](#websocket)

---

## Auth

### POST /auth/nonce

지갑 인증용 nonce 발급. EIP-4361 형식의 서명 메시지를 생성하고 Redis에 5분 TTL로 저장한다.

**Auth:** None

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| address | string | Yes | 0x-prefixed 42자 지갑 주소 |

**Example Request:**
```json
{
  "address": "0xA9cc7d43f3b5b06dE72dC8A5a4e7c9f0B1e27777"
}
```

**Response 200:**
```json
{
  "nonce": "openlaunch.io wants you to sign in with your wallet.\n\nAddress: 0xa9cc7d43f3b5b06de72dc8a5a4e7c9f0b1e27777\nNonce: 0xa9cc...7777:1709827200:550e8400-e29b-41d4-a716-446655440000\nIssued At: 2026-03-08T12:00:00+00:00"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 주소가 0x로 시작하지 않거나 42자가 아닌 경우 |

---

### POST /auth/session

서명 검증 후 세션을 생성한다. Nonce를 원자적으로 소비하여 재사용을 방지한다. 성공 시 httpOnly 쿠키(`session`)를 설정한다.

**Auth:** None

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| nonce | string | Yes | `/auth/nonce`에서 받은 전체 메시지 (1-256자) |
| signature | string | Yes | 0x-prefixed 65바이트 hex 서명 (132자) |
| chain_id | number | Yes | 체인 ID (43114 for Avalanche C-Chain) |

**Example Request:**
```json
{
  "nonce": "openlaunch.io wants you to sign in with your wallet.\n\nAddress: 0xa9cc7d43f3b5b06de72dc8a5a4e7c9f0b1e27777\nNonce: 0xa9cc...7777:1709827200:550e8400\nIssued At: 2026-03-08T12:00:00+00:00",
  "signature": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcd",
  "chain_id": 43114
}
```

**Response 200:**

Set-Cookie 헤더: `session=<uuid>; HttpOnly; Secure; Path=/; Max-Age=604800; SameSite=Lax`

```json
{
  "account_info": {
    "account_id": "0xa9cc7d43f3b5b06de72dc8a5a4e7c9f0b1e27777",
    "nickname": "",
    "bio": "",
    "image_uri": ""
  }
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 서명 형식 오류, nonce 만료/불일치, 유효하지 않은 hex |

---

### DELETE /auth/delete_session

현재 세션을 삭제하고 쿠키를 만료시킨다.

**Auth:** Session

**Response 200:**

Set-Cookie 헤더: `session=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax`

```json
{
  "success": true
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |

---

## Project

### GET /project/featured

추천 프로젝트 목록을 조회한다. `funding` 상태인 프로젝트를 funded 순으로 최대 10개 반환한다.

**Auth:** None

**Response 200:**
```json
{
  "projects": [
    {
      "project_info": {
        "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "name": "AvalancheSwap",
        "symbol": "ASWAP",
        "image_uri": "https://storage.openlaunch.io/images/aswap.png",
        "description": null,
        "tagline": "Next-gen DEX on Avalanche",
        "category": "defi",
        "creator": {
          "account_id": "0x1234567890abcdef1234567890abcdef12345678",
          "nickname": "alice_builder",
          "bio": "DeFi builder",
          "image_uri": "https://storage.openlaunch.io/images/alice.png"
        },
        "website": null,
        "twitter": null,
        "github": null,
        "telegram": null,
        "created_at": 1709827200
      },
      "market_info": {
        "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "status": "funding",
        "target_raise": "500000",
        "total_committed": "312500",
        "funded_percent": 62.5,
        "investor_count": 148
      },
      "milestone_completed": 0,
      "milestone_total": 4
    }
  ]
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### POST /project/create

새 프로젝트를 생성한다. 마일스톤 allocation 합계는 반드시 100이어야 한다.

**Auth:** Session

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | Yes | 프로젝트명 (2-50자) |
| symbol | string | Yes | 토큰 심볼 (2-10자, 대문자+숫자만) |
| tagline | string | Yes | 한 줄 소개 (5-120자) |
| description | string | Yes | 상세 설명 (20자 이상) |
| image_uri | string | Yes | 프로젝트 이미지 URI |
| website | string | No | 웹사이트 URL |
| twitter | string | No | 트위터 URL |
| github | string | No | GitHub URL |
| target_raise | string | Yes | 목표 모금액 (양수, 문자열) |
| token_supply | string | Yes | 토큰 총 공급량 (양수, 문자열) |
| milestones | array | Yes | 마일스톤 배열 (2-6개) |

**milestones 배열 항목:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| order | number | Yes | 마일스톤 순서 |
| title | string | Yes | 마일스톤 제목 (필수) |
| description | string | Yes | 마일스톤 설명 (필수) |
| fund_allocation_percent | number | Yes | 자금 배분 비율 (1-100, 합계 100) |

**Example Request:**
```json
{
  "name": "AvalancheSwap",
  "symbol": "ASWAP",
  "tagline": "Next-gen DEX on Avalanche C-Chain",
  "description": "A decentralized exchange built on Avalanche with milestone-based funding and investor protection.",
  "image_uri": "https://storage.openlaunch.io/images/aswap.png",
  "website": "https://avalancheswap.io",
  "twitter": "https://twitter.com/avalancheswap",
  "github": "https://github.com/avalancheswap",
  "target_raise": "500000",
  "token_supply": "1000000000",
  "milestones": [
    {
      "order": 1,
      "title": "MVP Launch",
      "description": "Core swap functionality with liquidity pools",
      "fund_allocation_percent": 30
    },
    {
      "order": 2,
      "title": "Beta Release",
      "description": "Limit orders, advanced charting, and mobile app",
      "fund_allocation_percent": 35
    },
    {
      "order": 3,
      "title": "Mainnet & Governance",
      "description": "Full mainnet launch with DAO governance",
      "fund_allocation_percent": 35
    }
  ]
}
```

**Response 200:**
```json
{
  "project_id": "0x550e8400e29b41d4a716446655440000abcdef12"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 유효성 검증 실패 (이름/심볼 길이, 마일스톤 합계 등) |
| 400 | BAD_REQUEST | 심볼이 이미 사용 중 |
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |

---

### GET /project/validate-symbol

토큰 심볼의 사용 가능 여부를 확인한다.

**Auth:** None

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| symbol | string | Yes | - | 검증할 토큰 심볼 |

**Example Request:**
```
GET /project/validate-symbol?symbol=ASWAP
```

**Response 200:**
```json
{
  "available": true
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /project/investor/:projectId

프로젝트 투자자 목록을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| projectId | string | 프로젝트 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 (max 100) |

**Example Request:**
```
GET /project/investor/0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c?page=1&limit=10
```

**Response 200:**
```json
{
  "data": [
    {
      "account_info": {
        "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "nickname": "whale_investor",
        "bio": "Early-stage crypto investor",
        "image_uri": "https://storage.openlaunch.io/images/whale.png"
      },
      "usdc_amount": "50000",
      "created_at": 1709913600
    }
  ],
  "total_count": 148
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /project/:projectId

프로젝트 상세 정보를 조회한다. 프로젝트 기본 정보, 마켓 정보, 마일스톤 목록을 포함한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| projectId | string | 프로젝트 ID |

**Example Request:**
```
GET /project/0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c
```

**Response 200:**
```json
{
  "project_info": {
    "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
    "name": "AvalancheSwap",
    "symbol": "ASWAP",
    "image_uri": "https://storage.openlaunch.io/images/aswap.png",
    "description": "A decentralized exchange built on Avalanche with milestone-based funding.",
    "tagline": "Next-gen DEX on Avalanche",
    "category": "defi",
    "creator": {
      "account_id": "0x1234567890abcdef1234567890abcdef12345678",
      "nickname": "alice_builder",
      "bio": "DeFi builder",
      "image_uri": "https://storage.openlaunch.io/images/alice.png"
    },
    "website": "https://avalancheswap.io",
    "twitter": "https://twitter.com/avalancheswap",
    "github": "https://github.com/avalancheswap",
    "telegram": null,
    "created_at": 1709827200
  },
  "market_info": {
    "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
    "status": "funding",
    "target_raise": "500000",
    "total_committed": "312500",
    "funded_percent": 62.5,
    "investor_count": 148
  },
  "milestones": [
    {
      "milestone_id": "ms_001",
      "order": 1,
      "title": "MVP Launch",
      "description": "Core swap functionality with liquidity pools",
      "fund_allocation_percent": 30,
      "fund_release_amount": "150000",
      "status": "completed",
      "funds_released": true,
      "evidence_uri": "https://storage.openlaunch.io/evidence/mvp_report.pdf",
      "submitted_at": 1711065600,
      "verified_at": 1711152000
    },
    {
      "milestone_id": "ms_002",
      "order": 2,
      "title": "Beta Release",
      "description": "Limit orders, advanced charting, and mobile app",
      "fund_allocation_percent": 35,
      "fund_release_amount": "175000",
      "status": "pending",
      "funds_released": false,
      "evidence_uri": null,
      "submitted_at": null,
      "verified_at": null
    }
  ]
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 프로젝트를 찾을 수 없음 |

---

## Milestone

### POST /milestone/submit/:milestoneId

마일스톤 증빙 자료를 제출한다.

> **TODO:** 현재 세션의 account_id가 해당 마일스톤의 프로젝트 creator인지 검증하는 로직이 구현 예정이다.

**Auth:** Session

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| milestoneId | string | 마일스톤 ID |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| evidence_text | string | Yes | 증빙 설명 텍스트 |
| evidence_uri | string | No | 증빙 파일 URI |

**Example Request:**
```json
{
  "evidence_text": "MVP 개발 완료. 스왑 기능, 유동성 풀 생성, 슬리피지 보호 구현 완료.",
  "evidence_uri": "https://storage.openlaunch.io/evidence/mvp_report.pdf"
}
```

**Response 200:**
```json
{
  "success": true
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /milestone/verification/:milestoneId

마일스톤 검증 상태를 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| milestoneId | string | 마일스톤 ID |

**Response 200:**
```json
{
  "milestone_id": "ms_001",
  "status": "in_verification",
  "submitted_at": 1711065600,
  "estimated_completion": 1711324800,
  "dispute_info": null
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

## Token

### GET /token/:tokenId

토큰 상세 정보를 조회한다. 토큰 기본 정보와 마켓 데이터를 포함한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID (컨트랙트 주소) |

**Example Request:**
```
GET /token/0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c
```

**Response 200:**
```json
{
  "token_info": {
    "token_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
    "name": "AvalancheSwap",
    "symbol": "ASWAP",
    "image_uri": "https://storage.openlaunch.io/images/aswap.png",
    "banner_uri": null,
    "description": "Next-gen DEX on Avalanche",
    "category": "defi",
    "is_graduated": false,
    "creator": {
      "account_id": "0x1234567890abcdef1234567890abcdef12345678",
      "nickname": "alice_builder",
      "bio": "",
      "image_uri": ""
    },
    "website": "https://avalancheswap.io",
    "twitter": "https://twitter.com/avalancheswap",
    "telegram": null,
    "created_at": 1709827200,
    "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c"
  },
  "market_info": {
    "market_type": "CURVE",
    "token_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
    "token_price": "0.0254",
    "native_price": "0.000892",
    "price": "0.0254",
    "ath_price": "0.0512",
    "total_supply": "1000000000",
    "volume": "182450",
    "holder_count": 342,
    "bonding_percent": 67.3,
    "milestone_completed": 1,
    "milestone_total": 3
  }
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 토큰을 찾을 수 없음 |

---

### GET /order/:sortType

토큰 목록을 정렬 기준에 따라 페이지네이션으로 조회한다. 카테고리, 검증 여부, 검색어 필터를 지원한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| sortType | string | 정렬 기준 (예: `newest`, `volume`, `price`) |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |
| category | string | No | - | 카테고리 필터 (예: `defi`, `gaming`) |
| verified_only | boolean | No | false | 검증된 토큰만 필터 |
| search | string | No | - | 이름/심볼 검색어 |
| is_ido | boolean | No | - | IDO 상태 필터: `true`=펀딩 중(졸업 전), `false`=졸업 완료 |

**Example Request:**
```
GET /order/volume?page=1&limit=10&category=defi&is_ido=true
```

**Response 200:**
```json
{
  "data": [
    {
      "token_info": { "..." : "ITokenInfo" },
      "market_info": { "..." : "IMarketInfo" }
    }
  ],
  "total_count": 45
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /order/project/:sortType

프로젝트 목록을 정렬 기준에 따라 페이지네이션으로 조회한다. 상태 필터를 지원한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| sortType | string | 정렬 기준 (예: `funded`, `newest`, `investors`) |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |
| status | string | No | - | 프로젝트 상태 필터 (`funding`, `active`, `completed`, `failed`) |

**Example Request:**
```
GET /order/project/funded?page=1&limit=10&status=funding
```

**Response 200:**
```json
{
  "data": [
    {
      "project_info": {
        "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "name": "AvalancheSwap",
        "symbol": "ASWAP",
        "image_uri": "https://storage.openlaunch.io/images/aswap.png",
        "description": null,
        "tagline": "Next-gen DEX on Avalanche",
        "category": "defi",
        "creator": { "account_id": "0x1234...5678", "nickname": "alice_builder", "bio": "", "image_uri": "" },
        "website": null,
        "twitter": null,
        "github": null,
        "telegram": null,
        "created_at": 1709827200
      },
      "market_info": {
        "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "status": "funding",
        "target_raise": "500000",
        "total_committed": "312500",
        "funded_percent": 62.5,
        "investor_count": 148
      },
      "milestone_completed": 0,
      "milestone_total": 4
    }
  ],
  "total_count": 23
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /trend

트렌딩 토큰 목록을 조회한다.

**Auth:** None

**Response 200:**
```json
[
  {
    "token_info": {
      "token_id": "0xabc123def456789abc123def456789abc123def4",
      "name": "MoonToken",
      "symbol": "MOON",
      "image_uri": "https://storage.openlaunch.io/images/moon.png",
      "banner_uri": null,
      "description": "To the moon",
      "category": "meme",
      "is_graduated": false,
      "creator": { "account_id": "0xcreator...", "nickname": "", "bio": "", "image_uri": "" },
      "website": null,
      "twitter": null,
      "telegram": null,
      "created_at": 1709913600,
      "project_id": null
    },
    "market_info": {
      "market_type": "CURVE",
      "token_id": "0xabc123def456789abc123def456789abc123def4",
      "token_price": "0.00087",
      "native_price": "0.0000305",
      "price": "0.00087",
      "ath_price": "0.0015",
      "total_supply": "1000000000",
      "volume": "94200",
      "holder_count": 1203,
      "bonding_percent": 42.1,
      "milestone_completed": 0,
      "milestone_total": 0
    }
  }
]
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

## Trade

### GET /trade/chart/:tokenAddress

토큰의 OHLCV 차트 데이터를 조회한다. TradingView 호환 형식이다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenAddress | string | 토큰 컨트랙트 주소 |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| resolution | string | Yes | - | 캔들 간격 (`1`, `5`, `15`, `60`, `240`, `D`, `W` 또는 `1m`, `5m`, `15m`, `1h`, `4h`, `1d`, `1w`) |
| from | number | Yes | - | 시작 Unix timestamp |
| to | number | Yes | - | 종료 Unix timestamp |
| countback | number | No | 300 | 최대 캔들 수 |
| chart_type | string | No | "price" | 차트 유형 |

**Example Request:**
```
GET /trade/chart/0x7a3b9c2e1f4d5a6b?resolution=1h&from=1709827200&to=1709913600&countback=100
```

**Response 200:**
```json
{
  "bars": [
    {
      "time": 1709827200,
      "open": "0.0250",
      "high": "0.0265",
      "low": "0.0248",
      "close": "0.0260",
      "volume": "45200"
    },
    {
      "time": 1709830800,
      "open": "0.0260",
      "high": "0.0272",
      "low": "0.0255",
      "close": "0.0268",
      "volume": "38900"
    }
  ]
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /trade/swap-history/:tokenId

토큰의 스왑 거래 내역을 페이지네이션으로 조회한다. 방향 정렬과 거래 유형 필터를 지원한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |
| direction | string | No | "DESC" | 정렬 방향 (`ASC` 또는 `DESC`) |
| trade_type | string | No | - | 거래 유형 필터 (`BUY`, `SELL`, 또는 생략 시 전체) |

**Example Request:**
```
GET /trade/swap-history/0x7a3b9c2e?page=1&limit=10&direction=DESC&trade_type=BUY
```

**Response 200:**
```json
{
  "data": [
    {
      "event_type": "BUY",
      "native_amount": "1500000000000000000",
      "token_amount": "59055118110236220000",
      "native_price": "28.50",
      "transaction_hash": "0xabc123def456789abc123def456789abc123def456789abc123def456789abc1",
      "value": "42.75",
      "account_info": {
        "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "nickname": "trader_bob",
        "bio": "",
        "image_uri": ""
      },
      "created_at": 1709913600
    }
  ],
  "total_count": 1247
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /trade/holder/:tokenId

토큰 보유자 목록을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Example Request:**
```
GET /trade/holder/0x7a3b9c2e?page=1&limit=10
```

**Response 200:**
```json
{
  "data": [
    {
      "account_info": {
        "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "nickname": "top_holder",
        "bio": "",
        "image_uri": ""
      },
      "balance": "15000000000000000000000"
    }
  ],
  "total_count": 342
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /trade/market/:tokenId

토큰의 시장 데이터를 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID |

**Response 200:**
```json
{
  "market_type": "CURVE",
  "token_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
  "token_price": "0.0254",
  "native_price": "0.000892",
  "price": "0.0254",
  "ath_price": "0.0512",
  "total_supply": "1000000000",
  "volume": "182450",
  "holder_count": 342,
  "bonding_percent": 67.3,
  "milestone_completed": 1,
  "milestone_total": 3
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 토큰의 마켓 데이터를 찾을 수 없음 |

---

### GET /trade/metrics/:tokenId

토큰의 타임프레임별 메트릭(가격 변동, 거래량, 거래 수)을 조회한다.

> **TODO:** 현재 placeholder 데이터를 반환한다. 실제 차트/스왑 데이터 기반 계산이 구현 예정이다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID |

**Response 200:**
```json
{
  "metrics": {
    "5m": {
      "price_change": "0",
      "volume": "182450",
      "trades": 0
    },
    "1h": {
      "price_change": "0",
      "volume": "182450",
      "trades": 0
    },
    "6h": {
      "price_change": "0",
      "volume": "182450",
      "trades": 0
    },
    "24h": {
      "price_change": "0",
      "volume": "182450",
      "trades": 0
    }
  }
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /trade/quote/:tokenId

토큰의 스왑 견적을 조회한다. 예상 출력량, 가격 영향, 최소 수령량, 수수료를 반환한다.

> **TODO:** 현재 placeholder 데이터를 반환한다. 실제 AMM 수학이 구현 예정이다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| tokenId | string | 토큰 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| amount | string | No | "" | 스왑할 금액 |
| type | string | No | "" | 거래 유형 (`BUY` 또는 `SELL`, 기본값은 BUY) |
| slippage | number | No | 3.0 | 슬리피지 허용률 (%) |

**Example Request:**
```
GET /trade/quote/0x7a3b9c2e?amount=1000&type=BUY&slippage=1.5
```

**Response 200:**
```json
{
  "expected_output": "0",
  "price_impact_percent": "0",
  "minimum_received": "0",
  "fee": "0"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

## Profile

### GET /profile/:address

계정 프로필 정보를 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| address | string | 지갑 주소 |

**Response 200:**
```json
{
  "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
  "nickname": "alice_builder",
  "bio": "DeFi builder and Avalanche enthusiast",
  "image_uri": "https://storage.openlaunch.io/images/alice.png"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 계정을 찾을 수 없음 |

---

### GET /profile/hold-token/:accountId

계정이 보유한 토큰 목록을 페이지네이션으로 조회한다. 각 토큰의 마켓 정보, 잔액, 마일스톤 진행 상황을 포함한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID (지갑 주소) |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Response 200:**
```json
{
  "data": [
    {
      "token_info": {
        "token_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "name": "AvalancheSwap",
        "symbol": "ASWAP",
        "image_uri": "https://storage.openlaunch.io/images/aswap.png",
        "banner_uri": null,
        "description": "Next-gen DEX",
        "category": "defi",
        "is_graduated": false,
        "creator": { "account_id": "0x1234...5678", "nickname": "", "bio": "", "image_uri": "" },
        "website": null,
        "twitter": null,
        "telegram": null,
        "created_at": 1709827200,
        "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c"
      },
      "market_info": {
        "market_type": "CURVE",
        "token_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
        "token_price": "0.0254",
        "native_price": "0.000892",
        "price": "0.0254",
        "ath_price": "0.0512",
        "total_supply": "1000000000",
        "volume": "182450",
        "holder_count": 342,
        "bonding_percent": 67.3,
        "milestone_completed": 1,
        "milestone_total": 3
      },
      "balance_info": {
        "balance": "12500000000000000000000",
        "token_price": "0.0254",
        "native_price": "0.000892",
        "created_at": 1709913600
      },
      "origin": "ido",
      "milestone_progress": {
        "completed": 1,
        "total": 3
      }
    }
  ],
  "total_count": 5
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /profile/swap-history/:accountId

계정의 스왑 거래 내역을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Response 200:**
```json
{
  "data": [
    {
      "event_type": "SELL",
      "native_amount": "500000000000000000",
      "token_amount": "20000000000000000000",
      "native_price": "28.50",
      "transaction_hash": "0xdef789abc123def456789abc123def456789abc123def456789abc123def789a",
      "value": "14.25",
      "account_info": {
        "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "nickname": "alice_builder",
        "bio": "",
        "image_uri": ""
      },
      "created_at": 1709900000
    }
  ],
  "total_count": 87
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /profile/ido-history/:accountId

계정의 IDO 참여 내역을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Response 200:**
```json
{
  "data": [
    {
      "project_info": {
        "project_id": "0x7a3b...8b9c",
        "name": "AvalancheSwap",
        "symbol": "ASWAP",
        "image_uri": "https://storage.openlaunch.io/images/aswap.png",
        "description": null,
        "tagline": "Next-gen DEX on Avalanche",
        "category": "defi",
        "creator": { "account_id": "0x1234...5678", "nickname": "", "bio": "", "image_uri": "" },
        "website": null,
        "twitter": null,
        "github": null,
        "telegram": null,
        "created_at": 1709827200
      },
      "market_info": {
        "project_id": "0x7a3b...8b9c",
        "status": "active",
        "target_raise": "500000",
        "total_committed": "500000",
        "funded_percent": 100.0,
        "investor_count": 210
      },
      "invested_amount": "5000",
      "tokens_received": "200000000000000000000000",
      "status": "active",
      "milestone_progress": {
        "completed": 1,
        "total": 3
      },
      "created_at": 1709870400
    }
  ],
  "total_count": 3
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /profile/refund-history/:accountId

계정의 환불 내역을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Response 200:**
```json
{
  "data": [
    {
      "project_info": {
        "project_id": "0xfailed...project",
        "name": "FailedProject",
        "symbol": "FAIL",
        "image_uri": "https://storage.openlaunch.io/images/fail.png",
        "description": null,
        "tagline": "This project failed",
        "category": "gaming",
        "creator": { "account_id": "0xbad...creator", "nickname": "", "bio": "", "image_uri": "" },
        "website": null,
        "twitter": null,
        "github": null,
        "telegram": null,
        "created_at": 1708963200
      },
      "market_info": {
        "project_id": "0xfailed...project",
        "status": "failed",
        "target_raise": "200000",
        "total_committed": "150000",
        "funded_percent": 75.0,
        "investor_count": 85
      },
      "original_investment": "3000",
      "refund_amount": "2250",
      "tokens_burned": "100000000000000000000000",
      "failed_milestone": "Beta Launch",
      "transaction_hash": "0xrefund_tx_hash_abc123def456789abc123def456789abc123def456789abc12",
      "created_at": 1710460800
    }
  ],
  "total_count": 1
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /profile/portfolio/:accountId

계정의 포트폴리오 요약 정보를 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID |

**Response 200:**
```json
{
  "portfolio_value": "18720.50",
  "total_invested_ido": "12450",
  "trading_pnl": "0",
  "trading_pnl_percent": 0.0,
  "active_idos": 3,
  "refunds_received": "2250"
}
```

> `trading_pnl`과 `trading_pnl_percent`는 MVP에서 placeholder (`"0"`, `0.0`)를 반환한다.

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /profile/tokens/created/:accountId

계정이 생성한 프로젝트/토큰 목록을 페이지네이션으로 조회한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| accountId | string | 계정 ID |

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| page | number | No | 1 | 페이지 번호 |
| limit | number | No | 20 | 페이지당 항목 수 |

**Response 200:**
```json
{
  "data": [
    {
      "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
      "name": "AvalancheSwap",
      "symbol": "ASWAP",
      "image_uri": "https://storage.openlaunch.io/images/aswap.png",
      "status": "funding",
      "created_at": 1709827200
    }
  ],
  "total_count": 2
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 500 | INTERNAL_ERROR | 서버 내부 오류 |

---

### GET /account/get_account

현재 로그인된 사용자의 계정 정보를 조회한다.

**Auth:** Session

**Response 200:**
```json
{
  "account_id": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
  "nickname": "alice_builder",
  "bio": "DeFi builder and Avalanche enthusiast",
  "image_uri": "https://storage.openlaunch.io/images/alice.png"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |
| 404 | NOT_FOUND | 계정을 찾을 수 없음 |

---

## Builder

### GET /builder/overview/:projectId

프로젝트 빌더 대시보드 개요를 조회한다. 프로젝트 요약, 마일스톤 목록, 현재 진행 중인 마일스톤 정보를 포함한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| projectId | string | 프로젝트 ID |

**Response 200:**
```json
{
  "project_id": "0x7a3b9c2e1f4d5a6b8c0e9f1a2b3c4d5e6f7a8b9c",
  "name": "AvalancheSwap",
  "symbol": "ASWAP",
  "image_uri": "https://storage.openlaunch.io/images/aswap.png",
  "status": "active",
  "target_raise": "500000",
  "usdc_raised": "500000",
  "investor_count": 210,
  "milestones": [
    {
      "milestone_id": "ms_001",
      "order": 1,
      "title": "MVP Launch",
      "description": "Core swap functionality",
      "fund_allocation_percent": 30,
      "fund_release_amount": "150000",
      "status": "completed",
      "funds_released": true,
      "evidence_uri": "https://storage.openlaunch.io/evidence/mvp.pdf",
      "submitted_at": 1711065600,
      "verified_at": 1711152000
    },
    {
      "milestone_id": "ms_002",
      "order": 2,
      "title": "Beta Release",
      "description": "Limit orders and mobile app",
      "fund_allocation_percent": 35,
      "fund_release_amount": "175000",
      "status": "in_verification",
      "funds_released": false,
      "evidence_uri": "https://storage.openlaunch.io/evidence/beta.pdf",
      "submitted_at": 1712275200,
      "verified_at": null
    }
  ],
  "current_milestone": {
    "order": 2,
    "title": "Beta Release",
    "status": "in_verification"
  },
  "total_milestones": 3,
  "created_at": 1709827200
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 프로젝트를 찾을 수 없음 |

---

### GET /builder/stats/:projectId

프로젝트 빌더 통계를 조회한다. 누적 펀딩 및 투자자 추이 데이터를 포함한다.

**Auth:** None

**Path Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| projectId | string | 프로젝트 ID |

**Response 200:**
```json
{
  "total_raised": "500000",
  "total_investors": 210,
  "milestones_completed": 1,
  "milestones_total": 3,
  "funds_released": "150000",
  "funding_over_time": [
    { "date": 1709769600, "cumulative": "125000" },
    { "date": 1709856000, "cumulative": "287500" },
    { "date": 1709942400, "cumulative": "500000" }
  ],
  "investors_over_time": [
    { "date": 1709769600, "count": 45 },
    { "date": 1709856000, "count": 132 },
    { "date": 1709942400, "count": 210 }
  ]
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 404 | NOT_FOUND | 프로젝트를 찾을 수 없음 |

---

## Upload

### POST /metadata/image

프로젝트 이미지를 Cloudflare R2에 업로드한다.

**Auth:** Session

**Content-Type:** `multipart/form-data`

**Form Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| file | binary | Yes | 이미지 파일 |

**Validation:**
- Content-Type: `image/png`, `image/jpeg`, `image/webp`, `image/gif` only
- File size: 5MB max
- Magic bytes verification (실제 파일 타입 검증)

**Response 200:**
```json
{
  "image_uri": "https://pub-f5d8da8e313244248f626b7d5dc6610d.r2.dev/{uuid}.png"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 지원하지 않는 파일 형식, 5MB 초과, 파일 비어있음 |
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |

---

### POST /metadata/create

메타데이터 JSON을 생성하여 Cloudflare R2에 업로드한다.

**Auth:** Session

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | Yes | 토큰 이름 (2-50자) |
| symbol | string | Yes | 토큰 심볼 (2-10자, 대문자+숫자만) |
| image_uri | string | Yes | R2 이미지 URL (`/metadata/image`로 업로드한 URI) |
| category | string | Yes | 카테고리 (1-50자, 자유 텍스트) |
| homepage | string | No | 홈페이지 URL (`https://`로 시작) |
| twitter | string | No | 트위터 URL (`https://`로 시작) |
| telegram | string | No | 텔레그램 URL (`https://`로 시작) |
| discord | string | No | 디스코드 URL (`https://`로 시작) |
| milestones | array | Yes | 마일스톤 목록 (2-6개, 합계 100%) |

**Milestone Object:**

| Field | Type | Description |
|-------|------|-------------|
| order | number | 순서 |
| title | string | 제목 (필수) |
| description | string | 설명 (필수) |
| fund_allocation_percent | number | 자금 배분 비율 (1-100, 합계 100) |

**Example Request:**
```json
{
  "name": "MyToken",
  "symbol": "MTK",
  "image_uri": "https://pub-f5d8da8e313244248f626b7d5dc6610d.r2.dev/abc.png",
  "category": "DeFi",
  "homepage": "https://mytoken.io",
  "milestones": [
    { "order": 1, "title": "MVP", "description": "Build MVP", "fund_allocation_percent": 50 },
    { "order": 2, "title": "Launch", "description": "Ship", "fund_allocation_percent": 50 }
  ]
}
```

**Response 200:**
```json
{
  "metadata_uri": "https://pub-aea7c48b8fdb4309ad12ff1799b80216.r2.dev/{uuid}.json"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 유효성 검사 실패 (필드 길이, 심볼 형식, image_uri 검증, 마일스톤 합계 등) |
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |

---

### POST /metadata/evidence

마일스톤 증빙 파일을 업로드한다.

**Auth:** Session

**Content-Type:** `multipart/form-data`

**Form Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| file | binary | Yes | 증빙 파일 (비어있으면 안 됨) |

**Response 200:**
```json
{
  "uri": "https://storage.openlaunch.io/evidence/milestone-mvp-report.pdf"
}
```

**Errors:**

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 파일이 비어있거나, multipart에 file 필드가 없는 경우 |
| 401 | UNAUTHORIZED | 인증되지 않은 요청 |

---

## Health

### GET /health

서버 헬스체크 엔드포인트.

**Auth:** None

**Response 200:**
```
OK
```

(plain text, not JSON)

---

## Common Types

### IAccountInfo

사용자 계정 정보.

```json
{
  "account_id": "string  // 0x-prefixed 지갑 주소",
  "nickname": "string    // 사용자 닉네임",
  "bio": "string         // 자기소개",
  "image_uri": "string   // 프로필 이미지 URI"
}
```

### IProjectInfo

프로젝트 기본 정보.

```json
{
  "project_id": "string",
  "name": "string",
  "symbol": "string",
  "image_uri": "string",
  "description": "string | null",
  "tagline": "string",
  "category": "string",
  "creator": "IAccountInfo",
  "website": "string | null",
  "twitter": "string | null",
  "github": "string | null",
  "telegram": "string | null",
  "created_at": "number  // Unix timestamp"
}
```

### IProjectMarketInfo

프로젝트 마켓/펀딩 정보.

```json
{
  "project_id": "string",
  "status": "\"funding\" | \"active\" | \"completed\" | \"failed\"",
  "target_raise": "string",
  "total_committed": "string",
  "funded_percent": "number  // 0-100",
  "investor_count": "number"
}
```

### ITokenInfo

토큰 기본 정보.

```json
{
  "token_id": "string",
  "name": "string",
  "symbol": "string",
  "image_uri": "string",
  "banner_uri": "string | null",
  "description": "string | null",
  "category": "string",
  "is_graduated": "boolean",
  "creator": "IAccountInfo",
  "website": "string | null",
  "twitter": "string | null",
  "telegram": "string | null",
  "created_at": "number",
  "project_id": "string | null"
}
```

### IMarketInfo

토큰 시장 데이터.

```json
{
  "market_type": "\"CURVE\" | \"DEX\" | \"IDO\"",
  "token_id": "string",
  "token_price": "string",
  "native_price": "string",
  "price": "string",
  "ath_price": "string",
  "total_supply": "string",
  "volume": "string       // 24h 거래량",
  "holder_count": "number",
  "bonding_percent": "number  // 0-100",
  "milestone_completed": "number",
  "milestone_total": "number"
}
```

### IMilestoneInfo

마일스톤 상세 정보.

```json
{
  "milestone_id": "string",
  "order": "number",
  "title": "string",
  "description": "string",
  "fund_allocation_percent": "number  // 1-100",
  "fund_release_amount": "string",
  "status": "\"completed\" | \"in_verification\" | \"submitted\" | \"pending\" | \"failed\"",
  "funds_released": "boolean",
  "evidence_uri": "string | null",
  "submitted_at": "number | null  // Unix timestamp",
  "verified_at": "number | null   // Unix timestamp"
}
```

### ISwapInfo

스왑 거래 정보.

```json
{
  "event_type": "\"BUY\" | \"SELL\"",
  "native_amount": "string",
  "token_amount": "string",
  "native_price": "string",
  "transaction_hash": "string",
  "value": "string",
  "account_info": "IAccountInfo",
  "created_at": "number"
}
```

### TradeQuote

스왑 견적 정보.

```json
{
  "expected_output": "string",
  "price_impact_percent": "string",
  "minimum_received": "string",
  "fee": "string"
}
```

### ChartBar

OHLCV 캔들 데이터.

```json
{
  "time": "number   // Unix timestamp",
  "open": "string",
  "high": "string",
  "low": "string",
  "close": "string",
  "volume": "string"
}
```

### PortfolioSummary

포트폴리오 요약.

```json
{
  "portfolio_value": "string",
  "total_invested_ido": "string",
  "trading_pnl": "string",
  "trading_pnl_percent": "number",
  "active_idos": "number",
  "refunds_received": "string"
}
```

---

## Error Response Format

모든 에러 응답은 다음 JSON 형식을 따른다:

```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE"
}
```

### Error Codes

| Status | Code | Description |
|--------|------|-------------|
| 400 | BAD_REQUEST | 잘못된 요청 (유효성 검증 실패) |
| 401 | UNAUTHORIZED | 인증 필요 (세션 없음/만료) |
| 403 | FORBIDDEN | 접근 권한 없음 |
| 404 | NOT_FOUND | 리소스를 찾을 수 없음 |
| 409 | CONFLICT | 리소스 충돌 |
| 429 | TOO_MANY_REQUESTS | 요청 한도 초과 |
| 500 | INTERNAL_ERROR | 서버 내부 오류 (상세 메시지는 숨겨짐) |

### Rate Limiting (429)

429 응답에는 `Retry-After` 헤더와 추가 필드가 포함된다:

```json
{
  "error": "Too many requests",
  "code": "TOO_MANY_REQUESTS",
  "retry_after": 30
}
```

> **보안 참고:** `INTERNAL_ERROR` 응답은 실제 에러 메시지를 노출하지 않고 항상 `"Internal server error"`를 반환한다. 상세 에러는 서버 로그에만 기록된다.

---

## Pagination

페이지네이션이 적용된 엔드포인트는 공통 쿼리 파라미터와 응답 형식을 사용한다.

### Query Parameters

| Param | Type | Default | Constraints | Description |
|-------|------|---------|-------------|-------------|
| page | number | 1 | min 1 | 페이지 번호 |
| limit | number | 20 | 1-100 | 페이지당 항목 수 |

### Response Format

```json
{
  "data": [],
  "total_count": 0
}
```

- `data`: 현재 페이지의 항목 배열
- `total_count`: 전체 항목 수

### Offset 계산

```
offset = (page - 1) * limit
```

유효하지 않은 값은 자동 보정된다: `page`는 최소 1, `limit`는 1-100으로 clamp된다.

---

## WebSocket

> **TODO:** WebSocket 엔드포인트(`/ws`)는 현재 구현되지 않았다. 향후 다음 기능이 추가될 예정이다.

### 예정된 Subscribe Methods

| Method | Description |
|--------|-------------|
| `subscribe_token` | 특정 토큰의 실시간 가격/거래 업데이트 구독 |
| `subscribe_project` | 프로젝트 펀딩 진행 상황 실시간 업데이트 구독 |
| `subscribe_milestone` | 마일스톤 상태 변경 알림 구독 |

### 예정된 Push Event Format

```json
{
  "type": "trade",
  "data": {
    "token_id": "0x...",
    "event_type": "BUY",
    "native_amount": "1000000000000000000",
    "token_amount": "50000000000000000000",
    "native_price": "28.50",
    "timestamp": 1709913600
  }
}
```

```json
{
  "type": "price_update",
  "data": {
    "token_id": "0x...",
    "price": "0.0256",
    "volume_24h": "185000",
    "timestamp": 1709913600
  }
}
```

```json
{
  "type": "milestone_update",
  "data": {
    "project_id": "0x...",
    "milestone_id": "ms_002",
    "status": "completed",
    "timestamp": 1709913600
  }
}
```
