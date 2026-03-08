use sqlx::PgPool;

pub async fn upsert(pool: &PgPool, data: &MarketDataRow) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        INSERT INTO market_data (token_id, market_type, token_price, ath_price,
                                  total_supply, volume_24h, holder_count, bonding_percent,
                                  milestone_completed, milestone_total, is_graduated, updated_at)
        VALUES ($1, $2, $3::NUMERIC, $4::NUMERIC, $5::NUMERIC, $6::NUMERIC,
                $7, $8::NUMERIC, $9, $10, $11, $12)
        ON CONFLICT (token_id) DO UPDATE SET
            market_type = $2,
            token_price = $3::NUMERIC,
            ath_price = GREATEST(market_data.ath_price, $4::NUMERIC),
            total_supply = $5::NUMERIC,
            volume_24h = $6::NUMERIC,
            holder_count = $7,
            bonding_percent = $8::NUMERIC,
            milestone_completed = $9,
            milestone_total = $10,
            is_graduated = $11,
            updated_at = $12
        "#,
    )
    .bind(&data.token_id)
    .bind(&data.market_type)
    .bind(&data.token_price)
    .bind(&data.ath_price)
    .bind(&data.total_supply)
    .bind(&data.volume_24h)
    .bind(data.holder_count)
    .bind(&data.bonding_percent)
    .bind(data.milestone_completed)
    .bind(data.milestone_total)
    .bind(data.is_graduated)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_token(pool: &PgPool, token_id: &str) -> anyhow::Result<Option<MarketDataRow>> {
    let row = sqlx::query_as::<_, MarketDataRow>(
        r#"
        SELECT token_id, market_type, token_price::TEXT as token_price,
               ath_price::TEXT as ath_price,
               total_supply::TEXT as total_supply, volume_24h::TEXT as volume_24h,
               holder_count, bonding_percent::TEXT as bonding_percent,
               milestone_completed, milestone_total, is_graduated
        FROM market_data WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Add volume to the existing volume_24h for a token.
pub async fn add_volume(pool: &PgPool, token_id: &str, volume: &str) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        UPDATE market_data
        SET volume_24h = volume_24h + $2::NUMERIC, updated_at = $3
        WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .bind(volume)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update holder_count from the balances table (count of accounts with balance > 0).
pub async fn refresh_holder_count(pool: &PgPool, token_id: &str) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        UPDATE market_data
        SET holder_count = (SELECT COUNT(*) FROM balances WHERE token_id = $1 AND balance > 0),
            updated_at = $2
        WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_graduated(pool: &PgPool, token_id: &str) -> anyhow::Result<()> {
    let now = crate::types::common::current_unix_timestamp();
    sqlx::query(
        r#"
        UPDATE market_data
        SET is_graduated = true, market_type = 'DEX', updated_at = $2
        WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub struct MarketDataRow {
    pub token_id: String,
    pub market_type: String,
    pub token_price: String,
    pub ath_price: String,
    pub total_supply: String,
    pub volume_24h: String,
    pub holder_count: i32,
    pub bonding_percent: String,
    pub milestone_completed: i32,
    pub milestone_total: i32,
    pub is_graduated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_row_fields() {
        let row = MarketDataRow {
            token_id: "0xtoken".to_string(),
            market_type: "CURVE".to_string(),
            token_price: "0.025".to_string(),
            ath_price: "0.050".to_string(),
            total_supply: "1000000".to_string(),
            volume_24h: "50000".to_string(),
            holder_count: 123,
            bonding_percent: "45.5".to_string(),
            milestone_completed: 2,
            milestone_total: 5,
            is_graduated: false,
        };
        assert_eq!(row.token_id, "0xtoken");
        assert_eq!(row.market_type, "CURVE");
        assert_eq!(row.holder_count, 123);
        assert!(!row.is_graduated);
    }

    #[test]
    fn test_market_data_row_graduated() {
        let row = MarketDataRow {
            token_id: "0xt".to_string(),
            market_type: "DEX".to_string(),
            token_price: "1".to_string(),
            ath_price: "2".to_string(),
            total_supply: "1000".to_string(),
            volume_24h: "100".to_string(),
            holder_count: 50,
            bonding_percent: "100".to_string(),
            milestone_completed: 5,
            milestone_total: 5,
            is_graduated: true,
        };
        assert!(row.is_graduated);
        assert_eq!(row.milestone_completed, row.milestone_total);
    }

    #[test]
    fn test_market_data_row_debug() {
        let row = MarketDataRow {
            token_id: "0x".to_string(),
            market_type: "IDO".to_string(),
            token_price: "0".to_string(),
            ath_price: "0".to_string(),
            total_supply: "0".to_string(),
            volume_24h: "0".to_string(),
            holder_count: 0,
            bonding_percent: "0".to_string(),
            milestone_completed: 0,
            milestone_total: 0,
            is_graduated: false,
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("MarketDataRow"));
    }
}
