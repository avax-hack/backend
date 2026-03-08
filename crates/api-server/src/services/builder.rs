use std::sync::Arc;

use sqlx;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::milestone;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::milestone::{IMilestoneInfo, MilestoneStatus};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentMilestone {
    pub order: i32,
    pub title: String,
    pub status: MilestoneStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderOverview {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub status: String,
    pub target_raise: String,
    pub usdc_raised: String,
    pub investor_count: i64,
    pub milestones: Vec<IMilestoneInfo>,
    pub current_milestone: Option<CurrentMilestone>,
    pub total_milestones: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingPoint {
    pub date: i64,
    pub cumulative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorCountPoint {
    pub date: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderStats {
    pub total_raised: String,
    pub total_investors: i64,
    pub milestones_completed: i32,
    pub milestones_total: i32,
    pub funds_released: String,
    pub funding_over_time: Vec<FundingPoint>,
    pub investors_over_time: Vec<InvestorCountPoint>,
}

pub async fn get_overview(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
) -> AppResult<BuilderOverview> {
    let row = sqlx::query_as::<_, ProjectSummaryRow>(
        r#"
        SELECT p.project_id, p.name, p.symbol, p.image_uri, p.status,
               p.target_raise::TEXT as target_raise, p.usdc_raised::TEXT as usdc_raised,
               p.created_at,
               COALESCE((SELECT COUNT(*) FROM investments i WHERE i.project_id = p.project_id), 0) as investor_count
        FROM projects p
        WHERE p.project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .ok_or_else(|| AppError::NotFound(format!("Project {project_id} not found")))?;

    let milestones = milestone::find_by_project(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?;

    let total_milestones = milestones.len() as i32;

    // The current milestone is the first milestone that isn't "completed",
    // ordered by the milestone's `order` field.
    let current_milestone = milestones
        .iter()
        .filter(|m| m.status != MilestoneStatus::Completed)
        .min_by_key(|m| m.order)
        .map(|m| CurrentMilestone {
            order: m.order,
            title: m.title.clone(),
            status: m.status.clone(),
        });

    Ok(BuilderOverview {
        project_id: row.project_id,
        name: row.name,
        symbol: row.symbol,
        image_uri: row.image_uri,
        status: row.status,
        target_raise: row.target_raise,
        usdc_raised: row.usdc_raised,
        investor_count: row.investor_count,
        milestones,
        current_milestone,
        total_milestones,
        created_at: row.created_at,
    })
}

pub async fn get_stats(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
) -> AppResult<BuilderStats> {
    let row = sqlx::query_as::<_, StatsRow>(
        r#"
        SELECT
            COALESCE(p.usdc_raised, 0)::TEXT as total_raised,
            COALESCE(p.usdc_released, 0)::TEXT as funds_released,
            COALESCE((SELECT COUNT(*) FROM investments i WHERE i.project_id = p.project_id), 0) as total_investors
        FROM projects p
        WHERE p.project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .ok_or_else(|| AppError::NotFound(format!("Project {project_id} not found")))?;

    let milestones = milestone::find_by_project(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?;

    let milestones_total = milestones.len() as i32;
    let milestones_completed = milestones
        .iter()
        .filter(|m| m.status == MilestoneStatus::Completed)
        .count() as i32;

    // Cumulative funding over time, grouped by day (unix day boundary).
    let funding_over_time = sqlx::query_as::<_, FundingRow>(
        r#"
        SELECT
            (created_at / 86400) * 86400 AS day,
            SUM(SUM(usdc_amount)) OVER (ORDER BY (created_at / 86400) * 86400)::TEXT AS cumulative
        FROM investments
        WHERE project_id = $1
        GROUP BY day
        ORDER BY day
        "#,
    )
    .bind(project_id)
    .fetch_all(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .into_iter()
    .map(|r| FundingPoint {
        date: r.day,
        cumulative: r.cumulative,
    })
    .collect();

    // Cumulative unique investor count over time, grouped by day.
    let investors_over_time = sqlx::query_as::<_, InvestorCountRow>(
        r#"
        SELECT
            day,
            SUM(daily_new) OVER (ORDER BY day) AS count
        FROM (
            SELECT
                (MIN(created_at) / 86400) * 86400 AS day,
                COUNT(*) AS daily_new
            FROM (
                SELECT account_id, MIN(created_at) AS created_at
                FROM investments
                WHERE project_id = $1
                GROUP BY account_id
            ) first_investments
            GROUP BY day
        ) daily_counts
        ORDER BY day
        "#,
    )
    .bind(project_id)
    .fetch_all(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .into_iter()
    .map(|r| InvestorCountPoint {
        date: r.day,
        count: r.count,
    })
    .collect();

    Ok(BuilderStats {
        total_raised: row.total_raised,
        total_investors: row.total_investors,
        milestones_completed,
        milestones_total,
        funds_released: row.funds_released,
        funding_over_time,
        investors_over_time,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct ProjectSummaryRow {
    project_id: String,
    name: String,
    symbol: String,
    image_uri: String,
    status: String,
    target_raise: String,
    usdc_raised: String,
    created_at: i64,
    investor_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct StatsRow {
    total_raised: String,
    funds_released: String,
    total_investors: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct FundingRow {
    day: i64,
    cumulative: String,
}

#[derive(Debug, sqlx::FromRow)]
struct InvestorCountRow {
    day: i64,
    count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_overview_serialization_roundtrip() {
        let overview = BuilderOverview {
            project_id: "proj_1".to_string(),
            name: "MyProject".to_string(),
            symbol: "MP".to_string(),
            image_uri: "img.png".to_string(),
            status: "funding".to_string(),
            target_raise: "100000".to_string(),
            usdc_raised: "50000".to_string(),
            investor_count: 10,
            milestones: vec![],
            current_milestone: None,
            total_milestones: 0,
            created_at: 1700000000,
        };

        let json = serde_json::to_string(&overview).unwrap();
        let deserialized: BuilderOverview = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project_id, "proj_1");
        assert_eq!(deserialized.name, "MyProject");
        assert_eq!(deserialized.symbol, "MP");
        assert_eq!(deserialized.status, "funding");
        assert_eq!(deserialized.target_raise, "100000");
        assert_eq!(deserialized.usdc_raised, "50000");
        assert_eq!(deserialized.investor_count, 10);
        assert!(deserialized.milestones.is_empty());
        assert!(deserialized.current_milestone.is_none());
        assert_eq!(deserialized.total_milestones, 0);
    }

    #[test]
    fn builder_overview_with_current_milestone() {
        let overview = BuilderOverview {
            project_id: "proj_1".to_string(),
            name: "MyProject".to_string(),
            symbol: "MP".to_string(),
            image_uri: "img.png".to_string(),
            status: "funding".to_string(),
            target_raise: "100000".to_string(),
            usdc_raised: "50000".to_string(),
            investor_count: 10,
            milestones: vec![],
            current_milestone: Some(CurrentMilestone {
                order: 2,
                title: "Beta Release".to_string(),
                status: MilestoneStatus::InVerification,
            }),
            total_milestones: 4,
            created_at: 1700000000,
        };

        let value: serde_json::Value = serde_json::to_value(&overview).unwrap();
        let cm = value.get("current_milestone").unwrap();
        assert_eq!(cm.get("order").unwrap(), 2);
        assert_eq!(cm.get("title").unwrap(), "Beta Release");
        assert_eq!(cm.get("status").unwrap(), "in_verification");
        assert_eq!(value.get("total_milestones").unwrap(), 4);
    }

    #[test]
    fn builder_stats_serialization_roundtrip() {
        let stats = BuilderStats {
            total_raised: "75000".to_string(),
            total_investors: 25,
            milestones_completed: 3,
            milestones_total: 5,
            funds_released: "30000".to_string(),
            funding_over_time: vec![
                FundingPoint { date: 1714521600, cumulative: "50000".to_string() },
            ],
            investors_over_time: vec![
                InvestorCountPoint { date: 1714521600, count: 150 },
            ],
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: BuilderStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_raised, "75000");
        assert_eq!(deserialized.total_investors, 25);
        assert_eq!(deserialized.milestones_completed, 3);
        assert_eq!(deserialized.milestones_total, 5);
        assert_eq!(deserialized.funds_released, "30000");
        assert_eq!(deserialized.funding_over_time.len(), 1);
        assert_eq!(deserialized.funding_over_time[0].date, 1714521600);
        assert_eq!(deserialized.funding_over_time[0].cumulative, "50000");
        assert_eq!(deserialized.investors_over_time.len(), 1);
        assert_eq!(deserialized.investors_over_time[0].count, 150);
    }

    #[test]
    fn builder_stats_json_field_names() {
        let stats = BuilderStats {
            total_raised: "0".to_string(),
            total_investors: 0,
            milestones_completed: 0,
            milestones_total: 0,
            funds_released: "0".to_string(),
            funding_over_time: vec![],
            investors_over_time: vec![],
        };

        let value: serde_json::Value = serde_json::to_value(&stats).unwrap();
        assert!(value.get("total_raised").is_some());
        assert!(value.get("total_investors").is_some());
        assert!(value.get("milestones_completed").is_some());
        assert!(value.get("milestones_total").is_some());
        assert!(value.get("funds_released").is_some());
        assert!(value.get("funding_over_time").is_some());
        assert!(value.get("investors_over_time").is_some());
    }
}
