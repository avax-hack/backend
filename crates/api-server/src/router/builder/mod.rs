use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use openlaunch_shared::error::AppResult;

use crate::services::builder as builder_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/overview/:projectId", get(get_overview))
        .route("/stats/:projectId", get(get_stats))
}

#[utoipa::path(
    get,
    path = "/builder/overview/{projectId}",
    tag = "builder",
    params(("projectId" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Builder overview", body = serde_json::Value)
    )
)]
pub async fn get_overview(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let overview = builder_service::get_overview(&state.db, &project_id).await?;
    Ok(Json(serde_json::json!(overview)))
}

#[utoipa::path(
    get,
    path = "/builder/stats/{projectId}",
    tag = "builder",
    params(("projectId" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Builder stats", body = serde_json::Value)
    )
)]
pub async fn get_stats(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let stats = builder_service::get_stats(&state.db, &project_id).await?;
    Ok(Json(serde_json::json!(stats)))
}
