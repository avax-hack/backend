use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

use super::account::IAccountInfo;
use super::milestone::IMilestoneInfo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Funding,
    Active,
    Completed,
    Failed,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Funding => "funding",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "funding" => Ok(Self::Funding),
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => anyhow::bail!("Unknown project status: {s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IProjectInfo {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: Option<String>,
    pub tagline: String,
    pub category: String,
    pub creator: IAccountInfo,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IProjectMarketInfo {
    pub project_id: String,
    pub status: ProjectStatus,
    pub target_raise: String,
    pub total_committed: String,
    pub funded_percent: f64,
    pub investor_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IProjectData {
    pub project_info: IProjectInfo,
    pub market_info: IProjectMarketInfo,
    pub milestones: Vec<IMilestoneInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IProjectListItem {
    pub project_info: IProjectInfo,
    pub market_info: IProjectMarketInfo,
    pub milestone_completed: i32,
    pub milestone_total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateProjectRequest {
    pub name: String,
    pub symbol: String,
    pub tagline: String,
    pub description: String,
    pub image_uri: String,
    pub category: String,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub target_raise: String,
    pub token_supply: String,
    pub deadline: i64,
    pub milestones: Vec<CreateMilestoneRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateMilestoneRequest {
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32,
}

impl CreateProjectRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.len() < 2 || self.name.len() > 50 {
            anyhow::bail!("Name must be 2-50 characters");
        }
        if self.symbol.len() < 2 || self.symbol.len() > 10 {
            anyhow::bail!("Symbol must be 2-10 characters");
        }
        if !self.symbol.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            anyhow::bail!("Symbol must be uppercase letters and digits only");
        }
        if self.tagline.len() < 5 || self.tagline.len() > 120 {
            anyhow::bail!("Tagline must be 5-120 characters");
        }
        if self.description.len() < 20 {
            anyhow::bail!("Description must be at least 20 characters");
        }
        if self.image_uri.is_empty() {
            anyhow::bail!("Image URI is required");
        }
        // Validate target_raise is a positive number
        let target: BigDecimal = self.target_raise.parse()
            .map_err(|_| anyhow::anyhow!("target_raise must be a valid number"))?;
        if target <= BigDecimal::from(0) {
            anyhow::bail!("target_raise must be positive");
        }
        // Validate token_supply is a positive number
        let supply: BigDecimal = self.token_supply.parse()
            .map_err(|_| anyhow::anyhow!("token_supply must be a valid number"))?;
        if supply <= BigDecimal::from(0) {
            anyhow::bail!("token_supply must be positive");
        }
        if self.milestones.len() < 2 || self.milestones.len() > 6 {
            anyhow::bail!("Must have 2-6 milestones");
        }
        let total_percent: i32 = self.milestones.iter().map(|m| m.fund_allocation_percent).sum();
        if total_percent != 100 {
            anyhow::bail!("Milestone allocations must sum to 100, got {total_percent}");
        }
        for m in &self.milestones {
            if m.title.is_empty() {
                anyhow::bail!("Milestone title is required");
            }
            if m.description.is_empty() {
                anyhow::bail!("Milestone description is required");
            }
            if m.fund_allocation_percent < 1 || m.fund_allocation_percent > 100 {
                anyhow::bail!("Milestone allocation must be 1-100");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_status_roundtrip() {
        assert_eq!(ProjectStatus::from_str("funding").unwrap(), ProjectStatus::Funding);
        assert_eq!(ProjectStatus::from_str("active").unwrap(), ProjectStatus::Active);
        assert_eq!(ProjectStatus::from_str("completed").unwrap(), ProjectStatus::Completed);
        assert_eq!(ProjectStatus::from_str("failed").unwrap(), ProjectStatus::Failed);
        assert!(ProjectStatus::from_str("unknown").is_err());
    }

    #[test]
    fn test_create_project_validation_valid() {
        let req = CreateProjectRequest {
            name: "TestProject".to_string(),
            symbol: "TEST".to_string(),
            tagline: "A test project for testing".to_string(),
            description: "This is a test project with enough description length".to_string(),
            image_uri: "https://img.png".to_string(),
            category: "defi".to_string(),
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            target_raise: "1000000".to_string(),
            token_supply: "1000000000".to_string(),
            deadline: 1717300000,
            milestones: vec![
                CreateMilestoneRequest {
                    order: 1,
                    title: "MVP".to_string(),
                    description: "Build MVP".to_string(),
                    fund_allocation_percent: 50,
                },
                CreateMilestoneRequest {
                    order: 2,
                    title: "Launch".to_string(),
                    description: "Launch product".to_string(),
                    fund_allocation_percent: 50,
                },
            ],
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_create_project_validation_bad_symbol() {
        let req = CreateProjectRequest {
            name: "TestProject".to_string(),
            symbol: "test".to_string(), // lowercase
            tagline: "A test project for testing".to_string(),
            description: "This is a test project with enough description length".to_string(),
            image_uri: "https://img.png".to_string(),
            category: "defi".to_string(),
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            target_raise: "1000000".to_string(),
            token_supply: "1000000000".to_string(),
            deadline: 1717300000,
            milestones: vec![
                CreateMilestoneRequest { order: 1, title: "MVP".to_string(), description: "Build".to_string(), fund_allocation_percent: 50 },
                CreateMilestoneRequest { order: 2, title: "Launch".to_string(), description: "Launch".to_string(), fund_allocation_percent: 50 },
            ],
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_bad_milestones() {
        let req = CreateProjectRequest {
            name: "TestProject".to_string(),
            symbol: "TEST".to_string(),
            tagline: "A test project for testing".to_string(),
            description: "This is a test project with enough description length".to_string(),
            image_uri: "https://img.png".to_string(),
            category: "defi".to_string(),
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            target_raise: "1000000".to_string(),
            token_supply: "1000000000".to_string(),
            deadline: 1717300000,
            milestones: vec![
                CreateMilestoneRequest { order: 1, title: "MVP".to_string(), description: "Build".to_string(), fund_allocation_percent: 60 },
                CreateMilestoneRequest { order: 2, title: "Launch".to_string(), description: "Launch".to_string(), fund_allocation_percent: 50 },
            ],
        };
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("sum to 100"));
    }

    fn valid_request() -> CreateProjectRequest {
        CreateProjectRequest {
            name: "TestProject".to_string(),
            symbol: "TEST".to_string(),
            tagline: "A test project for testing".to_string(),
            description: "This is a test project with enough description length".to_string(),
            image_uri: "https://img.png".to_string(),
            category: "defi".to_string(),
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            target_raise: "1000000".to_string(),
            token_supply: "1000000000".to_string(),
            deadline: 1717300000,
            milestones: vec![
                CreateMilestoneRequest { order: 1, title: "MVP".to_string(), description: "Build".to_string(), fund_allocation_percent: 50 },
                CreateMilestoneRequest { order: 2, title: "Launch".to_string(), description: "Ship".to_string(), fund_allocation_percent: 50 },
            ],
        }
    }

    #[test]
    fn test_create_project_validation_name_too_short() {
        let mut req = valid_request();
        req.name = "A".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Name"));
    }

    #[test]
    fn test_create_project_validation_name_too_long() {
        let mut req = valid_request();
        req.name = "A".repeat(51);
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Name"));
    }

    #[test]
    fn test_create_project_validation_symbol_too_short() {
        let mut req = valid_request();
        req.symbol = "A".to_string();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_symbol_too_long() {
        let mut req = valid_request();
        req.symbol = "ABCDEFGHIJK".to_string(); // 11 chars
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_tagline_too_short() {
        let mut req = valid_request();
        req.tagline = "Hi".to_string();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_tagline_too_long() {
        let mut req = valid_request();
        req.tagline = "A".repeat(121);
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_description_too_short() {
        let mut req = valid_request();
        req.description = "Short".to_string();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_empty_image() {
        let mut req = valid_request();
        req.image_uri = "".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Image"));
    }

    #[test]
    fn test_create_project_validation_too_few_milestones() {
        let mut req = valid_request();
        req.milestones = vec![
            CreateMilestoneRequest { order: 1, title: "Only".to_string(), description: "One".to_string(), fund_allocation_percent: 100 },
        ];
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_too_many_milestones() {
        let mut req = valid_request();
        req.milestones = (0..7).map(|i| CreateMilestoneRequest {
            order: i,
            title: format!("M{i}"),
            description: format!("Desc{i}"),
            fund_allocation_percent: if i < 6 { 14 } else { 16 },
        }).collect();
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_empty_milestone_title() {
        let mut req = valid_request();
        req.milestones[0].title = "".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("title"));
    }

    #[test]
    fn test_create_project_validation_empty_milestone_description() {
        let mut req = valid_request();
        req.milestones[0].description = "".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("description"));
    }

    #[test]
    fn test_create_project_validation_milestone_allocation_zero() {
        let mut req = valid_request();
        req.milestones[0].fund_allocation_percent = 0;
        req.milestones[1].fund_allocation_percent = 100;
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_project_validation_milestone_allocation_over_100() {
        let mut req = valid_request();
        req.milestones[0].fund_allocation_percent = 101;
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_project_status_as_str() {
        assert_eq!(ProjectStatus::Funding.as_str(), "funding");
        assert_eq!(ProjectStatus::Active.as_str(), "active");
        assert_eq!(ProjectStatus::Completed.as_str(), "completed");
        assert_eq!(ProjectStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn test_project_status_serde_roundtrip() {
        let status = ProjectStatus::Funding;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"funding\"");
        let parsed: ProjectStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ProjectStatus::Funding);
    }

    #[test]
    fn test_project_status_from_str_unknown() {
        let err = ProjectStatus::from_str("paused").unwrap_err();
        assert!(err.to_string().contains("Unknown project status"));
    }

    #[test]
    fn test_project_market_info_serialization() {
        let info = IProjectMarketInfo {
            project_id: "proj_1".to_string(),
            status: ProjectStatus::Funding,
            target_raise: "1000000".to_string(),
            total_committed: "500000".to_string(),
            funded_percent: 50.0,
            investor_count: 42,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IProjectMarketInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_id, "proj_1");
        assert_eq!(parsed.investor_count, 42);
        assert!((parsed.funded_percent - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_project_list_item_serialization() {
        let item = IProjectListItem {
            project_info: IProjectInfo {
                project_id: "p1".to_string(),
                name: "Proj".to_string(),
                symbol: "P".to_string(),
                image_uri: "i.png".to_string(),
                description: None,
                tagline: "Tag".to_string(),
                category: "defi".to_string(),
                creator: super::super::account::IAccountInfo::new("0x1".to_string()),
                website: None,
                twitter: None,
                github: None,
                telegram: None,
                created_at: 0,
            },
            market_info: IProjectMarketInfo {
                project_id: "p1".to_string(),
                status: ProjectStatus::Active,
                target_raise: "100".to_string(),
                total_committed: "50".to_string(),
                funded_percent: 50.0,
                investor_count: 5,
            },
            milestone_completed: 1,
            milestone_total: 3,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: IProjectListItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.milestone_completed, 1);
        assert_eq!(parsed.milestone_total, 3);
    }

    #[test]
    fn test_create_project_request_serialization() {
        let req = valid_request();
        let json = serde_json::to_string(&req).unwrap();
        let parsed: CreateProjectRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "TestProject");
        assert_eq!(parsed.milestones.len(), 2);
    }

    #[test]
    fn test_create_project_validation_symbol_with_digits() {
        let mut req = valid_request();
        req.symbol = "TEST1".to_string();
        assert!(req.validate().is_ok());
    }

    // --- Boundary value tests ---

    #[test]
    fn test_name_exactly_2_chars_valid() {
        let mut req = valid_request();
        req.name = "AB".to_string();
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_name_exactly_50_chars_valid() {
        let mut req = valid_request();
        req.name = "A".repeat(50);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_name_1_char_invalid() {
        let mut req = valid_request();
        req.name = "A".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Name must be 2-50 characters"));
    }

    #[test]
    fn test_name_51_chars_invalid() {
        let mut req = valid_request();
        req.name = "A".repeat(51);
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Name must be 2-50 characters"));
    }

    #[test]
    fn test_symbol_exactly_2_chars_valid() {
        let mut req = valid_request();
        req.symbol = "AB".to_string();
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_symbol_exactly_10_chars_valid() {
        let mut req = valid_request();
        req.symbol = "ABCDEFGHIJ".to_string(); // 10 chars
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_symbol_1_char_invalid() {
        let mut req = valid_request();
        req.symbol = "A".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Symbol must be 2-10 characters"));
    }

    #[test]
    fn test_symbol_11_chars_invalid() {
        let mut req = valid_request();
        req.symbol = "ABCDEFGHIJK".to_string(); // 11 chars
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Symbol must be 2-10 characters"));
    }

    #[test]
    fn test_tagline_exactly_5_chars_valid() {
        let mut req = valid_request();
        req.tagline = "Hello".to_string(); // 5 chars
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_tagline_exactly_120_chars_valid() {
        let mut req = valid_request();
        req.tagline = "A".repeat(120);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_tagline_4_chars_invalid() {
        let mut req = valid_request();
        req.tagline = "Abcd".to_string(); // 4 chars
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Tagline must be 5-120 characters"));
    }

    #[test]
    fn test_tagline_121_chars_invalid() {
        let mut req = valid_request();
        req.tagline = "A".repeat(121);
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Tagline must be 5-120 characters"));
    }

    #[test]
    fn test_exactly_2_milestones_valid() {
        let req = valid_request(); // already has 2 milestones
        assert_eq!(req.milestones.len(), 2);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_exactly_6_milestones_valid() {
        let mut req = valid_request();
        req.milestones = (1..=6).map(|i| CreateMilestoneRequest {
            order: i,
            title: format!("Milestone {i}"),
            description: format!("Description {i}"),
            fund_allocation_percent: if i <= 4 { 17 } else { 16 },
        }).collect();
        // 4*17 + 2*16 = 68 + 32 = 100
        assert_eq!(req.milestones.len(), 6);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_1_milestone_invalid() {
        let mut req = valid_request();
        req.milestones = vec![
            CreateMilestoneRequest {
                order: 1,
                title: "Only".to_string(),
                description: "One".to_string(),
                fund_allocation_percent: 100,
            },
        ];
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Must have 2-6 milestones"));
    }

    #[test]
    fn test_7_milestones_invalid() {
        let mut req = valid_request();
        req.milestones = (1..=7).map(|i| CreateMilestoneRequest {
            order: i,
            title: format!("M{i}"),
            description: format!("D{i}"),
            fund_allocation_percent: if i <= 2 { 15 } else { 14 },
        }).collect();
        // 2*15 + 5*14 = 30 + 70 = 100
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("Must have 2-6 milestones"));
    }

    #[test]
    fn test_target_raise_must_be_valid_number() {
        let mut req = valid_request();
        req.target_raise = "not_a_number".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("target_raise"));
    }

    #[test]
    fn test_target_raise_must_be_positive() {
        let mut req = valid_request();
        req.target_raise = "0".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("target_raise must be positive"));
    }

    #[test]
    fn test_target_raise_negative() {
        let mut req = valid_request();
        req.target_raise = "-100".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("target_raise must be positive"));
    }

    #[test]
    fn test_token_supply_must_be_valid_number() {
        let mut req = valid_request();
        req.token_supply = "abc".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("token_supply"));
    }

    #[test]
    fn test_token_supply_must_be_positive() {
        let mut req = valid_request();
        req.token_supply = "0".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("token_supply must be positive"));
    }

    #[test]
    fn test_token_supply_negative() {
        let mut req = valid_request();
        req.token_supply = "-1000".to_string();
        let err = req.validate().unwrap_err();
        assert!(err.to_string().contains("token_supply must be positive"));
    }
}
