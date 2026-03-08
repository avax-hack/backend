/// Errors that can occur during event processing.
/// The variant determines the retry strategy.
#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    /// Event can be safely skipped (e.g. duplicate, already processed).
    #[error("Skippable: {0}")]
    Skippable(String),

    /// Transient error that should be retried (e.g. RPC timeout, network issue).
    #[error("Retriable: {0}")]
    Retriable(#[source] anyhow::Error),

    /// Fatal error that should stop the handler (e.g. DB schema mismatch).
    #[error("Fatal: {0}")]
    Fatal(#[source] anyhow::Error),
}

impl ObserverError {
    pub fn is_skippable(&self) -> bool {
        matches!(self, Self::Skippable(_))
    }

    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::Retriable(_))
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }

    pub fn skippable(msg: impl Into<String>) -> Self {
        Self::Skippable(msg.into())
    }

    pub fn retriable(err: impl Into<anyhow::Error>) -> Self {
        Self::Retriable(err.into())
    }

    pub fn fatal(err: impl Into<anyhow::Error>) -> Self {
        Self::Fatal(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Constructor tests ───────────────────────────────────────────

    #[test]
    fn skippable_constructor_from_str() {
        let err = ObserverError::skippable("duplicate event");
        assert!(matches!(err, ObserverError::Skippable(msg) if msg == "duplicate event"));
    }

    #[test]
    fn skippable_constructor_from_string() {
        let err = ObserverError::skippable(String::from("already processed"));
        assert!(matches!(err, ObserverError::Skippable(msg) if msg == "already processed"));
    }

    #[test]
    fn retriable_constructor() {
        let err = ObserverError::retriable(anyhow::anyhow!("timeout"));
        assert!(matches!(err, ObserverError::Retriable(_)));
    }

    #[test]
    fn fatal_constructor() {
        let err = ObserverError::fatal(anyhow::anyhow!("schema mismatch"));
        assert!(matches!(err, ObserverError::Fatal(_)));
    }

    // ── Categorization tests ────────────────────────────────────────

    #[test]
    fn is_skippable_returns_true_for_skippable() {
        let err = ObserverError::skippable("dup");
        assert!(err.is_skippable());
        assert!(!err.is_retriable());
        assert!(!err.is_fatal());
    }

    #[test]
    fn is_retriable_returns_true_for_retriable() {
        let err = ObserverError::retriable(anyhow::anyhow!("net error"));
        assert!(!err.is_skippable());
        assert!(err.is_retriable());
        assert!(!err.is_fatal());
    }

    #[test]
    fn is_fatal_returns_true_for_fatal() {
        let err = ObserverError::fatal(anyhow::anyhow!("db gone"));
        assert!(!err.is_skippable());
        assert!(!err.is_retriable());
        assert!(err.is_fatal());
    }

    // ── Display / Error formatting tests ────────────────────────────

    #[test]
    fn skippable_display_contains_message() {
        let err = ObserverError::skippable("test msg");
        let display = format!("{err}");
        assert!(display.contains("Skippable"));
        assert!(display.contains("test msg"));
    }

    #[test]
    fn retriable_display_contains_prefix() {
        let err = ObserverError::retriable(anyhow::anyhow!("rpc timeout"));
        let display = format!("{err}");
        assert!(display.contains("Retriable"));
        assert!(display.contains("rpc timeout"));
    }

    #[test]
    fn fatal_display_contains_prefix() {
        let err = ObserverError::fatal(anyhow::anyhow!("broken"));
        let display = format!("{err}");
        assert!(display.contains("Fatal"));
        assert!(display.contains("broken"));
    }

    // ── Debug impl exists ───────────────────────────────────────────

    #[test]
    fn debug_format_does_not_panic() {
        let err = ObserverError::skippable("debug test");
        let _ = format!("{err:?}");
    }
}
