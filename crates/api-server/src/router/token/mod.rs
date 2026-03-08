use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::common::PaginationParams;
use openlaunch_shared::types::token::ITokenData;

use crate::services::token as token_service;
use crate::state::AppState;

use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/{tokenId}", get(get_token))
}

pub fn order_router() -> Router<AppState> {
    Router::new()
        .route("/project/{sortType}", get(get_project_list))
        .route("/{sortType}", get(get_token_list))
}

pub fn trend_router() -> Router<AppState> {
    Router::new().route("/", get(get_trend))
}

#[utoipa::path(
    get,
    path = "/token/{tokenId}",
    tag = "token",
    params(("tokenId" = String, Path, description = "Token ID")),
    responses(
        (status = 200, description = "Token details", body = serde_json::Value),
        (status = 404, description = "Token not found")
    )
)]
pub async fn get_token(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> AppResult<Json<ITokenData>> {
    let cache_key = format!("token:{token_id}");

    let data: ITokenData = state
        .cache
        .get_or_insert(&cache_key, || {
            let db = state.db.clone();
            let tid = token_id.clone();
            async move { token_service::get_token(&db, &tid).await.map_err(Into::into) }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(data))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectListQuery {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    status: Option<String>,
}

fn default_page() -> i64 { 1 }
fn default_limit() -> i64 { 20 }

#[utoipa::path(
    get,
    path = "/order/project/{sortType}",
    tag = "token",
    params(
        ("sortType" = String, Path, description = "Sort type"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
        ("status" = Option<String>, Query, description = "Project status filter"),
    ),
    responses(
        (status = 200, description = "Paginated project list", body = serde_json::Value)
    )
)]
pub async fn get_project_list(
    State(state): State<AppState>,
    Path(sort_type): Path<String>,
    Query(query): Query<ProjectListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let pagination = PaginationParams {
        page: query.page,
        limit: query.limit,
    };
    let result = crate::services::project::get_project_list(
        &state.db,
        &sort_type,
        &pagination,
        query.status.as_deref(),
    )
    .await?;

    Ok(Json(serde_json::json!(result)))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenListQuery {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    verified_only: Option<bool>,
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    is_ido: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/order/{sortType}",
    tag = "token",
    params(
        ("sortType" = String, Path, description = "Sort type"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
        ("category" = Option<String>, Query, description = "Category filter"),
        ("verified_only" = Option<bool>, Query, description = "Show only verified tokens"),
        ("search" = Option<String>, Query, description = "Search query"),
        ("is_ido" = Option<bool>, Query, description = "Filter by IDO status: true=funding, false=graduated"),
    ),
    responses(
        (status = 200, description = "Paginated token list", body = serde_json::Value)
    )
)]
pub async fn get_token_list(
    State(state): State<AppState>,
    Path(sort_type): Path<String>,
    Query(query): Query<TokenListQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let verified = query.verified_only.unwrap_or(false);
    let status_filter = match query.is_ido {
        Some(true) => Some("funding"),
        Some(false) => Some("active"),
        None => None,
    };
    let pagination = PaginationParams {
        page: query.page,
        limit: query.limit,
    };
    let cache_key = format!(
        "token_list|{}|{}|{}|{}|{}|{}|{}",
        sort_type,
        pagination.page,
        pagination.limit,
        query.category.as_deref().unwrap_or(""),
        verified,
        query.search.as_deref().unwrap_or(""),
        query.is_ido.map(|v| v.to_string()).unwrap_or_default(),
    );

    let cat = query.category.clone();
    let search = query.search.clone();

    let result: serde_json::Value = state
        .cache
        .get_or_insert(&cache_key, || {
            let db = state.db.clone();
            let st = sort_type.clone();
            let p = pagination.clone();
            let c = cat.clone();
            let s = search.clone();
            let sf = status_filter.map(|s| s.to_string());
            async move {
                let data = token_service::get_token_list_filtered(
                    &db, &st, &p, c.as_deref(), s.as_deref(), verified, sf.as_deref(),
                ).await?;
                Ok(serde_json::to_value(data)?)
            }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/trend/",
    tag = "token",
    responses(
        (status = 200, description = "Trending tokens", body = serde_json::Value)
    )
)]
pub async fn get_trend(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let data: serde_json::Value = state
        .cache
        .get_or_insert("token:trending", || {
            let db = state.db.clone();
            async move {
                let items = token_service::get_trending(&db).await?;
                Ok(serde_json::to_value(items)?)
            }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(data))
}
