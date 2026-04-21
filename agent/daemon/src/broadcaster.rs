//! EventBroadcaster — fans out events to all connected WebSocket clients.

use agent_protocol::events::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Distributes events to all connected clients.
#[derive(Clone)]
pub struct EventBroadcaster {
    clients: Arc<Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<Event>>>>,
    client_seqs: Arc<Mutex<HashMap<String, u64>>>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            client_seqs: Arc::new(Mutex::new(HashMap::new())),
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
        self.client_seqs.lock().await.remove(conn_id);
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

    /// Update the last acked sequence number for a client.
    pub async fn update_client_seq(&self, conn_id: &str, seq: u64) {
        self.client_seqs.lock().await.insert(conn_id.to_string(), seq);
    }

    /// Detect clients that are lagging behind by more than `threshold` events.
    pub async fn detect_lagging_clients(&self, current_seq: u64, threshold: u64) -> Vec<String> {
        let seqs = self.client_seqs.lock().await;
        let clients = self.clients.lock().await;
        let mut lagging = Vec::new();
        for (conn_id, _tx) in clients.iter() {
            let last_seq = seqs.get(conn_id).copied().unwrap_or(0);
            if current_seq.saturating_sub(last_seq) > threshold {
                lagging.push(conn_id.clone());
            }
        }
        lagging
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

    #[tokio::test]
    async fn lag_fast_client() {
        let broadcaster = EventBroadcaster::new();
        let mut rx = broadcaster.register("conn-1".to_string()).await;

        let event = Event {
            seq: 1,
            payload: EventPayload::Error(agent_protocol::events::ErrorData {
                message: "test".to_string(),
                source: None,
            }),
        };
        broadcaster.broadcast(event.clone()).await;
        broadcaster.update_client_seq("conn-1", 1).await;

        assert_eq!(rx.recv().await.unwrap().seq, 1);
        let lagging = broadcaster.detect_lagging_clients(1, 5).await;
        assert!(lagging.is_empty(), "fast client should not be lagging");
    }

    #[tokio::test]
    async fn lag_slow_client() {
        let broadcaster = EventBroadcaster::new();
        let _rx = broadcaster.register("conn-1".to_string()).await;

        for seq in 1..=10 {
            let event = Event {
                seq,
                payload: EventPayload::Error(agent_protocol::events::ErrorData {
                    message: format!("evt-{}", seq),
                    source: None,
                }),
            };
            broadcaster.broadcast(event).await;
        }

        let lagging = broadcaster.detect_lagging_clients(10, 5).await;
        assert_eq!(lagging, vec!["conn-1"], "slow client should be detected as lagging");
    }

    #[tokio::test]
    async fn lag_recovery() {
        let broadcaster = EventBroadcaster::new();
        let _rx = broadcaster.register("conn-1".to_string()).await;

        for seq in 1..=10 {
            let event = Event {
                seq,
                payload: EventPayload::Error(agent_protocol::events::ErrorData {
                    message: format!("evt-{}", seq),
                    source: None,
                }),
            };
            broadcaster.broadcast(event).await;
        }

        assert_eq!(
            broadcaster.detect_lagging_clients(10, 5).await,
            vec!["conn-1"]
        );

        broadcaster.update_client_seq("conn-1", 10).await;
        assert!(
            broadcaster.detect_lagging_clients(10, 5).await.is_empty(),
            "client should recover after updating seq"
        );
    }

    #[tokio::test]
    async fn unregister_clears_client_seqs() {
        let broadcaster = EventBroadcaster::new();
        let _rx = broadcaster.register("conn-1".to_string()).await;
        broadcaster.update_client_seq("conn-1", 42).await;

        // Verify seq was recorded
        let seqs = broadcaster.client_seqs.lock().await;
        assert_eq!(seqs.get("conn-1"), Some(&42));
        drop(seqs);

        // Unregister should clear both maps
        broadcaster.unregister("conn-1").await;

        let clients = broadcaster.clients.lock().await;
        assert!(clients.get("conn-1").is_none());
        drop(clients);

        let seqs = broadcaster.client_seqs.lock().await;
        assert!(seqs.get("conn-1").is_none(), "seq should be cleared after unregister");
    }
}
