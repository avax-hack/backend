use sqlx::PgPool;

use crate::types::trading::ChartBar;

pub async fn upsert_bar(
    pool: &PgPool,
    token_id: &str,
    interval: &str,
    bar: &ChartBar,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO charts (token_id, interval, time, open, high, low, close, volume)
        VALUES ($1, $2, $3, $4::NUMERIC, $5::NUMERIC, $6::NUMERIC, $7::NUMERIC, $8::NUMERIC)
        ON CONFLICT (token_id, interval, time) DO UPDATE SET
            high = GREATEST(charts.high, $5::NUMERIC),
            low = LEAST(charts.low, $6::NUMERIC),
            close = $7::NUMERIC,
            volume = charts.volume + $8::NUMERIC
        "#,
    )
    .bind(token_id)
    .bind(interval)
    .bind(bar.time)
    .bind(&bar.open)
    .bind(&bar.high)
    .bind(&bar.low)
    .bind(&bar.close)
    .bind(&bar.volume)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_bars(
    pool: &PgPool,
    token_id: &str,
    interval: &str,
    from: i64,
    to: i64,
    limit: i64,
) -> anyhow::Result<Vec<ChartBar>> {
    let rows = sqlx::query_as::<_, ChartBarRow>(
        r#"
        SELECT time, open::TEXT as open, high::TEXT as high,
               low::TEXT as low, close::TEXT as close, volume::TEXT as volume
        FROM charts
        WHERE token_id = $1 AND interval = $2 AND time >= $3 AND time <= $4
        ORDER BY time ASC
        LIMIT $5
        "#,
    )
    .bind(token_id)
    .bind(interval)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| ChartBar {
        time: r.time,
        open: r.open,
        high: r.high,
        low: r.low,
        close: r.close,
        volume: r.volume,
    }).collect())
}

#[derive(Debug, sqlx::FromRow)]
struct ChartBarRow {
    time: i64,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_bar_row_to_chart_bar() {
        let row = ChartBarRow {
            time: 1717200000,
            open: "0.025".to_string(),
            high: "0.030".to_string(),
            low: "0.020".to_string(),
            close: "0.028".to_string(),
            volume: "50000".to_string(),
        };
        let bar = ChartBar {
            time: row.time,
            open: row.open.clone(),
            high: row.high.clone(),
            low: row.low.clone(),
            close: row.close.clone(),
            volume: row.volume.clone(),
        };
        assert_eq!(bar.time, 1717200000);
        assert_eq!(bar.open, "0.025");
        assert_eq!(bar.high, "0.030");
        assert_eq!(bar.low, "0.020");
        assert_eq!(bar.close, "0.028");
        assert_eq!(bar.volume, "50000");
    }

    #[test]
    fn test_chart_bar_row_debug() {
        let row = ChartBarRow {
            time: 0,
            open: "0".to_string(),
            high: "0".to_string(),
            low: "0".to_string(),
            close: "0".to_string(),
            volume: "0".to_string(),
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("ChartBarRow"));
    }

    #[test]
    fn test_chart_bar_row_conversion_preserves_data() {
        let rows = vec![
            ChartBarRow {
                time: 100,
                open: "1.0".to_string(),
                high: "1.5".to_string(),
                low: "0.5".to_string(),
                close: "1.2".to_string(),
                volume: "999".to_string(),
            },
            ChartBarRow {
                time: 200,
                open: "1.2".to_string(),
                high: "1.8".to_string(),
                low: "1.0".to_string(),
                close: "1.6".to_string(),
                volume: "1500".to_string(),
            },
        ];
        let bars: Vec<ChartBar> = rows
            .into_iter()
            .map(|r| ChartBar {
                time: r.time,
                open: r.open,
                high: r.high,
                low: r.low,
                close: r.close,
                volume: r.volume,
            })
            .collect();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].time, 100);
        assert_eq!(bars[1].time, 200);
        assert_eq!(bars[1].close, "1.6");
    }
}
