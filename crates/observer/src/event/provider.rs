use std::sync::Arc;

use alloy::providers::{ProviderBuilder, RootProvider};

use openlaunch_shared::client::RpcClient;

use super::error::ObserverError;

/// The concrete provider type returned by `ProviderBuilder::new().connect_http()`.
pub type HttpProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    RootProvider,
>;

/// Create an alloy HTTP provider from the best available RPC provider.
pub fn create_provider(
    rpc: &Arc<RpcClient>,
) -> Result<HttpProvider, ObserverError> {
    let provider_id = rpc
        .best_provider()
        .ok_or_else(|| ObserverError::fatal(anyhow::anyhow!("No RPC providers available")))?;

    let state = rpc
        .get_provider(&provider_id)
        .ok_or_else(|| {
            ObserverError::fatal(anyhow::anyhow!("Provider {provider_id:?} not found"))
        })?;

    let provider_url: alloy::transports::http::reqwest::Url = state
        .url
        .parse()
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Invalid provider URL: {e}")))?;

    let provider = ProviderBuilder::new().connect_http(provider_url);
    Ok(provider)
}
