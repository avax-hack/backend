use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes trade events to per-token channels.
pub struct TradeEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl TradeEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a trade event for the given token address.
    pub fn publish_trade(&self, token_address: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Trade(token_address.to_lowercase());
        let event = WsEvent {
            method: "trade_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to trade events for the given token address.
    pub fn subscribe(&self, token_address: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Trade(token_address.to_lowercase());
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_trade_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = TradeEventProducer::new(inner);

        let mut rx = producer.subscribe("0xABC");
        producer.publish_trade("0xabc", serde_json::json!({"type": "BUY", "amount": "100"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "trade_subscribe");
    }
}
