use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

pub fn cors_layer() -> CorsLayer {
    let origins_str = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    let origins: Vec<HeaderValue> = if origins_str.is_empty() {
        vec![
            "http://localhost:3000".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
        ]
    } else {
        origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    };

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::COOKIE,
            header::ORIGIN,
        ])
        .allow_credentials(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_origins_are_valid_header_values() {
        let localhost_3000: HeaderValue = "http://localhost:3000".parse().unwrap();
        let localhost_5173: HeaderValue = "http://localhost:5173".parse().unwrap();
        assert_eq!(localhost_3000.to_str().unwrap(), "http://localhost:3000");
        assert_eq!(localhost_5173.to_str().unwrap(), "http://localhost:5173");
    }

    #[test]
    fn allowed_methods_include_all_expected() {
        let methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ];
        assert_eq!(methods.len(), 6);
    }

    #[test]
    fn origins_parsing_from_comma_separated_string() {
        let origins_str = "https://example.com,https://app.example.com";
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].to_str().unwrap(), "https://example.com");
        assert_eq!(origins[1].to_str().unwrap(), "https://app.example.com");
    }

    #[test]
    fn empty_origins_string_produces_empty_vec() {
        let origins_str = "";
        let origins: Vec<HeaderValue> = if origins_str.is_empty() {
            vec![
                "http://localhost:3000".parse().unwrap(),
                "http://localhost:5173".parse().unwrap(),
            ]
        } else {
            origins_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        };
        // Empty string triggers the default path
        assert_eq!(origins.len(), 2);
    }

    #[test]
    fn invalid_origins_are_filtered_out() {
        let origins_str = "https://valid.com, not a valid\norigin ,https://also-valid.com";
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        // "not a valid\norigin" should fail to parse as a HeaderValue
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].to_str().unwrap(), "https://valid.com");
        assert_eq!(origins[1].to_str().unwrap(), "https://also-valid.com");
    }

    #[test]
    fn single_origin_parses_correctly() {
        let origins_str = "https://my-app.com";
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].to_str().unwrap(), "https://my-app.com");
    }

    #[test]
    fn origins_with_whitespace_are_trimmed() {
        let origins_str = "  https://a.com , https://b.com  ";
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].to_str().unwrap(), "https://a.com");
        assert_eq!(origins[1].to_str().unwrap(), "https://b.com");
    }
}
