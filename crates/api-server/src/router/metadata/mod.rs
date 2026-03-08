use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};

use crate::middleware::auth::AuthUser;
use crate::services::upload as upload_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/image", post(upload_image))
        .route("/evidence", post(upload_evidence))
}

#[utoipa::path(
    post,
    path = "/metadata/image",
    tag = "metadata",
    request_body(content = String, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Image uploaded", body = serde_json::Value),
        (status = 400, description = "Invalid file"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn upload_image(
    State(_state): State<AppState>,
    AuthUser(_session): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let (filename, data) = extract_file(&mut multipart).await?;
    let uri = upload_service::upload_image(&filename, &data).await?;
    Ok(Json(serde_json::json!({ "uri": uri })))
}

#[utoipa::path(
    post,
    path = "/metadata/evidence",
    tag = "metadata",
    request_body(content = String, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Evidence uploaded", body = serde_json::Value),
        (status = 400, description = "Invalid file"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn upload_evidence(
    State(_state): State<AppState>,
    AuthUser(_session): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let (filename, data) = extract_file(&mut multipart).await?;
    let uri = upload_service::upload_evidence(&filename, &data).await?;
    Ok(Json(serde_json::json!({ "uri": uri })))
}

async fn extract_file(multipart: &mut Multipart) -> AppResult<(String, Vec<u8>)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {e}")))?
    {
        if field.name() == Some("file") {
            let filename = field
                .file_name()
                .unwrap_or("upload")
                .to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read file: {e}")))?;

            if data.is_empty() {
                return Err(AppError::BadRequest("Empty file".to_string()));
            }

            return Ok((filename, data.to_vec()));
        }
    }

    Err(AppError::BadRequest(
        "No file field found in multipart form".to_string(),
    ))
}
