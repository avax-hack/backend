pub mod dex;
pub mod ido;
pub mod pool;

use std::sync::Arc;

use crate::candle::CandleManager;
use crate::event::EventProducers;
use crate::event::core::{SubscriptionKey, WsEvent};

/// Update candles and broadcast chart updates for all intervals.
/// Shared between IDO and DEX receive handlers.
pub fn update_and_broadcast_candles(
    token_id: &str,
    price: f64,
    volume: f64,
    timestamp: i64,
    candle_mgr: &Arc<CandleManager>,
    producers: &Arc<EventProducers>,
) {
    candle_mgr.update(token_id, price, volume, timestamp);

    let token_lower = token_id.to_lowercase();
    for &(interval, _) in CandleManager::intervals() {
        if let Some(candle) = candle_mgr.get(&token_lower, interval) {
            let chart_data = serde_json::json!({
                "type": "CHART_UPDATE",
                "token_id": token_lower,
                "interval": interval,
                "o": format!("{:.18}", candle.open),
                "h": format!("{:.18}", candle.high),
                "l": format!("{:.18}", candle.low),
                "c": format!("{:.18}", candle.close),
                "v": format!("{:.2}", candle.volume),
                "t": candle.time,
            });
            let chart_key =
                SubscriptionKey::Chart(token_lower.clone(), interval.to_string())
                    .to_channel_key();
            producers.chart.publish(
                &chart_key,
                WsEvent {
                    method: "chart_subscribe".to_string(),
                    data: chart_data,
                },
            );
        }
    }
}

/// Convert an HTTP RPC URL to a WebSocket URL.
/// Handles Avalanche-style endpoints where `/rpc` → `/ws`.
pub fn rpc_url_to_ws(url: &str) -> String {
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return url.to_string();
    }
    let ws = url
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    if ws.ends_with("/rpc") {
        ws[..ws.len() - 4].to_string() + "/ws"
    } else {
        ws
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_url_to_ws() {
        assert_eq!(
            rpc_url_to_ws("https://api.avax-test.network/ext/bc/C/rpc"),
            "wss://api.avax-test.network/ext/bc/C/ws"
        );
        assert_eq!(rpc_url_to_ws("https://rpc.example.com"), "wss://rpc.example.com");
        assert_eq!(rpc_url_to_ws("http://localhost:8545"), "ws://localhost:8545");
        assert_eq!(rpc_url_to_ws("wss://already.ws"), "wss://already.ws");
    }
}
