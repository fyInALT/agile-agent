//! WebSocket client for connecting to the agent-daemon.

use agent_protocol::events::Event;
use agent_protocol::jsonrpc::*;
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};


/// Message from the server to the client.
#[derive(Debug, Clone)]
pub enum ServerMessage {
    Response(JsonRpcResponse),
    Error(JsonRpcErrorResponse),
    Notification(Event),
}

/// JSON-RPC 2.0 client over WebSocket.
pub struct WebSocketClient {
    request_tx: mpsc::UnboundedSender<ClientRequest>,
}

struct ClientRequest {
    id: RequestId,
    payload: String,
    respond: oneshot::Sender<anyhow::Result<JsonRpcResponse>>,
}

impl WebSocketClient {
    /// Connect to a daemon WebSocket URL and spawn read/write tasks.
    pub async fn connect(url: &str) -> Result<(Self, mpsc::UnboundedReceiver<ServerMessage>)> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<ClientRequest>();
        let (server_tx, server_rx) = mpsc::unbounded_channel::<ServerMessage>();
        let mut pending: HashMap<RequestId, oneshot::Sender<anyhow::Result<JsonRpcResponse>>> =
            HashMap::new();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(req) = request_rx.recv() => {
                        pending.insert(req.id.clone(), req.respond);
                        if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(req.payload)).await {
                            tracing::warn!("websocket send error: {}", e);
                            break;
                        }
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                // Try Event first (daemon broadcasts raw Event JSON).
                                if let Ok(event) = serde_json::from_str::<Event>(&text) {
                                    let _ = server_tx.send(ServerMessage::Notification(event));
                                    continue;
                                }
                                // Then try JSON-RPC response/error.
                                match serde_json::from_str::<JsonRpcMessage>(&text) {
                                    Ok(JsonRpcMessage::Response(resp)) => {
                                        if let Some(tx) = pending.remove(&resp.id) {
                                            let _ = tx.send(Ok(resp));
                                        }
                                    }
                                    Ok(JsonRpcMessage::Error(err)) => {
                                        if let Some(tx) = pending.remove(&err.id) {
                                            let _ = tx.send(Err(anyhow::anyhow!("{}: {}", err.error.code, err.error.message)));
                                        } else {
                                            let _ = server_tx.send(ServerMessage::Error(err));
                                        }
                                    }
                                    Ok(JsonRpcMessage::Notification(_)) => {
                                        // Server should not send JSON-RPC notifications.
                                    }
                                    Ok(JsonRpcMessage::Request(_)) => {
                                        // Server should not send requests.
                                    }
                                    Err(e) => {
                                        tracing::warn!("failed to parse server message: {}", e);
                                    }
                                }
                            }
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                                tracing::info!("server closed connection");
                                break;
                            }
                            Some(Err(e)) => {
                                tracing::warn!("websocket error: {}", e);
                                break;
                            }
                            None => {
                                tracing::info!("websocket stream ended");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Fail all pending requests.
            for (_, tx) in pending {
                let _ = tx.send(Err(anyhow::anyhow!("connection closed")));
            }
        });

        Ok((Self { request_tx }, server_rx))
    }

    /// Make a JSON-RPC call and await the response.
    pub async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse> {
        let id = RequestId::String(format!("req-{}", uuid::Uuid::new_v4()));
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
    ext: None,
        };
        let payload = serde_json::to_string(&JsonRpcMessage::Request(req))?;

        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ClientRequest {
                id,
                payload,
                respond: tx,
            })
            .map_err(|_| anyhow::anyhow!("client task dropped"))?;

        rx.await
            .map_err(|_| anyhow::anyhow!("response channel closed"))?
    }

    /// Send a JSON-RPC notification (fire-and-forget).
    pub fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
    ext: None,
        };
        let payload = serde_json::to_string(&JsonRpcMessage::Notification(notif))?;
        self.request_tx
            .send(ClientRequest {
                id: RequestId::String(format!("notif-{}", uuid::Uuid::new_v4())),
                payload,
                respond: oneshot::channel().0,
            })
            .map_err(|_| anyhow::anyhow!("client task dropped"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::jsonrpc::{JsonRpcErrorResponse, JsonRpcMessage, JsonRpcResponse};
    use tokio::net::TcpListener;

    /// Spawn a minimal mock daemon that echoes JSON-RPC requests as responses.
    async fn mock_daemon_server(listener: TcpListener) -> String {
        let local_addr = listener.local_addr().unwrap();
        let url = format!("ws://{}", local_addr);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws.split();

            while let Some(Ok(msg)) = read.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    // If it's a JSON-RPC request, echo back a response with same id.
                    if let Ok(JsonRpcMessage::Request(req)) = serde_json::from_str(&text) {
                        let resp = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id,
                            result: Some(serde_json::json!({"echo": req.method})),
    ext: None,
                        };
                        let json = serde_json::to_string(&JsonRpcMessage::Response(resp)).unwrap();
                        let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await;
                    }
                }
            }
        });

        url
    }

    #[tokio::test]
    async fn client_connects_and_correlates_request_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = mock_daemon_server(listener).await;

        let (client, mut server_rx) = WebSocketClient::connect(&url).await.unwrap();

        let resp = client
            .call("session.initialize", Some(serde_json::json!({"alias": "test"})))
            .await
            .unwrap();

        assert!(resp.result.is_some());
        assert_eq!(resp.result.unwrap()["echo"], "session.initialize");

        // No unsolicited server messages should be present.
        assert!(server_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn client_receives_server_events() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let url = format!("ws://{}", local_addr);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, _read) = ws.split();

            // Send an unsolicited event immediately after handshake.
            let event = Event {
                seq: 1,
                payload: agent_protocol::events::EventPayload::AgentSpawned(
                    agent_protocol::events::AgentSpawnedData {
                        agent_id: "a1".to_string(),
                        codename: "alpha".to_string(),
                        role: "Developer".to_string(),
                    },
                ),
            };
            let json = serde_json::to_string(&event).unwrap();
            let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await;
        });

        let (_client, mut server_rx) = WebSocketClient::connect(&url).await.unwrap();

        // Wait for the event to arrive.
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), server_rx.recv()).await;
        assert!(msg.is_ok(), "timed out waiting for event");
        match msg.unwrap() {
            Some(ServerMessage::Notification(ev)) => {
                assert_eq!(ev.seq, 1);
            }
            other => panic!("expected Notification, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn notify_does_not_wait_for_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = mock_daemon_server(listener).await;

        let (client, _server_rx) = WebSocketClient::connect(&url).await.unwrap();

        // notify should return immediately without panicking.
        client
            .notify("session.heartbeat", None)
            .expect("notify should succeed");
    }

    #[tokio::test]
    async fn error_propagation_for_unmatched_request() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let url = format!("ws://{}", local_addr);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, _read) = ws.split();

            // Send a JSON-RPC error with an unknown request ID (no pending request).
            let err = JsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::String("unknown-id".to_string()),
                error: JsonRpcError {
                    code: -32600,
                    message: "Invalid request".to_string(),
                    data: None,
                    ext: None,
                },
                ext: None,
            };
            let json = serde_json::to_string(&JsonRpcMessage::Error(err)).unwrap();
            let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await;
        });

        let (_client, mut server_rx) = WebSocketClient::connect(&url).await.unwrap();

        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), server_rx.recv()).await;
        assert!(msg.is_ok(), "timed out waiting for error");
        match msg.unwrap() {
            Some(ServerMessage::Error(err)) => {
                assert_eq!(err.error.code, -32600);
                assert_eq!(err.error.message, "Invalid request");
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }
}
