use std::sync::Arc;

use sqlx;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::{
    account, balance, investment, market, milestone, project, refund,
};
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::account::IAccountInfo;
use openlaunch_shared::types::common::{PaginatedResponse, PaginationParams};
use openlaunch_shared::types::profile::{
    BalanceInfo, HoldTokenItem, IdoParticipation, MilestoneProgress, PortfolioSummary,
    RefundHistoryItem,
};
use openlaunch_shared::types::project::{IProjectInfo, IProjectMarketInfo, ProjectStatus};
use openlaunch_shared::types::token::{IMarketInfo, ITokenInfo, MarketType};
use openlaunch_shared::types::trading::{ISwapInfo, TradeType};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedTokenInfo {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub status: String,
    pub created_at: i64,
}

pub async fn get_profile(
    db: &Arc<PostgresDatabase>,
    address: &str,
) -> AppResult<IAccountInfo> {
    account::find_by_id(db.reader(), address)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("Account {address} not found")))
}

pub async fn get_hold_tokens(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<HoldTokenItem>> {
    let (rows, total_count) = balance::find_by_account(db.reader(), account_id, pagination)
        .await
        .map_err(AppError::Internal)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let item = build_hold_token_item(db, account_id, &row).await?;
        items.push(item);
    }

    Ok(PaginatedResponse {
        data: items,
        total_count,
    })
}

pub async fn get_swap_history(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<ISwapInfo>> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, SwapRow>(
        r#"
        SELECT s.event_type, s.native_amount::TEXT as native_amount,
               s.token_amount::TEXT as token_amount, s.price::TEXT as price,
               s.value::TEXT as value, s.tx_hash, s.account_id, s.created_at
        FROM swaps s
        WHERE s.account_id = $1
        ORDER BY s.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM swaps WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or(0);

    let mut swaps = Vec::with_capacity(rows.len());
    for row in rows {
        let acct = account::find_by_id(db.reader(), &row.account_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.account_id.clone()));

        swaps.push(ISwapInfo {
            event_type: parse_trade_type(&row.event_type),
            native_amount: row.native_amount,
            token_amount: row.token_amount,
            native_price: row.price,
            transaction_hash: row.tx_hash,
            value: row.value,
            account_info: acct,
            created_at: row.created_at,
        });
    }

    Ok(PaginatedResponse {
        data: swaps,
        total_count: total,
    })
}

pub async fn get_ido_history(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<IdoParticipation>> {
    let (rows, total_count) = investment::find_by_account(db.reader(), account_id, pagination)
        .await
        .map_err(AppError::Internal)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let item = build_ido_participation(db, &row).await?;
        items.push(item);
    }

    Ok(PaginatedResponse {
        data: items,
        total_count,
    })
}

pub async fn get_refund_history(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<RefundHistoryItem>> {
    let (rows, total_count) =
        refund::find_enriched_by_account(db.reader(), account_id, pagination)
            .await
            .map_err(AppError::Internal)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let item = build_refund_history_item(db, &row).await?;
        items.push(item);
    }

    Ok(PaginatedResponse {
        data: items,
        total_count,
    })
}

