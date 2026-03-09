use sqlx::PgPool;

use crate::types::common::PaginationParams;

pub async fn upsert(
    pool: &PgPool,
    account_id: &str,
    token_id: &str,
    balance: &str,
) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        INSERT INTO balances (account_id, token_id, balance, updated_at)
        VALUES ($1, $2, $3::NUMERIC, $4)
        ON CONFLICT (account_id, token_id) DO UPDATE SET
            balance = $3::NUMERIC,
            updated_at = $4
        "#,
    )
    .bind(account_id)
    .bind(token_id)
    .bind(balance)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_balance(
    pool: &PgPool,
    account_id: &str,
    token_id: &str,
    amount: &str,
) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        INSERT INTO balances (account_id, token_id, balance, updated_at)
        VALUES ($1, $2, $3::NUMERIC, $4)
        ON CONFLICT (account_id, token_id) DO UPDATE SET
            balance = balances.balance + $3::NUMERIC,
            updated_at = $4
        "#,
    )
    .bind(account_id)
    .bind(token_id)
    .bind(amount)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_account(
    pool: &PgPool,
    account_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<BalanceRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, BalanceRow>(
        r#"
        SELECT b.token_id, b.balance::TEXT as balance, b.updated_at
        FROM balances b
        WHERE b.account_id = $1 AND b.balance > 0
        ORDER BY b.updated_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM balances WHERE account_id = $1 AND balance > 0",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

pub async fn find_holders(
    pool: &PgPool,
    token_id: &str,
    pagination: &PaginationParams,
) -> anyhow::Result<(Vec<HolderRow>, i64)> {
    let p = pagination.validated();

    let rows = sqlx::query_as::<_, HolderRow>(
        r#"
        SELECT b.account_id, b.balance::TEXT as balance
        FROM balances b
        WHERE b.token_id = $1 AND b.balance > 0
        ORDER BY b.balance DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(token_id)
    .bind(p.limit)
    .bind(p.offset())
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM balances WHERE token_id = $1 AND balance > 0",
    )
    .bind(token_id)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

#[derive(Debug, sqlx::FromRow)]
pub struct BalanceRow {
    pub token_id: String,
    pub balance: String,
    pub updated_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct HolderRow {
    pub account_id: String,
    pub balance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_row_fields() {
        let row = BalanceRow {
            token_id: "0xtoken".to_string(),
            balance: "1000000000000000000".to_string(),
            updated_at: 1717200000,
        };
        assert_eq!(row.token_id, "0xtoken");
        assert_eq!(row.balance, "1000000000000000000");
        assert_eq!(row.updated_at, 1717200000);
    }

    #[test]
    fn test_holder_row_fields() {
        let row = HolderRow {
            account_id: "0xholder".to_string(),
            balance: "50000".to_string(),
        };
        assert_eq!(row.account_id, "0xholder");
        assert_eq!(row.balance, "50000");
    }

    #[test]
    fn test_balance_row_debug() {
        let row = BalanceRow {
            token_id: "0x".to_string(),
            balance: "0".to_string(),
            updated_at: 0,
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("BalanceRow"));
    }

    #[test]
    fn test_holder_row_debug() {
        let row = HolderRow {
            account_id: "0x".to_string(),
            balance: "0".to_string(),
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("HolderRow"));
    }
}
