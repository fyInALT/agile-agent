//! JSON-RPC method router

use agent_protocol::jsonrpc::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::handler::Handler;

/// Shared handle to the router, cloneable across tasks.
#[derive(Clone)]
pub struct RouterHandle {
    inner: Arc<Router>,
}

impl RouterHandle {
    /// Dispatch a request to the appropriate handler.
    pub async fn dispatch(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        if let Some(handler) = self.inner.handlers.get(&req.method) {
            handler.handle(req).await
        } else {
            Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
            }))
        }
    }

    /// Dispatch a notification (fire-and-forget).
    pub async fn dispatch_notification(
        &self,
        notif: JsonRpcNotification,
    ) -> anyhow::Result<()> {
        // Most notifications from clients are no-ops at the router level
        // (e.g., heartbeat). If a notification needs handling, add it here.
        let _ = notif;
        Ok(())
    }
}

/// Builds and owns the method dispatch table.
pub struct Router {
    handlers: HashMap<String, Arc<dyn Handler>>,
}

impl Router {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for a method name.
    pub fn register(&mut self, method: &str, handler: Arc<dyn Handler>) {
        self.handlers.insert(method.to_string(), handler);
    }

    /// Build a cloneable handle.
    pub fn handle(self) -> RouterHandle {
        RouterHandle {
            inner: Arc::new(self),
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoHandler;

    #[async_trait::async_trait]
    impl Handler for EchoHandler {
        async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
            Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: req.params,
            }))
        }
    }

    #[tokio::test]
    async fn registered_method_routes_correctly() {
        let mut router = Router::new();
        router.register("echo", Arc::new(EchoHandler));
        let handle = router.handle();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "echo".to_string(),
            params: Some(serde_json::json!(42)),
        };

        let resp = handle.dispatch(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => assert_eq!(r.result, Some(serde_json::json!(42))),
            _ => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn unregistered_method_returns_empty_result() {
        let router = Router::new();
        let handle = router.handle();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "unknown".to_string(),
            params: None,
        };

        let resp = handle.dispatch(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => assert!(r.result.is_none()),
            _ => panic!("expected response"),
        }
    }
}
