use serde::{Deserialize, Serialize};

/// On-chain events emitted by OpenLaunch contracts.
/// Used by observer and websocket-server for event parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OnChainEvent {
    ProjectCreated(ProjectCreatedEvent),
    TokensPurchased(TokensPurchasedEvent),
    Graduated(GraduatedEvent),
    MilestoneApproved(MilestoneApprovedEvent),
    ProjectFailed(ProjectFailedEvent),
    Refunded(RefundedEvent),
    LiquidityAllocated(LiquidityAllocatedEvent),
    FeesCollected(FeesCollectedEvent),
    Transfer(TransferEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreatedEvent {
    pub token: String,
    pub creator: String,
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub ido_token_amount: String,
    pub token_price: String,
    pub deadline: i64,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensPurchasedEvent {
    pub token: String,
    pub buyer: String,
    pub usdc_amount: String,
    pub token_amount: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraduatedEvent {
    pub token: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneApprovedEvent {
    pub token: String,
    pub milestone_index: u64,
    pub usdc_released: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFailedEvent {
    pub token: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundedEvent {
    pub token: String,
    pub buyer: String,
    pub tokens_burned: String,
    pub usdc_returned: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityAllocatedEvent {
    pub token: String,
    pub pool: String,
    pub token_amount: String,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeesCollectedEvent {
    pub token: String,
    pub amount0: String,
    pub amount1: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEvent {
    pub token: String,
    pub from: String,
    pub to: String,
    pub amount: String,
    pub block_number: u64,
    pub tx_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_created_event_serialization() {
        let event = ProjectCreatedEvent {
            token: "0xtoken".to_string(),
            creator: "0xcreator".to_string(),
            name: "TestProject".to_string(),
            symbol: "TP".to_string(),
            token_uri: "https://meta.json".to_string(),
            ido_token_amount: "1000000".to_string(),
            token_price: "0.01".to_string(),
            deadline: 1717300000,
            block_number: 12345,
            tx_hash: "0xabc".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: ProjectCreatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token, "0xtoken");
        assert_eq!(parsed.symbol, "TP");
        assert_eq!(parsed.block_number, 12345);
    }

    #[test]
    fn test_tokens_purchased_event_serialization() {
        let event = TokensPurchasedEvent {
            token: "0xt".to_string(),
            buyer: "0xbuyer".to_string(),
            usdc_amount: "1000".to_string(),
            token_amount: "50000".to_string(),
            block_number: 100,
            tx_hash: "0xhash".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: TokensPurchasedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.buyer, "0xbuyer");
        assert_eq!(parsed.usdc_amount, "1000");
    }

    #[test]
    fn test_graduated_event_serialization() {
        let event = GraduatedEvent {
            token: "0xt".to_string(),
            block_number: 200,
            tx_hash: "0xgrad".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: GraduatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token, "0xt");
    }

    #[test]
    fn test_milestone_approved_event_serialization() {
        let event = MilestoneApprovedEvent {
            token: "0xt".to_string(),
            milestone_index: 2,
            usdc_released: "50000".to_string(),
            block_number: 300,
            tx_hash: "0xms".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: MilestoneApprovedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.milestone_index, 2);
    }

    #[test]
    fn test_refunded_event_serialization() {
        let event = RefundedEvent {
            token: "0xt".to_string(),
            buyer: "0xb".to_string(),
            tokens_burned: "100".to_string(),
            usdc_returned: "50".to_string(),
            block_number: 400,
            tx_hash: "0xref".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: RefundedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tokens_burned, "100");
        assert_eq!(parsed.usdc_returned, "50");
    }

    #[test]
    fn test_transfer_event_serialization() {
        let event = TransferEvent {
            token: "0xt".to_string(),
            from: "0xfrom".to_string(),
            to: "0xto".to_string(),
            amount: "999".to_string(),
            block_number: 500,
            tx_hash: "0xtx".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: TransferEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from, "0xfrom");
        assert_eq!(parsed.to, "0xto");
    }

    #[test]
    fn test_liquidity_allocated_event_serialization() {
        let event = LiquidityAllocatedEvent {
            token: "0xt".to_string(),
            pool: "0xpool".to_string(),
            token_amount: "10000".to_string(),
            tick_lower: -887220,
            tick_upper: 887220,
            block_number: 600,
            tx_hash: "0xliq".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: LiquidityAllocatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tick_lower, -887220);
        assert_eq!(parsed.tick_upper, 887220);
    }

    #[test]
    fn test_fees_collected_event_serialization() {
        let event = FeesCollectedEvent {
            token: "0xt".to_string(),
            amount0: "100".to_string(),
            amount1: "200".to_string(),
            block_number: 700,
            tx_hash: "0xfee".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: FeesCollectedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.amount0, "100");
        assert_eq!(parsed.amount1, "200");
    }

    #[test]
    fn test_on_chain_event_project_created_variant() {
        let event = OnChainEvent::ProjectCreated(ProjectCreatedEvent {
            token: "0xt".to_string(),
            creator: "0xc".to_string(),
            name: "N".to_string(),
            symbol: "S".to_string(),
            token_uri: "u".to_string(),
            ido_token_amount: "1".to_string(),
            token_price: "1".to_string(),
            deadline: 0,
            block_number: 0,
            tx_hash: "0x".to_string(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: OnChainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, OnChainEvent::ProjectCreated(_)));
    }

    #[test]
    fn test_on_chain_event_tokens_purchased_variant() {
        let event = OnChainEvent::TokensPurchased(TokensPurchasedEvent {
            token: "0x".to_string(),
            buyer: "0x".to_string(),
            usdc_amount: "0".to_string(),
            token_amount: "0".to_string(),
            block_number: 0,
            tx_hash: "0x".to_string(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: OnChainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, OnChainEvent::TokensPurchased(_)));
    }

    #[test]
    fn test_project_failed_event_serialization() {
        let event = ProjectFailedEvent {
            token: "0xfailed".to_string(),
            block_number: 999,
            tx_hash: "0xfail_hash".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: ProjectFailedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token, "0xfailed");
        assert_eq!(parsed.block_number, 999);
    }
}
