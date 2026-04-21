//! Per-connection state machine

use agent_protocol::events::{Event, MAX_INPUT_SIZE};
use agent_protocol::jsonrpc::*;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use crate::router::RouterHandle;

/// Simple token-bucket rate limiter per connection.
#[derive(Debug)]
pub struct RateLimiter {
    tokens: usize,
    max: usize,
    last_refill: std::time::Instant,
    refill_interval: std::time::Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with `max` requests per minute.
    pub fn new(max_per_minute: usize) -> Self {
        Self {
            tokens: max_per_minute,
            max: max_per_minute,
            last_refill: std::time::Instant::now(),
            refill_interval: std::time::Duration::from_secs(60),
        }
    }

    /// Attempt to consume one token. Returns `true` if allowed.
    pub fn try_acquire(&mut self) -> bool {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        if elapsed >= self.refill_interval {
            self.tokens = self.max;
            self.last_refill = now;
        }
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Unique identifier for a connection.
pub type ConnectionId = String;

/// Default heartbeat timeout: close connection after 120s of silence.
pub const DEFAULT_HEARTBEAT_TIMEOUT_SECS: u64 = 120;

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
    pub client_type: Option<agent_protocol::methods::ClientType>,
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
        rate_limiter: Option<RateLimiter>,
        audit_log: Option<crate::audit::AuditLog>,
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
                client_type: None,
            };
            let mut rate_limiter = rate_limiter;

            let heartbeat_timeout = std::time::Duration::from_secs(DEFAULT_HEARTBEAT_TIMEOUT_SECS);

            loop {
                // Wait for next message with heartbeat timeout, or an event.
                let next = tokio::select! {
                    msg = tokio::time::timeout(heartbeat_timeout, read.next()) => {
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
                                    heartbeat_timeout
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
                        let _req_span = tracing::info_span!("jsonrpc_request", connection_id = %conn.id);
                        let _enter = _req_span.enter();

                        // Rate limiting check
                        if let Some(ref mut limiter) = rate_limiter {
                            if !limiter.try_acquire() {
                                tracing::warn!("Rate limit exceeded on {}", conn.id);
                                // Attempt to echo the client's request ID; fallback to Null.
                                let req_id = serde_json::from_str::<JsonRpcRequest>(&text)
                                    .map(|r| r.id)
                                    .unwrap_or(RequestId::String(String::new()));
                                let err = JsonRpcErrorResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: req_id,
                                    error: JsonRpcError {
                                        code: -32000,
                                        message: "Rate limit exceeded".to_string(),
                                        data: Some(serde_json::json!({"retry_after": 60})),
                                        ext: None,
                                    },
                                    ext: None,
                                };
                                if let Ok(json) = serde_json::to_string(&err) {
                                    let _ = write.send(Message::Text(json)).await;
                                }
                                continue;
                            }
                        }

                        let text = if text.len() > MAX_INPUT_SIZE {
                            tracing::warn!(
                                "Input size {} exceeds MAX_INPUT_SIZE, truncating",
                                text.len()
                            );
                            text.chars().take(MAX_INPUT_SIZE).collect()
                        } else {
                            text
                        };

                        match conn.handle_message(&text, &router, audit_log.as_ref()).await {
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
                                    if let Ok(json) = serde_json::to_string(&err) {
                                        let _ = write.send(Message::Text(json)).await;
                                    }
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
        audit_log: Option<&crate::audit::AuditLog>,
    ) -> anyhow::Result<Option<JsonRpcMessage>> {
        let msg: JsonRpcMessage = serde_json::from_str(text)?;

        match msg {
            JsonRpcMessage::Request(req) => {
                // Parse client_type from initialize params
                if req.method == "session.initialize" {
                    if let Some(ref params) = req.params {
                        if let Ok(init) = serde_json::from_value::<agent_protocol::methods::InitializeParams>(params.clone()) {
                            self.client_type = Some(init.client_type);
                        }
                    }
                }

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

                // Debug commands restricted to CLI client type
                if matches!(req.method.as_str(), "session.debugDump" | "session.forceSnapshot" | "session.listConnections") {
                    if self.client_type != Some(agent_protocol::methods::ClientType::Cli) {
                        return Ok(Some(JsonRpcMessage::Error(JsonRpcErrorResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id,
                            error: JsonRpcError {
                                code: -32106,
                                message: "Not supported: debug commands restricted to CLI clients".to_string(),
                                data: None,
                                ext: None,
                            },
                            ext: None,
                        })));
                    }
                }

                let method = req.method.clone();
                let result = router.dispatch(req).await?;

                // Audit log for sensitive operations
                if let Some(log) = audit_log {
                    if matches!(method.as_str(), "agent.spawn" | "agent.stop" | "tool.approve") {
                        let entry = crate::audit::AuditEntry::new(
                            &method,
                            &self.addr.to_string(),
                            serde_json::json!({"connection_id": self.id}),
                        );
                        if let Err(e) = log.append(&entry).await {
                            tracing::warn!("Audit log append failed: {}", e);
                        }
                    }
                }

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
            client_type: None,
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

    #[test]
    fn rate_limiter_allows_up_to_max() {
        let mut limiter = RateLimiter::new(3);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn rate_limiter_refills_after_interval() {
        let mut limiter = RateLimiter::new(1);
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
        // Manually reset last_refill to simulate time passing
        limiter.last_refill = std::time::Instant::now() - std::time::Duration::from_secs(61);
        assert!(limiter.try_acquire());
    }
}
