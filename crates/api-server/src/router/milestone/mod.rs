use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use openlaunch_shared::error::AppResult;
use openlaunch_shared::types::milestone::MilestoneSubmitRequest;

use crate::middleware::auth::AuthUser;
use crate::services::milestone as milestone_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/submit/:milestoneId", post(submit_milestone))
        .route("/verification/:milestoneId", get(get_verification))
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
    AuthUser(_session): AuthUser,
    Path(milestone_id): Path<String>,
    Json(body): Json<MilestoneSubmitRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // TODO: Verify that _session.account_id is the project creator for this milestone
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
