use serde::{Deserialize, Serialize};

use super::account::IAccountInfo;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ITokenInfo {
    pub token_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub banner_uri: Option<String>,
    pub description: Option<String>,
    pub category: String,
    pub is_graduated: bool,
    pub creator: IAccountInfo,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IMarketInfo {
    pub market_type: MarketType,
    pub token_id: String,
    pub token_price: String,
    pub price: String,
    pub ath_price: String,
    pub total_supply: String,
    pub volume: String,
    pub holder_count: i64,
    pub bonding_percent: f64,
    pub milestone_completed: i32,
    pub milestone_total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum MarketType {
    Curve,
    Dex,
    Ido,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ITokenData {
    pub token_info: ITokenInfo,
    pub market_info: IMarketInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ITokenMetricsData {
    pub metrics: std::collections::HashMap<String, TimeframeMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TimeframeMetrics {
    pub price_change: String,
    pub volume: String,
    pub trades: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_type_serialization() {
        let ido = MarketType::Ido;
        let json = serde_json::to_string(&ido).unwrap();
        assert_eq!(json, "\"IDO\"");

        let dex = MarketType::Dex;
        let json = serde_json::to_string(&dex).unwrap();
        assert_eq!(json, "\"DEX\"");
    }

    #[test]
    fn test_market_type_curve() {
        let curve = MarketType::Curve;
        let json = serde_json::to_string(&curve).unwrap();
        assert_eq!(json, "\"CURVE\"");
    }

    #[test]
    fn test_market_type_deserialization() {
        let parsed: MarketType = serde_json::from_str("\"IDO\"").unwrap();
        assert_eq!(parsed, MarketType::Ido);
        let parsed: MarketType = serde_json::from_str("\"DEX\"").unwrap();
        assert_eq!(parsed, MarketType::Dex);
        let parsed: MarketType = serde_json::from_str("\"CURVE\"").unwrap();
        assert_eq!(parsed, MarketType::Curve);
    }

    #[test]
    fn test_market_type_invalid_deserialization() {
        let result = serde_json::from_str::<MarketType>("\"UNKNOWN\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_market_type_equality() {
        assert_eq!(MarketType::Ido, MarketType::Ido);
        assert_ne!(MarketType::Ido, MarketType::Dex);
    }

    #[test]
    fn test_timeframe_metrics_serialization() {
        let metrics = TimeframeMetrics {
            price_change: "5.23".to_string(),
            volume: "1000000".to_string(),
            trades: 42,
        };
        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: TimeframeMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.price_change, "5.23");
        assert_eq!(parsed.volume, "1000000");
        assert_eq!(parsed.trades, 42);
    }

    #[test]
    fn test_token_metrics_data_serialization() {
        let mut metrics_map = std::collections::HashMap::new();
        metrics_map.insert(
            "1h".to_string(),
            TimeframeMetrics {
                price_change: "1.5".to_string(),
                volume: "50000".to_string(),
                trades: 10,
            },
        );
        let data = ITokenMetricsData { metrics: metrics_map };
        let json = serde_json::to_string(&data).unwrap();
        let parsed: ITokenMetricsData = serde_json::from_str(&json).unwrap();
        assert!(parsed.metrics.contains_key("1h"));
        assert_eq!(parsed.metrics["1h"].trades, 10);
    }

    #[test]
    fn test_token_info_serialization() {
        let info = ITokenInfo {
            token_id: "0xtoken".to_string(),
            name: "TestToken".to_string(),
            symbol: "TT".to_string(),
            image_uri: "img.png".to_string(),
            banner_uri: None,
            description: Some("A test token".to_string()),
            category: "defi".to_string(),
            is_graduated: false,
            creator: IAccountInfo::new("0xcreator".to_string()),
            website: None,
            twitter: None,
            telegram: None,
            created_at: 1717200000,
            project_id: Some("proj_001".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ITokenInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token_id, "0xtoken");
        assert_eq!(parsed.symbol, "TT");
        assert!(!parsed.is_graduated);
        assert_eq!(parsed.project_id, Some("proj_001".to_string()));
    }

    #[test]
    fn test_market_info_serialization() {
        let info = IMarketInfo {
            market_type: MarketType::Curve,
            token_id: "0xtoken".to_string(),
            token_price: "0.025".to_string(),

            price: "0.025".to_string(),
            ath_price: "0.050".to_string(),
            total_supply: "1000000".to_string(),
            volume: "50000".to_string(),
            holder_count: 123,
            bonding_percent: 45.5,
            milestone_completed: 2,
            milestone_total: 5,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IMarketInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.market_type, MarketType::Curve);
        assert_eq!(parsed.holder_count, 123);
        assert!((parsed.bonding_percent - 45.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_data_serialization() {
        let data = ITokenData {
            token_info: ITokenInfo {
                token_id: "0xt".to_string(),
                name: "T".to_string(),
                symbol: "T".to_string(),
                image_uri: "i.png".to_string(),
                banner_uri: None,
                description: None,
                category: "gaming".to_string(),
                is_graduated: true,
                creator: IAccountInfo::new("0xc".to_string()),
                website: None,
                twitter: None,
                telegram: None,
                created_at: 0,
                project_id: None,
            },
            market_info: IMarketInfo {
                market_type: MarketType::Dex,
                token_id: "0xt".to_string(),
                token_price: "1".to_string(),
    
                price: "1".to_string(),
                ath_price: "2".to_string(),
                total_supply: "1000".to_string(),
                volume: "100".to_string(),
                holder_count: 5,
                bonding_percent: 100.0,
                milestone_completed: 0,
                milestone_total: 0,
            },
        };
        let json = serde_json::to_string(&data).unwrap();
        let parsed: ITokenData = serde_json::from_str(&json).unwrap();
        assert!(parsed.token_info.is_graduated);
        assert_eq!(parsed.market_info.market_type, MarketType::Dex);
    }
}
