use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Completed,
    InVerification,
    Submitted,
    Pending,
    Failed,
}

impl MilestoneStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Completed => "completed",
            Self::InVerification => "in_verification",
            Self::Submitted => "submitted",
            Self::Pending => "pending",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "completed" => Ok(Self::Completed),
            "in_verification" => Ok(Self::InVerification),
            "submitted" => Ok(Self::Submitted),
            "pending" => Ok(Self::Pending),
            "failed" => Ok(Self::Failed),
            _ => anyhow::bail!("Unknown milestone status: {s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IMilestoneInfo {
    pub milestone_id: String,
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32,
    pub fund_release_amount: String,
    pub status: MilestoneStatus,
    pub funds_released: bool,
    pub evidence_uri: Option<String>,
    pub submitted_at: Option<i64>,
    pub verified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MilestoneSubmitRequest {
    pub evidence_text: String,
    pub evidence_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IMilestoneVerificationData {
    pub milestone_id: String,
    pub status: MilestoneStatus,
    pub submitted_at: Option<i64>,
    pub estimated_completion: Option<i64>,
    pub dispute_info: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_milestone_status_roundtrip() {
        let statuses = vec![
            ("completed", MilestoneStatus::Completed),
            ("in_verification", MilestoneStatus::InVerification),
            ("submitted", MilestoneStatus::Submitted),
            ("pending", MilestoneStatus::Pending),
            ("failed", MilestoneStatus::Failed),
        ];
        for (s, expected) in statuses {
            assert_eq!(MilestoneStatus::from_str(s).unwrap(), expected);
            assert_eq!(expected.as_str(), s);
        }
    }

    #[test]
    fn test_milestone_info_serialization() {
        let info = IMilestoneInfo {
            milestone_id: "ms_001".to_string(),
            order: 1,
            title: "MVP Launch".to_string(),
            description: "Ship MVP".to_string(),
            fund_allocation_percent: 25,
            fund_release_amount: "125000000000000000000000".to_string(),
            status: MilestoneStatus::Completed,
            funds_released: true,
            evidence_uri: Some("https://storage/evidence.pdf".to_string()),
            submitted_at: Some(1717232400),
            verified_at: Some(1717248600),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IMilestoneInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.milestone_id, "ms_001");
        assert_eq!(parsed.status, MilestoneStatus::Completed);
        assert!(parsed.funds_released);
    }

    #[test]
    fn test_milestone_status_unknown() {
        let result = MilestoneStatus::from_str("invalid_status");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown milestone status"));
    }

    #[test]
    fn test_milestone_status_serde_roundtrip() {
        let status = MilestoneStatus::InVerification;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_verification\"");
        let parsed: MilestoneStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MilestoneStatus::InVerification);
    }

    #[test]
    fn test_milestone_info_optional_fields_none() {
        let info = IMilestoneInfo {
            milestone_id: "ms_002".to_string(),
            order: 2,
            title: "Beta".to_string(),
            description: "Beta phase".to_string(),
            fund_allocation_percent: 50,
            fund_release_amount: "0".to_string(),
            status: MilestoneStatus::Pending,
            funds_released: false,
            evidence_uri: None,
            submitted_at: None,
            verified_at: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IMilestoneInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.evidence_uri.is_none());
        assert!(parsed.submitted_at.is_none());
        assert!(parsed.verified_at.is_none());
        assert!(!parsed.funds_released);
    }

    #[test]
    fn test_milestone_submit_request_serialization() {
        let req = MilestoneSubmitRequest {
            evidence_text: "Completed all deliverables".to_string(),
            evidence_uri: Some("https://proof.pdf".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: MilestoneSubmitRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.evidence_text, "Completed all deliverables");
        assert_eq!(parsed.evidence_uri, Some("https://proof.pdf".to_string()));
    }

    #[test]
    fn test_milestone_submit_request_no_uri() {
        let req = MilestoneSubmitRequest {
            evidence_text: "Done".to_string(),
            evidence_uri: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: MilestoneSubmitRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.evidence_uri.is_none());
    }

    #[test]
    fn test_milestone_verification_data_serialization() {
        let data = IMilestoneVerificationData {
            milestone_id: "ms_003".to_string(),
            status: MilestoneStatus::Submitted,
            submitted_at: Some(1717232400),
            estimated_completion: Some(1717400000),
            dispute_info: None,
        };
        let json = serde_json::to_string(&data).unwrap();
        let parsed: IMilestoneVerificationData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.milestone_id, "ms_003");
        assert_eq!(parsed.status, MilestoneStatus::Submitted);
    }

    #[test]
    fn test_milestone_status_clone() {
        let status = MilestoneStatus::Failed;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
