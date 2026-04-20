//! Health check handler and basic daemon metrics.

use crate::handler::Handler;
use agent_protocol::jsonrpc::*;
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};

/// Simple counters for observability.
#[derive(Debug, Default)]
pub struct DaemonMetrics {
    pub events_broadcasted: AtomicU64,
    pub messages_handled: AtomicU64,
    pub connections_accepted: AtomicU64,
}

impl DaemonMetrics {
    pub fn inc_events(&self) {
        self.events_broadcasted.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_messages(&self) {
        self.messages_handled.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_connections(&self) {
        self.connections_accepted.fetch_add(1, Ordering::Relaxed);
    }
}

/// JSON-RPC handler for `session.health`.
pub struct HealthHandler {
    metrics: std::sync::Arc<DaemonMetrics>,
}

impl HealthHandler {
    pub fn new(metrics: std::sync::Arc<DaemonMetrics>) -> Self {
        Self { metrics }
    }
}

#[async_trait::async_trait]
impl Handler for HealthHandler {
    async fn handle(&self, req: JsonRpcRequest) -> Result<JsonRpcMessage> {
        let result = serde_json::json!({
            "status": "healthy",
            "events_broadcasted": self.metrics.events_broadcasted.load(Ordering::Relaxed),
            "messages_handled": self.metrics.messages_handled.load(Ordering::Relaxed),
            "connections_accepted": self.metrics.connections_accepted.load(Ordering::Relaxed),
        });
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(result),
            ext: None,
        }))
    }
}

/// JSON-RPC handler for `session.metrics` returning Prometheus text format.
pub struct MetricsHandler {
    metrics: std::sync::Arc<DaemonMetrics>,
}

impl MetricsHandler {
    pub fn new(metrics: std::sync::Arc<DaemonMetrics>) -> Self {
        Self { metrics }
    }
}

#[async_trait::async_trait]
impl Handler for MetricsHandler {
    async fn handle(&self, req: JsonRpcRequest) -> Result<JsonRpcMessage> {
        let text = format!(
            "# HELP agent_events_total Total number of events broadcasted\n\
             # TYPE agent_events_total counter\n\
             agent_events_total {}\n\
             # HELP websocket_connections_active Number of active WebSocket connections\n\
             # TYPE websocket_connections_active gauge\n\
             websocket_connections_active {}\n\
             # HELP messages_handled_total Total number of messages handled\n\
             # TYPE messages_handled_total counter\n\
             messages_handled_total {}\n",
            self.metrics.events_broadcasted.load(Ordering::Relaxed),
            self.metrics.connections_accepted.load(Ordering::Relaxed),
            self.metrics.messages_handled.load(Ordering::Relaxed),
        );
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::Value::String(text)),
            ext: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn health_returns_metrics() {
        let metrics = Arc::new(DaemonMetrics::default());
        metrics.inc_events();
        metrics.inc_connections();

        let handler = HealthHandler::new(metrics);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: agent_protocol::jsonrpc::RequestId::String("1".to_string()),
            method: "session.health".to_string(),
            params: None,
            ext: None,
        };

        let resp = Handler::handle(&handler, req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let obj = r.result.unwrap();
                assert_eq!(obj["status"], "healthy");
                assert_eq!(obj["events_broadcasted"], 1);
                assert_eq!(obj["connections_accepted"], 1);
            }
            other => panic!("expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn metrics_returns_prometheus_text() {
        let metrics = Arc::new(DaemonMetrics::default());
        metrics.inc_events();
        metrics.inc_messages();

        let handler = MetricsHandler::new(metrics);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: agent_protocol::jsonrpc::RequestId::String("1".to_string()),
            method: "session.metrics".to_string(),
            params: None,
            ext: None,
        };

        let resp = Handler::handle(&handler, req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let text = r.result.unwrap().as_str().unwrap().to_string();
                assert!(text.contains("agent_events_total"));
                assert!(text.contains("websocket_connections_active"));
                assert!(text.contains("messages_handled_total"));
                assert!(text.contains("# TYPE agent_events_total counter"));
            }
            other => panic!("expected Response, got {:?}", other),
        }
    }
}
