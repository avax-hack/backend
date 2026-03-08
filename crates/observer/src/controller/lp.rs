use sqlx::PgPool;

/// Insert a liquidity position from a LiquidityAllocated event.
pub async fn insert_liquidity_position(
    pool: &PgPool,
    token_id: &str,
    pool_id: &str,
    tick_lower: i32,
    tick_upper: i32,
    liquidity: &str,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO liquidity_positions (token_id, pool_id, tick_lower, tick_upper, liquidity, created_at)
        VALUES ($1, $2, $3, $4, $5::NUMERIC, $6)
        ON CONFLICT (token_id) DO UPDATE SET
            pool_id = $2,
            tick_lower = $3,
            tick_upper = $4,
            liquidity = $5::NUMERIC
        "#,
    )
    .bind(token_id)
    .bind(pool_id)
    .bind(tick_lower)
    .bind(tick_upper)
    .bind(liquidity)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert a fee collection record from a FeesCollected event.
pub async fn insert_fee_collection(
    pool: &PgPool,
    token_id: &str,
    amount0: &str,
    amount1: &str,
    tx_hash: &str,
    block_number: i64,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fee_collections (token_id, amount0, amount1, tx_hash, block_number, created_at)
        VALUES ($1, $2::NUMERIC, $3::NUMERIC, $4, $5, $6)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(token_id)
    .bind(amount0)
    .bind(amount1)
    .bind(tx_hash)
    .bind(block_number)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}
