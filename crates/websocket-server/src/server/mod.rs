pub mod socket;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
    routing::get,
};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;

use crate::event::EventProducers;
use self::socket::connection::ConnectionState;
use self::socket::rpc;

/// Shared application state for the HTTP/WS server.
#[derive(Clone)]
pub struct AppState {
    pub producers: Arc<EventProducers>,
    pub connection_count: Arc<AtomicUsize>,
    pub max_connections: usize,
}

/// Build the axum router with WebSocket and health endpoints.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route("/health", get(health_handler))
        .with_state(state)
}

/// Health check endpoint.
async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok"}))
}

/// WebSocket upgrade handler.
async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    let current = state.connection_count.load(Ordering::SeqCst);
    if current >= state.max_connections {
        tracing::warn!(
            current_connections = current,
            max = state.max_connections,
            "Connection limit reached, rejecting WebSocket upgrade"
        );
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Too many connections".to_string(),
        ));
    }
    state.connection_count.fetch_add(1, Ordering::SeqCst);
    Ok(ws.on_upgrade(move |socket| handle_ws_connection(socket, state)))
}

/// Main WebSocket connection handler.
/// Reads JSON-RPC messages from the client, dispatches them, and forwards
/// subscription events back through the socket.
async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let mut conn = ConnectionState::new();

    // Channel for outbound messages from subscription tasks -> WS sink.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(256);

    // Bug 13 fix: Use a oneshot channel so the sink task can signal the read
    // loop to exit when the outbound channel closes or a send error occurs.
    let (sink_done_tx, mut sink_done_rx) = tokio::sync::oneshot::channel::<()>();

    // Task to forward outbound messages to the WebSocket sink.
    let sink_task = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            if ws_sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
        // Signal the read loop that the sink is done.
        let _ = sink_done_tx.send(());
    });

    // Read loop: parse and dispatch incoming messages.
    // Bug 13 fix: Also watch for sink task completion to avoid zombie connections.
    loop {
        let msg = tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(m)) => m,
                    _ => break,
                }
            }
            _ = &mut sink_done_rx => {
                tracing::debug!("Sink task exited, closing read loop");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => continue,
            _ => continue,
        };

        // Bug 5 fix: Reject oversized messages before parsing.
        const MAX_MESSAGE_SIZE: usize = 16_384; // 16 KB
        if text.len() > MAX_MESSAGE_SIZE {
            let response = rpc::JsonRpcResponse::error(
                serde_json::Value::Null,
                -32600,
                format!("Message too large: {} bytes (max {MAX_MESSAGE_SIZE})", text.len()),
            );
            if let Ok(json) = serde_json::to_string(&response) {
                if outbound_tx.send(json).await.is_err() {
                    break;
                }
            }
            continue;
        }

        let response = match rpc::parse_request(&text) {
            Ok(request) => rpc::dispatch(&request, &state.producers, &mut conn, &outbound_tx),
            Err(err_response) => err_response,
        };

        if let Ok(json) = serde_json::to_string(&response) {
            if outbound_tx.send(json).await.is_err() {
                break;
            }
        }
    }

    // Client disconnected: clean up all subscriptions.
    conn.cleanup_all();
    drop(outbound_tx);

    // Wait for the sink task to finish.
    let _ = sink_task.await;

    // Bug 6 fix: Decrement connection counter on disconnect.
    state.connection_count.fetch_sub(1, Ordering::SeqCst);

    tracing::debug!("WebSocket connection closed");
}
