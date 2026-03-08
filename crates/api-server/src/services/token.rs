use std::sync::Arc;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::{account, market, project};
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::account::IAccountInfo;
use openlaunch_shared::types::common::{PaginatedResponse, PaginationParams};
use openlaunch_shared::types::token::{IMarketInfo, ITokenData, ITokenInfo, MarketType};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenListItem {
    pub token_info: ITokenInfo,
    pub market_info: IMarketInfo,
}

/// Get token data by project_id (token_id == project_id in this system).
pub async fn get_token(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
) -> AppResult<ITokenData> {
    let token_id = token_id.to_lowercase();
    let token_id = token_id.as_str();
    let project_row = project::find_by_id(db.reader(), token_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("Token {token_id} not found")))?;

    let creator = account::find_by_id(db.reader(), &project_row.creator)
        .await
        .map_err(AppError::Internal)?
        .unwrap_or_else(|| IAccountInfo::new(project_row.creator.clone()));

    let market_row = market::find_by_token(db.reader(), token_id)
        .await
        .map_err(AppError::Internal)?;

    let is_graduated = market_row.as_ref().map(|m| m.is_graduated).unwrap_or(false);
    let token_info = build_token_info(&project_row, &creator, is_graduated);
    let market_info = build_market_info(token_id, market_row.as_ref());

    Ok(ITokenData {
        token_info,
        market_info,
    })
}

/// Get sorted/paginated token list.
pub async fn get_token_list(
    db: &Arc<PostgresDatabase>,
    sort_type: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<TokenListItem>> {
    get_token_list_filtered(db, sort_type, pagination, None, None, false, None).await
}

/// Get sorted/paginated/filtered token list.
pub async fn get_token_list_filtered(
    db: &Arc<PostgresDatabase>,
    sort_type: &str,
    pagination: &PaginationParams,
    category: Option<&str>,
    search: Option<&str>,
    verified_only: bool,
    status: Option<&str>,
) -> AppResult<PaginatedResponse<TokenListItem>> {
    let (rows, total_count) = project::find_list_filtered(
        db.reader(), sort_type, pagination, status, category, search, verified_only,
    )
    .await
    .map_err(AppError::Internal)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let creator = account::find_by_id(db.reader(), &row.creator)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.creator.clone()));

        let market_row = market::find_by_token(db.reader(), &row.project_id)
            .await
            .map_err(AppError::Internal)?;

        let token_info = ITokenInfo {
            token_id: row.project_id.clone(),
            name: row.name,
            symbol: row.symbol,
            image_uri: row.image_uri,
            banner_uri: None,
            description: None,
            category: row.category,
            is_graduated: market_row
                .as_ref()
                .map(|m| m.is_graduated)
                .unwrap_or(false),
            creator,
            website: None,
            twitter: None,
            telegram: None,
            created_at: row.created_at,
            project_id: Some(row.project_id.clone()),
        };

        let market_info = build_market_info(&row.project_id, market_row.as_ref());

        items.push(TokenListItem {
            token_info,
            market_info,
        });
    }

    Ok(PaginatedResponse {
        data: items,
        total_count,
    })
}

/// Get trending tokens (top volume in last 24h).
pub async fn get_trending(
    db: &Arc<PostgresDatabase>,
) -> AppResult<Vec<TokenListItem>> {
    // Reuse token list sorted by recent activity for now
    let result = get_token_list(
        db,
        "recent",
        &PaginationParams { page: 1, limit: 10 },
    )
    .await?;
    Ok(result.data)
}

fn build_token_info(
    row: &project::ProjectRow,
    creator: &IAccountInfo,
    is_graduated: bool,
) -> ITokenInfo {
    ITokenInfo {
        token_id: row.project_id.clone(),
        name: row.name.clone(),
        symbol: row.symbol.clone(),
        image_uri: row.image_uri.clone(),
        banner_uri: None,
        description: row.description.clone(),
        category: row.category.clone(),
        is_graduated,
        creator: creator.clone(),
        website: row.website.clone(),
        twitter: row.twitter.clone(),
        telegram: row.telegram.clone(),
        created_at: row.created_at,
        project_id: Some(row.project_id.clone()),
    }
}

pub fn build_market_info_from_row(
    token_id: &str,
    row: &market::MarketDataRow,
) -> IMarketInfo {
    build_market_info(token_id, Some(row))
}

