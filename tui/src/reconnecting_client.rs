//! Reconnecting WebSocket client with exponential backoff.

use crate::protocol_state::ConnectionState;
use crate::websocket_client::{ServerMessage, WebSocketClient};
use std::time::Duration;
use tokio::sync::mpsc;

/// WebSocket client that automatically reconnects on disconnect.
pub struct ReconnectingClient {
    url: String,
    state_tx: mpsc::UnboundedSender<ConnectionState>,
    server_tx: mpsc::UnboundedSender<ServerMessage>,
}

impl ReconnectingClient {
    pub fn new(url: String) -> (Self, mpsc::UnboundedReceiver<ConnectionState>, mpsc::UnboundedReceiver<ServerMessage>) {
        let (state_tx, state_rx) = mpsc::unbounded_channel();
        let (server_tx, server_rx) = mpsc::unbounded_channel();
        (Self { url, state_tx, server_tx }, state_rx, server_rx)
    }

    /// Run the reconnect loop until explicitly cancelled.
    pub async fn run(&self) {
        let mut attempt = 0u32;
        loop {
            let _ = self.state_tx.send(ConnectionState::Connecting);
            match WebSocketClient::connect(&self.url).await {
                Ok((_client, mut rx)) => {
                    attempt = 0;
                    let _ = self.state_tx.send(ConnectionState::Connected);
                    // Forward events until the channel closes.
                    while let Some(msg) = rx.recv().await {
                        if self.server_tx.send(msg).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("websocket connect error: {}, will retry", e);
                }
            }
            let _ = self.state_tx.send(ConnectionState::Reconnecting);
            let delay = Self::backoff_delay(attempt);
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    fn backoff_delay(attempt: u32) -> Duration {
        let base = Duration::from_millis(100);
        let max = Duration::from_secs(30);
        let exp = base * 2u32.saturating_pow(attempt);
        if exp > max { max } else { exp }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::events::{Event, EventPayload, AgentSpawnedData};
    use tokio::net::TcpListener;

    #[test]
    fn backoff_delay_clamps_at_max() {
        assert_eq!(ReconnectingClient::backoff_delay(0), Duration::from_millis(100));
        assert_eq!(ReconnectingClient::backoff_delay(1), Duration::from_millis(200));
        assert_eq!(ReconnectingClient::backoff_delay(2), Duration::from_millis(400));
        assert_eq!(ReconnectingClient::backoff_delay(10), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn reconnect_success_connects_and_forwards_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let url = format!("ws://{}", local_addr);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            use futures::{SinkExt, StreamExt};
            let (mut write, _read) = ws.split();

            let event = Event {
                seq: 1,
                payload: EventPayload::AgentSpawned(AgentSpawnedData {
                    agent_id: "a1".to_string(),
                    codename: "alpha".to_string(),
                    role: "Developer".to_string(),
                }),
            };
            let json = serde_json::to_string(&event).unwrap();
            let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await;
        });

        let (client, mut state_rx, mut server_rx) = ReconnectingClient::new(url);
        let client = Box::leak(Box::new(client));

        let run_handle = tokio::spawn(client.run());

        let state1 = tokio::time::timeout(Duration::from_secs(3), state_rx.recv()).await;
        assert_eq!(state1.unwrap(), Some(ConnectionState::Connecting));

        let state2 = tokio::time::timeout(Duration::from_secs(3), state_rx.recv()).await;
        assert_eq!(state2.unwrap(), Some(ConnectionState::Connected));

        let msg = tokio::time::timeout(Duration::from_secs(3), server_rx.recv()).await;
        match msg.unwrap() {
            Some(ServerMessage::Notification(ev)) => assert_eq!(ev.seq, 1),
            other => panic!("expected Notification, got {:?}", other),
        }

        run_handle.abort();
    }
}
