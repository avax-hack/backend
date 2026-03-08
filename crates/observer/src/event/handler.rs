use std::future::Future;
use std::time::Duration;

use super::error::ObserverError;

/// Configuration for retry behavior on retriable errors.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff_ms: 500,
            max_backoff_ms: 30_000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    fn backoff_duration(&self, attempt: u32) -> Duration {
        let backoff_ms = (self.initial_backoff_ms as f64)
            * self.backoff_factor.powi(attempt as i32);
        let clamped = backoff_ms.min(self.max_backoff_ms as f64) as u64;
        Duration::from_millis(clamped)
    }
}

/// Runs an async closure with retry logic based on `ObserverError` variants.
///
/// - `Skippable` errors are logged and the function returns `Ok(())`.
/// - `Retriable` errors are retried up to `config.max_attempts` with exponential backoff.
/// - `Fatal` errors are returned immediately.
pub async fn run_event_handler_with_retry<F, Fut>(
    name: &str,
    config: &RetryConfig,
    mut f: F,
) -> Result<(), ObserverError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), ObserverError>>,
{
    let mut attempt: u32 = 0;

    loop {
        match f().await {
            Ok(()) => return Ok(()),
            Err(ObserverError::Skippable(msg)) => {
                tracing::warn!(handler = name, %msg, "Skippable error, continuing");
                return Ok(());
            }
            Err(ObserverError::Fatal(err)) => {
                tracing::error!(handler = name, %err, "Fatal error, stopping handler");
                return Err(ObserverError::Fatal(err));
            }
            Err(ObserverError::Retriable(err)) => {
                attempt += 1;
                if attempt >= config.max_attempts {
                    tracing::error!(
                        handler = name,
                        %err,
                        attempts = attempt,
                        "Max retry attempts reached, giving up"
                    );
                    return Err(ObserverError::Fatal(err));
                }

                let backoff = config.backoff_duration(attempt - 1);
                tracing::warn!(
                    handler = name,
                    %err,
                    attempt,
                    max_attempts = config.max_attempts,
                    backoff_ms = backoff.as_millis() as u64,
                    "Retriable error, backing off"
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── RetryConfig default tests ───────────────────────────────────

    #[test]
    fn retry_config_default_values() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_attempts, 5);
        assert_eq!(cfg.initial_backoff_ms, 500);
        assert_eq!(cfg.max_backoff_ms, 30_000);
        assert!((cfg.backoff_factor - 2.0).abs() < f64::EPSILON);
    }

    // ── Backoff duration tests ──────────────────────────────────────

    #[test]
    fn backoff_duration_attempt_zero() {
        let cfg = RetryConfig {
            initial_backoff_ms: 100,
            backoff_factor: 2.0,
            max_backoff_ms: 10_000,
            ..RetryConfig::default()
        };
        // attempt=0 => 100 * 2^0 = 100ms
        assert_eq!(cfg.backoff_duration(0), Duration::from_millis(100));
    }

    #[test]
    fn backoff_duration_exponential_growth() {
        let cfg = RetryConfig {
            initial_backoff_ms: 100,
            backoff_factor: 2.0,
            max_backoff_ms: 100_000,
            ..RetryConfig::default()
        };
        // attempt=1 => 100 * 2^1 = 200ms
        assert_eq!(cfg.backoff_duration(1), Duration::from_millis(200));
        // attempt=2 => 100 * 2^2 = 400ms
        assert_eq!(cfg.backoff_duration(2), Duration::from_millis(400));
        // attempt=3 => 100 * 2^3 = 800ms
        assert_eq!(cfg.backoff_duration(3), Duration::from_millis(800));
    }

    #[test]
    fn backoff_duration_clamped_to_max() {
        let cfg = RetryConfig {
            initial_backoff_ms: 1000,
            backoff_factor: 10.0,
            max_backoff_ms: 5000,
            ..RetryConfig::default()
        };
        // attempt=2 => 1000 * 10^2 = 100_000, clamped to 5000
        assert_eq!(cfg.backoff_duration(2), Duration::from_millis(5000));
    }

    // ── run_event_handler_with_retry tests ──────────────────────────

    #[tokio::test]
    async fn retry_handler_returns_ok_on_immediate_success() {
        let cfg = RetryConfig::default();
        let result =
            run_event_handler_with_retry("test", &cfg, || async { Ok(()) }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retry_handler_returns_ok_on_skippable_error() {
        let cfg = RetryConfig::default();
        let result = run_event_handler_with_retry("test", &cfg, || async {
            Err(ObserverError::skippable("dup"))
        })
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retry_handler_returns_fatal_immediately() {
        let cfg = RetryConfig::default();
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err(ObserverError::fatal(anyhow::anyhow!("boom"))) }
        })
        .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ObserverError::Fatal(_)));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_handler_retries_then_succeeds() {
        // Use tokio test-util for time control (auto_advance)
        tokio::time::pause();

        let cfg = RetryConfig {
            max_attempts: 5,
            initial_backoff_ms: 10,
            max_backoff_ms: 100,
            backoff_factor: 2.0,
        };
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            let n = call_count.fetch_add(1, Ordering::SeqCst);
            async move {
                if n < 2 {
                    Err(ObserverError::retriable(anyhow::anyhow!("transient")))
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_handler_exhausts_attempts_returns_fatal() {
        tokio::time::pause();

        let cfg = RetryConfig {
            max_attempts: 3,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_factor: 1.0,
        };
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err(ObserverError::retriable(anyhow::anyhow!("always fail"))) }
        })
        .await;
        assert!(result.is_err());
        // Exhausted retries should convert to Fatal
        assert!(matches!(result.unwrap_err(), ObserverError::Fatal(_)));
        // Called max_attempts times (3)
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_handler_with_max_attempts_one_no_retry() {
        let cfg = RetryConfig {
            max_attempts: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_factor: 1.0,
        };
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err(ObserverError::retriable(anyhow::anyhow!("fail"))) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_handler_max_attempts_zero_never_succeeds() {
        // Edge case: max_attempts = 0 means the very first retriable error
        // triggers immediate failure (attempt 1 >= 0), so the closure is
        // called exactly once.
        let cfg = RetryConfig {
            max_attempts: 0,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_factor: 1.0,
        };
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err(ObserverError::retriable(anyhow::anyhow!("fail"))) }
        })
        .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ObserverError::Fatal(_)));
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "With max_attempts=0, closure should still be called once before checking the limit"
        );
    }

    #[tokio::test]
    async fn retry_handler_max_attempts_zero_still_returns_ok_on_success() {
        // Even with max_attempts=0, if the first call succeeds, it returns Ok
        let cfg = RetryConfig {
            max_attempts: 0,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_factor: 1.0,
        };
        let result =
            run_event_handler_with_retry("test", &cfg, || async { Ok(()) }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retry_handler_exact_call_count_for_each_max_attempts() {
        // Verify the closure is called exactly max_attempts times for
        // persistent retriable errors, for several values of max_attempts.
        tokio::time::pause();

        for max in 1..=6u32 {
            let cfg = RetryConfig {
                max_attempts: max,
                initial_backoff_ms: 1,
                max_backoff_ms: 10,
                backoff_factor: 1.0,
            };
            let call_count = AtomicU32::new(0);
            let _result = run_event_handler_with_retry("test", &cfg, || {
                call_count.fetch_add(1, Ordering::SeqCst);
                async { Err(ObserverError::retriable(anyhow::anyhow!("fail"))) }
            })
            .await;
            assert_eq!(
                call_count.load(Ordering::SeqCst),
                max,
                "With max_attempts={max}, closure should be called exactly {max} times"
            );
        }
    }

    #[tokio::test]
    async fn retry_handler_succeeds_on_last_attempt() {
        // Verify that succeeding on the very last attempt still returns Ok
        tokio::time::pause();

        let cfg = RetryConfig {
            max_attempts: 4,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_factor: 1.0,
        };
        let call_count = AtomicU32::new(0);
        let result = run_event_handler_with_retry("test", &cfg, || {
            let n = call_count.fetch_add(1, Ordering::SeqCst);
            async move {
                // Fail on attempts 0, 1, 2; succeed on attempt 3 (the 4th call)
                if n < 3 {
                    Err(ObserverError::retriable(anyhow::anyhow!("transient")))
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert!(result.is_ok(), "Should succeed on the last allowed attempt");
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }
}
