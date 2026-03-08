use serde::{Deserialize, Serialize};

use super::account::IAccountInfo;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NonceRequest {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NonceResponse {
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SessionRequest {
    pub nonce: String,
    pub signature: String,
    pub chain_id: u64,
}

impl SessionRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.signature.starts_with("0x") || self.signature.len() != 132 {
            anyhow::bail!("Signature must be 0x-prefixed 65-byte hex string (132 chars)");
        }
        if !self.signature[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("Signature contains invalid hex characters");
        }
        if self.nonce.is_empty() || self.nonce.len() > 256 {
            anyhow::bail!("Nonce must be 1-256 characters");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SessionResponse {
    pub account_info: IAccountInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SessionInfo {
    pub session_id: String,
    pub account_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl SessionInfo {
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_request_validation_valid() {
        let sig = format!("0x{}", "a".repeat(130));
        let req = SessionRequest {
            nonce: "test-nonce".to_string(),
            signature: sig,
            chain_id: 43114,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_session_request_validation_bad_sig() {
        let req = SessionRequest {
            nonce: "test-nonce".to_string(),
            signature: "0x1234".to_string(),
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_session_expired() {
        let info = SessionInfo {
            session_id: "test".to_string(),
            account_id: "0x123".to_string(),
            created_at: 0,
            expires_at: 0,
        };
        assert!(info.is_expired());
    }

    #[test]
    fn test_session_not_expired() {
        let future = chrono::Utc::now().timestamp() + 3600;
        let info = SessionInfo {
            session_id: "test".to_string(),
            account_id: "0x123".to_string(),
            created_at: 0,
            expires_at: future,
        };
        assert!(!info.is_expired());
    }

    #[test]
    fn test_session_request_validation_empty_nonce() {
        let sig = format!("0x{}", "a".repeat(130));
        let req = SessionRequest {
            nonce: "".to_string(),
            signature: sig,
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
        assert!(req.validate().unwrap_err().to_string().contains("Nonce"));
    }

    #[test]
    fn test_session_request_validation_nonce_too_long() {
        let sig = format!("0x{}", "a".repeat(130));
        let req = SessionRequest {
            nonce: "x".repeat(257),
            signature: sig,
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_session_request_validation_sig_no_prefix() {
        let req = SessionRequest {
            nonce: "test".to_string(),
            signature: "a".repeat(132),
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_session_request_validation_sig_invalid_hex() {
        let sig = format!("0x{}zz", "a".repeat(128));
        let req = SessionRequest {
            nonce: "test".to_string(),
            signature: sig,
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
        assert!(req.validate().unwrap_err().to_string().contains("hex"));
    }

    #[test]
    fn test_session_request_validation_sig_wrong_length() {
        let req = SessionRequest {
            nonce: "test".to_string(),
            signature: format!("0x{}", "a".repeat(100)),
            chain_id: 43114,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_nonce_request_serialization() {
        let req = NonceRequest { address: "0xabc".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: NonceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.address, "0xabc");
    }

    #[test]
    fn test_nonce_response_serialization() {
        let resp = NonceResponse { nonce: "random-nonce-123".to_string() };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: NonceResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.nonce, "random-nonce-123");
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            session_id: "sid_001".to_string(),
            account_id: "0xabc".to_string(),
            created_at: 1000,
            expires_at: 2000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "sid_001");
        assert_eq!(parsed.account_id, "0xabc");
        assert_eq!(parsed.created_at, 1000);
        assert_eq!(parsed.expires_at, 2000);
    }

    #[test]
    fn test_session_response_serialization() {
        let resp = SessionResponse {
            account_info: IAccountInfo::new("0x123".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: SessionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.account_info.account_id, "0x123");
    }
}
