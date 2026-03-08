use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes price update events to per-token channels.
pub struct PriceEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl PriceEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a price update for the given token address.
    pub fn publish_price(&self, token_address: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Price(token_address.to_lowercase());
        let event = WsEvent {
            method: "price_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to price events for the given token address.
    pub fn subscribe(&self, token_address: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Price(token_address.to_lowercase());
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_price_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = PriceEventProducer::new(inner);

        let mut rx = producer.subscribe("0xDEF");
        producer.publish_price("0xdef", serde_json::json!({"price": "0.025"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "price_subscribe");
    }
}
