//! agent.* method handlers

use agent_protocol::jsonrpc::*;
use agent_protocol::methods::*;

use crate::handler::Handler;
use crate::session_mgr::SessionManager;
use std::sync::Arc;

/// Handles agent lifecycle methods.
pub struct AgentHandler {
    session_mgr: Arc<SessionManager>,
}

impl AgentHandler {
    pub fn new(session_mgr: Arc<SessionManager>) -> Self {
        Self { session_mgr }
    }
}

#[async_trait::async_trait]
impl Handler for AgentHandler {
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        match req.method.as_str() {
            "agent.spawn" => self.handle_spawn(req).await,
            "agent.stop" => self.handle_stop(req).await,
            "agent.list" => self.handle_list(req).await,
            _ => Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
            })),
        }
    }
}

impl AgentHandler {
    async fn handle_spawn(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let params: AgentSpawnParams = req
            .params
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or_else(|| anyhow::anyhow!("missing or invalid params"))?;

        match self.session_mgr.spawn_agent(params.provider).await {
            Ok(snapshot) => Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::to_value(snapshot)?),
            })),
            Err(e) => Ok(JsonRpcMessage::Error(JsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                error: JsonRpcError {
                    code: -32000,
                    message: e.to_string(),
                    data: None,
                },
            })),
        }
    }

    async fn handle_stop(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let params: AgentStopParams = req
            .params
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        match self.session_mgr.stop_agent(&params.agent_id).await {
            Ok(()) => Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::to_value(AgentStopResult {
                    agent_id: params.agent_id,
                    stopped: true,
                })?),
            })),
            Err(e) => Ok(JsonRpcMessage::Error(JsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                error: JsonRpcError {
                    code: -32101,
                    message: e.to_string(),
                    data: None,
                },
            })),
        }
    }

    async fn handle_list(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let params: AgentListParams = req
            .params
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let agents = self.session_mgr.list_agents(params.include_stopped).await;
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::to_value(AgentListResult { agents })?),
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::state::AgentSnapshot;
    use agent_types::WorkplaceId;

    #[tokio::test]
    async fn spawn_returns_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-1"))
                .await
                .unwrap(),
        );
        let handler = AgentHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "agent.spawn".to_string(),
            params: Some(serde_json::json!({"provider": "mock", "role": "developer"})),
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let result: AgentSnapshot = serde_json::from_value(r.result.unwrap()).unwrap();
                assert!(!result.id.is_empty());
            }
            _ => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn stop_unknown_agent_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-1"))
                .await
                .unwrap(),
        );
        let handler = AgentHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "agent.stop".to_string(),
            params: Some(serde_json::json!({"agentId": "nonexistent", "force": false})),
        };

        let resp = handler.handle(req).await.unwrap();
        assert!(
            matches!(resp, JsonRpcMessage::Error(_)),
            "expected error response"
        );
    }

    #[tokio::test]
    async fn list_returns_spawned_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-1"))
                .await
                .unwrap(),
        );
        let handler = AgentHandler::new(mgr.clone());

        // Spawn an agent first.
        mgr.spawn_agent(agent_types::ProviderKind::Mock).await.unwrap();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "agent.list".to_string(),
            params: None,
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let result: AgentListResult = serde_json::from_value(r.result.unwrap()).unwrap();
                assert_eq!(result.agents.len(), 1);
            }
            _ => panic!("expected response"),
        }
    }
}
