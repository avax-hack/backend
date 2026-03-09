pub mod dex;
pub mod ido;
pub mod pool;

/// Convert an HTTP RPC URL to a WebSocket URL.
/// Handles Avalanche-style endpoints where `/rpc` → `/ws`.
pub fn rpc_url_to_ws(url: &str) -> String {
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return url.to_string();
    }
    let ws = url
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    if ws.ends_with("/rpc") {
        ws[..ws.len() - 4].to_string() + "/ws"
    } else {
        ws
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_url_to_ws() {
        assert_eq!(
            rpc_url_to_ws("https://api.avax-test.network/ext/bc/C/rpc"),
            "wss://api.avax-test.network/ext/bc/C/ws"
        );
        assert_eq!(rpc_url_to_ws("https://rpc.example.com"), "wss://rpc.example.com");
        assert_eq!(rpc_url_to_ws("http://localhost:8545"), "ws://localhost:8545");
        assert_eq!(rpc_url_to_ws("wss://already.ws"), "wss://already.ws");
    }
}
