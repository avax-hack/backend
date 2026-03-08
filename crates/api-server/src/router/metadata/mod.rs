use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};

use crate::middleware::auth::AuthUser;
use crate::services::metadata as metadata_service;
use crate::services::metadata::CreateMetadataRequest;
use crate::services::upload as upload_service;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/image", post(upload_image))
        .route("/evidence", post(upload_evidence))
        .route("/create", post(create_metadata))
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
    State(state): State<AppState>,
    AuthUser(_session): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let (_filename, data) = extract_file(&mut multipart).await?;
    let uri = upload_service::upload_image(&state.r2, &data).await?;
    Ok(Json(serde_json::json!({ "image_uri": uri })))
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
    State(state): State<AppState>,
    AuthUser(_session): AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let (filename, data) = extract_file(&mut multipart).await?;
    let uri = upload_service::upload_evidence(&state.r2, &filename, &data).await?;
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

            return Ok((filename, data.to_vec()));
        }
    }

    Err(AppError::BadRequest(
        "No file field found in multipart form".to_string(),
    ))
}

#[utoipa::path(
    post,
    path = "/metadata/create",
    tag = "metadata",
    request_body = CreateMetadataRequest,
    responses(
        (status = 200, description = "Metadata created", body = serde_json::Value),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn create_metadata(
    State(state): State<AppState>,
    AuthUser(_session): AuthUser,
    Json(body): Json<CreateMetadataRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let uri = metadata_service::create_metadata(&state.r2, &body).await?;
    Ok(Json(serde_json::json!({ "metadata_uri": uri })))
}
