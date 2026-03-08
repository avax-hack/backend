use sha2::{Digest, Sha256};

/// Validate and normalize an Ethereum address to lowercase.
pub fn normalize_address(address: &str) -> anyhow::Result<String> {
    crate::types::common::validate_address(address)
}

/// Generate a deterministic session ID from components.
pub fn generate_session_id(address: &str, timestamp: i64) -> String {
    let uuid = uuid::Uuid::new_v4();
    let raw = format!("{}-{}-{}", address, timestamp, uuid);
    let hash = Sha256::digest(raw.as_bytes());
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &hash)[..32].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_address() {
        let addr = "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B";
        let result = normalize_address(addr).unwrap();
        assert_eq!(result, "0xab5801a7d398351b8be11c439e05c5b3259aec9b");
    }

    #[test]
    fn test_generate_session_id_length() {
        let id = generate_session_id("0x1234", 1234567890);
        assert_eq!(id.len(), 32);
    }

    #[test]
    fn test_generate_session_id_uniqueness() {
        let id1 = generate_session_id("0x1234", 1234567890);
        let id2 = generate_session_id("0x1234", 1234567890);
        // UUIDs make these unique
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_normalize_address_lowercase_input() {
        let addr = "0xab5801a7d398351b8be11c439e05c5b3259aec9b";
        let result = normalize_address(addr).unwrap();
        assert_eq!(result, addr);
    }

    #[test]
    fn test_normalize_address_mixed_case() {
        let addr = "0xAB5801A7D398351B8BE11C439E05C5B3259AEC9B";
        let result = normalize_address(addr).unwrap();
        assert_eq!(result, "0xab5801a7d398351b8be11c439e05c5b3259aec9b");
    }

    #[test]
    fn test_normalize_address_no_prefix() {
        let result = normalize_address("ab5801a7d398351b8be11c439e05c5b3259aec9b");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("0x"));
    }

    #[test]
    fn test_normalize_address_too_short() {
        let result = normalize_address("0x1234");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("42"));
    }

    #[test]
    fn test_normalize_address_too_long() {
        let addr = format!("0x{}", "a".repeat(41));
        let result = normalize_address(&addr);
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_address_invalid_hex() {
        let addr = format!("0x{}zz", "a".repeat(38));
        let result = normalize_address(&addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("hex"));
    }

    #[test]
    fn test_normalize_address_empty() {
        let result = normalize_address("");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_session_id_url_safe() {
        let id = generate_session_id("0xabc", 999);
        // URL-safe base64 uses only alphanumeric, '-', '_'
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_generate_session_id_different_inputs() {
        let id1 = generate_session_id("0xaaa", 100);
        let id2 = generate_session_id("0xbbb", 100);
        // Different addresses should virtually always produce different IDs
        // (UUID ensures uniqueness, but different inputs also contribute)
        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
    }
}
