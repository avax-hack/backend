use redis::AsyncCommands;

use super::RedisDatabase;

const WHITELIST_TOKENS_KEY: &str = "whitelist:tokens";

impl RedisDatabase {
    /// Add a token address to the whitelist set.
    pub async fn whitelist_add_token(&self, token_id: &str) -> anyhow::Result<()> {
        let mut conn = self.conn();
        let () = conn.sadd(WHITELIST_TOKENS_KEY, token_id).await?;
        Ok(())
    }

    /// Get all token addresses from the whitelist set.
    pub async fn whitelist_get_tokens(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.conn();
        let members: Vec<String> = conn.smembers(WHITELIST_TOKENS_KEY).await?;
        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_key() {
        assert_eq!(WHITELIST_TOKENS_KEY, "whitelist:tokens");
    }
}