pub async fn get_portfolio(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
) -> AppResult<PortfolioSummary> {
    // Compute portfolio_value: SUM(balance * token_price) across all held tokens
    let portfolio_value = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT COALESCE(SUM(b.balance * COALESCE(m.token_price, 0)), 0)::TEXT
        FROM balances b
        LEFT JOIN market_data m ON m.token_id = b.token_id
        WHERE b.account_id = $1 AND b.balance > 0
        "#,
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or_else(|| "0".to_string());

    let total_invested_ido = sqlx::query_scalar::<_, Option<String>>(
        "SELECT COALESCE(SUM(usdc_amount), 0)::TEXT FROM investments WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or_else(|| "0".to_string());

    let refunds_received = sqlx::query_scalar::<_, Option<String>>(
        "SELECT COALESCE(SUM(usdc_returned), 0)::TEXT FROM refunds WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or_else(|| "0".to_string());

    let active_idos = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT COUNT(DISTINCT i.project_id)
        FROM investments i
        JOIN projects p ON p.project_id = i.project_id
        WHERE i.account_id = $1 AND p.status IN ('funding', 'active')
        "#,
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or(0);

    // trading_pnl is a placeholder for MVP
    let trading_pnl = "0".to_string();
    let trading_pnl_percent = 0.0;

    Ok(PortfolioSummary {
        portfolio_value,
        total_invested_ido,
        trading_pnl,
        trading_pnl_percent,
        active_idos,
        refunds_received,
    })
}

pub async fn get_created_tokens(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<CreatedTokenInfo>> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, CreatedTokenRow>(
        r#"
        SELECT project_id, name, symbol, image_uri, status, created_at
        FROM projects
        WHERE creator = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM projects WHERE creator = $1",
    )
    .bind(account_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or(0);

    let data = rows
        .into_iter()
        .map(|r| CreatedTokenInfo {
            project_id: r.project_id,
            name: r.name,
            symbol: r.symbol,
            image_uri: r.image_uri,
            status: r.status,
            created_at: r.created_at,
        })
        .collect();

    Ok(PaginatedResponse {
        data,
        total_count: total,
    })
}

/// Get account info for authenticated user.
pub async fn get_account(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
) -> AppResult<IAccountInfo> {
    account::find_by_id(db.reader(), account_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Account not found".to_string()))
}

// --- Private helpers ---

async fn build_hold_token_item(
    db: &Arc<PostgresDatabase>,
    account_id: &str,
    row: &balance::BalanceRow,
) -> AppResult<HoldTokenItem> {
    let project_row = project::find_by_id(db.reader(), &row.token_id)
        .await
        .map_err(AppError::Internal)?;

    let market_row = market::find_by_token(db.reader(), &row.token_id)
        .await
        .map_err(AppError::Internal)?;

    let token_info = build_token_info_from_project(db, &row.token_id, project_row.as_ref()).await?;
    let market_info = build_market_info(&row.token_id, market_row.as_ref());

    let token_price = market_row
        .as_ref()
        .map(|m| m.token_price.clone())
        .unwrap_or_else(|| "0".to_string());
    let native_price = market_row
        .as_ref()
        .map(|m| m.native_price.clone())
        .unwrap_or_else(|| "0".to_string());

    let balance_info = BalanceInfo {
        balance: row.balance.clone(),
        token_price,
        native_price,
        created_at: row.updated_at,
    };

    // Determine origin: "ido" if investment exists, otherwise "trade"
    let has_investment = sqlx::query_scalar::<_, Option<bool>>(
        "SELECT EXISTS(SELECT 1 FROM investments WHERE account_id = $1 AND project_id = $2)",
    )
    .bind(account_id)
    .bind(&row.token_id)
    .fetch_one(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .unwrap_or(false);

    let origin = if has_investment { "ido" } else { "trade" }.to_string();

    let milestone_progress = build_milestone_progress(db, &row.token_id).await?;

    Ok(HoldTokenItem {
        token_info,
        market_info,
        balance_info,
        origin,
        milestone_progress,
    })
}

async fn build_ido_participation(
    db: &Arc<PostgresDatabase>,
    row: &investment::IdoHistoryRow,
) -> AppResult<IdoParticipation> {
    let (project_info, project_market_info) = build_project_infos(db, &row.project_id).await?;
    let milestone_progress = build_milestone_progress(db, &row.project_id).await?;

    Ok(IdoParticipation {
        project_info,
        market_info: project_market_info,
        invested_amount: row.usdc_amount.clone(),
        tokens_received: row.token_amount.clone(),
        status: row.status.clone(),
        milestone_progress,
        created_at: row.created_at,
    })
}

async fn build_refund_history_item(
    db: &Arc<PostgresDatabase>,
    row: &refund::EnrichedRefundRow,
) -> AppResult<RefundHistoryItem> {
    let (project_info, project_market_info) = build_project_infos(db, &row.project_id).await?;

    Ok(RefundHistoryItem {
        project_info,
        market_info: project_market_info,
        original_investment: row.original_investment.clone(),
        refund_amount: row.usdc_returned.clone(),
        tokens_burned: row.tokens_burned.clone(),
        failed_milestone: row.failed_milestone.clone(),
        transaction_hash: row.tx_hash.clone(),
        created_at: row.created_at,
    })
}

async fn build_project_infos(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
) -> AppResult<(IProjectInfo, IProjectMarketInfo)> {
    let project_row = project::find_by_id(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?;

    match project_row {
        Some(p) => {
            let creator = account::find_by_id(db.reader(), &p.creator)
                .await
                .map_err(AppError::Internal)?
                .unwrap_or_else(|| IAccountInfo::new(p.creator.clone()));

            let investor_count = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT COUNT(*) FROM investments WHERE project_id = $1",
            )
            .bind(project_id)
            .fetch_one(db.reader())
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .unwrap_or(0);

            let usdc_raised = p.usdc_raised.clone().unwrap_or_else(|| "0".to_string());
            let target_raise = p.target_raise.clone().unwrap_or_else(|| "0".to_string());
            let funded_percent = compute_funded_percent(&usdc_raised, &target_raise);

            let project_info = IProjectInfo {
                project_id: p.project_id.clone(),
                name: p.name.clone(),
                symbol: p.symbol.clone(),
                image_uri: p.image_uri.clone(),
                description: p.description.clone(),
                category: p.category.clone(),
                creator,
                website: p.website.clone(),
                twitter: p.twitter.clone(),
                github: p.github.clone(),
                telegram: p.telegram.clone(),
                created_at: p.created_at,
            };

            let project_market_info = IProjectMarketInfo {
                project_id: p.project_id.clone(),
                status: ProjectStatus::from_str(&p.status)
                    .unwrap_or(ProjectStatus::Funding),
                target_raise,
                total_committed: usdc_raised,
                funded_percent,
                investor_count,
            };

            Ok((project_info, project_market_info))
        }
        None => {
            let empty_project = IProjectInfo {
                project_id: project_id.to_string(),
                name: String::new(),
                symbol: String::new(),
                image_uri: String::new(),
                description: None,
                category: String::new(),
                creator: IAccountInfo::new(String::new()),
                website: None,
                twitter: None,
                github: None,
                telegram: None,
                created_at: 0,
            };
            let empty_market = IProjectMarketInfo {
                project_id: project_id.to_string(),
                status: ProjectStatus::Funding,
                target_raise: "0".to_string(),
                total_committed: "0".to_string(),
                funded_percent: 0.0,
                investor_count: 0,
            };
            Ok((empty_project, empty_market))
        }
    }
}

async fn build_token_info_from_project(
    db: &Arc<PostgresDatabase>,
    token_id: &str,
    project_row: Option<&project::ProjectRow>,
) -> AppResult<ITokenInfo> {
    match project_row {
        Some(p) => {
            let creator = account::find_by_id(db.reader(), &p.creator)
                .await
                .map_err(AppError::Internal)?
                .unwrap_or_else(|| IAccountInfo::new(p.creator.clone()));

            Ok(ITokenInfo {
                token_id: token_id.to_string(),
                name: p.name.clone(),
                symbol: p.symbol.clone(),
                image_uri: p.image_uri.clone(),
                banner_uri: None,
                description: p.description.clone(),
                category: p.category.clone(),
                is_graduated: false,
                creator,
                website: p.website.clone(),
                twitter: p.twitter.clone(),
                telegram: p.telegram.clone(),
                created_at: p.created_at,
                project_id: Some(p.project_id.clone()),
            })
        }
        None => Ok(ITokenInfo {
            token_id: token_id.to_string(),
            name: String::new(),
            symbol: String::new(),
            image_uri: String::new(),
            banner_uri: None,
            description: None,
            category: String::new(),
            is_graduated: false,
            creator: IAccountInfo::new(String::new()),
            website: None,
            twitter: None,
            telegram: None,
            created_at: 0,
            project_id: None,
        }),
    }
}

async fn build_milestone_progress(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
) -> AppResult<MilestoneProgress> {
    let milestones = milestone::find_by_project(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?;

    let total = milestones.len() as i32;
    let completed = milestones
        .iter()
        .filter(|m| {
            m.status == openlaunch_shared::types::milestone::MilestoneStatus::Completed
        })
        .count() as i32;

    Ok(MilestoneProgress { completed, total })
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
                native_price: m.native_price.clone(),
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
            native_price: "0".to_string(),
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

fn compute_funded_percent(raised: &str, target: &str) -> f64 {
    let raised_f: f64 = raised.parse().unwrap_or(0.0);
    let target_f: f64 = target.parse().unwrap_or(0.0);
    if target_f <= 0.0 {
        0.0
    } else {
        (raised_f / target_f * 100.0).min(100.0)
    }
}

fn parse_trade_type(s: &str) -> TradeType {
    match s.to_uppercase().as_str() {
        "BUY" => TradeType::Buy,
        _ => TradeType::Sell,
    }
}

#[cfg(test)]
mod tests_pure {
    use super::*;

    #[test]
    fn parse_trade_type_buy_uppercase() {
        assert!(matches!(parse_trade_type("BUY"), TradeType::Buy));
    }

    #[test]
    fn parse_trade_type_buy_lowercase() {
        assert!(matches!(parse_trade_type("buy"), TradeType::Buy));
    }

    #[test]
    fn parse_trade_type_buy_mixed_case() {
        assert!(matches!(parse_trade_type("Buy"), TradeType::Buy));
    }

    #[test]
    fn parse_trade_type_sell_uppercase() {
        assert!(matches!(parse_trade_type("SELL"), TradeType::Sell));
    }

    #[test]
    fn parse_trade_type_sell_lowercase() {
        assert!(matches!(parse_trade_type("sell"), TradeType::Sell));
    }

    #[test]
    fn parse_trade_type_unknown_defaults_to_sell() {
        assert!(matches!(parse_trade_type("unknown"), TradeType::Sell));
        assert!(matches!(parse_trade_type(""), TradeType::Sell));
    }

    #[test]
    fn compute_funded_percent_zero_target() {
        assert_eq!(compute_funded_percent("100", "0"), 0.0);
    }

    #[test]
    fn compute_funded_percent_half() {
        let result = compute_funded_percent("500", "1000");
        assert!((result - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_capped() {
        let result = compute_funded_percent("2000", "1000");
        assert!((result - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_invalid() {
        assert_eq!(compute_funded_percent("abc", "xyz"), 0.0);
    }

    #[test]
    fn build_market_info_none_returns_defaults() {
        let info = build_market_info("tok_abc", None);
        assert_eq!(info.token_id, "tok_abc");
        assert_eq!(info.token_price, "0");
        assert_eq!(info.holder_count, 0);
    }

    #[test]
    fn build_market_info_with_data() {
        let row = market::MarketDataRow {
            token_id: "tok_1".to_string(),
            market_type: "CURVE".to_string(),
            token_price: "1.50".to_string(),
            native_price: "0.001".to_string(),
            ath_price: "5.00".to_string(),
            total_supply: "1000000".to_string(),
            volume_24h: "50000".to_string(),
            holder_count: 42,
            bonding_percent: "75.5".to_string(),
            milestone_completed: 2,
            milestone_total: 5,
            is_graduated: false,
        };
        let info = build_market_info("tok_1", Some(&row));
        assert!(matches!(info.market_type, MarketType::Curve));
        assert_eq!(info.token_price, "1.50");
        assert_eq!(info.holder_count, 42);
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SwapRow {
    event_type: String,
    native_amount: String,
    token_amount: String,
    price: String,
    value: String,
    tx_hash: String,
    account_id: String,
    created_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CreatedTokenRow {
    project_id: String,
    name: String,
    symbol: String,
    image_uri: String,
    status: String,
    created_at: i64,
}
