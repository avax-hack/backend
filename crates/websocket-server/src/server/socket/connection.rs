use std::collections::HashMap;

use tokio::task::JoinHandle;

use crate::config_local;

/// Tracks active subscription tasks for a single WebSocket client connection.
pub struct ConnectionState {
    /// Map from subscription channel key to the spawned forwarding task.
    subscriptions: HashMap<String, JoinHandle<()>>,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    /// Register a subscription task for the given key.
    /// If a task already exists for this key, the old task is aborted first.
    /// Returns false if the subscription limit is reached (and the key is new).
    pub fn subscribe(&mut self, key: String, handle: JoinHandle<()>) -> bool {
        if let Some(old_handle) = self.subscriptions.remove(&key) {
            old_handle.abort();
        } else if self.subscriptions.len() >= *config_local::WS_MAX_SUBSCRIPTIONS_PER_CONN {
            handle.abort();
            return false;
        }
        self.subscriptions.insert(key, handle);
        true
    }

    /// Unsubscribe from a specific key, aborting the forwarding task.
    /// Returns true if a subscription existed and was removed.
    pub fn unsubscribe(&mut self, key: &str) -> bool {
        if let Some(handle) = self.subscriptions.remove(key) {
            handle.abort();
            true
        } else {
            false
        }
    }

    /// Abort all subscription tasks. Called on client disconnect.
    pub fn cleanup_all(&mut self) {
        for (_, handle) in self.subscriptions.drain() {
            handle.abort();
        }
    }

    /// Returns the number of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Check if a subscription exists for the given key.
    pub fn has_subscription(&self, key: &str) -> bool {
        self.subscriptions.contains_key(key)
    }
}

impl Drop for ConnectionState {
    fn drop(&mut self) {
        self.cleanup_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_state_subscribe_and_cleanup() {
        let mut state = ConnectionState::new();
        assert_eq!(state.subscription_count(), 0);

        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        state.subscribe("trade:0xabc".to_string(), handle);
        assert_eq!(state.subscription_count(), 1);
        assert!(state.has_subscription("trade:0xabc"));

        state.cleanup_all();
        assert_eq!(state.subscription_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_state_replaces_existing() {
        let mut state = ConnectionState::new();

        let handle1 = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        state.subscribe("trade:0xabc".to_string(), handle1);

        let handle2 = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        state.subscribe("trade:0xabc".to_string(), handle2);
        assert_eq!(state.subscription_count(), 1);

        state.cleanup_all();
    }

    #[tokio::test]
    async fn test_connection_state_unsubscribe() {
        let mut state = ConnectionState::new();

        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        state.subscribe("price:0xdef".to_string(), handle);
        assert!(state.unsubscribe("price:0xdef"));
        assert!(!state.unsubscribe("price:0xdef"));
        assert_eq!(state.subscription_count(), 0);
    }
}
