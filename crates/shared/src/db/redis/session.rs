use redis::AsyncCommands;

use super::RedisDatabase;
use crate::types::auth::SessionInfo;

const SESSION_PREFIX: &str = "session:";
const NONCE_PREFIX: &str = "nonce:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_prefix() {
        assert_eq!(SESSION_PREFIX, "session:");
    }

    #[test]
    fn test_nonce_prefix() {
        assert_eq!(NONCE_PREFIX, "nonce:");
    }

    #[test]
    fn test_session_key_format() {
        let session_id = "abc123";
        let key = format!("{SESSION_PREFIX}{session_id}");
        assert_eq!(key, "session:abc123");
    }

    #[test]
    fn test_nonce_key_format() {
        let address = "0xabc";
        let key = format!("{NONCE_PREFIX}{address}");
        assert_eq!(key, "nonce:0xabc");
    }
}

impl RedisDatabase {
    pub async fn set_session(&self, session_id: &str, info: &SessionInfo, ttl_secs: u64) -> anyhow::Result<()> {
        let key = format!("{SESSION_PREFIX}{session_id}");
        let value = serde_json::to_string(info)?;
        let mut conn = self.conn();
        let () = conn.set_ex(&key, &value, ttl_secs).await?;
        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<SessionInfo>> {
        let key = format!("{SESSION_PREFIX}{session_id}");
        let mut conn = self.conn();
        let value: Option<String> = conn.get(&key).await?;
        match value {
            Some(v) => Ok(Some(serde_json::from_str(&v)?)),
            None => Ok(None),
        }
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let key = format!("{SESSION_PREFIX}{session_id}");
        let mut conn = self.conn();
        let () = conn.del(&key).await?;
        Ok(())
    }

    pub async fn set_nonce(&self, address: &str, nonce: &str, ttl_secs: u64) -> anyhow::Result<()> {
        let key = format!("{NONCE_PREFIX}{address}");
        let mut conn = self.conn();
        let () = conn.set_ex(&key, nonce, ttl_secs).await?;
        Ok(())
    }

    /// Atomically get and delete a nonce (prevents replay attacks).
    pub async fn get_and_delete_nonce(&self, address: &str) -> anyhow::Result<Option<String>> {
        let key = format!("{NONCE_PREFIX}{address}");
        let mut conn = self.conn();
        let value: Option<String> = redis::cmd("GETDEL").arg(&key).query_async(&mut conn).await?;
        Ok(value)
    }
}