fn build_market_info(
    token_id: &str,
    market_row: Option<&market::MarketDataRow>,
) -> IMarketInfo {
    match market_row {
        Some(m) => {
            let market_type = match m.market_type.as_str() {
                "CURVE" => MarketType::Curve,
                "DEX" => MarketType::Dex,
                _ => MarketType::Ido,
            };
            IMarketInfo {
                market_type,
                token_id: token_id.to_string(),
                token_price: m.token_price.clone(),
                price: m.token_price.clone(),
                ath_price: m.ath_price.clone(),
                total_supply: m.total_supply.clone(),
                volume: m.volume_24h.clone(),
                holder_count: m.holder_count as i64,
                bonding_percent: m.bonding_percent.parse().unwrap_or(0.0),
                milestone_completed: m.milestone_completed,
                milestone_total: m.milestone_total,
            }
        }
        None => IMarketInfo {
            market_type: MarketType::Ido,
            token_id: token_id.to_string(),
            token_price: "0".to_string(),
            price: "0".to_string(),
            ath_price: "0".to_string(),
            total_supply: "0".to_string(),
            volume: "0".to_string(),
            holder_count: 0,
            bonding_percent: 0.0,
            milestone_completed: 0,
            milestone_total: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_market_data_row(market_type: &str, bonding_percent: &str) -> market::MarketDataRow {
        market::MarketDataRow {
            token_id: "tok_1".to_string(),
            market_type: market_type.to_string(),
            token_price: "1.50".to_string(),
            ath_price: "5.00".to_string(),
            total_supply: "1000000".to_string(),
            volume_24h: "50000".to_string(),
            holder_count: 42,
            bonding_percent: bonding_percent.to_string(),
            milestone_completed: 2,
            milestone_total: 5,
            is_graduated: false,
        }
    }

    #[test]
    fn build_market_info_none_returns_defaults() {
        let info = build_market_info("tok_abc", None);
        assert_eq!(info.token_id, "tok_abc");
        assert_eq!(info.token_price, "0");

        assert_eq!(info.price, "0");
        assert_eq!(info.holder_count, 0);
        assert!((info.bonding_percent - 0.0).abs() < f64::EPSILON);
        assert_eq!(info.milestone_completed, 0);
        assert_eq!(info.milestone_total, 0);
    }

    #[test]
    fn build_market_info_curve_type() {
        let row = make_market_data_row("CURVE", "75.5");
        let info = build_market_info("tok_1", Some(&row));
        assert!(matches!(info.market_type, MarketType::Curve));
        assert_eq!(info.token_price, "1.50");
        assert_eq!(info.price, "1.50");

        assert_eq!(info.ath_price, "5.00");
        assert_eq!(info.total_supply, "1000000");
        assert_eq!(info.volume, "50000");
        assert_eq!(info.holder_count, 42);
        assert!((info.bonding_percent - 75.5).abs() < f64::EPSILON);
        assert_eq!(info.milestone_completed, 2);
        assert_eq!(info.milestone_total, 5);
    }

    #[test]
    fn build_market_info_dex_type() {
        let row = make_market_data_row("DEX", "100.0");
        let info = build_market_info("tok_2", Some(&row));
        assert!(matches!(info.market_type, MarketType::Dex));
    }

    #[test]
    fn build_market_info_unknown_type_defaults_to_ido() {
        let row = make_market_data_row("UNKNOWN", "0");
        let info = build_market_info("tok_3", Some(&row));
        assert!(matches!(info.market_type, MarketType::Ido));
    }

    #[test]
    fn build_market_info_empty_type_defaults_to_ido() {
        let row = make_market_data_row("", "0");
        let info = build_market_info("tok_4", Some(&row));
        assert!(matches!(info.market_type, MarketType::Ido));
    }

    #[test]
    fn build_market_info_invalid_bonding_percent_defaults_to_zero() {
        let row = make_market_data_row("CURVE", "not_a_number");
        let info = build_market_info("tok_5", Some(&row));
        assert!((info.bonding_percent - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn build_market_info_from_row_delegates_correctly() {
        let row = make_market_data_row("DEX", "50.0");
        let info = build_market_info_from_row("tok_6", &row);
        assert!(matches!(info.market_type, MarketType::Dex));
        assert_eq!(info.token_id, "tok_6");
    }

    #[test]
    fn token_list_item_serialization_roundtrip() {
        let item = TokenListItem {
            token_info: ITokenInfo {
                token_id: "t1".to_string(),
                name: "TestToken".to_string(),
                symbol: "TT".to_string(),
                image_uri: "img.png".to_string(),
                banner_uri: None,
                description: Some("A test".to_string()),
                category: "defi".to_string(),
                is_graduated: false,
                creator: IAccountInfo::new("0xabc".to_string()),
                website: None,
                twitter: None,
                telegram: None,
                created_at: 1700000000,
                project_id: Some("p1".to_string()),
            },
            market_info: build_market_info("t1", None),
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: TokenListItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.token_info.name, "TestToken");
        assert_eq!(deserialized.token_info.symbol, "TT");
        assert_eq!(deserialized.market_info.token_id, "t1");
    }
}
