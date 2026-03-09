use std::sync::Arc;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::db::postgres::controller::{
    account, investment, milestone, project,
};
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::account::IAccountInfo;
use openlaunch_shared::types::common::{PaginatedResponse, PaginationParams};
use openlaunch_shared::types::project::{
    CreateProjectRequest, IProjectData, IProjectInfo, IProjectListItem, IProjectMarketInfo,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorInfo {
    pub account_info: IAccountInfo,
    pub usdc_amount: String,
    pub created_at: i64,
}

pub async fn get_project(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
) -> AppResult<IProjectData> {
    let row = project::find_by_id(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("Project {project_id} not found")))?;

    let creator = account::find_by_id(db.reader(), &row.creator)
        .await
        .map_err(AppError::Internal)?
        .unwrap_or_else(|| IAccountInfo::new(row.creator.clone()));

    let milestones = milestone::find_by_project(db.reader(), project_id)
        .await
        .map_err(AppError::Internal)?;

    let investor_count = {
        let (_, total) = investment::find_by_project(
            db.reader(),
            project_id,
            &PaginationParams { page: 1, limit: 1 },
        )
        .await
        .map_err(AppError::Internal)?;
        total
    };

    let usdc_raised = row.usdc_raised.clone().unwrap_or_else(|| "0".to_string());
    let target_raise = row.target_raise.clone().unwrap_or_else(|| "0".to_string());
    let funded_percent = compute_funded_percent(&usdc_raised, &target_raise);

    let project_info = build_project_info(&row, &creator);
    let market_info = IProjectMarketInfo {
        project_id: row.project_id.clone(),
        status: openlaunch_shared::types::project::ProjectStatus::from_str(&row.status)
            .unwrap_or(openlaunch_shared::types::project::ProjectStatus::Funding),
        target_raise,
        total_committed: usdc_raised,
        funded_percent,
        investor_count,
    };

    Ok(IProjectData {
        project_info,
        market_info,
        milestones,
    })
}

pub async fn get_featured(
    db: &Arc<PostgresDatabase>,
) -> AppResult<Vec<IProjectListItem>> {
    let pagination = PaginationParams { page: 1, limit: 10 };
    let (rows, _) = project::find_list(db.reader(), "funded", &pagination, Some("funding"))
        .await
        .map_err(AppError::Internal)?;

    build_project_list_items(db, rows).await
}

pub async fn get_project_list(
    db: &Arc<PostgresDatabase>,
    sort_type: &str,
    pagination: &PaginationParams,
    status: Option<&str>,
) -> AppResult<PaginatedResponse<IProjectListItem>> {
    let (rows, total_count) = project::find_list(db.reader(), sort_type, pagination, status)
        .await
        .map_err(AppError::Internal)?;

    let items = build_project_list_items(db, rows).await?;

    Ok(PaginatedResponse {
        data: items,
        total_count,
    })
}

pub async fn create_project(
    db: &Arc<PostgresDatabase>,
    creator: &str,
    request: &CreateProjectRequest,
) -> AppResult<String> {
    request
        .validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let is_available = project::validate_symbol(db.reader(), &request.symbol)
        .await
        .map_err(AppError::Internal)?;

    if !is_available {
        return Err(AppError::BadRequest(format!(
            "Symbol {} is already taken",
            request.symbol
        )));
    }

    // Ensure account exists
    account::upsert(db.writer(), creator)
        .await
        .map_err(AppError::Internal)?;

    // project_id will be the on-chain token address; for pre-chain creation use a placeholder
    let project_id = format!("0x{}", uuid::Uuid::new_v4().simple());
    let now = openlaunch_shared::types::common::current_unix_timestamp();

    // Compute token_price = target_raise / token_supply, ido_supply = total_supply = token_supply
    // Pre-chain placeholder: tx_hash is empty until on-chain deployment
    let target_raise_bd: bigdecimal::BigDecimal = request
        .target_raise
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid target_raise".to_string()))?;
    let token_supply_bd: bigdecimal::BigDecimal = request
        .token_supply
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid token_supply".to_string()))?;

    let milestones: Vec<(i32, String, String, i32)> = request
        .milestones
        .iter()
        .map(|m| {
            (
                m.order,
                m.title.clone(),
                m.description.clone(),
                m.fund_allocation_percent * 100, // convert to basis points
            )
        })
        .collect();

    // Wrap project + milestone creation in a single transaction
    let mut tx = db.writer().begin().await.map_err(|e| AppError::Internal(e.into()))?;

    project::insert_with_tx(
        &mut tx,
        &project_id,
        &request.name,
        &request.symbol,
        &request.image_uri,
        Some(&request.description),
        &request.category,
        creator,
        &target_raise_bd.to_string(),
        &token_supply_bd.to_string(),
        &token_supply_bd.to_string(),
        request.deadline,
        request.website.as_deref(),
        request.twitter.as_deref(),
        request.github.as_deref(),
        request.telegram.as_deref(),
        now,
    )
    .await
    .map_err(AppError::Internal)?;

    milestone::insert_batch_with_tx(&mut tx, &project_id, &milestones)
        .await
        .map_err(AppError::Internal)?;

    tx.commit().await.map_err(|e| AppError::Internal(e.into()))?;

    Ok(project_id)
}

pub async fn validate_symbol(
    db: &Arc<PostgresDatabase>,
    symbol: &str,
) -> AppResult<bool> {
    project::validate_symbol(db.reader(), symbol)
        .await
        .map_err(AppError::Internal)
}

