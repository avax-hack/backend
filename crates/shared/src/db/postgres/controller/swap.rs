use sqlx::PgPool;

use crate::types::common::PaginationParams;

pub async fn insert(
    pool: &PgPool,
    token_id: &str,
    account_id: &str,
    event_type: &str,
    native_amount: &str,
    token_amount: &str,
    price: &str,
    value: &str,
    tx_hash: &str,
    block_number: i64,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO swaps (token_id, account_id, event_type, native_amount, token_amount, price, value, tx_hash, block_number, created_at)
        VALUES ($1, $2, $3, $4::NUMERIC, $5::NUMERIC, $6::NUMERIC, $7::NUMERIC, $8, $9, $10)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(token_id)
    .bind(account_id)
    .bind(event_type)
    .bind(native_amount)
    .bind(token_amount)
    .bind(price)
    .bind(value)
    .bind(tx_hash)
    .bind(block_number)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_token(
    pool: &PgPool,
    token_id: &str,
    pagination: &PaginationParams,
    trade_type: Option<&str>,
) -> anyhow::Result<(Vec<SwapRow>, i64)> {
    find_by_token_ordered(pool, token_id, pagination, trade_type, "DESC").await
}

pub async fn find_by_token_ordered(
    pool: &PgPool,
    token_id: &str,
    pagination: &PaginationParams,
    trade_type: Option<&str>,
    direction: &str,
) -> anyhow::Result<(Vec<SwapRow>, i64)> {
    let p = pagination.validated();

    let order_dir = match direction.to_uppercase().as_str() {
        "ASC" => "ASC",
        _ => "DESC",
    };

    let query = format!(
        r#"
        SELECT s.event_type, s.native_amount::TEXT as native_amount,
               s.token_amount::TEXT as token_amount, s.price::TEXT as price,
               s.value::TEXT as value, s.tx_hash, s.account_id, s.created_at
        FROM swaps s
        WHERE s.token_id = $1 AND ($4::TEXT IS NULL OR s.event_type = $4)
        ORDER BY s.created_at {order_dir}
        LIMIT $2 OFFSET $3
        "#,
    );

    let rows = sqlx::query_as::<_, SwapRow>(&query)
    .bind(token_id)
    .bind(p.limit)
    .bind(p.offset())
    .bind(trade_type)
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM swaps WHERE token_id = $1 AND ($2::TEXT IS NULL OR event_type = $2)",
    )
    .bind(token_id)
    .bind(trade_type)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

#[derive(Debug, sqlx::FromRow)]
pub struct SwapRow {
    pub event_type: String,
    pub native_amount: String,
    pub token_amount: String,
    pub price: String,
    pub value: String,
    pub tx_hash: String,
    pub account_id: String,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_row_fields() {
        let row = SwapRow {
            event_type: "BUY".to_string(),
            native_amount: "1000000000000000000".to_string(),
            token_amount: "50000".to_string(),
            price: "0.02".to_string(),
            value: "25.00".to_string(),
            tx_hash: "0xabc123".to_string(),
            account_id: "0xbuyer".to_string(),
            created_at: 1717200000,
        };
        assert_eq!(row.event_type, "BUY");
        assert_eq!(row.tx_hash, "0xabc123");
        assert_eq!(row.created_at, 1717200000);
    }

    #[test]
    fn test_swap_row_sell() {
        let row = SwapRow {
            event_type: "SELL".to_string(),
            native_amount: "500".to_string(),
            token_amount: "25000".to_string(),
            price: "0.02".to_string(),
            value: "10".to_string(),
            tx_hash: "0xdef".to_string(),
            account_id: "0xseller".to_string(),
            created_at: 1717200001,
        };
        assert_eq!(row.event_type, "SELL");
    }

    #[test]
    fn test_swap_row_debug() {
        let row = SwapRow {
            event_type: "BUY".to_string(),
            native_amount: "1".to_string(),
            token_amount: "1".to_string(),
            price: "1".to_string(),
            value: "1".to_string(),
            tx_hash: "0x".to_string(),
            account_id: "0x".to_string(),
            created_at: 0,
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("SwapRow"));
    }
}
