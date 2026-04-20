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
                Ok((client, mut rx)) => {
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

    #[test]
    fn backoff_delay_clamps_at_max() {
        assert_eq!(ReconnectingClient::backoff_delay(0), Duration::from_millis(100));
        assert_eq!(ReconnectingClient::backoff_delay(1), Duration::from_millis(200));
        assert_eq!(ReconnectingClient::backoff_delay(2), Duration::from_millis(400));
        assert_eq!(ReconnectingClient::backoff_delay(10), Duration::from_secs(30));
    }
}
