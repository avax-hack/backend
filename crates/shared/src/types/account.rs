use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IAccountInfo {
    pub account_id: String,
    pub nickname: String,
    pub bio: String,
    pub image_uri: String,
}

impl Default for IAccountInfo {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            nickname: String::new(),
            bio: String::new(),
            image_uri: String::new(),
        }
    }
}

impl IAccountInfo {
    pub fn new(account_id: String) -> Self {
        Self {
            account_id,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateAccountRequest {
    pub nickname: Option<String>,
    pub bio: Option<String>,
    pub image_uri: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_info_default() {
        let info = IAccountInfo::default();
        assert!(info.account_id.is_empty());
        assert!(info.nickname.is_empty());
    }

    #[test]
    fn test_account_info_new() {
        let info = IAccountInfo::new("0x1234".to_string());
        assert_eq!(info.account_id, "0x1234");
        assert!(info.nickname.is_empty());
    }

    #[test]
    fn test_account_info_serialization() {
        let info = IAccountInfo {
            account_id: "0xabc".to_string(),
            nickname: "test".to_string(),
            bio: "hello".to_string(),
            image_uri: "https://img.png".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IAccountInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.account_id, "0xabc");
        assert_eq!(parsed.nickname, "test");
    }

    #[test]
    fn test_account_info_deserialization_from_json() {
        let json = r#"{"account_id":"0x999","nickname":"alice","bio":"hi","image_uri":"img.jpg"}"#;
        let info: IAccountInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.account_id, "0x999");
        assert_eq!(info.nickname, "alice");
        assert_eq!(info.bio, "hi");
        assert_eq!(info.image_uri, "img.jpg");
    }

    #[test]
    fn test_account_info_clone() {
        let info = IAccountInfo::new("0xabc".to_string());
        let cloned = info.clone();
        assert_eq!(info.account_id, cloned.account_id);
    }

    #[test]
    fn test_account_info_debug() {
        let info = IAccountInfo::new("0x123".to_string());
        let debug = format!("{:?}", info);
        assert!(debug.contains("0x123"));
    }

    #[test]
    fn test_update_account_request_serialization() {
        let req = UpdateAccountRequest {
            nickname: Some("new_name".to_string()),
            bio: None,
            image_uri: Some("https://new.png".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: UpdateAccountRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.nickname, Some("new_name".to_string()));
        assert!(parsed.bio.is_none());
        assert_eq!(parsed.image_uri, Some("https://new.png".to_string()));
    }

    #[test]
    fn test_update_account_request_all_none() {
        let req = UpdateAccountRequest {
            nickname: None,
            bio: None,
            image_uri: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: UpdateAccountRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.nickname.is_none());
        assert!(parsed.bio.is_none());
        assert!(parsed.image_uri.is_none());
    }

    #[test]
    fn test_update_account_request_deserialization_partial() {
        let json = r#"{"nickname":"bob"}"#;
        let req: UpdateAccountRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.nickname, Some("bob".to_string()));
        assert!(req.bio.is_none());
    }
}
