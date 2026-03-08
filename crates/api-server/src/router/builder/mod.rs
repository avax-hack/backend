use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use openlaunch_shared::db::postgres::controller::project;
use openlaunch_shared::error::{AppError, AppResult};

use crate::middleware::auth::AuthUser;
use crate::services::builder as builder_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/overview/{projectId}", get(get_overview))
        .route("/stats/{projectId}", get(get_stats))
}

/// Verify the authenticated user is the project owner.
async fn verify_project_owner(
    state: &AppState,
    project_id: &str,
    account_id: &str,
) -> AppResult<()> {
    let row = project::find_by_id(state.db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("Project {project_id} not found")))?;

    if row.creator.to_lowercase() != account_id.to_lowercase() {
        return Err(AppError::Forbidden(
            "Only the project creator can access builder endpoints".to_string(),
        ));
    }

    Ok(())
}

#[utoipa::path(
    get,
    path = "/builder/overview/{projectId}",
    tag = "builder",
    params(("projectId" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Builder overview", body = serde_json::Value),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not the project owner")
    )
)]
pub async fn get_overview(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Path(project_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let project_id = project_id.to_lowercase();
    verify_project_owner(&state, &project_id, &session.account_id).await?;
    let overview = builder_service::get_overview(&state.db, &project_id).await?;
    Ok(Json(serde_json::json!(overview)))
}

#[utoipa::path(
    get,
    path = "/builder/stats/{projectId}",
    tag = "builder",
    params(("projectId" = String, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Builder stats", body = serde_json::Value),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not the project owner")
    )
)]
pub async fn get_stats(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Path(project_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let project_id = project_id.to_lowercase();
    verify_project_owner(&state, &project_id, &session.account_id).await?;
    let stats = builder_service::get_stats(&state.db, &project_id).await?;
    Ok(Json(serde_json::json!(stats)))
}
