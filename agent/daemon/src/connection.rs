//! Per-connection state machine

use agent_protocol::events::{Event, MAX_INPUT_SIZE};
use agent_protocol::jsonrpc::*;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use crate::router::RouterHandle;

/// Unique identifier for a connection.
pub type ConnectionId = String;

/// Heartbeat timeout: close connection after 120s of silence.
const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Tracks active connections and enforces a hard limit.
#[derive(Debug, Clone)]
pub struct ConnectionTracker {
    active: Arc<AtomicUsize>,
    max: usize,
}

impl ConnectionTracker {
    pub fn new(max: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }

    /// Attempt to acquire a connection slot. Returns `false` if at limit.
    pub fn try_connect(&self) -> bool {
        let current = self.active.fetch_add(1, Ordering::SeqCst);
        if current >= self.max {
            self.active.fetch_sub(1, Ordering::SeqCst);
            false
        } else {
            true
        }
    }

    /// Release a connection slot.
    pub fn disconnect(&self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }

    /// Current number of tracked connections.
    pub fn count(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }
}

/// Validates that the origin header refers to localhost.
fn is_localhost_origin(origin: &str) -> bool {
    origin.is_empty()
        || origin.starts_with("http://localhost")
        || origin.starts_with("https://localhost")
        || origin.starts_with("http://127.0.0.1")
        || origin.starts_with("https://127.0.0.1")
        || origin == "null"
}

/// Validates the bearer token from query param or Authorization header.
fn check_bearer_token(
    req: &tokio_tungstenite::tungstenite::http::Request<()>,
    bearer_token: &Option<String>,
) -> Result<(), tokio_tungstenite::tungstenite::http::Response<Option<String>>> {
    if let Some(expected) = bearer_token {
        let provided = req
            .uri()
            .query()
            .and_then(|q| {
                q.split('&').find_map(|pair| {
                    let mut kv = pair.splitn(2, '=');
                    let k = kv.next()?;
                    if k == "token" { kv.next() } else { None }
                })
            })
            .or_else(|| {
                req.headers().get("authorization").and_then(|h| {
                    let s = h.to_str().ok()?;
                    let mut parts = s.splitn(2, ' ');
                    if parts.next()?.eq_ignore_ascii_case("Bearer") {
                        parts.next()
                    } else {
                        None
                    }
                })
            });
        if provided != Some(expected.as_str()) {
            return Err(
                tokio_tungstenite::tungstenite::http::Response::builder()
                    .status(401)
                    .body(Some("Unauthorized".to_string()))
                    .unwrap(),
            );
        }
    }
    Ok(())
}

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
    ///
    /// If `event_rx` is provided, events are forwarded to the client as
    /// JSON-RPC notifications.
    pub fn spawn(
        stream: TcpStream,
        addr: std::net::SocketAddr,
        router: RouterHandle,
        mut event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<Event>>,
        tracker: Option<ConnectionTracker>,
        bearer_token: Option<String>,
    ) -> ConnectionId {
        if let Some(ref t) = tracker {
            if !t.try_connect() {
                tracing::warn!("Connection from {} rejected: max connections reached", addr);
                // Drop the TCP stream immediately.
                return String::new();
            }
        }

        let id = format!("conn-{}", uuid::Uuid::new_v4());
        let id_clone = id.clone();
        let tracker_clone = tracker.clone();

        tokio::spawn(async move {
            let ws_stream = match tokio_tungstenite::accept_hdr_async(
                stream,
                |req: &tokio_tungstenite::tungstenite::http::Request<()>,
                 response: tokio_tungstenite::tungstenite::http::Response<()>| {
                    if let Some(origin) = req.headers().get("origin") {
                        let origin_str = origin.to_str().unwrap_or("");
                        if !is_localhost_origin(origin_str) {
                            tracing::warn!("Rejected non-localhost origin: {}", origin_str);
                            return Err(
                                tokio_tungstenite::tungstenite::http::Response::builder()
                                    .status(403)
                                    .body(Some("Non-localhost origin rejected".to_string()))
                                    .unwrap(),
                            );
                        }
                    }
                    if let Err(err_response) = check_bearer_token(req, &bearer_token) {
                        return Err(err_response);
                    }
                    Ok(response)
                },
            )
            .await
            {
                Ok(ws) => ws,
                Err(e) => {
                    tracing::warn!("WebSocket upgrade failed for {}: {}", addr, e);
                    if let Some(ref t) = tracker_clone {
                        t.disconnect();
                    }
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
                // Wait for next message with heartbeat timeout, or an event.
                let next = tokio::select! {
                    msg = tokio::time::timeout(HEARTBEAT_TIMEOUT, read.next()) => {
                        match msg {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                Some(ConnInput::WsText(text))
                            }
                            Ok(Some(Ok(Message::Ping(_)))) |
                            Ok(Some(Ok(Message::Pong(_)))) |
                            Ok(Some(Ok(Message::Frame(_)))) => {
                                continue;
                            }
                            Ok(Some(Ok(Message::Close(_)))) => {
                                tracing::debug!("Client {} sent close frame", conn.id);
                                break;
                            }
                            Ok(Some(Ok(Message::Binary(_)))) => {
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
                                break;
                            }
                            Err(_) => {
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
                    event = async {
                        match &mut event_rx {
                            Some(rx) => rx.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        match event {
                            Some(ev) => Some(ConnInput::Event(ev)),
                            None => {
                                tracing::debug!("Event channel closed for {}", conn.id);
                                break;
                            }
                        }
                    }
                };

                let input = match next {
                    Some(i) => i,
                    None => continue,
                };

                match input {
                    ConnInput::WsText(text) => {
                        let text = if text.len() > MAX_INPUT_SIZE {
                            tracing::warn!(
                                "Input size {} exceeds MAX_INPUT_SIZE, truncating",
                                text.len()
                            );
                            text.chars().take(MAX_INPUT_SIZE).collect()
                        } else {
                            text
                        };

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
                                            ext: None,
                                        },
                                        ext: None,
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
                    ConnInput::Event(event) => {
                        let json = match serde_json::to_string(&event) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::error!("Failed to serialize event: {}", e);
                                continue;
                            }
                        };
                        if let Err(e) = write.send(Message::Text(json)).await {
                            tracing::warn!("Event send error on {}: {}", conn.id, e);
                            break;
                        }
                    }
                }
            }

            conn.state = ConnectionState::Closing;
            tracing::debug!("Connection {} closed", conn.id);
            if let Some(ref t) = tracker_clone {
                t.disconnect();
            }
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
                            ext: None,
                        },
                        ext: None,
                    })));
                }

                let result = router.dispatch(req).await?;

                if self.state == ConnectionState::Connected {
                    if let JsonRpcMessage::Response(ref resp) = result {
                        if resp.result.is_some() {
                            self.state = ConnectionState::Initialized;
                        }
                    }
                }

                Ok(Some(result))
            }
            JsonRpcMessage::Notification(notif) => {
                router.dispatch_notification(notif).await?;
                Ok(None)
            }
            _ => Err(anyhow::anyhow!("Invalid message direction from client")),
        }
    }
}

