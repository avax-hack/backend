# WebSocket Chart Improvement Design

## Goal

Upgrade the WebSocket chart system to provide full OHLCV candles across 6 timeframes with in-memory candle management, resolution-based subscriptions, and DEX swap event support.

## Architecture

In-memory CandleManager holds active candles per (token_id, interval). Events from IDO and DEX streams update candles and broadcast to subscribers.

```
IDO Stream (TokensPurchased)  ─┐
                                ├→ CandleManager (in-memory, DashMap)
DEX Stream (Swap) ─────────────┘     ├─ 6 timeframes (1m, 5m, 15m, 1h, 4h, 1d)
                                     ├─ OHLCV update: H=max, L=min, C=price, V+=amount
                                     ├─ Open locked at candle creation
                                     └─ Broadcast → chart_subscribe subscribers
```

## Components

### CandleManager
- `DashMap<(String, String), Candle>` — keyed by (token_id, interval)
- On event: update all 6 timeframes
- On time bucket change: create new candle (open = previous close)
- On server start: load current active candles from DB

### Subscription Change
- `chart_subscribe` adds `resolution` parameter (e.g. "1", "5", "15", "60", "240", "1D")
- Channel key changes from `chart:{token_id}` to `chart:{token_id}:{interval}`
- SubscriptionKey::Chart holds (token_id, interval)

### DEX Stream
- New `stream/dex/` module
- Listens to Uniswap V3 Swap events (same contract as observer)
- Feeds price + volume into CandleManager

### Broadcast Format
```json
{
  "type": "CHART_UPDATE",
  "token_id": "0x...",
  "interval": "5m",
  "o": "0.001000",
  "h": "0.001500",
  "l": "0.000900",
  "c": "0.001200",
  "v": "50000.00",
  "t": 1773054300
}
```

## Decisions
- In-memory candles (no Redis) — fastest, restore from DB on restart
- 6 timeframes matching observer (1m, 5m, 15m, 1h, 4h, 1d)
- IDO + DEX events both feed chart
- Open price never changes after candle creation
