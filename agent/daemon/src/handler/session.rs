//! session.* method handlers

use agent_protocol::jsonrpc::*;
use agent_protocol::methods::*;
use agent_protocol::state::*;

use crate::handler::Handler;

/// Handles session lifecycle methods.
pub struct SessionHandler;

#[async_trait::async_trait]
impl Handler for SessionHandler {
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        match req.method.as_str() {
            "session.initialize" => self.handle_initialize(req).await,
            _ => Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
            }),
        }
    }
}

impl SessionHandler {
    async fn handle_initialize(
        &self,
        req: JsonRpcRequest,
    ) -> anyhow::Result<JsonRpcResponse> {
        // Parse params (optional)
        let _params: Option<InitializeParams> = req
            .params
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .unwrap_or_default();

        // Return a hardcoded snapshot for Sprint 2 stub
        let snapshot = SessionState {
            session_id: "sess-stub-001".to_string(),
            alias: "test-session".to_string(),
            server_time: chrono::Utc::now().to_rfc3339(),
            last_event_seq: 0,
            app_state: agent_protocol::state::AppStateSnapshot {
                transcript: vec![],
                input: agent_protocol::state::InputState {
                    text: "".to_string(),
                    multiline: false,
                },
                status: agent_protocol::state::SessionStatus::Idle,
            },
            agents: vec![],
            workplace: agent_protocol::state::WorkplaceSnapshot {
                id: "wp-stub".to_string(),
                path: std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                backlog: agent_protocol::state::BacklogSnapshot { items: vec![] },
                skills: vec![],
            },
            focused_agent_id: None,
            protocol_version: agent_protocol::PROTOCOL_VERSION.to_string(),
        };

        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::to_value(snapshot)?),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_returns_snapshot() {
        let handler = SessionHandler;
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "session.initialize".to_string(),
            params: Some(serde_json::json!({"client_type": "tui"})),
        };

        let resp = handler.handle(req).await.unwrap();
        assert!(resp.result.is_some());
        let snapshot: SessionState = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(snapshot.protocol_version, agent_protocol::PROTOCOL_VERSION);
    }
}
