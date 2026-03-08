use sqlx::PgPool;

use crate::types::common::PaginationParams;

pub async fn find_by_id(pool: &PgPool, project_id: &str) -> anyhow::Result<Option<ProjectRow>> {
    let row = sqlx::query_as::<_, ProjectRow>(
        r#"
        SELECT project_id, name, symbol, image_uri, description, tagline, category,
               creator, status, target_raise::TEXT, token_price::TEXT,
               ido_supply::TEXT, ido_sold::TEXT, total_supply::TEXT,
               usdc_raised::TEXT, usdc_released::TEXT, tokens_refunded::TEXT,
               deadline, website, twitter, github, telegram, created_at, tx_hash
        FROM projects WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn validate_symbol(pool: &PgPool, symbol: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, Option<bool>>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE symbol = $1)",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    Ok(!exists)
}

pub async fn find_list(
    pool: &PgPool,
    sort_type: &str,
    pagination: &PaginationParams,
    status: Option<&str>,
) -> anyhow::Result<(Vec<ProjectListRow>, i64)> {
    find_list_filtered(pool, sort_type, pagination, status, None, None, false).await
}

pub async fn find_list_filtered(
    pool: &PgPool,
    sort_type: &str,
    pagination: &PaginationParams,
    status: Option<&str>,
    category: Option<&str>,
    search: Option<&str>,
    verified_only: bool,
) -> anyhow::Result<(Vec<ProjectListRow>, i64)> {
    let p = pagination.validated();
    let order_clause = match sort_type {
        "recent" => "p.created_at DESC",
        "funded" => "p.usdc_raised DESC",
        "target" => "p.target_raise DESC",
        "investors" => "investor_count DESC",
        _ => "p.created_at DESC",
    };

    let search_pattern = search.map(|s| format!("%{s}%"));

    let verified_join = if verified_only {
        "INNER JOIN milestones m_v ON m_v.project_id = p.project_id AND m_v.status = 'completed'"
    } else {
        ""
    };

    // Use the computed order_clause for dynamic sorting.
    let query = format!(
        r#"
        SELECT DISTINCT p.project_id, p.name, p.symbol, p.image_uri, p.tagline, p.category,
               p.creator, p.status, p.target_raise::TEXT, p.usdc_raised::TEXT,
               p.created_at,
               COALESCE((SELECT COUNT(*) FROM investments i WHERE i.project_id = p.project_id), 0) as investor_count
        FROM projects p
        {verified_join}
        WHERE ($3::TEXT IS NULL OR p.status = $3)
          AND ($4::TEXT IS NULL OR p.category = $4)
          AND ($5::TEXT IS NULL OR (p.name ILIKE $5 OR p.symbol ILIKE $5))
        ORDER BY {order_clause}
        LIMIT $1 OFFSET $2
        "#,
    );
    let rows = sqlx::query_as::<_, ProjectListRow>(&query)
    .bind(p.limit)
    .bind(p.offset())
    .bind(status)
    .bind(category)
    .bind(&search_pattern)
    .fetch_all(pool)
    .await?;

    let count_query = format!(
        r#"
        SELECT COUNT(DISTINCT p.project_id) FROM projects p
        {verified_join}
        WHERE ($1::TEXT IS NULL OR p.status = $1)
          AND ($2::TEXT IS NULL OR p.category = $2)
          AND ($3::TEXT IS NULL OR (p.name ILIKE $3 OR p.symbol ILIKE $3))
        "#,
    );
    let total = sqlx::query_scalar::<_, Option<i64>>(&count_query)
    .bind(status)
    .bind(category)
    .bind(&search_pattern)
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok((rows, total))
}

pub async fn update_status(pool: &PgPool, project_id: &str, status: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE projects SET status = $2 WHERE project_id = $1")
        .bind(project_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub struct ProjectRow {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: Option<String>,
    pub tagline: String,
    pub category: String,
    pub creator: String,
    pub status: String,
    pub target_raise: Option<String>,
    pub token_price: Option<String>,
    pub ido_supply: Option<String>,
    pub ido_sold: Option<String>,
    pub total_supply: Option<String>,
    pub usdc_raised: Option<String>,
    pub usdc_released: Option<String>,
    pub tokens_refunded: Option<String>,
    pub deadline: i64,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub created_at: i64,
    pub tx_hash: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ProjectListRow {
    pub project_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub tagline: String,
    pub category: String,
    pub creator: String,
    pub status: String,
    pub target_raise: Option<String>,
    pub usdc_raised: Option<String>,
    pub created_at: i64,
    pub investor_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_row_fields() {
        let row = ProjectRow {
            project_id: "proj_1".to_string(),
            name: "TestProject".to_string(),
            symbol: "TP".to_string(),
            image_uri: "img.png".to_string(),
            description: Some("A project".to_string()),
            tagline: "Test".to_string(),
            category: "defi".to_string(),
            creator: "0xcreator".to_string(),
            status: "funding".to_string(),
            target_raise: Some("1000000".to_string()),
            token_price: Some("0.01".to_string()),
            ido_supply: Some("100000000".to_string()),
            ido_sold: Some("50000000".to_string()),
            total_supply: Some("1000000000".to_string()),
            usdc_raised: Some("500000".to_string()),
            usdc_released: Some("0".to_string()),
            tokens_refunded: Some("0".to_string()),
            deadline: 1717300000,
            website: Some("https://test.com".to_string()),
            twitter: None,
            github: None,
            telegram: None,
            created_at: 1717200000,
            tx_hash: "0xhash".to_string(),
        };
        assert_eq!(row.project_id, "proj_1");
        assert_eq!(row.status, "funding");
        assert_eq!(row.deadline, 1717300000);
    }

    #[test]
    fn test_project_row_optional_fields_none() {
        let row = ProjectRow {
            project_id: "proj_2".to_string(),
            name: "Minimal".to_string(),
            symbol: "MIN".to_string(),
            image_uri: "".to_string(),
            description: None,
            tagline: "Minimal project".to_string(),
            category: "other".to_string(),
            creator: "0x1".to_string(),
            status: "active".to_string(),
            target_raise: None,
            token_price: None,
            ido_supply: None,
            ido_sold: None,
            total_supply: None,
            usdc_raised: None,
            usdc_released: None,
            tokens_refunded: None,
            deadline: 0,
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            created_at: 0,
            tx_hash: "0x".to_string(),
        };
        assert!(row.description.is_none());
        assert!(row.target_raise.is_none());
        assert!(row.website.is_none());
    }

    #[test]
    fn test_project_list_row_fields() {
        let row = ProjectListRow {
            project_id: "p1".to_string(),
            name: "Proj".to_string(),
            symbol: "P".to_string(),
            image_uri: "i.png".to_string(),
            tagline: "Tag".to_string(),
            category: "gaming".to_string(),
            creator: "0xc".to_string(),
            status: "funding".to_string(),
            target_raise: Some("100".to_string()),
            usdc_raised: Some("50".to_string()),
            created_at: 1000,
            investor_count: 5,
        };
        assert_eq!(row.investor_count, 5);
        assert_eq!(row.usdc_raised, Some("50".to_string()));
    }

    #[test]
    fn test_project_row_debug() {
        let row = ProjectRow {
            project_id: "p".to_string(),
            name: "N".to_string(),
            symbol: "S".to_string(),
            image_uri: "".to_string(),
            description: None,
            tagline: "t".to_string(),
            category: "c".to_string(),
            creator: "0x".to_string(),
            status: "funding".to_string(),
            target_raise: None,
            token_price: None,
            ido_supply: None,
            ido_sold: None,
            total_supply: None,
            usdc_raised: None,
            usdc_released: None,
            tokens_refunded: None,
            deadline: 0,
            website: None,
            twitter: None,
            github: None,
            telegram: None,
            created_at: 0,
            tx_hash: "0x".to_string(),
        };
        let debug = format!("{:?}", row);
        assert!(debug.contains("ProjectRow"));
    }
}
