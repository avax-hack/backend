use alloy::signers::local::PrivateKeySigner;
use anyhow::Context;

/// Holds wallet signers for transaction submission.
/// Fields are optional to allow graceful startup in dev environments
/// where private keys may not be configured.
#[derive(Clone)]
pub struct Wallets {
    pub graduate: Option<PrivateKeySigner>,
    pub collector: Option<PrivateKeySigner>,
}

impl Wallets {
    pub fn graduate_signer(&self) -> anyhow::Result<&PrivateKeySigner> {
        self.graduate
            .as_ref()
            .context("GRADUATE_PRIVATE_KEY not configured")
    }

    pub fn collector_signer(&self) -> anyhow::Result<&PrivateKeySigner> {
        self.collector
            .as_ref()
            .context("COLLECTOR_PRIVATE_KEY not configured")
    }
}

/// Parse a hex-encoded private key into a `PrivateKeySigner`.
/// Accepts keys with or without the "0x" prefix.
fn parse_private_key(hex_key: &str) -> anyhow::Result<PrivateKeySigner> {
    let trimmed = hex_key.trim();
    let signer: PrivateKeySigner = trimmed
        .parse()
        .context("Failed to parse private key")?;
    Ok(signer)
}

/// Load wallet signers from environment variables.
/// Missing keys are logged as warnings but do not cause failure,
/// enabling partial operation (e.g., only graduate or only collect).
pub fn load_wallets_from_env() -> Wallets {
    let graduate = match std::env::var("GRADUATE_PRIVATE_KEY") {
        Ok(key) if !key.trim().is_empty() => match parse_private_key(&key) {
            Ok(signer) => {
                tracing::info!(
                    address = %signer.address(),
                    "Graduate wallet loaded"
                );
                Some(signer)
            }
            Err(err) => {
                tracing::error!(%err, "Failed to parse GRADUATE_PRIVATE_KEY");
                None
            }
        },
        _ => {
            tracing::warn!("GRADUATE_PRIVATE_KEY not set, graduate job will be disabled");
            None
        }
    };

    let collector = match std::env::var("COLLECTOR_PRIVATE_KEY") {
        Ok(key) if !key.trim().is_empty() => match parse_private_key(&key) {
            Ok(signer) => {
                tracing::info!(
                    address = %signer.address(),
                    "Collector wallet loaded"
                );
                Some(signer)
            }
            Err(err) => {
                tracing::error!(%err, "Failed to parse COLLECTOR_PRIVATE_KEY");
                None
            }
        },
        _ => {
            tracing::warn!("COLLECTOR_PRIVATE_KEY not set, collect job will be disabled");
            None
        }
    };

    Wallets {
        graduate,
        collector,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_private_key_with_prefix() {
        // A well-known test private key (do NOT use in production)
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let signer = parse_private_key(key).unwrap();
        assert!(!signer.address().is_zero());
    }

    #[test]
    fn test_parse_private_key_without_prefix() {
        let key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let signer = parse_private_key(key).unwrap();
        assert!(!signer.address().is_zero());
    }

    #[test]
    fn test_parse_private_key_invalid() {
        let result = parse_private_key("not-a-valid-key");
        assert!(result.is_err());
    }
}
