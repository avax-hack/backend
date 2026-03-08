pub mod execute;
pub mod stream;

use std::sync::Arc;

use openlaunch_shared::client::RpcClient;

use crate::keystore::Wallets;
use crate::metrics::TxBotMetrics;

/// A task representing a token that should be graduated.
#[derive(Debug, Clone)]
pub struct GraduateTask {
    pub token_address: String,
}

/// Spawn both the graduate stream (event watcher) and executor (TX sender).
///
/// Returns a pair of `JoinHandle`s for the stream and execute tasks.
pub fn spawn(
    rpc: Arc<RpcClient>,
    wallets: Wallets,
    metrics: Arc<TxBotMetrics>,
) -> (tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = tokio::sync::mpsc::channel::<GraduateTask>(256);

    let stream_rpc = Arc::clone(&rpc);
    let stream_handle = tokio::spawn(async move {
        if let Err(err) = stream::run(stream_rpc, tx).await {
            tracing::error!(%err, "Graduate stream exited with error");
        }
    });

    let exec_handle = tokio::spawn(async move {
        if let Err(err) = execute::run(rpc, wallets, rx, metrics).await {
            tracing::error!(%err, "Graduate executor exited with error");
        }
    });

    (stream_handle, exec_handle)
}
