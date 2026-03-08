use bigdecimal::BigDecimal;
use sqlx::PgPool;
use std::str::FromStr;

/// Insert a new project from a ProjectCreated on-chain event.
pub async fn insert_from_event(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    symbol: &str,
    token_uri: &str,
    creator: &str,
    ido_token_amount: &str,
    token_price: &str,
    deadline: i64,
    total_supply: &str,
    tx_hash: &str,
    created_at: i64,
) -> anyhow::Result<()> {
    let ido_amount = BigDecimal::from_str(ido_token_amount).unwrap_or_default();
    let price = BigDecimal::from_str(token_price).unwrap_or_default();
    let target_raise = (&ido_amount * &price).to_string();

    sqlx::query(
        r#"
        INSERT INTO projects (
            project_id, name, symbol, image_uri, tagline, category, creator,
            status, target_raise, token_price, ido_supply, total_supply,
            deadline, tx_hash, created_at
        )
        VALUES (
            $1, $2, $3, $4, '', 'general', $5,
            'funding', $6::NUMERIC, $7::NUMERIC, $8::NUMERIC, $9::NUMERIC,
            $10, $11, $12
        )
        ON CONFLICT (project_id) DO NOTHING
        "#,
    )
    .bind(project_id)
    .bind(name)
    .bind(symbol)
    .bind(token_uri)
    .bind(creator)
    .bind(&target_raise)
    .bind(token_price)
    .bind(ido_token_amount)
    .bind(total_supply)
    .bind(deadline)
    .bind(tx_hash)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update USDC raised for a project after a token purchase.
pub async fn add_usdc_raised(
    pool: &PgPool,
    project_id: &str,
    usdc_amount: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE projects
        SET usdc_raised = usdc_raised + $2::NUMERIC
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .bind(usdc_amount)
    .execute(pool)
    .await?;
    Ok(())
}
