use sqlx::PgPool;

use crate::types::account::IAccountInfo;
use crate::types::common::current_unix_timestamp;

pub async fn find_by_id(pool: &PgPool, account_id: &str) -> anyhow::Result<Option<IAccountInfo>> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT account_id, nickname, bio, image_uri FROM accounts WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn upsert(pool: &PgPool, account_id: &str) -> anyhow::Result<IAccountInfo> {
    let now = current_unix_timestamp();
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        INSERT INTO accounts (account_id, nickname, bio, image_uri, created_at, updated_at)
        VALUES ($1, '', '', '', $2, $2)
        ON CONFLICT (account_id) DO UPDATE SET updated_at = $2
        RETURNING account_id, nickname, bio, image_uri
        "#,
    )
    .bind(account_id)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub async fn update(
    pool: &PgPool,
    account_id: &str,
    nickname: Option<&str>,
    bio: Option<&str>,
    image_uri: Option<&str>,
) -> anyhow::Result<IAccountInfo> {
    let now = current_unix_timestamp();
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        UPDATE accounts SET
            nickname = COALESCE($2, nickname),
            bio = COALESCE($3, bio),
            image_uri = COALESCE($4, image_uri),
            updated_at = $5
        WHERE account_id = $1
        RETURNING account_id, nickname, bio, image_uri
        "#,
    )
    .bind(account_id)
    .bind(nickname)
    .bind(bio)
    .bind(image_uri)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    account_id: String,
    nickname: String,
    bio: String,
    image_uri: String,
}

impl From<AccountRow> for IAccountInfo {
    fn from(row: AccountRow) -> Self {
        Self {
            account_id: row.account_id,
            nickname: row.nickname,
            bio: row.bio,
            image_uri: row.image_uri,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_row_to_account_info() {
        let row = AccountRow {
            account_id: "0xabc".to_string(),
            nickname: "alice".to_string(),
            bio: "hi there".to_string(),
            image_uri: "https://img.png".to_string(),
        };
        let info: IAccountInfo = row.into();
        assert_eq!(info.account_id, "0xabc");
        assert_eq!(info.nickname, "alice");
        assert_eq!(info.bio, "hi there");
        assert_eq!(info.image_uri, "https://img.png");
    }

    #[test]
    fn test_account_row_to_account_info_empty_fields() {
        let row = AccountRow {
            account_id: "0x123".to_string(),
            nickname: "".to_string(),
            bio: "".to_string(),
            image_uri: "".to_string(),
        };
        let info: IAccountInfo = row.into();
        assert_eq!(info.account_id, "0x123");
        assert!(info.nickname.is_empty());
        assert!(info.bio.is_empty());
    }
}
