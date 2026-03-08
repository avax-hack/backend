use std::sync::Arc;

use sqlx;

use openlaunch_shared::db::postgres::PostgresDatabase;
use openlaunch_shared::error::{AppError, AppResult};
use openlaunch_shared::types::common::current_unix_timestamp;
use openlaunch_shared::types::milestone::{
    IMilestoneVerificationData, MilestoneStatus, MilestoneSubmitRequest,
};

/// Submit evidence for a milestone.
/// The caller must be the project creator (checked in handler).
pub async fn submit_evidence(
    db: &Arc<PostgresDatabase>,
    milestone_id: &str,
    request: &MilestoneSubmitRequest,
) -> AppResult<()> {
    if request.evidence_text.is_empty() {
        return Err(AppError::BadRequest(
            "Evidence text is required".to_string(),
        ));
    }

    let now = current_unix_timestamp();

    sqlx::query(
        r#"
        UPDATE milestones SET
            evidence_text = $2,
            evidence_uri = $3,
            status = 'submitted',
            submitted_at = $4
        WHERE id = $1 AND status = 'pending'
        "#,
    )
    .bind(parse_milestone_db_id(milestone_id)?)
    .bind(&request.evidence_text)
    .bind(&request.evidence_uri)
    .bind(now)
    .execute(db.writer())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}

/// Get verification status for a milestone.
pub async fn get_verification(
    db: &Arc<PostgresDatabase>,
    milestone_id: &str,
) -> AppResult<IMilestoneVerificationData> {
    let db_id = parse_milestone_db_id(milestone_id)?;

    let row = sqlx::query_as::<_, VerificationRow>(
        r#"
        SELECT id, status, submitted_at, verified_at
        FROM milestones
        WHERE id = $1
        "#,
    )
    .bind(db_id)
    .fetch_optional(db.reader())
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .ok_or_else(|| AppError::NotFound(format!("Milestone {milestone_id} not found")))?;

    let status =
        MilestoneStatus::from_str(&row.status).unwrap_or(MilestoneStatus::Pending);

    // Estimate completion based on status
    let estimated_completion = match status {
        MilestoneStatus::Submitted | MilestoneStatus::InVerification => {
            row.submitted_at
                .map(|t| t + 7 * 86400) // ~7 days for verification
        }
        _ => None,
    };

    Ok(IMilestoneVerificationData {
        milestone_id: milestone_id.to_string(),
        status,
        submitted_at: row.submitted_at,
        estimated_completion,
        dispute_info: None,
    })
}

/// Parse milestone_id format "ms_001" to DB integer id.
fn parse_milestone_db_id(milestone_id: &str) -> AppResult<i32> {
    milestone_id
        .strip_prefix("ms_")
        .and_then(|s| s.parse::<i32>().ok())
        .filter(|&id| id >= 0)
        .ok_or_else(|| {
            AppError::BadRequest(format!("Invalid milestone ID format: {milestone_id}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_milestone_db_id_valid() {
        assert_eq!(parse_milestone_db_id("ms_1").unwrap(), 1);
        assert_eq!(parse_milestone_db_id("ms_001").unwrap(), 1);
        assert_eq!(parse_milestone_db_id("ms_42").unwrap(), 42);
        assert_eq!(parse_milestone_db_id("ms_999").unwrap(), 999);
    }

    #[test]
    fn parse_milestone_db_id_zero() {
        assert_eq!(parse_milestone_db_id("ms_0").unwrap(), 0);
    }

    #[test]
    fn parse_milestone_db_id_missing_prefix() {
        let result = parse_milestone_db_id("42");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("Invalid milestone ID format"));
            }
            other => panic!("Expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn parse_milestone_db_id_wrong_prefix() {
        let result = parse_milestone_db_id("milestone_1");
        assert!(result.is_err());
    }

    #[test]
    fn parse_milestone_db_id_non_numeric_suffix() {
        let result = parse_milestone_db_id("ms_abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_milestone_db_id_empty_suffix() {
        let result = parse_milestone_db_id("ms_");
        assert!(result.is_err());
    }

    #[test]
    fn parse_milestone_db_id_empty_string() {
        let result = parse_milestone_db_id("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_milestone_db_id_negative_number_is_invalid() {
        let result = parse_milestone_db_id("ms_-1");
        assert!(result.is_err(), "negative milestone IDs should be rejected");
        match result.unwrap_err() {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("Invalid milestone ID format"));
            }
            other => panic!("Expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn parse_milestone_db_id_overflow() {
        let result = parse_milestone_db_id("ms_99999999999999");
        assert!(result.is_err());
    }
}

#[derive(Debug, sqlx::FromRow)]
struct VerificationRow {
    #[allow(dead_code)]
    id: i32,
    status: String,
    submitted_at: Option<i64>,
    #[allow(dead_code)]
    verified_at: Option<i64>,
}
