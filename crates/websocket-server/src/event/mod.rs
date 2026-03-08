pub mod core;
pub mod trade;
pub mod price;
pub mod project;
pub mod milestone;
pub mod new_content;

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;

use self::core::WsEvent;
use crate::config_local;

/// Trait for publishing and subscribing to WebSocket events.
pub trait EventProducer: Send + Sync {
    fn publish(&self, key: &str, event: WsEvent);
    fn subscribe(&self, key: &str) -> broadcast::Receiver<WsEvent>;
}

/// Default implementation backed by a DashMap of broadcast channels.
pub struct BroadcastEventProducer {
    channels: DashMap<String, broadcast::Sender<WsEvent>>,
}

impl BroadcastEventProducer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            channels: DashMap::new(),
        })
    }

    /// Get or create a broadcast sender for the given key.
    fn get_or_create_sender(&self, key: &str) -> broadcast::Sender<WsEvent> {
        self.channels
            .entry(key.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(*config_local::WS_CHANNEL_SIZE);
                tx
            })
            .clone()
    }
}

impl EventProducer for BroadcastEventProducer {
    fn publish(&self, key: &str, event: WsEvent) {
        let tx = self.get_or_create_sender(key);
        if let Err(e) = tx.send(event) {
            tracing::warn!(
                channel = %key,
                "Event dropped, no active subscribers for channel: {}",
                e
            );
        }
        // Bug 14 fix: Remove channels with no active subscribers to prevent unbounded growth.
        if tx.receiver_count() == 0 {
            self.channels.remove(key);
        }
    }

    fn subscribe(&self, key: &str) -> broadcast::Receiver<WsEvent> {
        let tx = self.get_or_create_sender(key);
        tx.subscribe()
    }
}

/// Shared collection of all event producers used across the application.
pub struct EventProducers {
    pub trade: Arc<dyn EventProducer>,
    pub price: Arc<dyn EventProducer>,
    pub project: Arc<dyn EventProducer>,
    pub milestone: Arc<dyn EventProducer>,
    pub new_content: Arc<dyn EventProducer>,
}

impl EventProducers {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            trade: BroadcastEventProducer::new(),
            price: BroadcastEventProducer::new(),
            project: BroadcastEventProducer::new(),
            milestone: BroadcastEventProducer::new(),
            new_content: BroadcastEventProducer::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_publish_subscribe() {
        let producer = BroadcastEventProducer::new();
        let mut rx = producer.subscribe("test:key");

        let event = WsEvent {
            method: "test".to_string(),
            data: serde_json::json!({"hello": "world"}),
        };
        producer.publish("test:key", event.clone());

        let received = rx.try_recv().unwrap();
        assert_eq!(received.method, "test");
    }

    #[test]
    fn test_publish_without_subscribers_does_not_panic() {
        let producer = BroadcastEventProducer::new();
        let event = WsEvent {
            method: "test".to_string(),
            data: serde_json::Value::Null,
        };
        // Should not panic even without subscribers.
        producer.publish("no_one_listening", event);
    }
}
