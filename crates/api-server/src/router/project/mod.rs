use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::common::{PaginatedResponse, PaginationParams};
use openlaunch_shared::types::project::{CreateProjectRequest, IProjectData, IProjectListItem};

use crate::middleware::auth::AuthUser;
use crate::services::project as project_service;
use crate::state::AppState;

use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/featured", get(get_featured))
        .route("/create", post(create_project))
        .route("/validate-symbol", get(validate_symbol))
        .route("/investor/{projectId}", get(get_investors))
        .route("/{projectId}", get(get_project))
}

#[utoipa::path(
    get,
    path = "/project/{projectId}",
    tag = "project",
    params(("projectId" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Project details", body = serde_json::Value),
        (status = 404, description = "Project not found")
    )
)]
pub async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> AppResult<Json<IProjectData>> {
    let cache_key = format!("project:{project_id}");

    let data = state
        .cache
        .get_or_insert(&cache_key, || {
            let db = state.db.clone();
            let pid = project_id.clone();
            async move { project_service::get_project(&db, &pid).await.map_err(Into::into) }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(data))
}

#[utoipa::path(
    get,
    path = "/project/featured",
    tag = "project",
    responses(
        (status = 200, description = "Featured projects", body = serde_json::Value)
    )
)]
pub async fn get_featured(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let data: Vec<IProjectListItem> = state
        .cache
        .get_or_insert("project:featured", || {
            let db = state.db.clone();
            async move { project_service::get_featured(&db).await.map_err(Into::into) }
        })
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(serde_json::json!({ "projects": data })))
}

#[utoipa::path(
    post,
    path = "/project/create",
    tag = "project",
    request_body = CreateProjectRequest,
    responses(
        (status = 200, description = "Project created", body = serde_json::Value),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn create_project(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Json(body): Json<CreateProjectRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let project_id =
        project_service::create_project(&state.db, &session.account_id, &body).await?;

    Ok(Json(serde_json::json!({
        "project_id": project_id
    })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SymbolQuery {
    symbol: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_query_deserialization() {
        let query: SymbolQuery = serde_json::from_str(r#"{"symbol":"ABC"}"#).unwrap();
        assert_eq!(query.symbol, "ABC");
    }

    #[test]
    fn symbol_query_missing_symbol_fails() {
        let result: Result<SymbolQuery, _> = serde_json::from_str("{}");
        assert!(result.is_err());
    }

    #[test]
    fn symbol_query_empty_symbol() {
        let query: SymbolQuery = serde_json::from_str(r#"{"symbol":""}"#).unwrap();
        assert_eq!(query.symbol, "");
    }
}

#[utoipa::path(
    get,
    path = "/project/validate-symbol",
    tag = "project",
    params(("symbol" = String, Query, description = "Token symbol to validate")),
    responses(
        (status = 200, description = "Symbol availability", body = serde_json::Value)
    )
)]
pub async fn validate_symbol(
    State(state): State<AppState>,
    Query(query): Query<SymbolQuery>,
) -> AppResult<Json<serde_json::Value>> {
    if query.symbol.is_empty() {
        return Err(AppError::BadRequest("symbol is required".to_string()));
    }
    if query.symbol.len() > 10 {
        return Err(AppError::BadRequest("symbol must be 10 characters or fewer".to_string()));
    }
    let is_available = project_service::validate_symbol(&state.db, &query.symbol).await?;

    Ok(Json(serde_json::json!({
        "available": is_available
    })))
}

#[utoipa::path(
    get,
    path = "/project/investor/{projectId}",
    tag = "project",
    params(
        ("projectId" = String, Path, description = "Project ID"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Project investors", body = serde_json::Value)
    )
)]
pub async fn get_investors(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> AppResult<Json<PaginatedResponse<project_service::InvestorInfo>>> {
    let result =
        project_service::get_investors(&state.db, &project_id, &pagination).await?;

    Ok(Json(result))
}
