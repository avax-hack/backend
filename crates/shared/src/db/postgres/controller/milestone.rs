use sqlx::PgPool;

use crate::types::milestone::IMilestoneInfo;

pub async fn find_by_project(pool: &PgPool, project_id: &str) -> anyhow::Result<Vec<IMilestoneInfo>> {
    let rows = sqlx::query_as::<_, MilestoneRow>(
        r#"
        SELECT id, project_id, milestone_index, title, description,
               allocation_bps, status, funds_released, release_amount::TEXT,
               evidence_uri, evidence_text, submitted_at, verified_at, tx_hash
        FROM milestones
        WHERE project_id = $1
        ORDER BY milestone_index ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn insert_batch(
    pool: &PgPool,
    project_id: &str,
    milestones: &[(i32, String, String, i32)],
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    for (index, title, description, allocation_bps) in milestones {
        sqlx::query(
            r#"
            INSERT INTO milestones (project_id, milestone_index, title, description, allocation_bps, status, funds_released)
            VALUES ($1, $2, $3, $4, $5, 'pending', FALSE)
            "#,
        )
        .bind(project_id)
        .bind(index)
        .bind(title)
        .bind(description)
        .bind(allocation_bps)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn update_status(
    pool: &PgPool,
    project_id: &str,
    milestone_index: i32,
    status: &str,
    tx_hash: Option<&str>,
    release_amount: Option<&str>,
) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    let funds_released = status == "completed";
    let verified_at = if funds_released { Some(now) } else { None };

    sqlx::query(
        r#"
        UPDATE milestones SET
            status = $3,
            funds_released = $4,
            verified_at = $5,
            tx_hash = COALESCE($6, tx_hash),
            release_amount = COALESCE($7::NUMERIC, release_amount)
        WHERE project_id = $1 AND milestone_index = $2
        "#,
    )
    .bind(project_id)
    .bind(milestone_index)
    .bind(status)
    .bind(funds_released)
    .bind(verified_at)
    .bind(tx_hash)
    .bind(release_amount.map(|s| s.to_string()))
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct MilestoneRow {
    id: i32,
    project_id: String,
    milestone_index: i32,
    title: String,
    description: String,
    allocation_bps: i32,
    status: String,
    funds_released: bool,
    release_amount: Option<String>,
    evidence_uri: Option<String>,
    evidence_text: Option<String>,
    submitted_at: Option<i64>,
    verified_at: Option<i64>,
    tx_hash: Option<String>,
}

impl From<MilestoneRow> for IMilestoneInfo {
    fn from(row: MilestoneRow) -> Self {
        Self {
            milestone_id: format!("ms_{:03}", row.id),
            order: row.milestone_index,
            title: row.title,
            description: row.description,
            fund_allocation_percent: row.allocation_bps / 100,
            fund_release_amount: row.release_amount.unwrap_or_else(|| "0".to_string()),
            status: crate::types::milestone::MilestoneStatus::from_str(&row.status)
                .unwrap_or(crate::types::milestone::MilestoneStatus::Pending),
            funds_released: row.funds_released,
            evidence_uri: row.evidence_uri,
            submitted_at: row.submitted_at,
            verified_at: row.verified_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::milestone::MilestoneStatus;

    fn sample_row() -> MilestoneRow {
        MilestoneRow {
            id: 1,
            project_id: "proj_001".to_string(),
            milestone_index: 0,
            title: "MVP".to_string(),
            description: "Build MVP".to_string(),
            allocation_bps: 5000,
            status: "pending".to_string(),
            funds_released: false,
            release_amount: None,
            evidence_uri: None,
            evidence_text: None,
            submitted_at: None,
            verified_at: None,
            tx_hash: None,
        }
    }

    #[test]
    fn test_milestone_row_to_info_basic() {
        let row = sample_row();
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.milestone_id, "ms_001");
        assert_eq!(info.order, 0);
        assert_eq!(info.title, "MVP");
        assert_eq!(info.fund_allocation_percent, 50); // 5000 / 100
        assert_eq!(info.fund_release_amount, "0");
        assert_eq!(info.status, MilestoneStatus::Pending);
        assert!(!info.funds_released);
    }

    #[test]
    fn test_milestone_row_to_info_with_release() {
        let mut row = sample_row();
        row.status = "completed".to_string();
        row.funds_released = true;
        row.release_amount = Some("500000000000".to_string());
        row.verified_at = Some(1717248600);
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.status, MilestoneStatus::Completed);
        assert!(info.funds_released);
        assert_eq!(info.fund_release_amount, "500000000000");
        assert_eq!(info.verified_at, Some(1717248600));
    }

    #[test]
    fn test_milestone_row_to_info_unknown_status_defaults_to_pending() {
        let mut row = sample_row();
        row.status = "unknown_garbage".to_string();
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.status, MilestoneStatus::Pending);
    }

    #[test]
    fn test_milestone_row_to_info_id_formatting() {
        let mut row = sample_row();
        row.id = 42;
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.milestone_id, "ms_042");
    }

    #[test]
    fn test_milestone_row_to_info_large_id() {
        let mut row = sample_row();
        row.id = 9999;
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.milestone_id, "ms_9999");
    }

    #[test]
    fn test_milestone_row_to_info_with_evidence() {
        let mut row = sample_row();
        row.evidence_uri = Some("https://proof.pdf".to_string());
        row.submitted_at = Some(1717232400);
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.evidence_uri, Some("https://proof.pdf".to_string()));
        assert_eq!(info.submitted_at, Some(1717232400));
    }

    #[test]
    fn test_milestone_row_allocation_bps_conversion() {
        let mut row = sample_row();
        row.allocation_bps = 2500; // 25%
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.fund_allocation_percent, 25);
    }

    #[test]
    fn test_milestone_row_allocation_bps_small() {
        let mut row = sample_row();
        row.allocation_bps = 100; // 1%
        let info: IMilestoneInfo = row.into();
        assert_eq!(info.fund_allocation_percent, 1);
    }

    #[test]
    fn test_milestone_row_all_statuses() {
        for (status_str, expected) in [
            ("completed", MilestoneStatus::Completed),
            ("in_verification", MilestoneStatus::InVerification),
            ("submitted", MilestoneStatus::Submitted),
            ("pending", MilestoneStatus::Pending),
            ("failed", MilestoneStatus::Failed),
        ] {
            let mut row = sample_row();
            row.status = status_str.to_string();
            let info: IMilestoneInfo = row.into();
            assert_eq!(info.status, expected);
        }
    }
}
