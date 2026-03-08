use std::future::Future;
use std::time::Duration;

/// Configuration for retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_factor: f64,
}

impl RetryConfig {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            initial_backoff_ms: 1000,
            max_backoff_ms: 60_000,
            backoff_factor: 2.0,
        }
    }

    pub fn with_backoff(self, initial_ms: u64, max_ms: u64, factor: f64) -> Self {
        Self {
            initial_backoff_ms: initial_ms,
            max_backoff_ms: max_ms,
            backoff_factor: factor,
            ..self
        }
    }

    fn backoff_duration(&self, attempt: u32) -> Duration {
        let backoff_ms = (self.initial_backoff_ms as f64)
            * self.backoff_factor.powi(attempt.saturating_sub(1) as i32);
        let capped_ms = backoff_ms.min(self.max_backoff_ms as f64) as u64;
        Duration::from_millis(capped_ms)
    }
}

/// Check whether an error is worth retrying.
///
/// Returns `false` for errors that indicate a permanent failure (e.g. invalid
/// input, already-processed state, or on-chain revert). Everything else is
/// assumed to be transient and therefore retriable.
fn is_retriable(error: &anyhow::Error) -> bool {
    let msg = error.to_string().to_lowercase();

    let non_retriable_patterns = [
        "parse",
        "invalid",
        "already graduated",
        "not active",
        "reverted",
    ];

    !non_retriable_patterns.iter().any(|p| msg.contains(p))
}

/// Execute an async task with exponential backoff retries.
///
/// The closure `f` is called on each attempt. If it returns `Ok(T)`, the result
/// is returned immediately. On `Err`, the error is logged and the next attempt
/// is scheduled after a backoff delay — unless the error is non-retriable, in
/// which case it is returned immediately.
///
/// Returns the final error if all attempts are exhausted.
pub async fn run_with_retry<F, Fut, T>(
    config: &RetryConfig,
    task_name: &str,
    f: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=config.max_attempts {
        tracing::info!(
            task = task_name,
            attempt,
            max_attempts = config.max_attempts,
            "Executing task attempt"
        );

        match f().await {
            Ok(result) => {
                tracing::info!(
                    task = task_name,
                    attempt,
                    "Task completed successfully"
                );
                return Ok(result);
            }
            Err(err) => {
                tracing::warn!(
                    task = task_name,
                    attempt,
                    max_attempts = config.max_attempts,
                    error = %err,
                    "Task attempt failed"
                );

                if !is_retriable(&err) {
                    tracing::warn!(
                        task = task_name,
                        error = %err,
                        "Error is non-retriable, aborting retries"
                    );
                    return Err(err);
                }

                last_error = Some(err);

                if attempt < config.max_attempts {
                    let backoff = config.backoff_duration(attempt);
                    tracing::info!(
                        task = task_name,
                        backoff_ms = backoff.as_millis() as u64,
                        "Waiting before next retry"
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("Task '{task_name}' failed after {} attempts", config.max_attempts)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_run_with_retry_succeeds_first_try() {
        let config = RetryConfig::new(3).with_backoff(10, 100, 2.0);
        let result = run_with_retry(&config, "test_ok", || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_run_with_retry_succeeds_after_failures() {
        let attempts = AtomicU32::new(0);
        let config = RetryConfig::new(3).with_backoff(10, 100, 2.0);
        let result = run_with_retry(&config, "test_retry", || {
            let count = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(anyhow::anyhow!("fail"))
                } else {
                    Ok("success")
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_run_with_retry_exhausts_attempts() {
        let config = RetryConfig::new(2).with_backoff(10, 100, 2.0);
        let result: anyhow::Result<()> = run_with_retry(&config, "test_fail", || async {
            Err(anyhow::anyhow!("always fails"))
        })
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_is_retriable_transient_errors() {
        assert!(is_retriable(&anyhow::anyhow!("connection timeout")));
        assert!(is_retriable(&anyhow::anyhow!("network error")));
        assert!(is_retriable(&anyhow::anyhow!("server unavailable")));
    }

    #[test]
    fn test_is_retriable_non_retriable_errors() {
        assert!(!is_retriable(&anyhow::anyhow!("failed to parse address")));
        assert!(!is_retriable(&anyhow::anyhow!("invalid token address")));
        assert!(!is_retriable(&anyhow::anyhow!("token already graduated")));
        assert!(!is_retriable(&anyhow::anyhow!("project not active")));
        assert!(!is_retriable(&anyhow::anyhow!("transaction reverted")));
    }

    #[tokio::test]
    async fn test_run_with_retry_aborts_on_non_retriable_error() {
        let attempts = AtomicU32::new(0);
        let config = RetryConfig::new(5).with_backoff(10, 100, 2.0);
        let result: anyhow::Result<()> = run_with_retry(&config, "test_no_retry", || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err(anyhow::anyhow!("transaction reverted")) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "should not retry non-retriable errors");
    }

    #[test]
    fn test_backoff_duration_capped() {
        let config = RetryConfig::new(10).with_backoff(1000, 5000, 2.0);
        // attempt 1: 1000ms, attempt 2: 2000ms, attempt 3: 4000ms, attempt 4: 5000ms (capped)
        assert_eq!(config.backoff_duration(1).as_millis(), 1000);
        assert_eq!(config.backoff_duration(2).as_millis(), 2000);
        assert_eq!(config.backoff_duration(3).as_millis(), 4000);
        assert_eq!(config.backoff_duration(4).as_millis(), 5000); // capped
        assert_eq!(config.backoff_duration(10).as_millis(), 5000); // still capped
    }
}
