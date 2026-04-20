//! High-level protocol client for TUI → daemon communication.
//!
//! Encapsulates auto-link, WebSocket connection, session initialization,
//! and event application into a single handle.

use agent_protocol::client::auto_link::auto_link;
use agent_protocol::events::Event;
use agent_protocol::jsonrpc::JsonRpcResponse;
use agent_protocol::WorkplaceId;
use anyhow::Result;
use std::path::Path;
use std::time::Duration;

use crate::event_handler::apply_event;
use crate::ui_state::ProtocolState;
use crate::websocket_client::{ServerMessage, WebSocketClient};

/// Handle to the daemon via the protocol.
///
/// Owned by the TUI event loop; drives `ProtocolState` updates from the
/// daemon event stream.
pub struct ProtocolClient {
    ws: WebSocketClient,
    state: ProtocolState,
}

impl ProtocolClient {
    /// Discover (or spawn) a daemon for the given workplace, connect, and initialize a session.
    pub async fn connect(
        workplace_id: &WorkplaceId,
        daemon_json_path: &Path,
    ) -> Result<Option<(Self, tokio::sync::mpsc::UnboundedReceiver<ServerMessage>)>> {
        let result = match auto_link(
            workplace_id,
            daemon_json_path,
            None,
            Duration::from_secs(10),
        )
        .await?
        {
            result => result,
        };

        let (ws, server_rx) = WebSocketClient::connect(&result.websocket_url).await?;

        let init_resp = ws
            .call(
                "session.initialize",
                Some(serde_json::json!({
                    "workplace_id": workplace_id.as_str(),
                })),
            )
            .await?;

        let mut state = ProtocolState::default();
        state.connection_state = crate::ui_state::ConnectionState::Connected;

        // Deserialize snapshot into state if present.
        if let Some(result_val) = init_resp.result {
            if let Ok(snapshot) = serde_json::from_value::<agent_protocol::state::SessionState>(result_val) {
                state.agents = snapshot.agents;
                state.focused_agent_id = snapshot.focused_agent_id;
                state.transcript_items = snapshot.app_state.transcript;
            }
        }

        Ok(Some((Self { ws, state }, server_rx)))
    }

    /// Mutable access to the protocol-driven state.
    pub fn state(&self) -> &ProtocolState {
        &self.state
    }

    /// Mutable access to the protocol-driven state.
    pub fn state_mut(&mut self) -> &mut ProtocolState {
        &mut self.state
    }

    /// Send user input to the daemon.
    pub async fn send_input(&self, text: &str, target_agent_id: Option<&str>) -> Result<JsonRpcResponse> {
        let params = serde_json::json!({
            "text": text,
            "target_agent_id": target_agent_id,
        });
        self.ws.call("session.sendInput", Some(params)).await
    }

    /// Send a heartbeat.
    pub fn heartbeat(&self) -> Result<()> {
        self.ws.notify("session.heartbeat", None)
    }

    /// Apply a single event to the local state.
    pub fn apply_event(&mut self, event: &Event) {
        apply_event(&mut self.state, event);
    }

    /// Update connection state.
    pub fn set_connection_state(&mut self, cs: crate::ui_state::ConnectionState) {
        self.state.connection_state = cs;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[tokio::test]
    async fn connect_returns_err_when_no_daemon() {
        let tmp = TempDir::new().unwrap();
        let wp_id = WorkplaceId::new("wp-test");
        let daemon_json = tmp.path().join("daemon.json");
        let result = ProtocolClient::connect(&wp_id, &daemon_json).await;
        // auto_link times out because no daemon exists.
        assert!(result.is_err());
    }
}
