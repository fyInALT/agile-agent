//! Per-connection state machine

use agent_protocol::jsonrpc::*;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use crate::router::RouterHandle;

/// Unique identifier for a connection.
pub type ConnectionId = String;

/// Heartbeat timeout: close connection after 120s of silence.
const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// State of a single client connection.
pub struct Connection {
    pub id: ConnectionId,
    pub addr: std::net::SocketAddr,
    pub state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Initialized,
    Closing,
}

impl Connection {
    /// Spawn a new task to handle this TCP connection.
    pub fn spawn(
        stream: TcpStream,
        addr: std::net::SocketAddr,
        router: RouterHandle,
    ) -> ConnectionId {
        let id = format!("conn-{}", uuid::Uuid::new_v4());
        let id_clone = id.clone();

        tokio::spawn(async move {
            let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    tracing::warn!("WebSocket upgrade failed for {}: {}", addr, e);
                    return;
                }
            };

            let (mut write, mut read) = ws_stream.split();
            let mut conn = Connection {
                id: id_clone.clone(),
                addr,
                state: ConnectionState::Connected,
            };

            loop {
                // Wait for next message with heartbeat timeout.
                let next_msg = tokio::time::timeout(HEARTBEAT_TIMEOUT, read.next()).await;

                match next_msg {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        match conn.handle_message(&text, &router).await {
                            Ok(Some(response)) => {
                                let json = match serde_json::to_string(&response) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        tracing::error!("Failed to serialize response: {}", e);
                                        continue;
                                    }
                                };
                                if let Err(e) = write.send(Message::Text(json)).await {
                                    tracing::warn!("Send error on {}: {}", conn.id, e);
                                    break;
                                }
                            }
                            Ok(None) => {
                                // Notification — no response
                            }
                            Err(e) => {
                                tracing::warn!("Message handling error on {}: {}", conn.id, e);
                                // Send error response if we can parse the ID
                                if let Ok(msg) = serde_json::from_str::<JsonRpcRequest>(&text) {
                                    let err = JsonRpcErrorResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: msg.id,
                                        error: JsonRpcError {
                                            code: -32603,
                                            message: format!("Internal error: {}", e),
                                            data: None,
                                        },
                                    };
                                    let _ = write
                                        .send(Message::Text(
                                            serde_json::to_string(&err).unwrap(),
                                        ))
                                        .await;
                                }
                            }
                        }
                    }
                    Ok(Some(Ok(Message::Ping(_)))) |
                    Ok(Some(Ok(Message::Pong(_)))) |
                    Ok(Some(Ok(Message::Frame(_)))) => {
                        // Ignore control frames — they count as activity
                        continue;
                    }
                    Ok(Some(Ok(Message::Close(_)))) => {
                        tracing::debug!("Client {} sent close frame", conn.id);
                        break;
                    }
                    Ok(Some(Ok(Message::Binary(_)))) => {
                        // Reject binary frames per spec
                        let _ = write
                            .send(Message::Close(Some(
                                tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Unsupported,
                                    reason: "Binary frames not supported".into(),
                                },
                            )))
                            .await;
                        break;
                    }
                    Ok(Some(Err(e))) => {
                        tracing::warn!("WebSocket error on {}: {}", conn.id, e);
                        break;
                    }
                    Ok(None) => {
                        // Stream closed
                        break;
                    }
                    Err(_) => {
                        // Heartbeat timeout
                        tracing::warn!(
                            "Connection {} timed out after {:?} of inactivity",
                            conn.id,
                            HEARTBEAT_TIMEOUT
                        );
                        let _ = write
                            .send(Message::Close(Some(
                                tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Away,
                                    reason: "Heartbeat timeout".into(),
                                },
                            )))
                            .await;
                        break;
                    }
                }
            }

            conn.state = ConnectionState::Closing;
            tracing::debug!("Connection {} closed", conn.id);
        });

        id
    }

    /// Handle a single incoming text message.
    async fn handle_message(
        &mut self,
        text: &str,
        router: &RouterHandle,
    ) -> anyhow::Result<Option<JsonRpcMessage>> {
        let msg: JsonRpcMessage = serde_json::from_str(text)?;

        match msg {
            JsonRpcMessage::Request(req) => {
                // Initialization gate
                if self.state == ConnectionState::Connected
                    && req.method != "session.initialize"
                {
                    return Ok(Some(JsonRpcMessage::Error(JsonRpcErrorResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        error: JsonRpcError {
                            code: -32100,
                            message: "Session not initialized".to_string(),
                            data: None,
                        },
                    })));
                }

                let response = router.dispatch(req).await?;

                if self.state == ConnectionState::Connected && response.result.is_some() {
                    self.state = ConnectionState::Initialized;
                }

                Ok(Some(JsonRpcMessage::Response(response)))
            }
            JsonRpcMessage::Notification(notif) => {
                router.dispatch_notification(notif).await?;
                Ok(None)
            }
            _ => Err(anyhow::anyhow!("Invalid message direction from client")),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_state_transitions() {
        let mut conn = Connection {
            id: "test".to_string(),
            addr: "127.0.0.1:12345".parse().unwrap(),
            state: ConnectionState::Connected,
        };
        assert_eq!(conn.state, ConnectionState::Connected);
        conn.state = ConnectionState::Initialized;
        assert_eq!(conn.state, ConnectionState::Initialized);
    }
}
