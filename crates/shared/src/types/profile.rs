use serde::{Deserialize, Serialize};

use super::project::{IProjectInfo, IProjectMarketInfo};
use super::token::{IMarketInfo, ITokenInfo};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BalanceInfo {
    pub balance: String,
    pub token_price: String,
    pub native_price: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MilestoneProgress {
    pub completed: i32,
    pub total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HoldTokenItem {
    pub token_info: ITokenInfo,
    pub market_info: IMarketInfo,
    pub balance_info: BalanceInfo,
    pub origin: String,
    pub milestone_progress: MilestoneProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PortfolioSummary {
    pub portfolio_value: String,
    pub total_invested_ido: String,
    pub trading_pnl: String,
    pub trading_pnl_percent: f64,
    pub active_idos: i64,
    pub refunds_received: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdoParticipation {
    pub project_info: IProjectInfo,
    pub market_info: IProjectMarketInfo,
    pub invested_amount: String,
    pub tokens_received: String,
    pub status: String,
    pub milestone_progress: MilestoneProgress,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RefundHistoryItem {
    pub project_info: IProjectInfo,
    pub market_info: IProjectMarketInfo,
    pub original_investment: String,
    pub refund_amount: String,
    pub tokens_burned: String,
    pub failed_milestone: Option<String>,
    pub transaction_hash: String,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::account::IAccountInfo;
    use crate::types::project::ProjectStatus;
    use crate::types::token::MarketType;

    fn sample_token_info() -> ITokenInfo {
        ITokenInfo {
            token_id: "0xtoken".to_string(),
            name: "Test".to_string(),
            symbol: "TST".to_string(),
            image_uri: "img.png".to_string(),
            banner_uri: None,
            description: None,
            category: "defi".to_string(),
            is_graduated: false,
            creator: IAccountInfo::new("0xcreator".to_string()),
            website: None,
            twitter: None,
            telegram: None,
            created_at: 1714608000,
            project_id: Some("proj_1".to_string()),
        }
    }

    fn sample_market_info() -> IMarketInfo {
        IMarketInfo {
            market_type: MarketType::Ido,
            token_id: "0xtoken".to_string(),
            token_price: "0.256".to_string(),
            native_price: "32.50".to_string(),
            price: "0.256".to_string(),
            ath_price: "0.5".to_string(),
            total_supply: "1000000".to_string(),
            volume: "50000".to_string(),
            holder_count: 100,
            bonding_percent: 0.0,
            milestone_completed: 3,
            milestone_total: 4,
        }
    }

    fn sample_project_info() -> IProjectInfo {
        IProjectInfo {
            project_id: "proj_1".to_string(),
            name: "TestProject".to_string(),
            symbol: "TP".to_string(),
            image_uri: "img.png".to_string(),
            description: None,
            tagline: "A test project".to_string(),
            category: "defi".to_string(),
            creator: IAccountInfo::new("0xcreator".to_string()),
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            created_at: 1714608000,
        }
    }

    fn sample_project_market_info() -> IProjectMarketInfo {
        IProjectMarketInfo {
            project_id: "proj_1".to_string(),
            status: ProjectStatus::Active,
            target_raise: "1000000".to_string(),
            total_committed: "500000".to_string(),
            funded_percent: 50.0,
            investor_count: 42,
        }
    }

    #[test]
    fn test_hold_token_item_serialization() {
        let item = HoldTokenItem {
            token_info: sample_token_info(),
            market_info: sample_market_info(),
            balance_info: BalanceInfo {
                balance: "12500000000000000000000".to_string(),
                token_price: "0.256".to_string(),
                native_price: "32.50".to_string(),
                created_at: 1714608000,
            },
            origin: "ido".to_string(),
            milestone_progress: MilestoneProgress {
                completed: 3,
                total: 4,
            },
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: HoldTokenItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.balance_info.balance, "12500000000000000000000");
        assert_eq!(parsed.origin, "ido");
        assert_eq!(parsed.milestone_progress.completed, 3);
        assert_eq!(parsed.milestone_progress.total, 4);
    }

    #[test]
    fn test_portfolio_summary_serialization() {
        let summary = PortfolioSummary {
            portfolio_value: "18720000000000000000000".to_string(),
            total_invested_ido: "12450000000000000000000".to_string(),
            trading_pnl: "4270000000000000000000".to_string(),
            trading_pnl_percent: 34.3,
            active_idos: 5,
            refunds_received: "1200000000000000000000".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: PortfolioSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.active_idos, 5);
        assert!((parsed.trading_pnl_percent - 34.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ido_participation_serialization() {
        let item = IdoParticipation {
            project_info: sample_project_info(),
            market_info: sample_project_market_info(),
            invested_amount: "2000000000000000000000".to_string(),
            tokens_received: "12500000000000000000000".to_string(),
            status: "active".to_string(),
            milestone_progress: MilestoneProgress {
                completed: 3,
                total: 4,
            },
            created_at: 1714608000,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: IdoParticipation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.invested_amount, "2000000000000000000000");
        assert_eq!(parsed.status, "active");
    }

    #[test]
    fn test_refund_history_item_serialization() {
        let item = RefundHistoryItem {
            project_info: sample_project_info(),
            market_info: sample_project_market_info(),
            original_investment: "2000000000000000000000".to_string(),
            refund_amount: "1500000000000000000000".to_string(),
            tokens_burned: "10000000000000000000000".to_string(),
            failed_milestone: Some("Beta Launch".to_string()),
            transaction_hash: "0xabc123".to_string(),
            created_at: 1714608000,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: RefundHistoryItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.refund_amount, "1500000000000000000000");
        assert_eq!(parsed.failed_milestone, Some("Beta Launch".to_string()));
        assert_eq!(parsed.transaction_hash, "0xabc123");
    }

    #[test]
    fn test_refund_history_item_no_failed_milestone() {
        let item = RefundHistoryItem {
            project_info: sample_project_info(),
            market_info: sample_project_market_info(),
            original_investment: "1000".to_string(),
            refund_amount: "800".to_string(),
            tokens_burned: "5000".to_string(),
            failed_milestone: None,
            transaction_hash: "0xdef".to_string(),
            created_at: 0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: RefundHistoryItem = serde_json::from_str(&json).unwrap();
        assert!(parsed.failed_milestone.is_none());
    }

    #[test]
    fn test_milestone_progress_serialization() {
        let progress = MilestoneProgress {
            completed: 2,
            total: 5,
        };
        let json = serde_json::to_string(&progress).unwrap();
        let parsed: MilestoneProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.completed, 2);
        assert_eq!(parsed.total, 5);
    }

    #[test]
    fn test_balance_info_serialization() {
        let info = BalanceInfo {
            balance: "999".to_string(),
            token_price: "1.5".to_string(),
            native_price: "0.001".to_string(),
            created_at: 12345,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: BalanceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.balance, "999");
        assert_eq!(parsed.created_at, 12345);
    }

    #[test]
    fn test_portfolio_summary_zero_values() {
        let summary = PortfolioSummary {
            portfolio_value: "0".to_string(),
            total_invested_ido: "0".to_string(),
            trading_pnl: "0".to_string(),
            trading_pnl_percent: 0.0,
            active_idos: 0,
            refunds_received: "0".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: PortfolioSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.active_idos, 0);
        assert!((parsed.trading_pnl_percent - 0.0).abs() < f64::EPSILON);
    }
}
