use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use openlaunch_shared::error::AppResult;
use openlaunch_shared::types::milestone::MilestoneSubmitRequest;

use crate::middleware::auth::AuthUser;
use crate::services::milestone as milestone_service;
use crate::services::milestone::verify_milestone_ownership;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/submit/{milestoneId}", post(submit_milestone))
        .route("/verification/{milestoneId}", get(get_verification))
}

#[utoipa::path(
    post,
    path = "/milestone/submit/{milestoneId}",
    tag = "milestone",
    params(("milestoneId" = String, Path, description = "Milestone ID")),
    request_body = MilestoneSubmitRequest,
    responses(
        (status = 200, description = "Milestone submitted", body = serde_json::Value),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn submit_milestone(
    State(state): State<AppState>,
    AuthUser(session): AuthUser,
    Path(milestone_id): Path<String>,
    Json(body): Json<MilestoneSubmitRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify that the authenticated user is the project creator for this milestone
    verify_milestone_ownership(&state.db, &milestone_id, &session.account_id).await?;

    milestone_service::submit_evidence(&state.db, &milestone_id, &body).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[utoipa::path(
    get,
    path = "/milestone/verification/{milestoneId}",
    tag = "milestone",
    params(("milestoneId" = String, Path, description = "Milestone ID")),
    responses(
        (status = 200, description = "Milestone verification data", body = serde_json::Value)
    )
)]
pub async fn get_verification(
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let data =
        milestone_service::get_verification(&state.db, &milestone_id).await?;
    Ok(Json(serde_json::json!(data)))
}
