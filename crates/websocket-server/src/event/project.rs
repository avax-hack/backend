use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes project-level events to per-project channels.
pub struct ProjectEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl ProjectEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a project event for the given project (token) address.
    pub fn publish_project(&self, project_id: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Project(project_id.to_lowercase());
        let event = WsEvent {
            method: "project_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to project events for the given project (token) address.
    pub fn subscribe(&self, project_id: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Project(project_id.to_lowercase());
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_project_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = ProjectEventProducer::new(inner);

        let mut rx = producer.subscribe("0x123");
        producer.publish_project("0x123", serde_json::json!({"status": "graduated"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "project_subscribe");
    }
}
