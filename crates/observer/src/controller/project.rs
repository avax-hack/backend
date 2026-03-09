use sqlx::PgPool;

use openlaunch_shared::utils::price::wei_to_display;

const USDC_DECIMALS: u32 = 6;
const TOKEN_DECIMALS: u32 = 18;

/// Insert a new project from a ProjectCreated on-chain event.
/// All numeric values are normalized from raw on-chain wei to human-readable strings.
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
    let ido_display = wei_to_display(ido_token_amount, TOKEN_DECIMALS)?;
    let price_display = wei_to_display(token_price, USDC_DECIMALS)?;
    let total_supply_display = wei_to_display(total_supply, TOKEN_DECIMALS)?;

    // target_raise (USDC) = ido_token_amount (token, 18 dec) * token_price (USDC, 6 dec)
    // = raw_ido * raw_price / 10^(18+6) = raw_ido * raw_price / 10^24
    let raw_product = {
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        let ido_bd = BigDecimal::from_str(ido_token_amount).unwrap_or_default();
        let price_bd = BigDecimal::from_str(token_price).unwrap_or_default();
        ido_bd * price_bd
    };
    let target_raise_display = wei_to_display(&raw_product.to_string(), TOKEN_DECIMALS + USDC_DECIMALS)?;

    // Try to link with pre-chain project by updating its project_id to the on-chain token address
    let updated = sqlx::query(
        r#"
        UPDATE projects
        SET project_id = $1, image_uri = $4, token_price = $7::NUMERIC,
            ido_supply = $8::NUMERIC, total_supply = $9::NUMERIC,
            target_raise = $6::NUMERIC, deadline = $10, tx_hash = $11
        WHERE symbol = $3 AND tx_hash = ''
        "#,
    )
    .bind(project_id)
    .bind(name)
    .bind(symbol)
    .bind(token_uri)
    .bind(creator)
    .bind(&target_raise_display)
    .bind(&price_display)
    .bind(&ido_display)
    .bind(&total_supply_display)
    .bind(deadline)
    .bind(tx_hash)
    .bind(created_at)
    .execute(pool)
    .await?;

    // If no pre-chain project matched, insert a new one
    if updated.rows_affected() == 0 {
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
        .bind(&target_raise_display)
        .bind(&price_display)
        .bind(&ido_display)
        .bind(&total_supply_display)
        .bind(deadline)
        .bind(tx_hash)
        .bind(created_at)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Update USDC raised for a project after a token purchase.
/// `usdc_amount` is raw on-chain value (6 decimals), normalized before storing.
pub async fn add_usdc_raised(
    pool: &PgPool,
    project_id: &str,
    usdc_amount: &str,
) -> anyhow::Result<()> {
    let usdc_display = wei_to_display(usdc_amount, USDC_DECIMALS)?;

    sqlx::query(
        r#"
        UPDATE projects
        SET usdc_raised = usdc_raised + $2::NUMERIC
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .bind(&usdc_display)
    .execute(pool)
    .await?;
    Ok(())
}
