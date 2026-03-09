use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes chart bar updates to per-token channels.
pub struct ChartEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl ChartEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a chart bar update for the given token address and interval.
    pub fn publish_chart(&self, token_address: &str, interval: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Chart(token_address.to_lowercase(), interval.to_string());
        let event = WsEvent {
            method: "chart_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to chart bar updates for the given token address and interval.
    pub fn subscribe(&self, token_address: &str, interval: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Chart(token_address.to_lowercase(), interval.to_string());
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_chart_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = ChartEventProducer::new(inner);

        let mut rx = producer.subscribe("0xABC", "1m");
        producer.publish_chart("0xabc", "1m", serde_json::json!({"time": 100, "close": "1.5"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "chart_subscribe");
    }
}
