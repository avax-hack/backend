use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes milestone events to per-project channels.
pub struct MilestoneEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl MilestoneEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a milestone event for the given project (token) address.
    pub fn publish_milestone(&self, project_id: &str, data: serde_json::Value) {
        let key = SubscriptionKey::Milestone(project_id.to_lowercase());
        let event = WsEvent {
            method: "milestone_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to milestone events for the given project (token) address.
    pub fn subscribe(&self, project_id: &str) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::Milestone(project_id.to_lowercase());
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_milestone_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = MilestoneEventProducer::new(inner);

        let mut rx = producer.subscribe("0xABC");
        producer.publish_milestone(
            "0xabc",
            serde_json::json!({"milestone_index": 1, "usdc_released": "50000"}),
        );

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "milestone_subscribe");
    }
}
