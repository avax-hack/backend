use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict")]
    Conflict,

    #[error("Too many requests")]
    TooManyRequests { retry_after: u64 },

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, code) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone(), "BAD_REQUEST"),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone(), "UNAUTHORIZED"),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone(), "FORBIDDEN"),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone(), "NOT_FOUND"),
            AppError::Conflict => (StatusCode::CONFLICT, "Resource conflict".to_string(), "CONFLICT"),
            AppError::TooManyRequests { retry_after } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    axum::Json(json!({
                        "error": "Too many requests",
                        "code": "TOO_MANY_REQUESTS",
                        "retry_after": retry_after
                    })),
                ).into_response();
                response.headers_mut().insert(
                    "Retry-After",
                    retry_after.to_string().parse().unwrap(),
                );
                return response;
            }
            AppError::Internal(err) => {
                tracing::error!("Internal error: {err:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), "INTERNAL_ERROR")
            }
        };

        (
            status,
            axum::Json(json!({
                "error": error_message,
                "code": code
            })),
        ).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn test_bad_request_display() {
        let err = AppError::BadRequest("invalid input".to_string());
        assert_eq!(err.to_string(), "Bad request: invalid input");
    }

    #[test]
    fn test_unauthorized_display() {
        let err = AppError::Unauthorized("no token".to_string());
        assert_eq!(err.to_string(), "Unauthorized: no token");
    }

    #[test]
    fn test_forbidden_display() {
        let err = AppError::Forbidden("access denied".to_string());
        assert_eq!(err.to_string(), "Forbidden: access denied");
    }

    #[test]
    fn test_not_found_display() {
        let err = AppError::NotFound("resource missing".to_string());
        assert_eq!(err.to_string(), "Not found: resource missing");
    }

    #[test]
    fn test_conflict_display() {
        let err = AppError::Conflict;
        assert_eq!(err.to_string(), "Conflict");
    }

    #[test]
    fn test_too_many_requests_display() {
        let err = AppError::TooManyRequests { retry_after: 30 };
        assert_eq!(err.to_string(), "Too many requests");
    }

    #[test]
    fn test_internal_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("db connection failed");
        let err: AppError = anyhow_err.into();
        assert!(matches!(err, AppError::Internal(_)));
        assert_eq!(err.to_string(), "Internal error: db connection failed");
    }

    #[test]
    fn test_bad_request_response_status() {
        let err = AppError::BadRequest("bad".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_unauthorized_response_status() {
        let err = AppError::Unauthorized("no auth".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_forbidden_response_status() {
        let err = AppError::Forbidden("forbidden".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_not_found_response_status() {
        let err = AppError::NotFound("missing".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_conflict_response_status() {
        let err = AppError::Conflict;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_too_many_requests_response_status_and_header() {
        let err = AppError::TooManyRequests { retry_after: 45 };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_header = response.headers().get("Retry-After").unwrap();
        assert_eq!(retry_header.to_str().unwrap(), "45");
    }

    #[test]
    fn test_internal_response_status() {
        let err = AppError::Internal(anyhow::anyhow!("oops"));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_app_result_ok() {
        let result: AppResult<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_app_result_err() {
        let result: AppResult<i32> = Err(AppError::BadRequest("nope".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_error_debug_format() {
        let err = AppError::BadRequest("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("BadRequest"));
    }

    #[tokio::test]
    async fn test_internal_error_does_not_leak_message() {
        use http_body_util::BodyExt;

        let secret_msg = "database password is hunter2";
        let err = AppError::Internal(anyhow::anyhow!(secret_msg));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // The response body must say "Internal server error", NOT the real error
        assert_eq!(body["error"], "Internal server error");
        assert!(!body["error"].as_str().unwrap().contains(secret_msg));
    }

    #[tokio::test]
    async fn test_response_body_json_structure() {
        use http_body_util::BodyExt;

        let err = AppError::BadRequest("invalid field".to_string());
        let response = err.into_response();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Must contain "error" and "code" fields
        assert!(body.get("error").is_some(), "response body missing 'error' field");
        assert!(body.get("code").is_some(), "response body missing 'code' field");
        assert_eq!(body["error"], "invalid field");
        assert_eq!(body["code"], "BAD_REQUEST");
    }

    #[tokio::test]
    async fn test_too_many_requests_response_body_json_structure() {
        use http_body_util::BodyExt;

        let err = AppError::TooManyRequests { retry_after: 60 };
        let response = err.into_response();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert!(body.get("error").is_some(), "response body missing 'error' field");
        assert!(body.get("code").is_some(), "response body missing 'code' field");
        assert_eq!(body["error"], "Too many requests");
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
        assert_eq!(body["retry_after"], 60);
    }

    #[tokio::test]
    async fn test_not_found_response_body_contains_message() {
        use http_body_util::BodyExt;

        let err = AppError::NotFound("project xyz not found".to_string());
        let response = err.into_response();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["error"], "project xyz not found");
        assert_eq!(body["code"], "NOT_FOUND");
    }
}
