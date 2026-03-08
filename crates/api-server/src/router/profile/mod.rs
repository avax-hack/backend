use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use openlaunch_shared::error::AppResult;
use openlaunch_shared::types::common::PaginationParams;

use crate::middleware::auth::AuthUser;
use crate::services::profile as profile_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:address", get(get_profile))
        .route("/hold-token/:accountId", get(get_hold_tokens))
        .route("/swap-history/:accountId", get(get_swap_history))
        .route("/ido-history/:accountId", get(get_ido_history))
        .route("/refund-history/:accountId", get(get_refund_history))
        .route("/portfolio/:accountId", get(get_portfolio))
        .route("/tokens/created/:accountId", get(get_created_tokens))
}

pub fn account_router() -> Router<AppState> {
    Router::new().route("/get_account", get(get_account))
}

#[utoipa::path(
    get,
    path = "/profile/{address}",
    tag = "profile",
    params(("address" = String, Path, description = "Wallet address")),
    responses(
        (status = 200, description = "User profile", body = serde_json::Value)
    )
)]
pub async fn get_profile(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let profile = profile_service::get_profile(&state.db, &address).await?;
    Ok(Json(serde_json::json!(profile)))
}

#[utoipa::path(
    get,
    path = "/profile/hold-token/{accountId}",
    tag = "profile",
    params(
        ("accountId" = String, Path, description = "Account ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Held tokens", body = serde_json::Value)
    )
)]
pub async fn get_hold_tokens(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        profile_service::get_hold_tokens(&state.db, &account_id, &pagination).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/profile/swap-history/{accountId}",
    tag = "profile",
    params(
        ("accountId" = String, Path, description = "Account ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Swap history", body = serde_json::Value)
    )
)]
pub async fn get_swap_history(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        profile_service::get_swap_history(&state.db, &account_id, &pagination).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/profile/ido-history/{accountId}",
    tag = "profile",
    params(
        ("accountId" = String, Path, description = "Account ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "IDO history", body = serde_json::Value)
    )
)]
pub async fn get_ido_history(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        profile_service::get_ido_history(&state.db, &account_id, &pagination).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/profile/refund-history/{accountId}",
    tag = "profile",
    params(
        ("accountId" = String, Path, description = "Account ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Refund history", body = serde_json::Value)
    )
)]
pub async fn get_refund_history(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        profile_service::get_refund_history(&state.db, &account_id, &pagination).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/profile/portfolio/{accountId}",
    tag = "profile",
    params(("accountId" = String, Path, description = "Account ID")),
    responses(
        (status = 200, description = "Portfolio data", body = serde_json::Value)
    )
)]
pub async fn get_portfolio(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let result = profile_service::get_portfolio(&state.db, &account_id).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/profile/tokens/created/{accountId}",
    tag = "profile",
    params(
        ("accountId" = String, Path, description = "Account ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Created tokens", body = serde_json::Value)
    )
)]
pub async fn get_created_tokens(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<serde_json::Value>> {
    let result =
        profile_service::get_created_tokens(&state.db, &account_id, &pagination).await?;
    Ok(Json(serde_json::json!(result)))
}

#[utoipa::path(
    get,
    path = "/account/get_account",
    tag = "profile",
    responses(
        (status = 200, description = "Current account info", body = serde_json::Value),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn get_account(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let account =
        profile_service::get_account(&state.db, &session.account_id).await?;
    Ok(Json(serde_json::json!(account)))
}
