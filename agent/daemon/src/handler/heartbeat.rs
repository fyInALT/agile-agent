//! Heartbeat handler — updates connection last-seen timestamp.

use agent_protocol::jsonrpc::*;
use async_trait::async_trait;

use super::Handler;

/// Handler for `session.heartbeat` — no-op at the router level.
///
/// The per-connection timeout tracking is handled by [`Connection`](crate::connection::Connection).
pub struct HeartbeatHandler;

#[async_trait]
impl Handler for HeartbeatHandler {
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        // Heartbeat is a no-op for routing; respond with empty success.
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::json!({})),
    ext: None,
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn heartbeat_returns_empty_success() {
        let handler = HeartbeatHandler;
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("hb-1".to_string()),
            method: "session.heartbeat".to_string(),
            params: None,
    ext: None,
        };
        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                assert!(r.result.is_some());
                assert_eq!(r.result.unwrap(), serde_json::json!({}));
            }
            _ => panic!("expected response"),
        }
    }
}
