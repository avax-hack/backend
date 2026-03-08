use serde::{Deserialize, Serialize};

use super::account::IAccountInfo;
use super::token::ITokenInfo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ISwapInfo {
    pub event_type: TradeType,
    pub native_amount: String,
    pub token_amount: String,
    pub native_price: String,
    pub transaction_hash: String,
    pub value: String,
    pub account_info: IAccountInfo,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ISwapWithTokenInfo {
    pub event_type: TradeType,
    pub token_info: ITokenInfo,
    pub native_amount: String,
    pub token_amount: String,
    pub native_price: String,
    pub transaction_hash: String,
    pub value: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChartBar {
    pub time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TradeQuote {
    pub expected_output: String,
    pub price_impact_percent: String,
    pub minimum_received: String,
    pub fee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChartRequest {
    pub resolution: String,
    pub from: i64,
    pub to: i64,
    #[serde(default = "default_countback")]
    pub countback: i64,
    #[serde(default = "default_chart_type")]
    pub chart_type: String,
}

fn default_countback() -> i64 {
    300
}

fn default_chart_type() -> String {
    "price".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_type_serialization() {
        let buy = TradeType::Buy;
        let json = serde_json::to_string(&buy).unwrap();
        assert_eq!(json, "\"BUY\"");
    }

    #[test]
    fn test_chart_bar_serialization() {
        let bar = ChartBar {
            time: 1717200000,
            open: "0.0254".to_string(),
            high: "0.0260".to_string(),
            low: "0.0250".to_string(),
            close: "0.0256".to_string(),
            volume: "15000".to_string(),
        };
        let json = serde_json::to_string(&bar).unwrap();
        let parsed: ChartBar = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.time, 1717200000);
        assert_eq!(parsed.close, "0.0256");
    }

    #[test]
    fn test_trade_type_sell_serialization() {
        let sell = TradeType::Sell;
        let json = serde_json::to_string(&sell).unwrap();
        assert_eq!(json, "\"SELL\"");
    }

    #[test]
    fn test_trade_type_deserialization() {
        let buy: TradeType = serde_json::from_str("\"BUY\"").unwrap();
        assert_eq!(buy, TradeType::Buy);
        let sell: TradeType = serde_json::from_str("\"SELL\"").unwrap();
        assert_eq!(sell, TradeType::Sell);
    }

    #[test]
    fn test_trade_type_invalid_deserialization() {
        let result = serde_json::from_str::<TradeType>("\"HOLD\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_trade_type_equality() {
        assert_eq!(TradeType::Buy, TradeType::Buy);
        assert_ne!(TradeType::Buy, TradeType::Sell);
    }

    #[test]
    fn test_trade_quote_serialization() {
        let quote = TradeQuote {
            expected_output: "1000".to_string(),
            price_impact_percent: "0.5".to_string(),
            minimum_received: "995".to_string(),
            fee: "3".to_string(),
        };
        let json = serde_json::to_string(&quote).unwrap();
        let parsed: TradeQuote = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.expected_output, "1000");
        assert_eq!(parsed.price_impact_percent, "0.5");
        assert_eq!(parsed.minimum_received, "995");
        assert_eq!(parsed.fee, "3");
    }

    #[test]
    fn test_chart_request_serialization() {
        let req = ChartRequest {
            resolution: "1h".to_string(),
            from: 1717200000,
            to: 1717286400,
            countback: 100,
            chart_type: "price".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ChartRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.resolution, "1h");
        assert_eq!(parsed.from, 1717200000);
        assert_eq!(parsed.to, 1717286400);
        assert_eq!(parsed.countback, 100);
    }

    #[test]
    fn test_chart_request_defaults() {
        let json = r#"{"resolution":"5m","from":1000,"to":2000}"#;
        let parsed: ChartRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.countback, 300);
        assert_eq!(parsed.chart_type, "price");
    }

    #[test]
    fn test_swap_info_serialization() {
        let swap = ISwapInfo {
            event_type: TradeType::Buy,
            native_amount: "1000000000000000000".to_string(),
            token_amount: "50000000000000000000".to_string(),
            native_price: "25.00".to_string(),
            transaction_hash: "0xabcdef1234567890".to_string(),
            value: "25.00".to_string(),
            account_info: super::super::account::IAccountInfo::new("0xbuyer".to_string()),
            created_at: 1717200000,
        };
        let json = serde_json::to_string(&swap).unwrap();
        let parsed: ISwapInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, TradeType::Buy);
        assert_eq!(parsed.transaction_hash, "0xabcdef1234567890");
    }

    #[test]
    fn test_chart_bar_clone() {
        let bar = ChartBar {
            time: 100,
            open: "1".to_string(),
            high: "2".to_string(),
            low: "0.5".to_string(),
            close: "1.5".to_string(),
            volume: "999".to_string(),
        };
        let cloned = bar.clone();
        assert_eq!(bar.time, cloned.time);
        assert_eq!(bar.close, cloned.close);
    }
}
