use axum::{
    extract::{FromRequestParts, Request},
    http::request::Parts,
    middleware::Next,
    response::Response,
};

use openlaunch_shared::error::AppError;
use openlaunch_shared::types::auth::SessionInfo;

use crate::state::AppState;

/// Middleware that extracts session from cookie and injects SessionInfo into extensions.
/// Does NOT reject unauthenticated requests - just adds SessionInfo if valid.
pub async fn session_middleware(
    state: axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let mut request = request;

    if let Some(session_id) = extract_session_cookie(request.headers()) {
        if let Ok(Some(info)) = state.redis.get_session(&session_id).await {
            if !info.is_expired() {
                request.extensions_mut().insert(info);
            }
        }
    }

    next.run(request).await
}

fn extract_session_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .find_map(|cookie| {
            let cookie = cookie.trim();
            cookie
                .strip_prefix("session=")
                .map(|v| v.to_string())
        })
}

/// Extractor that requires a valid session. Returns 401 if not authenticated.
#[derive(Debug, Clone)]
pub struct AuthUser(pub SessionInfo);

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<SessionInfo>()
            .cloned()
            .map(AuthUser)
            .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, header};

    #[test]
    fn extract_session_cookie_finds_session() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("session=abc123; other=xyz"),
        );
        let result = extract_session_cookie(&headers);
        assert_eq!(result, Some("abc123".to_string()));
    }

    #[test]
    fn extract_session_cookie_returns_none_when_missing() {
        let headers = HeaderMap::new();
        let result = extract_session_cookie(&headers);
        assert_eq!(result, None);
    }

    #[test]
    fn extract_session_cookie_returns_none_for_other_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; lang=en"),
        );
        let result = extract_session_cookie(&headers);
        assert_eq!(result, None);
    }

    #[test]
    fn extract_session_cookie_handles_multiple_cookie_headers() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("theme=dark"),
        );
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("session=my_session_id"),
        );
        let result = extract_session_cookie(&headers);
        assert_eq!(result, Some("my_session_id".to_string()));
    }

    #[test]
    fn extract_session_cookie_trims_cookie_name_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=a;  session=trimmed_val"),
        );
        let result = extract_session_cookie(&headers);
        // trim() removes leading whitespace before "session=", value is extracted as-is
        assert_eq!(result, Some("trimmed_val".to_string()));
    }

    #[test]
    fn extract_session_cookie_takes_first_session() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("session=first; session=second"),
        );
        let result = extract_session_cookie(&headers);
        assert_eq!(result, Some("first".to_string()));
    }

    #[test]
    fn extract_session_cookie_handles_empty_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("session="),
        );
        let result = extract_session_cookie(&headers);
        assert_eq!(result, Some("".to_string()));
    }

    #[tokio::test]
    async fn auth_user_extractor_fails_without_session() {
        let mut parts = axum::http::Request::builder()
            .body(())
            .unwrap()
            .into_parts()
            .0;

        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Unauthorized(msg) => {
                assert_eq!(msg, "Authentication required");
            }
            other => panic!("Expected Unauthorized, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn auth_user_extractor_succeeds_with_session() {
        let mut parts = axum::http::Request::builder()
            .body(())
            .unwrap()
            .into_parts()
            .0;

        let session = SessionInfo {
            session_id: "sid-123".to_string(),
            account_id: "0xabc".to_string(),
            created_at: 1000,
            expires_at: 9999999999,
        };
        parts.extensions.insert(session.clone());

        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        let auth_user = result.unwrap();
        assert_eq!(auth_user.0.session_id, "sid-123");
        assert_eq!(auth_user.0.account_id, "0xabc");
    }
}
