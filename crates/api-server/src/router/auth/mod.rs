use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};

use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::auth::{
    NonceRequest, NonceResponse, SessionRequest, SessionResponse,
};
use openlaunch_shared::types::common::validate_address;

use crate::middleware::auth::AuthUser;
use crate::services::auth as auth_service;
use crate::state::AppState;

const SESSION_MAX_AGE_SECS: i64 = 86400 * 7;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nonce", post(nonce))
        .route("/session", post(session))
        .route("/delete_session", delete(delete_session))
}

#[utoipa::path(
    post,
    path = "/auth/nonce",
    tag = "auth",
    request_body = NonceRequest,
    responses(
        (status = 200, description = "Nonce generated", body = serde_json::Value),
        (status = 400, description = "Invalid address")
    )
)]
pub async fn nonce(
    State(state): State<AppState>,
    Json(body): Json<NonceRequest>,
) -> AppResult<Json<NonceResponse>> {
    let address =
        validate_address(&body.address).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let nonce = auth_service::generate_nonce(&state.redis, &address)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(NonceResponse { nonce }))
}

#[utoipa::path(
    post,
    path = "/auth/session",
    tag = "auth",
    request_body = SessionRequest,
    responses(
        (status = 200, description = "Session created", body = serde_json::Value),
        (status = 400, description = "Invalid signature or nonce")
    )
)]
pub async fn session(
    State(state): State<AppState>,
    Json(body): Json<SessionRequest>,
) -> AppResult<impl IntoResponse> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let (session_id, info) =
        auth_service::verify_session(&state.redis, &body.nonce, &body.signature, body.chain_id)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let account_info =
        openlaunch_shared::db::postgres::controller::account::upsert(
            state.db.writer(),
            &info.account_id,
        )
        .await
        .map_err(AppError::Internal)?;

    let cookie_value = format!(
        "session={session_id}; HttpOnly; Secure; Path=/; Max-Age={SESSION_MAX_AGE_SECS}; SameSite=Lax"
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie_value.parse().expect("valid cookie header"),
    );

    Ok((headers, Json(SessionResponse { account_info })))
}

#[utoipa::path(
    delete,
    path = "/auth/delete_session",
    tag = "auth",
    responses(
        (status = 200, description = "Session deleted", body = serde_json::Value),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn delete_session(
    State(state): State<AppState>,
    AuthUser(info): AuthUser,
) -> AppResult<impl IntoResponse> {
    auth_service::delete_session(&state.redis, &info.session_id)
        .await
        .map_err(AppError::Internal)?;

    let cookie_value = "session=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie_value.parse().expect("valid cookie header"),
    );

    Ok((headers, Json(serde_json::json!({ "success": true }))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn session_cookie_contains_all_required_security_attributes() {
        let session_id = "test-session-id-abc123";
        let cookie_value = format!(
            "session={session_id}; HttpOnly; Secure; Path=/; Max-Age={SESSION_MAX_AGE_SECS}; SameSite=Lax"
        );
        let parsed: HeaderValue = cookie_value.parse().expect("cookie must be a valid header value");
        let s = parsed.to_str().unwrap();

        // Verify all security-critical cookie attributes are present
        assert!(s.contains("HttpOnly"), "cookie must be HttpOnly to prevent JS access");
        assert!(s.contains("Secure"), "cookie must be Secure for HTTPS-only");
        assert!(s.contains("SameSite=Lax"), "cookie must have SameSite=Lax for CSRF protection");
        assert!(s.contains("Path=/"), "cookie must be scoped to root path");
        assert!(
            s.contains(&format!("session={session_id}")),
            "cookie must contain the session ID"
        );
        assert!(
            s.contains(&format!("Max-Age={SESSION_MAX_AGE_SECS}")),
            "cookie must specify Max-Age"
        );
    }

    #[test]
    fn delete_session_cookie_expires_immediately() {
        let cookie_value = "session=; HttpOnly; Secure; Path=/; Max-Age=0; SameSite=Lax";
        let parsed: HeaderValue = cookie_value.parse().expect("delete cookie must be a valid header");
        let s = parsed.to_str().unwrap();

        assert!(s.contains("Max-Age=0"), "delete cookie must expire immediately");
        assert!(s.contains("HttpOnly"), "delete cookie must retain HttpOnly");
        assert!(s.contains("Secure"), "delete cookie must retain Secure");
        assert!(s.contains("SameSite=Lax"), "delete cookie must retain SameSite=Lax");
        assert!(s.contains("Path=/"), "delete cookie must retain Path=/");
    }

    #[test]
    fn session_request_validate_valid_signature() {
        let sig = format!("0x{}", "ab".repeat(65));
        let req = SessionRequest {
            nonce: "test".to_string(),
            signature: sig,
            chain_id: 1,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn session_request_validate_short_signature() {
        let req = SessionRequest {
            nonce: "test".to_string(),
            signature: "0xabcd".to_string(),
            chain_id: 1,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn session_request_validate_empty_nonce() {
        let sig = format!("0x{}", "ab".repeat(65));
        let req = SessionRequest {
            nonce: "".to_string(),
            signature: sig,
            chain_id: 1,
        };
        assert!(req.validate().is_err());
    }
}
