use std::sync::Arc;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::{account, balance, chart, market, swap};
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::account::IAccountInfo;
use openlaunch_shared::types::common::{PaginatedResponse, PaginationParams};
use openlaunch_shared::types::token::IMarketInfo;
use openlaunch_shared::types::trading::{ChartBar, ChartRequest, ISwapInfo, TradeQuote, TradeType};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderInfo {
    pub account_info: IAccountInfo,
    pub balance: String,
}

/// Get chart bars for a token address.
pub async fn get_chart(
    db: &Arc<PostgresDatabase>,
    token_address: &str,
    params: &ChartRequest,
) -> AppResult<Vec<ChartBar>> {
    let interval = resolve_interval(&params.resolution);

    chart::find_bars(
        db.reader(),
        token_address,
        &interval,
        params.from,
        params.to,
        params.countback,
    )
    .await
    .map_err(AppError::Internal)
}

/// Get paginated swap history for a token.
pub async fn get_swap_history(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<ISwapInfo>> {
    get_swap_history_ordered(db, token_id, pagination, None, "DESC").await
}

/// Get paginated swap history with filtering and sort direction.
pub async fn get_swap_history_ordered(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
    pagination: &PaginationParams,
    trade_type: Option<&str>,
    direction: &str,
) -> AppResult<PaginatedResponse<ISwapInfo>> {
    let (rows, total_count) = swap::find_by_token_ordered(
        db.reader(), token_id, pagination, trade_type, direction,
    )
    .await
    .map_err(AppError::Internal)?;

    let mut swaps = Vec::with_capacity(rows.len());
    for row in rows {
        let account_info = account::find_by_id(db.reader(), &row.account_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.account_id.clone()));

        let event_type = match row.event_type.to_uppercase().as_str() {
            "BUY" => TradeType::Buy,
            _ => TradeType::Sell,
        };

        swaps.push(ISwapInfo {
            event_type,
            native_amount: row.native_amount,
            token_amount: row.token_amount,

            transaction_hash: row.tx_hash,
            value: row.value,
            account_info,
            created_at: row.created_at,
        });
    }

    Ok(PaginatedResponse {
        data: swaps,
        total_count,
    })
}

/// Get paginated token holders.
pub async fn get_holders(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<HolderInfo>> {
    let (rows, total_count) = balance::find_holders(db.reader(), token_id, pagination)
        .await
        .map_err(AppError::Internal)?;

    let mut holders = Vec::with_capacity(rows.len());
    for row in rows {
        let account_info = account::find_by_id(db.reader(), &row.account_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.account_id.clone()));

        holders.push(HolderInfo {
            account_info,
            balance: row.balance,
        });
    }

    Ok(PaginatedResponse {
        data: holders,
        total_count,
    })
}

/// Get market data for a token.
pub async fn get_market(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
) -> AppResult<IMarketInfo> {
    let market_row = market::find_by_token(db.reader(), token_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("Market data for {token_id} not found")))?;

    Ok(crate::services::token::build_market_info_from_row(
        token_id,
        &market_row,
    ))
}

/// Get token metrics (price changes, volume, trades across timeframes).
/// For now returns basic data from market_data table.
pub async fn get_metrics(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
) -> AppResult<openlaunch_shared::types::token::ITokenMetricsData> {
    let market_row = market::find_by_token(db.reader(), token_id)
        .await
        .map_err(AppError::Internal)?;

    let volume = market_row
        .as_ref()
        .map(|m| m.volume_24h.clone())
        .unwrap_or_else(|| "0".to_string());

    let mut metrics = std::collections::HashMap::new();
    // Placeholder: in production, compute from chart/swap data per timeframe
    for timeframe in &["5m", "1h", "6h", "24h"] {
        metrics.insert(
            timeframe.to_string(),
            openlaunch_shared::types::token::TimeframeMetrics {
                price_change: "0".to_string(),
                volume: volume.clone(),
                trades: 0,
            },
        );
    }

    Ok(openlaunch_shared::types::token::ITokenMetricsData { metrics })
}

/// Get a swap quote for a token.
/// Placeholder: actual AMM math would go here.
pub async fn get_quote(
    _db: &Arc<PostgresDatabase>,
    _token_id: &str,
    _amount: &str,
    _is_buy: bool,
    _slippage: f64,
) -> AppResult<TradeQuote> {
    Ok(TradeQuote {
        expected_output: "0".to_string(),
        price_impact_percent: "0".to_string(),
        minimum_received: "0".to_string(),
        fee: "0".to_string(),
    })
}

fn resolve_interval(resolution: &str) -> String {
    match resolution {
        "1" | "1m" => "1m".to_string(),
        "5" | "5m" => "5m".to_string(),
        "15" | "15m" => "15m".to_string(),
        "60" | "1h" | "1H" => "1h".to_string(),
        "240" | "4h" | "4H" => "4h".to_string(),
        "D" | "1D" | "1d" => "1d".to_string(),
        "W" | "1W" | "1w" => "1w".to_string(),
        _ => "1h".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_interval_1_minute_variants() {
        assert_eq!(resolve_interval("1"), "1m");
        assert_eq!(resolve_interval("1m"), "1m");
    }

    #[test]
    fn resolve_interval_5_minute_variants() {
        assert_eq!(resolve_interval("5"), "5m");
        assert_eq!(resolve_interval("5m"), "5m");
    }

    #[test]
    fn resolve_interval_15_minute_variants() {
        assert_eq!(resolve_interval("15"), "15m");
        assert_eq!(resolve_interval("15m"), "15m");
    }

    #[test]
    fn resolve_interval_1_hour_variants() {
        assert_eq!(resolve_interval("60"), "1h");
        assert_eq!(resolve_interval("1h"), "1h");
        assert_eq!(resolve_interval("1H"), "1h");
    }

    #[test]
    fn resolve_interval_4_hour_variants() {
        assert_eq!(resolve_interval("240"), "4h");
        assert_eq!(resolve_interval("4h"), "4h");
        assert_eq!(resolve_interval("4H"), "4h");
    }

    #[test]
    fn resolve_interval_1_day_variants() {
        assert_eq!(resolve_interval("D"), "1d");
        assert_eq!(resolve_interval("1D"), "1d");
        assert_eq!(resolve_interval("1d"), "1d");
    }

    #[test]
    fn resolve_interval_1_week_variants() {
        assert_eq!(resolve_interval("W"), "1w");
        assert_eq!(resolve_interval("1W"), "1w");
        assert_eq!(resolve_interval("1w"), "1w");
    }

    #[test]
    fn resolve_interval_unknown_defaults_to_1h() {
        assert_eq!(resolve_interval("unknown"), "1h");
        assert_eq!(resolve_interval(""), "1h");
        assert_eq!(resolve_interval("30"), "1h");
        assert_eq!(resolve_interval("2h"), "1h");
    }

    #[test]
    fn holder_info_serialization_roundtrip() {
        let info = HolderInfo {
            account_info: IAccountInfo::new("0xabc".to_string()),
            balance: "12345.67".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: HolderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.balance, "12345.67");
    }
}
