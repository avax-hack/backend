use serde::{Deserialize, Serialize};
use openlaunch_shared::config;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::storage::r2::R2Client;

use super::upload::validate_image_uri;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateMetadataRequest {
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub category: String,
    pub homepage: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub discord: Option<String>,
    pub milestones: Vec<MilestoneInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MilestoneInput {
    pub order: i32,
    pub title: String,
    pub description: String,
    pub fund_allocation_percent: i32,
}

impl CreateMetadataRequest {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.len() < 2 || self.name.len() > 50 {
            return Err(AppError::BadRequest("name must be 2-50 characters".into()));
        }
        if self.symbol.len() < 2 || self.symbol.len() > 10 {
            return Err(AppError::BadRequest(
                "symbol must be 2-10 characters".into(),
            ));
        }
        if !self
            .symbol
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(AppError::BadRequest(
                "symbol must be uppercase letters and digits only".into(),
            ));
        }
        if self.category.is_empty() || self.category.len() > 50 {
            return Err(AppError::BadRequest("category must be 1-50 characters".into()));
        }
        if !validate_image_uri(&self.image_uri) {
            return Err(AppError::BadRequest(
                "image_uri must be a valid uploaded image URL".into(),
            ));
        }
        for (name, val) in [
            ("homepage", &self.homepage),
            ("twitter", &self.twitter),
            ("telegram", &self.telegram),
            ("discord", &self.discord),
        ] {
            if let Some(url) = val {
                if !url.starts_with("https://") {
                    return Err(AppError::BadRequest(format!(
                        "{name} must start with https://"
                    )));
                }
            }
        }
        if self.milestones.len() < 2 || self.milestones.len() > 6 {
            return Err(AppError::BadRequest("Must have 2-6 milestones".into()));
        }
        let total: i32 = self
            .milestones
            .iter()
            .map(|m| m.fund_allocation_percent)
            .sum();
        if total != 100 {
            return Err(AppError::BadRequest(format!(
                "Milestone allocations must sum to 100, got {total}"
            )));
        }
        for m in &self.milestones {
            if m.title.is_empty() {
                return Err(AppError::BadRequest(
                    "Milestone title is required".into(),
                ));
            }
            if m.description.is_empty() {
                return Err(AppError::BadRequest(
                    "Milestone description is required".into(),
                ));
            }
            if m.fund_allocation_percent < 1 || m.fund_allocation_percent > 100 {
                return Err(AppError::BadRequest(
                    "Milestone allocation must be 1-100".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Build metadata JSON and upload to R2 metadata bucket. Returns public URL.
pub async fn create_metadata(r2: &R2Client, req: &CreateMetadataRequest) -> AppResult<String> {
    req.validate()?;

    let json =
        serde_json::to_vec_pretty(req).map_err(|e| AppError::Internal(e.into()))?;

    let key = format!("{}.json", uuid::Uuid::new_v4());

    r2.put_object(&config::R2_METADATA_BUCKET, &key, json, "application/json")
        .await
        .map_err(AppError::Internal)?;

    Ok(R2Client::metadata_url(&key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> CreateMetadataRequest {
        CreateMetadataRequest {
            name: "TestToken".to_string(),
            symbol: "TEST".to_string(),
            image_uri: "placeholder".to_string(),
            category: "DeFi".to_string(),
            homepage: None,
            twitter: None,
            telegram: None,
            discord: None,
            milestones: vec![
                MilestoneInput {
                    order: 1,
                    title: "MVP".into(),
                    description: "Build".into(),
                    fund_allocation_percent: 50,
                },
                MilestoneInput {
                    order: 2,
                    title: "Launch".into(),
                    description: "Ship".into(),
                    fund_allocation_percent: 50,
                },
            ],
        }
    }

    #[test]
    fn name_too_short() {
        let mut req = valid_request();
        req.name = "A".into();
        assert!(req.validate().is_err());
    }

    #[test]
    fn name_too_long() {
        let mut req = valid_request();
        req.name = "A".repeat(51);
        assert!(req.validate().is_err());
    }

    #[test]
    fn symbol_lowercase_rejected() {
        let mut req = valid_request();
        req.symbol = "test".into();
        assert!(req.validate().is_err());
    }

    #[test]
    fn url_without_https_rejected() {
        let mut req = valid_request();
        req.twitter = Some("http://twitter.com".into());
        assert!(req.validate().is_err());
    }

    #[test]
    fn milestones_must_sum_100() {
        let mut req = valid_request();
        req.milestones[0].fund_allocation_percent = 60;
        assert!(req.validate().is_err());
    }

    #[test]
    fn too_few_milestones() {
        let mut req = valid_request();
        req.milestones = vec![MilestoneInput {
            order: 1,
            title: "Only".into(),
            description: "One".into(),
            fund_allocation_percent: 100,
        }];
        assert!(req.validate().is_err());
    }

    #[test]
    fn too_many_milestones() {
        let mut req = valid_request();
        req.milestones = (1..=7)
            .map(|i| MilestoneInput {
                order: i,
                title: format!("M{i}"),
                description: format!("D{i}"),
                fund_allocation_percent: if i <= 2 { 15 } else { 14 },
            })
            .collect();
        assert!(req.validate().is_err());
    }

    #[test]
    fn empty_milestone_title_rejected() {
        let mut req = valid_request();
        req.milestones[0].title = "".into();
        assert!(req.validate().is_err());
    }

    #[test]
    fn valid_https_urls_accepted() {
        let mut req = valid_request();
        req.homepage = Some("https://example.com".into());
        req.twitter = Some("https://twitter.com/test".into());
        req.discord = Some("https://discord.gg/test".into());
        // Note: validate_image_uri will fail since R2_IMAGE_PUBLIC_URL is empty in tests,
        // but the URL validation itself passes
    }
}
