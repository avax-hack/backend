use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::event::core::SubscriptionKey;
use crate::event::EventProducers;
use super::connection::ConnectionState;

/// Incoming JSON-RPC 2.0 request.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

/// Outgoing JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: serde_json::Value,
}

/// Outgoing JSON-RPC 2.0 push notification (no id).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcPush {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError { code, message }),
            id,
        }
    }
}

impl JsonRpcPush {
    pub fn new(method: String, subscription: String, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method,
            params: serde_json::json!({
                "subscription": subscription,
                "result": result,
            }),
        }
    }
}

/// Parse a raw JSON text into a JSON-RPC request.
pub fn parse_request(text: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    serde_json::from_str::<JsonRpcRequest>(text).map_err(|e| {
        JsonRpcResponse::error(
            serde_json::Value::Null,
            -32700,
            format!("Parse error: {e}"),
        )
    })
}

/// Dispatch a parsed JSON-RPC request. Starts subscription forwarding tasks as needed.
///
/// Returns a JSON-RPC response to send back to the client.
pub fn dispatch(
    request: &JsonRpcRequest,
    producers: &Arc<EventProducers>,
    conn: &mut ConnectionState,
    outbound_tx: &mpsc::Sender<String>,
) -> JsonRpcResponse {
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32600,
            "Invalid jsonrpc version, expected 2.0".to_string(),
        );
    }

    match request.method.as_str() {
        "trade_subscribe" => {
            handle_keyed_subscribe(
                request,
                "token_id",
                |id| SubscriptionKey::Trade(id),
                &producers.trade,
                conn,
                outbound_tx,
            )
        }
        "price_subscribe" => {
            handle_keyed_subscribe(
                request,
                "token_id",
                |id| SubscriptionKey::Price(id),
                &producers.price,
                conn,
                outbound_tx,
            )
        }
        "project_subscribe" => {
            handle_keyed_subscribe(
                request,
                "project_id",
                |id| SubscriptionKey::Project(id),
                &producers.project,
                conn,
                outbound_tx,
            )
        }
        "milestone_subscribe" => {
            handle_keyed_subscribe(
                request,
                "project_id",
                |id| SubscriptionKey::Milestone(id),
                &producers.milestone,
                conn,
                outbound_tx,
            )
        }
        "new_content_subscribe" => {
            handle_global_subscribe(request, &producers.new_content, conn, outbound_tx)
        }
        _ => JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

/// Handle a subscription that is keyed by an identifier (token_id or project_id).
fn handle_keyed_subscribe(
    request: &JsonRpcRequest,
    param_name: &str,
    make_key: impl FnOnce(String) -> SubscriptionKey,
    producer: &Arc<dyn crate::event::EventProducer>,
    conn: &mut ConnectionState,
    outbound_tx: &mpsc::Sender<String>,
) -> JsonRpcResponse {
    let id_value = request.params.get(param_name).and_then(|v| v.as_str());

    let Some(raw_id) = id_value else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            format!("Missing required param: {param_name}"),
        );
    };

    let normalized_id = raw_id.to_lowercase();
    let sub_key = make_key(normalized_id);
    let channel_key = sub_key.to_channel_key();
    let method = request.method.clone();

    let mut rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let push = JsonRpcPush::new(method.clone(), channel_key_for_task.clone(), event.data);
                    if let Ok(json) = serde_json::to_string(&push) {
                        if tx.send(json).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(channel = %channel_key_for_task, lagged = n, "Subscriber lagged, terminating subscription");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    conn.subscribe(channel_key, handle);

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"subscribed": true}),
    )
}

/// Handle a global subscription (new_content) that requires no key parameter.
fn handle_global_subscribe(
    request: &JsonRpcRequest,
    producer: &Arc<dyn crate::event::EventProducer>,
    conn: &mut ConnectionState,
    outbound_tx: &mpsc::Sender<String>,
) -> JsonRpcResponse {
    let sub_key = SubscriptionKey::NewContent;
    let channel_key = sub_key.to_channel_key();
    let method = request.method.clone();

    let mut rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let push = JsonRpcPush::new(method.clone(), channel_key_for_task.clone(), event.data);
                    if let Ok(json) = serde_json::to_string(&push) {
                        if tx.send(json).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "new_content subscriber lagged, terminating subscription");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    conn.subscribe(channel_key, handle);

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"subscribed": true}),
    )
}
