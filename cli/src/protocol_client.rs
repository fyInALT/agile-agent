//! Blocking-friendly protocol client for the CLI.

use agent_protocol::events::Event;
use agent_protocol::jsonrpc::*;
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Message from the server to the client.
#[derive(Debug, Clone)]
pub enum ServerMessage {
    Response(JsonRpcResponse),
    Error(JsonRpcErrorResponse),
    Notification(Event),
}

/// CLI protocol client with blocking request/response and event subscription.
pub struct ProtocolClient {
    request_tx: mpsc::UnboundedSender<ClientRequest>,
    event_rx: mpsc::UnboundedReceiver<ServerMessage>,
    _rt: tokio::runtime::Runtime,
}

struct ClientRequest {
    id: RequestId,
    payload: String,
    respond: oneshot::Sender<anyhow::Result<JsonRpcResponse>>,
}

impl ProtocolClient {
    /// Connect to a daemon WebSocket URL and spawn background tasks.
    pub fn connect(url: &str) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        let (ws_stream, _) = rt.block_on(tokio_tungstenite::connect_async(url))?;
        let (mut write, mut read) = ws_stream.split();

        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<ClientRequest>();
        let (server_tx, server_rx) = mpsc::unbounded_channel::<ServerMessage>();
        let mut pending: HashMap<RequestId, oneshot::Sender<anyhow::Result<JsonRpcResponse>>> =
            HashMap::new();

        rt.spawn(async move {
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
                                if let Ok(event) = serde_json::from_str::<Event>(&text) {
                                    let _ = server_tx.send(ServerMessage::Notification(event));
                                    continue;
                                }
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
                                    _ => {}
                                }
                            }
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break,
                            Some(Err(_)) => break,
                            None => break,
                            _ => {}
                        }
                    }
                }
            }
            for (_, tx) in pending {
                let _ = tx.send(Err(anyhow::anyhow!("connection closed")));
            }
        });

        Ok(Self {
            request_tx,
            event_rx: server_rx,
            _rt: rt,
        })
    }

    /// Make a blocking JSON-RPC call with timeout.
    pub fn request(
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

        self._rt
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(30), rx).await
            })
            .map_err(|_| anyhow::anyhow!("request timed out"))?
            .map_err(|_| anyhow::anyhow!("response channel closed"))?
    }

    /// Send a fire-and-forget notification.
    pub fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
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

    /// Try to receive the next event without blocking.
    pub fn try_recv_event(&mut self) -> Option<Event> {
        match self.event_rx.try_recv() {
            Ok(ServerMessage::Notification(ev)) => Some(ev),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn cli_client_request_response_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let url = format!("ws://{}", local_addr);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws.split();

            while let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) = read.next().await {
                if let Ok(JsonRpcMessage::Request(req)) = serde_json::from_str(&text) {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: Some(serde_json::json!({"echo": req.method})),
                    };
                    let json = serde_json::to_string(&JsonRpcMessage::Response(resp)).unwrap();
                    let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await;
                }
            }
        });

        let client = ProtocolClient::connect(&url).unwrap();
        let resp = client.request("session.initialize", None).unwrap();
        assert!(resp.result.is_some());
    }
}
