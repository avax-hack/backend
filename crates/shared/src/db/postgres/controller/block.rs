use sqlx::PgPool;

pub async fn get_last_block(pool: &PgPool, event_type: &str) -> anyhow::Result<Option<i64>> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT last_block FROM block_progress WHERE event_type = $1",
    )
    .bind(event_type)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn set_last_block(pool: &PgPool, event_type: &str, block_number: i64) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        INSERT INTO block_progress (event_type, last_block, updated_at)
        VALUES ($1, $2, $3)
        ON CONFLICT (event_type) DO UPDATE SET
            last_block = $2,
            updated_at = $3
        "#,
    )
    .bind(event_type)
    .bind(block_number)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
