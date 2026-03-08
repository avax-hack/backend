use sqlx::PgPool;

use crate::types::common::PaginationParams;

pub async fn insert(
    pool: &PgPool,
    project_id: &str,
    account_id: &str,
    usdc_amount: &str,
    token_amount: &str,
    tx_hash: &str,
    block_number: i64,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO investments (project_id, account_id, usdc_amount, token_amount, tx_hash, block_number, created_at)
        VALUES ($1, $2, $3::NUMERIC, $4::NUMERIC, $5, $6, $7)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(project_id)
    .bind(account_id)
    .bind(usdc_amount)
    .bind(token_amount)
    .bind(tx_hash)
    .bind(block_number)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_project(
    pool: &PgPool,
    project_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<InvestmentRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, InvestmentRow>(
        r#"
        SELECT i.account_id, i.usdc_amount::TEXT as usdc_amount, i.created_at
        FROM investments i
        WHERE i.project_id = $1
        ORDER BY i.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(project_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM investments WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

#[derive(Debug, sqlx::FromRow)]
pub struct InvestmentRow {
    pub account_id: String,
    pub usdc_amount: String,
    pub created_at: i64,
}

pub async fn find_by_account(
    pool: &PgPool,
    account_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<IdoHistoryRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, IdoHistoryRow>(
        r#"
        SELECT i.project_id,
               i.usdc_amount::TEXT as usdc_amount,
               COALESCE(i.token_amount, 0)::TEXT as token_amount,
               COALESCE(p.status, 'funding') as status,
               i.created_at
        FROM investments i
        LEFT JOIN projects p ON p.project_id = i.project_id
        WHERE i.account_id = $1
        ORDER BY i.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM investments WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

#[derive(Debug, sqlx::FromRow)]
pub struct IdoHistoryRow {
    pub project_id: String,
    pub usdc_amount: String,
    pub token_amount: String,
    pub status: String,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_investment_row_fields() {
        let row = InvestmentRow {
            account_id: "0xinvestor".to_string(),
            usdc_amount: "1000000".to_string(),
            created_at: 1717200000,
        };
        assert_eq!(row.account_id, "0xinvestor");
        assert_eq!(row.usdc_amount, "1000000");
        assert_eq!(row.created_at, 1717200000);
    }

    #[test]
    fn test_investment_row_debug() {
        let row = InvestmentRow {
            account_id: "0x".to_string(),
            usdc_amount: "0".to_string(),
            created_at: 0,
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("InvestmentRow"));
    }
}