enum ConnInput {
    WsText(String),
    Event(Event),
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

    #[test]
    fn tracker_allows_up_to_max() {
        let tracker = ConnectionTracker::new(3);
        assert!(tracker.try_connect());
        assert!(tracker.try_connect());
        assert!(tracker.try_connect());
        assert!(!tracker.try_connect());
        assert_eq!(tracker.count(), 3);
    }

    #[test]
    fn tracker_release_allows_new_connection() {
        let tracker = ConnectionTracker::new(1);
        assert!(tracker.try_connect());
        assert!(!tracker.try_connect());
        tracker.disconnect();
        assert!(tracker.try_connect());
    }

    #[test]
    fn localhost_origin_accepted() {
        assert!(is_localhost_origin("http://localhost:3000"));
        assert!(is_localhost_origin("https://127.0.0.1:8080"));
        assert!(is_localhost_origin(""));
        assert!(is_localhost_origin("null"));
    }

    #[test]
    fn non_localhost_origin_rejected() {
        assert!(!is_localhost_origin("http://evil.com"));
        assert!(!is_localhost_origin("https://example.com"));
    }

    #[test]
    fn input_truncation_preserves_char_boundaries() {
        let text: String = "a".repeat(MAX_INPUT_SIZE + 100);
        let truncated: String = text.chars().take(MAX_INPUT_SIZE).collect();
        assert_eq!(truncated.len(), MAX_INPUT_SIZE);
    }

    #[test]
    fn token_auth_accepted() {
        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri("ws://localhost:1234/v1/session?token=secret123")
            .body(())
            .unwrap();
        assert!(check_bearer_token(&req, &Some("secret123".to_string())).is_ok());
    }

    #[test]
    fn token_auth_accepted_via_header() {
        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri("ws://localhost:1234/v1/session")
            .header("Authorization", "Bearer secret123")
            .body(())
            .unwrap();
        assert!(check_bearer_token(&req, &Some("secret123".to_string())).is_ok());
    }

    #[test]
    fn token_auth_rejected() {
        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri("ws://localhost:1234/v1/session?token=wrong")
            .body(())
            .unwrap();
        let result = check_bearer_token(&req, &Some("secret123".to_string()));
        assert!(result.is_err());
        let resp = result.unwrap_err();
        assert_eq!(resp.status(), 401);
    }

    #[test]
    fn no_token_required_when_none() {
        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri("ws://localhost:1234/v1/session")
            .body(())
            .unwrap();
        assert!(check_bearer_token(&req, &None).is_ok());
    }
}
