pub mod core;
pub mod chart;
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
    /// Remove channels with zero active subscribers.
    fn cleanup_unused(&self);
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
        // Send returns Err when there are no receivers — this is normal.
        let _ = tx.send(event);
    }

    fn subscribe(&self, key: &str) -> broadcast::Receiver<WsEvent> {
        let tx = self.get_or_create_sender(key);
        tx.subscribe()
    }

    fn cleanup_unused(&self) {
        self.channels.retain(|_, tx| tx.receiver_count() > 0);
    }
}

/// Shared collection of all event producers used across the application.
pub struct EventProducers {
    pub trade: Arc<dyn EventProducer>,
    pub price: Arc<dyn EventProducer>,
    pub chart: Arc<dyn EventProducer>,
    pub project: Arc<dyn EventProducer>,
    pub milestone: Arc<dyn EventProducer>,
    pub new_content: Arc<dyn EventProducer>,
}

impl EventProducers {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            trade: BroadcastEventProducer::new(),
            price: BroadcastEventProducer::new(),
            chart: BroadcastEventProducer::new(),
            project: BroadcastEventProducer::new(),
            milestone: BroadcastEventProducer::new(),
            new_content: BroadcastEventProducer::new(),
        })
    }

    /// Periodically remove broadcast channels with zero subscribers.
    pub fn spawn_cleanup_task(self: &Arc<Self>) {
        let producers = Arc::clone(self);
        let interval_secs = *config_local::WS_CLEANUP_INTERVAL_SECS;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(interval_secs),
            );
            loop {
                interval.tick().await;
                producers.trade.cleanup_unused();
                producers.price.cleanup_unused();
                producers.chart.cleanup_unused();
                producers.project.cleanup_unused();
                producers.milestone.cleanup_unused();
                producers.new_content.cleanup_unused();
            }
        });
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
