use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

use super::RedisDatabase;

const CACHE_PREFIX: &str = "cache:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_prefix() {
        assert_eq!(CACHE_PREFIX, "cache:");
    }

    #[test]
    fn test_cache_key_format() {
        let key = "project:123";
        let full_key = format!("{CACHE_PREFIX}{key}");
        assert_eq!(full_key, "cache:project:123");
    }

    #[test]
    fn test_cache_key_with_special_chars() {
        let key = "user:0xabc:balance";
        let full_key = format!("{CACHE_PREFIX}{key}");
        assert_eq!(full_key, "cache:user:0xabc:balance");
    }
}

impl RedisDatabase {
    pub async fn cache_get<T: DeserializeOwned>(&self, key: &str) -> anyhow::Result<Option<T>> {
        let full_key = format!("{CACHE_PREFIX}{key}");
        let mut conn = self.conn();
        let value: Option<String> = conn.get(&full_key).await?;
        match value {
            Some(v) => Ok(Some(serde_json::from_str(&v)?)),
            None => Ok(None),
        }
    }

    pub async fn cache_set<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> anyhow::Result<()> {
        let full_key = format!("{CACHE_PREFIX}{key}");
        let json = serde_json::to_string(value)?;
        let mut conn = self.conn();
        let () = conn.set_ex(&full_key, &json, ttl_secs).await?;
        Ok(())
    }

    pub async fn cache_delete(&self, key: &str) -> anyhow::Result<()> {
        let full_key = format!("{CACHE_PREFIX}{key}");
        let mut conn = self.conn();
        let () = conn.del(&full_key).await?;
        Ok(())
    }
}
