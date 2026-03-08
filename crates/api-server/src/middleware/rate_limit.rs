use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use openlaunch_shared::error::AppError;

use crate::state::AppState;

const DEFAULT_MAX_REQUESTS: u64 = 60;

/// Redis-based per-IP rate limiting middleware.
/// Default: 60 requests per minute per IP.
/// Returns 429 with Retry-After header when exceeded.
pub async fn rate_limit_middleware(
    state: axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&request);

    match state.redis.check_rate_limit(&ip, DEFAULT_MAX_REQUESTS).await {
        Ok(result) if result.allowed => next.run(request).await,
        Ok(result) => {
            AppError::TooManyRequests {
                retry_after: result.retry_after,
            }
            .into_response()
        }
        Err(err) => {
            tracing::error!("Rate limit check failed, denying request (fail closed): {err}");
            AppError::ServiceUnavailable("Service temporarily unavailable".to_string())
                .into_response()
        }
    }
}

fn extract_client_ip(request: &Request) -> String {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, Request as HttpRequest};

    fn build_request_with_headers(headers: Vec<(&str, &str)>) -> Request {
        let mut builder = HttpRequest::builder().method("GET").uri("/test");
        for (key, value) in headers {
            builder = builder.header(key, HeaderValue::from_str(value).unwrap());
        }
        builder.body(axum::body::Body::empty()).unwrap()
    }

    #[test]
    fn extract_ip_from_x_forwarded_for_single() {
        let req = build_request_with_headers(vec![("x-forwarded-for", "192.168.1.1")]);
        assert_eq!(extract_client_ip(&req), "192.168.1.1");
    }

    #[test]
    fn extract_ip_from_x_forwarded_for_multiple() {
        let req = build_request_with_headers(vec![
            ("x-forwarded-for", "10.0.0.1, 10.0.0.2, 10.0.0.3"),
        ]);
        assert_eq!(extract_client_ip(&req), "10.0.0.1");
    }

    #[test]
    fn extract_ip_from_x_forwarded_for_with_spaces() {
        let req = build_request_with_headers(vec![
            ("x-forwarded-for", "  203.0.113.50 , 70.41.3.18"),
        ]);
        assert_eq!(extract_client_ip(&req), "203.0.113.50");
    }

    #[test]
    fn extract_ip_falls_back_to_x_real_ip() {
        let req = build_request_with_headers(vec![("x-real-ip", "172.16.0.5")]);
        assert_eq!(extract_client_ip(&req), "172.16.0.5");
    }

    #[test]
    fn extract_ip_prefers_x_forwarded_for_over_x_real_ip() {
        let req = build_request_with_headers(vec![
            ("x-forwarded-for", "10.0.0.1"),
            ("x-real-ip", "172.16.0.5"),
        ]);
        assert_eq!(extract_client_ip(&req), "10.0.0.1");
    }

    #[test]
    fn extract_ip_returns_unknown_when_no_headers() {
        let req = build_request_with_headers(vec![]);
        assert_eq!(extract_client_ip(&req), "unknown");
    }

    #[test]
    fn default_max_requests_is_sixty() {
        assert_eq!(DEFAULT_MAX_REQUESTS, 60);
    }
}
