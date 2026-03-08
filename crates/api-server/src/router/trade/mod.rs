use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::common::PaginationParams;
use openlaunch_shared::types::trading::ChartRequest;

use crate::services::trade as trade_service;
use crate::state::AppState;

use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chart/{tokenAddress}", get(get_chart))
        .route("/swap-history/{tokenId}", get(get_swap_history))
        .route("/holder/{tokenId}", get(get_holders))
        .route("/market/{tokenId}", get(get_market))
        .route("/metrics/{tokenId}", get(get_metrics))
        .route("/quote/{tokenId}", get(get_quote))
}

#[utoipa::path(
    get,
    path = "/trade/chart/{tokenAddress}",
    tag = "trade",
    params(
        ("tokenAddress" = String, Path, description = "Token contract address"),
        ("resolution" = String, Query, description = "Chart resolution"),
        ("from" = i64, Query, description = "Start timestamp"),
        ("to" = i64, Query, description = "End timestamp"),
        ("countback" = Option<i64>, Query, description = "Number of bars"),
        ("chart_type" = Option<String>, Query, description = "Chart type"),
    ),
    responses(
        (status = 200, description = "Chart data", body = serde_json::Value)
    )
)]
pub async fn get_chart(
    State(state): State<AppState>,
    Path(token_address): Path<String>,
    Query(params): Query<ChartRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let bars = trade_service::get_chart(&state.db, &token_address, &params).await?;
    Ok(Json(serde_json::json!({ "bars": bars })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SwapHistoryQuery {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    trade_type: Option<String>,
}

fn default_page() -> i64 { 1 }
fn default_limit() -> i64 { 20 }

#[utoipa::path(
    get,
    path = "/trade/swap-history/{tokenId}",
    tag = "trade",
    params(
        ("tokenId" = String, Path, description = "Token ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
        ("direction" = Option<String>, Query, description = "Sort direction (ASC/DESC)"),
        ("trade_type" = Option<String>, Query, description = "Filter by trade type (BUY/SELL)"),
    ),
    responses(
        (status = 200, description = "Swap history", body = serde_json::Value)
    )
)]
pub async fn get_swap_history(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
    Query(query): Query<SwapHistoryQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let pagination = PaginationParams {
        page: query.page,
        limit: query.limit,
    };
    let trade_type_filter = match query.trade_type.as_deref() {
        Some("BUY") => Some("BUY"),
        Some("SELL") => Some("SELL"),
        _ => None, // "ALL" or missing means no filter
    };
    let direction = query.direction.as_deref().unwrap_or("DESC");
    let result =
        trade_service::get_swap_history_ordered(
            &state.db, &token_id, &pagination, trade_type_filter, direction,
        ).await?;
    Ok(Json(serde_json::to_value(result).map_err(|e| AppError::Internal(e.into()))?))
}

#[utoipa::path(
    get,
    path = "/trade/holder/{tokenId}",
    tag = "trade",
    params(
        ("tokenId" = String, Path, description = "Token ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Token holders", body = serde_json::Value)
    )
)]
pub async fn get_holders(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        trade_service::get_holders(&state.db, &token_id, &pagination).await?;
    Ok(Json(serde_json::to_value(result).map_err(|e| AppError::Internal(e.into()))?))
}

#[utoipa::path(
    get,
    path = "/trade/market/{tokenId}",
    tag = "trade",
    params(("tokenId" = String, Path, description = "Token ID")),
    responses(
        (status = 200, description = "Market data", body = serde_json::Value)
    )
)]
pub async fn get_market(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let cache_key = format!("market:{token_id}");

    let data: serde_json::Value = state
        .cache
        .get_or_insert(&cache_key, || {
            let db = state.db.clone();
            let tid = token_id.clone();
            async move {
                let market = trade_service::get_market(&db, &tid).await?;
                Ok(serde_json::to_value(market)?)
            }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(data))
}

#[utoipa::path(
    get,
    path = "/trade/metrics/{tokenId}",
    tag = "trade",
    params(("tokenId" = String, Path, description = "Token ID")),
    responses(
        (status = 200, description = "Token metrics", body = serde_json::Value)
    )
)]
pub async fn get_metrics(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let cache_key = format!("metrics:{token_id}");

    let data: serde_json::Value = state
        .cache
        .get_or_insert(&cache_key, || {
            let db = state.db.clone();
            let tid = token_id.clone();
            async move {
                let metrics = trade_service::get_metrics(&db, &tid).await?;
                Ok(serde_json::to_value(metrics)?)
            }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(data))
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuoteQuery {
    #[serde(default)]
    amount: String,
    #[serde(default, rename = "type")]
    trade_type: String,
    #[serde(default = "default_slippage")]
    slippage: f64,
}

fn default_slippage() -> f64 { 3.0 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_query_defaults() {
        let query: QuoteQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.amount, "");
        assert_eq!(query.trade_type, "");
        assert!((query.slippage - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn quote_query_with_values() {
        let query: QuoteQuery =
            serde_json::from_str(r#"{"amount":"100.5","type":"BUY","slippage":1.5}"#).unwrap();
        assert_eq!(query.amount, "100.5");
        assert_eq!(query.trade_type, "BUY");
        assert!((query.slippage - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn quote_query_partial_values() {
        let query: QuoteQuery = serde_json::from_str(r#"{"amount":"50","type":"SELL"}"#).unwrap();
        assert_eq!(query.amount, "50");
        assert_eq!(query.trade_type, "SELL");
        assert!((query.slippage - 3.0).abs() < f64::EPSILON);
    }
}

#[utoipa::path(
    get,
    path = "/trade/quote/{tokenId}",
    tag = "trade",
    params(
        ("tokenId" = String, Path, description = "Token ID"),
        ("amount" = Option<String>, Query, description = "Trade amount"),
        ("type" = Option<String>, Query, description = "Trade type (BUY/SELL)"),
        ("slippage" = Option<f64>, Query, description = "Slippage tolerance (default 3.0)"),
    ),
    responses(
        (status = 200, description = "Quote data", body = serde_json::Value)
    )
)]
pub async fn get_quote(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
    Query(query): Query<QuoteQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let is_buy = query.trade_type.to_uppercase() != "SELL";
    let quote =
        trade_service::get_quote(
            &state.db, &token_id, &query.amount, is_buy, query.slippage,
        )
            .await?;
    Ok(Json(serde_json::to_value(quote).map_err(|e| AppError::Internal(e.into()))?))
}
