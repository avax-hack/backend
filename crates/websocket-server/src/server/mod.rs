pub mod socket;

use std::sync::Arc;

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
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Main WebSocket connection handler.
/// Reads JSON-RPC messages from the client, dispatches them, and forwards
/// subscription events back through the socket.
async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let mut conn = ConnectionState::new();

    // Channel for outbound messages from subscription tasks -> WS sink.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(256);

    // Task to forward outbound messages to the WebSocket sink.
    let sink_task = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            if ws_sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop: parse and dispatch incoming messages.
    while let Some(Ok(msg)) = ws_stream.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => continue,
            _ => continue,
        };

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

    tracing::debug!("WebSocket connection closed");
}
