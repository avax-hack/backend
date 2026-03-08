pub mod execute;
pub mod stream;

use std::sync::Arc;

use openlaunch_shared::client::RpcClient;
use openlaunch_shared::db::postgres::PostgresDatabase;

use crate::keystore::Wallets;
use crate::metrics::TxBotMetrics;

/// A task representing a token whose LP fees should be collected.
#[derive(Debug, Clone)]
pub struct CollectTask {
    pub token_address: String,
}

/// Spawn both the collect-fees stream (DB poller) and executor (TX sender).
///
/// Returns a pair of `JoinHandle`s for the stream and execute tasks.
pub fn spawn(
    rpc: Arc<RpcClient>,
    db: Arc<PostgresDatabase>,
    wallets: Wallets,
    metrics: Arc<TxBotMetrics>,
) -> (tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = tokio::sync::mpsc::channel::<CollectTask>(256);

    let stream_rpc = Arc::clone(&rpc);
    let stream_handle = tokio::spawn(async move {
        if let Err(err) = stream::run(stream_rpc, db, tx).await {
            tracing::error!(%err, "Collect stream exited with error");
        }
    });

    let exec_handle = tokio::spawn(async move {
        if let Err(err) = execute::run(rpc, wallets, rx, metrics).await {
            tracing::error!(%err, "Collect executor exited with error");
        }
    });

    (stream_handle, exec_handle)
}
