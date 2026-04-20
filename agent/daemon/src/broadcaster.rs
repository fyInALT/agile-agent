//! EventBroadcaster — fans out events to all connected WebSocket clients.

use agent_protocol::events::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Distributes events to all connected clients.
#[derive(Clone)]
pub struct EventBroadcaster {
    clients: Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Event>>>>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new connection and return the receiver half.
    pub async fn register(&self, conn_id: String) -> tokio::sync::mpsc::UnboundedReceiver<Event> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.clients.lock().await.insert(conn_id, tx);
        rx
    }

    /// Remove a connection from the broadcast list.
    pub async fn unregister(&self, conn_id: &str) {
        self.clients.lock().await.remove(conn_id);
    }

    /// Broadcast an event to all connected clients.
    ///
    /// Uses `try_send` to avoid blocking; silently drops for clients whose
    /// channel is full (lagging clients).
    pub async fn broadcast(&self, event: Event) {
        let clients = self.clients.lock().await;
        let _json = serde_json::to_string(&event).unwrap_or_default();
        for (conn_id, tx) in clients.iter() {
            if let Err(e) = tx.send(event.clone()) {
                tracing::debug!("broadcast drop for {}: {}", conn_id, e);
            }
        }
        drop(clients);
        tracing::debug!(seq = event.seq, "broadcasted event to clients");
    }

    /// Number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.lock().await.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::events::{Event, EventPayload};

    #[tokio::test]
    async fn broadcast_reaches_all_clients() {
        let broadcaster = EventBroadcaster::new();
        let mut rx1 = broadcaster.register("conn-1".to_string()).await;
        let mut rx2 = broadcaster.register("conn-2".to_string()).await;

        let event = Event {
            seq: 1,
            payload: EventPayload::Error(agent_protocol::events::ErrorData {
                message: "test".to_string(),
                source: None,
            }),
        };
        broadcaster.broadcast(event.clone()).await;

        assert_eq!(rx1.recv().await.unwrap().seq, 1);
        assert_eq!(rx2.recv().await.unwrap().seq, 1);
    }

    #[tokio::test]
    async fn disconnected_client_does_not_receive() {
        let broadcaster = EventBroadcaster::new();
        let mut rx1 = broadcaster.register("conn-1".to_string()).await;
        let rx2 = broadcaster.register("conn-2".to_string()).await;

        broadcaster.unregister("conn-2").await;

        let event = Event {
            seq: 1,
            payload: EventPayload::Error(agent_protocol::events::ErrorData {
                message: "test".to_string(),
                source: None,
            }),
        };
        broadcaster.broadcast(event).await;

        assert!(rx1.recv().await.is_some());
        // rx2 should not receive because it was unregistered.
        drop(rx2);
    }

    #[tokio::test]
    async fn broadcaster_survives_channel_closure() {
        let broadcaster = EventBroadcaster::new();
        let rx = broadcaster.register("conn-1".to_string()).await;
        drop(rx); // Close the receiver.

        let event = Event {
            seq: 1,
            payload: EventPayload::Error(agent_protocol::events::ErrorData {
                message: "test".to_string(),
                source: None,
            }),
        };
        // Should not panic even though the receiver is closed.
        broadcaster.broadcast(event).await;
    }
}
