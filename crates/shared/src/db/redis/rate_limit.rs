use redis::AsyncCommands;

use super::RedisDatabase;

const RATE_PREFIX: &str = "rate:";
const WINDOW_SECS: u64 = 60;

pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u64,
    pub retry_after: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_result_allowed() {
        let result = RateLimitResult {
            allowed: true,
            remaining: 9,
            retry_after: 0,
        };
        assert!(result.allowed);
        assert_eq!(result.remaining, 9);
        assert_eq!(result.retry_after, 0);
    }

    #[test]
    fn test_rate_limit_result_denied() {
        let result = RateLimitResult {
            allowed: false,
            remaining: 0,
            retry_after: 45,
        };
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
        assert_eq!(result.retry_after, 45);
    }

    #[test]
    fn test_rate_prefix_constant() {
        assert_eq!(RATE_PREFIX, "rate:");
    }

    #[test]
    fn test_window_secs_constant() {
        assert_eq!(WINDOW_SECS, 60);
    }
}

impl RedisDatabase {
    pub async fn check_rate_limit(
        &self,
        identifier: &str,
        max_requests: u64,
    ) -> anyhow::Result<RateLimitResult> {
        let now = chrono::Utc::now().timestamp() as u64;
        let window = now / WINDOW_SECS;
        let key = format!("{RATE_PREFIX}{identifier}:{window}");

        let mut conn = self.conn();
        let count: u64 = conn.incr(&key, 1u64).await?;

        if count == 1 {
            let () = conn.expire(&key, (WINDOW_SECS * 2) as i64).await?;
        }

        if count > max_requests {
            let retry_after = WINDOW_SECS - (now % WINDOW_SECS);
            Ok(RateLimitResult {
                allowed: false,
                remaining: 0,
                retry_after,
            })
        } else {
            Ok(RateLimitResult {
                allowed: true,
                remaining: max_requests - count,
                retry_after: 0,
            })
        }
    }
}
