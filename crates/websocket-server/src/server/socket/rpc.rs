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
        "chart_subscribe" => {
            handle_chart_subscribe(request, &producers.chart, conn, outbound_tx)
        }
        "new_content_subscribe" => {
            handle_global_subscribe(request, &producers.new_content, conn, outbound_tx)
        }
        "trade_unsubscribe" => {
            handle_keyed_unsubscribe(request, "token_id", |id| SubscriptionKey::Trade(id), conn)
        }
        "price_unsubscribe" => {
            handle_keyed_unsubscribe(request, "token_id", |id| SubscriptionKey::Price(id), conn)
        }
        "project_unsubscribe" => {
            handle_keyed_unsubscribe(request, "project_id", |id| SubscriptionKey::Project(id), conn)
        }
        "milestone_unsubscribe" => {
            handle_keyed_unsubscribe(request, "project_id", |id| SubscriptionKey::Milestone(id), conn)
        }
        "chart_unsubscribe" => {
            handle_chart_unsubscribe(request, conn)
        }
        "new_content_unsubscribe" => {
            let channel_key = SubscriptionKey::NewContent.to_channel_key();
            let removed = conn.unsubscribe(&channel_key);
            JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({"unsubscribed": removed}),
            )
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

    let rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = spawn_subscription_task(rx, tx, method, channel_key_for_task);

    if !conn.subscribe(channel_key, handle) {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32000,
            "Subscription limit reached".to_string(),
        );
    }

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

    let rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = spawn_subscription_task(rx, tx, method, channel_key_for_task);

    if !conn.subscribe(channel_key, handle) {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32000,
            "Subscription limit reached".to_string(),
        );
    }

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"subscribed": true}),
    )
}

/// Handle chart subscription with token_id and resolution parameters.
fn handle_chart_subscribe(
    request: &JsonRpcRequest,
    producer: &Arc<dyn crate::event::EventProducer>,
    conn: &mut ConnectionState,
    outbound_tx: &mpsc::Sender<String>,
) -> JsonRpcResponse {
    let token_id = request.params.get("token_id").and_then(|v| v.as_str());
    let resolution = request.params.get("resolution").and_then(|v| v.as_str());

    let Some(raw_id) = token_id else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Missing required param: token_id".to_string(),
        );
    };

    let interval = resolve_interval(resolution.unwrap_or("1"));

    let Some(interval) = interval else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Invalid resolution. Supported: 1, 5, 15, 60, 240, 1D".to_string(),
        );
    };

    let normalized_id = raw_id.to_lowercase();
    let sub_key = SubscriptionKey::Chart(normalized_id, interval.to_string());
    let channel_key = sub_key.to_channel_key();
    let method = request.method.clone();

    let rx = producer.subscribe(&channel_key);
    let tx = outbound_tx.clone();
    let channel_key_for_task = channel_key.clone();

    let handle = spawn_subscription_task(rx, tx, method, channel_key_for_task);

    if !conn.subscribe(channel_key, handle) {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32000,
            "Subscription limit reached".to_string(),
        );
    }

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"subscribed": true}),
    )
}

/// Handle a keyed unsubscribe request.
fn handle_keyed_unsubscribe(
    request: &JsonRpcRequest,
    param_name: &str,
    make_key: impl FnOnce(String) -> SubscriptionKey,
    conn: &mut ConnectionState,
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
    let removed = conn.unsubscribe(&channel_key);

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"unsubscribed": removed}),
    )
}

/// Handle chart unsubscribe with token_id and resolution parameters.
fn handle_chart_unsubscribe(
    request: &JsonRpcRequest,
    conn: &mut ConnectionState,
) -> JsonRpcResponse {
    let token_id = request.params.get("token_id").and_then(|v| v.as_str());
    let resolution = request.params.get("resolution").and_then(|v| v.as_str());

    let Some(raw_id) = token_id else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Missing required param: token_id".to_string(),
        );
    };

    let interval = resolve_interval(resolution.unwrap_or("1"));
    let Some(interval) = interval else {
        return JsonRpcResponse::error(
            request.id.clone(),
            -32602,
            "Invalid resolution. Supported: 1, 5, 15, 60, 240, 1D".to_string(),
        );
    };

    let normalized_id = raw_id.to_lowercase();
    let sub_key = SubscriptionKey::Chart(normalized_id, interval.to_string());
    let channel_key = sub_key.to_channel_key();
    let removed = conn.unsubscribe(&channel_key);

    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({"unsubscribed": removed}),
    )
}

/// Spawn a subscription forwarding task that sends events to the client.
/// On lag, notifies the client with an error push before terminating.
fn spawn_subscription_task(
    mut rx: tokio::sync::broadcast::Receiver<crate::event::core::WsEvent>,
    tx: mpsc::Sender<String>,
    method: String,
    channel_key: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let push = JsonRpcPush::new(method.clone(), channel_key.clone(), event.data);
                    if let Ok(json) = serde_json::to_string(&push) {
                        if tx.send(json).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(channel = %channel_key, lagged = n, "Subscriber lagged, terminating subscription");
                    let error_push = JsonRpcPush::new(
                        method.clone(),
                        channel_key.clone(),
                        serde_json::json!({
                            "type": "SUBSCRIPTION_ERROR",
                            "error": "lagged",
                            "missed": n,
                            "message": "Subscription terminated due to slow consumption",
                        }),
                    );
                    if let Ok(json) = serde_json::to_string(&error_push) {
                        let _ = tx.send(json).await;
                    }
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    })
}

/// Map TradingView-style resolution strings to interval names.
/// Validated against `CandleManager::INTERVALS` to stay in sync.
fn resolve_interval(resolution: &str) -> Option<&'static str> {
    let label = match resolution {
        "1" | "1m" => "1m",
        "5" | "5m" => "5m",
        "15" | "15m" => "15m",
        "60" | "1h" => "1h",
        "240" | "4h" => "4h",
        "1D" | "1d" => "1d",
        _ => return None,
    };
    // Verify the label exists in CandleManager to catch desync at runtime.
    if crate::candle::INTERVALS.iter().any(|(l, _)| *l == label) {
        Some(label)
    } else {
        None
    }
}