pub async fn get_investors(
    db: &Arc<PostgresDatabase>,
    project_id: &str,
    pagination: &PaginationParams,
) -> AppResult<PaginatedResponse<InvestorInfo>> {
    let (rows, total_count) = investment::find_by_project(db.reader(), project_id, pagination)
        .await
        .map_err(AppError::Internal)?;

    let mut investors = Vec::with_capacity(rows.len());
    for row in rows {
        let acct = account::find_by_id(db.reader(), &row.account_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.account_id.clone()));

        investors.push(InvestorInfo {
            account_info: acct,
            usdc_amount: row.usdc_amount,
            created_at: row.created_at,
        });
    }

    Ok(PaginatedResponse {
        data: investors,
        total_count,
    })
}

fn build_project_info(
    row: &project::ProjectRow,
    creator: &IAccountInfo,
) -> IProjectInfo {
    IProjectInfo {
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        symbol: row.symbol.clone(),
        image_uri: row.image_uri.clone(),
        description: row.description.clone(),
        category: row.category.clone(),
        creator: creator.clone(),
        website: row.website.clone(),
        twitter: row.twitter.clone(),
        github: row.github.clone(),
        telegram: row.telegram.clone(),
        created_at: row.created_at,
    }
}

async fn build_project_list_items(
    db: &Arc<PostgresDatabase>,
    rows: Vec<project::ProjectListRow>,
) -> AppResult<Vec<IProjectListItem>> {
    let mut items = Vec::with_capacity(rows.len());

    for row in rows {
        let creator = account::find_by_id(db.reader(), &row.creator)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| IAccountInfo::new(row.creator.clone()));

        let milestones = milestone::find_by_project(db.reader(), &row.project_id)
            .await
            .map_err(AppError::Internal)?;

        let milestone_total = milestones.len() as i32;
        let milestone_completed = milestones
            .iter()
            .filter(|m| {
                m.status == openlaunch_shared::types::milestone::MilestoneStatus::Completed
            })
            .count() as i32;

        let usdc_raised = row.usdc_raised.clone().unwrap_or_else(|| "0".to_string());
        let target_raise = row.target_raise.clone().unwrap_or_else(|| "0".to_string());
        let funded_percent = compute_funded_percent(&usdc_raised, &target_raise);

        let project_info = IProjectInfo {
            project_id: row.project_id.clone(),
            name: row.name.clone(),
            symbol: row.symbol.clone(),
            image_uri: row.image_uri.clone(),
            description: None,
            category: row.category.clone(),
            creator,
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            created_at: row.created_at,
        };

        let market_info = IProjectMarketInfo {
            project_id: row.project_id.clone(),
            status: openlaunch_shared::types::project::ProjectStatus::from_str(&row.status)
                .unwrap_or(openlaunch_shared::types::project::ProjectStatus::Funding),
            target_raise,
            total_committed: usdc_raised,
            funded_percent,
            investor_count: row.investor_count,
        };

        items.push(IProjectListItem {
            project_info,
            market_info,
            milestone_completed,
            milestone_total,
        });
    }

    Ok(items)
}

fn compute_funded_percent(raised: &str, target: &str) -> f64 {
    let raised_f: f64 = raised.parse().unwrap_or(0.0);
    let target_f: f64 = target.parse().unwrap_or(0.0);
    if target_f <= 0.0 {
        0.0
    } else {
        (raised_f / target_f * 100.0).min(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_funded_percent_zero_target_returns_zero() {
        assert_eq!(compute_funded_percent("100", "0"), 0.0);
    }

    #[test]
    fn compute_funded_percent_negative_target_returns_zero() {
        assert_eq!(compute_funded_percent("100", "-10"), 0.0);
    }

    #[test]
    fn compute_funded_percent_half_funded() {
        let result = compute_funded_percent("500", "1000");
        assert!((result - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_fully_funded() {
        let result = compute_funded_percent("1000", "1000");
        assert!((result - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_capped_at_100() {
        let result = compute_funded_percent("2000", "1000");
        assert!((result - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_zero_raised() {
        let result = compute_funded_percent("0", "1000");
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_invalid_raised_string() {
        let result = compute_funded_percent("not_a_number", "1000");
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_funded_percent_invalid_target_string() {
        let result = compute_funded_percent("500", "invalid");
        assert_eq!(result, 0.0);
    }

    #[test]
    fn compute_funded_percent_both_invalid() {
        let result = compute_funded_percent("abc", "xyz");
        assert_eq!(result, 0.0);
    }

    #[test]
    fn compute_funded_percent_decimal_values() {
        let result = compute_funded_percent("33.33", "100.0");
        assert!((result - 33.33).abs() < 0.01);
    }

    #[test]
    fn compute_funded_percent_large_numbers_no_overflow() {
        // Use values near the limits of what string-based amounts might hold
        let raised = "999999999999999999";
        let target = "1000000000000000000";
        let result = compute_funded_percent(raised, target);
        assert!(result > 99.0 && result <= 100.0, "large numbers should compute correctly, got {result}");
    }

    #[test]
    fn compute_funded_percent_extremely_large_equal_values() {
        let huge = "99999999999999999999999";
        let result = compute_funded_percent(huge, huge);
        assert!((result - 100.0).abs() < f64::EPSILON, "equal large values should be 100%");
    }

    #[test]
    fn compute_funded_percent_tiny_fraction() {
        let result = compute_funded_percent("1", "999999999999999999");
        assert!(result >= 0.0 && result < 0.01, "tiny fraction should be near zero, got {result}");
    }
}
