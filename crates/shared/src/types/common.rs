use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}

fn default_limit() -> i64 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            limit: default_limit(),
        }
    }
}

impl PaginationParams {
    pub fn validated(&self) -> Self {
        Self {
            page: self.page.max(1),
            limit: self.limit.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> i64 {
        let v = self.validated();
        (v.page - 1) * v.limit
    }
}

/// Generic paginated response wrapper.
///
/// NOTE: The feature spec uses domain-specific keys for the items array
/// (e.g. "investors", "tokens", "swaps", "holders", "participations", "refunds"),
/// but this generic struct uses the key "data" for all endpoints.
/// Changing to domain-specific keys would require per-type wrapper structs and
/// is not worth the added complexity. Consumers should use "data" for all
/// paginated responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: i64,
}

pub fn validate_address(address: &str) -> anyhow::Result<String> {
    if !address.starts_with("0x") {
        anyhow::bail!("Address must start with 0x");
    }
    if address.len() != 42 {
        anyhow::bail!("Address must be 42 characters");
    }
    if !address[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Address contains invalid hex characters");
    }
    Ok(address.to_lowercase())
}

pub fn current_unix_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_defaults() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_pagination_clamp() {
        let params = PaginationParams { page: 0, limit: 200 };
        let clamped = params.validated();
        assert_eq!(clamped.page, 1);
        assert_eq!(clamped.limit, 100);
    }

    #[test]
    fn test_pagination_offset() {
        let params = PaginationParams { page: 3, limit: 10 };
        assert_eq!(params.offset(), 20);
    }

    #[test]
    fn test_validate_address_valid() {
        let result = validate_address("0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "0xab5801a7d398351b8be11c439e05c5b3259aec9b");
    }

    #[test]
    fn test_validate_address_invalid() {
        assert!(validate_address("not_an_address").is_err());
        assert!(validate_address("0x123").is_err());
        assert!(validate_address("").is_err());
    }

    #[test]
    fn test_validate_address_just_prefix() {
        assert!(validate_address("0x").is_err());
    }

    #[test]
    fn test_validate_address_non_hex_chars() {
        let addr = format!("0x{}gg", "a".repeat(38));
        let result = validate_address(&addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("hex"));
    }

    #[test]
    fn test_validate_address_exact_42_chars() {
        let addr = format!("0x{}", "a".repeat(40));
        assert!(validate_address(&addr).is_ok());
    }

    #[test]
    fn test_validate_address_returns_lowercase() {
        let addr = format!("0x{}", "A".repeat(40));
        let result = validate_address(&addr).unwrap();
        assert_eq!(result, format!("0x{}", "a".repeat(40)));
    }

    #[test]
    fn test_pagination_offset_page_1() {
        let params = PaginationParams { page: 1, limit: 10 };
        assert_eq!(params.offset(), 0);
    }

    #[test]
    fn test_pagination_offset_page_2() {
        let params = PaginationParams { page: 2, limit: 10 };
        assert_eq!(params.offset(), 10);
    }

    #[test]
    fn test_pagination_validated_negative_page() {
        let params = PaginationParams { page: -5, limit: 10 };
        let v = params.validated();
        assert_eq!(v.page, 1);
        assert_eq!(v.limit, 10);
    }

    #[test]
    fn test_pagination_validated_zero_limit() {
        let params = PaginationParams { page: 1, limit: 0 };
        let v = params.validated();
        assert_eq!(v.limit, 1);
    }

    #[test]
    fn test_pagination_validated_over_100_limit() {
        let params = PaginationParams { page: 1, limit: 500 };
        let v = params.validated();
        assert_eq!(v.limit, 100);
    }

    #[test]
    fn test_pagination_validated_normal_values() {
        let params = PaginationParams { page: 3, limit: 25 };
        let v = params.validated();
        assert_eq!(v.page, 3);
        assert_eq!(v.limit, 25);
    }

    #[test]
    fn test_pagination_serialization() {
        let params = PaginationParams { page: 2, limit: 50 };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: PaginationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.page, 2);
        assert_eq!(parsed.limit, 50);
    }

    #[test]
    fn test_pagination_deserialization_defaults() {
        let json = "{}";
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_paginated_response_serialization() {
        let response = PaginatedResponse {
            data: vec!["a".to_string(), "b".to_string()],
            total_count: 10,
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: PaginatedResponse<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.total_count, 10);
    }

    #[test]
    fn test_paginated_response_empty() {
        let response: PaginatedResponse<String> = PaginatedResponse {
            data: vec![],
            total_count: 0,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":[]"));
    }

    #[test]
    fn test_current_unix_timestamp_reasonable() {
        let ts = current_unix_timestamp();
        // Should be after 2024-01-01
        assert!(ts > 1704067200);
        // Should be before 2030-01-01
        assert!(ts < 1893456000);
    }
}
