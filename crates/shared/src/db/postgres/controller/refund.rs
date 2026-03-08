use sqlx::PgPool;

use crate::types::common::PaginationParams;

pub async fn insert(
    pool: &PgPool,
    project_id: &str,
    account_id: &str,
    tokens_burned: &str,
    usdc_returned: &str,
    tx_hash: &str,
    block_number: i64,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO refunds (project_id, account_id, tokens_burned, usdc_returned, tx_hash, block_number, created_at)
        VALUES ($1, $2, $3::NUMERIC, $4::NUMERIC, $5, $6, $7)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(project_id)
    .bind(account_id)
    .bind(tokens_burned)
    .bind(usdc_returned)
    .bind(tx_hash)
    .bind(block_number)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_account(
    pool: &PgPool,
    account_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<RefundRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, RefundRow>(
        r#"
        SELECT r.project_id, r.tokens_burned::TEXT as tokens_burned,
               r.usdc_returned::TEXT as usdc_returned,
               r.tx_hash, r.created_at
        FROM refunds r
        WHERE r.account_id = $1
        ORDER BY r.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM refunds WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

pub async fn find_enriched_by_account(
    pool: &PgPool,
    account_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<EnrichedRefundRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, EnrichedRefundRow>(
        r#"
        SELECT r.project_id,
               r.tokens_burned::TEXT as tokens_burned,
               r.usdc_returned::TEXT as usdc_returned,
               r.tx_hash,
               r.created_at,
               COALESCE((SELECT SUM(i.usdc_amount)::TEXT FROM investments i
                         WHERE i.account_id = r.account_id AND i.project_id = r.project_id), '0') as original_investment,
               (SELECT m.title FROM milestones m
                WHERE m.project_id = r.project_id AND m.status = 'failed'
                ORDER BY m.milestone_index ASC LIMIT 1) as failed_milestone
        FROM refunds r
        WHERE r.account_id = $1
        ORDER BY r.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM refunds WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

#[derive(Debug, sqlx::FromRow)]
pub struct RefundRow {
    pub project_id: String,
    pub tokens_burned: String,
    pub usdc_returned: String,
    pub tx_hash: String,
    pub created_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EnrichedRefundRow {
    pub project_id: String,
    pub tokens_burned: String,
    pub usdc_returned: String,
    pub tx_hash: String,
    pub created_at: i64,
    pub original_investment: String,
    pub failed_milestone: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refund_row_fields() {
        let row = RefundRow {
            project_id: "proj_001".to_string(),
            tokens_burned: "50000".to_string(),
            usdc_returned: "25000".to_string(),
            tx_hash: "0xrefund_hash".to_string(),
            created_at: 1717200000,
        };
        assert_eq!(row.project_id, "proj_001");
        assert_eq!(row.tokens_burned, "50000");
        assert_eq!(row.usdc_returned, "25000");
        assert_eq!(row.tx_hash, "0xrefund_hash");
    }

    #[test]
    fn test_refund_row_debug() {
        let row = RefundRow {
            project_id: "p".to_string(),
            tokens_burned: "0".to_string(),
            usdc_returned: "0".to_string(),
            tx_hash: "0x".to_string(),
            created_at: 0,
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("RefundRow"));
    }
}
