use serde::{Deserialize, Serialize};

/// A generic WebSocket event that can be broadcast to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    pub method: String,
    pub data: serde_json::Value,
}

/// Identifies a subscription channel.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum SubscriptionKey {
    /// Trade events for a specific token address.
    Trade(String),
    /// Price updates for a specific token address.
    Price(String),
    /// Project-level events for a specific project (token address).
    Project(String),
    /// Milestone events for a specific project (token address).
    Milestone(String),
    /// Chart bar updates for a specific token address.
    Chart(String),
    /// Global broadcast for new content (new projects, graduations, etc.).
    NewContent,
}

impl SubscriptionKey {
    /// Convert to a string key used in the channel map.
    pub fn to_channel_key(&self) -> String {
        match self {
            Self::Trade(id) => format!("trade:{id}"),
            Self::Price(id) => format!("price:{id}"),
            Self::Project(id) => format!("project:{id}"),
            Self::Milestone(id) => format!("milestone:{id}"),
            Self::Chart(id) => format!("chart:{id}"),
            Self::NewContent => "new_content".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_key_to_channel_key() {
        let key = SubscriptionKey::Trade("0xabc".to_string());
        assert_eq!(key.to_channel_key(), "trade:0xabc");

        let key = SubscriptionKey::NewContent;
        assert_eq!(key.to_channel_key(), "new_content");
    }

    #[test]
    fn test_ws_event_serialization() {
        let event = WsEvent {
            method: "trade_subscribe".to_string(),
            data: serde_json::json!({"type": "TRADE"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: WsEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "trade_subscribe");
    }
}
