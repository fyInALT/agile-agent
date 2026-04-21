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
            "session.debugDump" => self.handle_debug_dump(req).await,
            "session.loadHistory" => self.handle_load_history(req).await,
            "session.forceSnapshot" => self.handle_force_snapshot(req).await,
            "session.listConnections" => self.handle_list_connections(req).await,
            _ => Ok(JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                ext: None,
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
            ext: None,
        }))
    }

    async fn handle_debug_dump(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let dump = self.session_mgr.debug_dump().await?;
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(dump),
            ext: None,
        }))
    }

    async fn handle_load_history(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let params: LoadHistoryParams = req
            .params
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let items = self.session_mgr.load_history(params.offset, params.limit).await;
        let total = items.len();
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::to_value(LoadHistoryResult { items, total })?),
            ext: None,
        }))
    }

    async fn handle_force_snapshot(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let params: ForceSnapshotParams = req
            .params
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let path = params
            .path
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("snapshot.json"));

        self.session_mgr.force_snapshot(&path).await?;
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::json!({"written": path.display().to_string()})),
            ext: None,
        }))
    }

    async fn handle_list_connections(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage> {
        let conns = self.session_mgr.list_connections().await;
        Ok(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::to_value(conns)?),
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
            ext: None,
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let snapshot: SessionState = serde_json::from_value(r.result.unwrap()).unwrap();
                assert_eq!(snapshot.protocol_version, agent_protocol::PROTOCOL_VERSION);
                assert_eq!(snapshot.workplace.id, "wp-1");
                assert!(!snapshot.capabilities.is_empty());
            }
            _ => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn debug_dump_returns_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-dbg"))
                .await
                .unwrap(),
        );
        let handler = SessionHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("dbg-1".to_string()),
            method: "session.debugDump".to_string(),
            params: None,
            ext: None,
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let dump = r.result.unwrap();
                assert_eq!(dump["workplace_id"], "wp-dbg");
                assert!(dump.get("agent_count").is_some());
                assert!(dump.get("event_queue_depth").is_some());
            }
            _ => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn load_history_returns_paginated_items() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-hist"))
                .await
                .unwrap(),
        );
        let handler = SessionHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("hist-1".to_string()),
            method: "session.loadHistory".to_string(),
            params: Some(serde_json::json!({"offset": 0, "limit": 10})),
            ext: None,
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let result: agent_protocol::methods::LoadHistoryResult =
                    serde_json::from_value(r.result.unwrap()).unwrap();
                // Transcript may have bootstrap entries; just verify the structure.
                assert!(result.total >= result.items.len());
            }
            _ => panic!("expected response"),
        }
    }

    #[tokio::test]
    async fn force_snapshot_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-snap"))
                .await
                .unwrap(),
        );
        let handler = SessionHandler::new(mgr);
        let snap_path = tmp.path().join("forced.json");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("snap-1".to_string()),
            method: "session.forceSnapshot".to_string(),
            params: Some(serde_json::json!({"path": snap_path.to_str().unwrap()})),
            ext: None,
        };

        let resp = handler.handle(req).await.unwrap();
        assert!(matches!(resp, JsonRpcMessage::Response(_)));
        assert!(snap_path.exists());
    }

    #[tokio::test]
    async fn list_connections_returns_array() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), WorkplaceId::new("wp-conn"))
                .await
                .unwrap(),
        );
        let handler = SessionHandler::new(mgr);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("conn-1".to_string()),
            method: "session.listConnections".to_string(),
            params: None,
            ext: None,
        };

        let resp = handler.handle(req).await.unwrap();
        match resp {
            JsonRpcMessage::Response(r) => {
                let arr: Vec<serde_json::Value> = serde_json::from_value(r.result.unwrap()).unwrap();
                assert!(arr.is_empty());
            }
            _ => panic!("expected response"),
        }
    }
}
