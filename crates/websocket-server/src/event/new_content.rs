use std::sync::Arc;

use crate::event::core::{SubscriptionKey, WsEvent};
use crate::event::EventProducer;

/// Publishes global new-content events (new projects, graduations, etc.).
pub struct NewContentEventProducer {
    inner: Arc<dyn EventProducer>,
}

impl NewContentEventProducer {
    pub fn new(inner: Arc<dyn EventProducer>) -> Self {
        Self { inner }
    }

    /// Publish a new content event to the global channel.
    pub fn publish_new_content(&self, data: serde_json::Value) {
        let key = SubscriptionKey::NewContent;
        let event = WsEvent {
            method: "new_content_subscribe".to_string(),
            data,
        };
        self.inner.publish(&key.to_channel_key(), event);
    }

    /// Subscribe to the global new-content channel.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent> {
        let key = SubscriptionKey::NewContent;
        self.inner.subscribe(&key.to_channel_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BroadcastEventProducer;

    #[test]
    fn test_new_content_event_producer() {
        let inner = BroadcastEventProducer::new();
        let producer = NewContentEventProducer::new(inner);

        let mut rx = producer.subscribe();
        producer.publish_new_content(serde_json::json!({"type": "PROJECT_CREATED", "token": "0x123"}));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.method, "new_content_subscribe");
    }
}
