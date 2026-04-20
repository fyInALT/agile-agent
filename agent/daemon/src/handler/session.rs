//! session.* method handlers

use agent_protocol::jsonrpc::*;
use agent_protocol::methods::*;

use crate::handler::Handler;
use crate::session_mgr::SessionManager;
use std::sync::Arc;

/// Handles session lifecycle methods.
pub struct SessionHandler {
    session_mgr: Arc<SessionManager>,
}

impl SessionHandler {
    pub fn new(session_mgr: Arc<SessionManager>) -> Self {
        Self { session_mgr }
    }
}

#[async_trait::async_trait]
impl Handler for SessionHandler {
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        match req.method.as_str() {
            "session.initialize" => self.handle_initialize(req).await,
            _ => Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
            })),
        }
    }
}

impl SessionHandler {
    async fn handle_initialize(
        &self,
        req: JsonRpcRequest,
    ) -> anyhow::Result<JsonRpcMessage> {
        // Parse params (optional)
        let _params: Option<InitializeParams> = req
            .params
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .unwrap_or_default();

        // Return a live snapshot from SessionManager
        let snapshot = self.session_mgr.snapshot().await?;

        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::to_value(snapshot)?),
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::state::SessionState;
    use agent_types::WorkplaceId;

    #[tokio::test]
    async fn initialize_returns_live_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-1"))
                .await
                .unwrap(),
        );
        let handler = SessionHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "session.initialize".to_string(),
            params: Some(serde_json::json!({"client_type": "tui"})),
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let snapshot: SessionState = serde_json::from_value(r.result.unwrap()).unwrap();
                assert_eq!(snapshot.protocol_version, agent_protocol::PROTOCOL_VERSION);
                assert_eq!(snapshot.workplace.id, "wp-1");
            }
            _ => panic!("expected response"),
        }
    }
}
