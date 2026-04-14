//! EventAggregator for non-blocking multi-channel polling
//!
//! Aggregates events from multiple agent event channels.
//!
//! # Thread Safety Role
//!
//! EventAggregator is the **critical bridge** between provider threads and main thread:
//!
//! ```text
//! Provider Threads                Main Thread (TUI)
//! ┌─────────────────┐            ┌─────────────────────────┐
//! │ Thread 1        │            │ EventAggregator         │
//! │  event_tx ──────┼───────────▶│  receivers: HashMap     │
//! │                 │            │  poll_all()             │
//! └─────────────────┘            │  poll_with_timeout()    │
//!                                └─────────────────────────┘
//! ┌─────────────────┐                       │
//! │ Thread 2        │                       ▼
//! │  event_tx ──────┼──────▶  AgentEvent    │
//! │                 │        (tagged with   │
//! └─────────────────┘        agent_id)      │
//!                                            ▼
//!                                State Mutation (Main Thread)
//! ```
//!
//! ## Key Thread Safety Properties
//!
//! 1. **Owned by main thread** - EventAggregator lives in main thread
//! 2. **Receivers from providers** - HashMap stores mpsc::Receiver from each provider
//! 3. **Non-blocking poll** - `try_recv()` never blocks, safe for UI loop
//! 4. **Tagged events** - Each event carries `AgentId` for routing
//!
//! ## Memory Safety
//!
//! - Receivers are `mpsc::Receiver` - thread-safe by design
//! - Provider threads own Senders, main thread owns Receivers
//! - When provider thread finishes, Sender drops → Receiver detects disconnect

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::agent_runtime::AgentId;
use crate::agent_slot::{AgentSlotStatus, TaskCompletionResult, TaskId, ThreadOutcome};
use crate::provider::ProviderEvent;

/// Agent event wrapping provider events with agent context
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// Event from provider thread
    FromProvider {
        agent_id: AgentId,
        event: ProviderEvent,
    },
    /// Agent status changed
    StatusChanged {
        agent_id: AgentId,
        old_status: AgentSlotStatus,
        new_status: AgentSlotStatus,
    },
    /// Agent completed a task
    TaskCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        result: TaskCompletionResult,
    },
    /// Agent encountered an error
    AgentError {
        agent_id: AgentId,
        error: String,
    },
    /// Agent thread finished
    ThreadFinished {
        agent_id: AgentId,
        outcome: ThreadOutcome,
    },
}

impl AgentEvent {
    /// Get the agent ID for this event
    pub fn agent_id(&self) -> &AgentId {
        match self {
            Self::FromProvider { agent_id, .. } => agent_id,
            Self::StatusChanged { agent_id, .. } => agent_id,
            Self::TaskCompleted { agent_id, .. } => agent_id,
            Self::AgentError { agent_id, .. } => agent_id,
            Self::ThreadFinished { agent_id, .. } => agent_id,
        }
    }

    /// Create a FromProvider event
    pub fn from_provider(agent_id: AgentId, event: ProviderEvent) -> Self {
        Self::FromProvider { agent_id, event }
    }

    /// Create a StatusChanged event
    pub fn status_changed(
        agent_id: AgentId,
        old_status: AgentSlotStatus,
        new_status: AgentSlotStatus,
    ) -> Self {
        Self::StatusChanged { agent_id, old_status, new_status }
    }

    /// Create a TaskCompleted event
    pub fn task_completed(agent_id: AgentId, task_id: TaskId, result: TaskCompletionResult) -> Self {
        Self::TaskCompleted { agent_id, task_id, result }
    }

    /// Create an AgentError event
    pub fn error(agent_id: AgentId, error: String) -> Self {
        Self::AgentError { agent_id, error }
    }

    /// Create a ThreadFinished event
    pub fn thread_finished(agent_id: AgentId, outcome: ThreadOutcome) -> Self {
        Self::ThreadFinished { agent_id, outcome }
    }
}

/// Result of polling all channels
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollResult {
    /// Events collected from all channels
    pub events: Vec<AgentEvent>,
    /// Channels that are empty (no events available)
    pub empty_channels: Vec<AgentId>,
    /// Channels that are disconnected (sender closed)
    pub disconnected_channels: Vec<AgentId>,
}

impl PollResult {
    /// Check if any events were collected
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Check if any channels disconnected
    pub fn has_disconnected(&self) -> bool {
        !self.disconnected_channels.is_empty()
    }
}

/// Aggregator for polling multiple agent event channels
///
/// Provides non-blocking polling across all registered channels,
/// returning events tagged with their source agent ID.
#[derive(Debug)]
pub struct EventAggregator {
    /// Map of agent IDs to their event receivers
    receivers: HashMap<AgentId, Receiver<ProviderEvent>>,
}

impl EventAggregator {
    /// Create a new empty aggregator
    pub fn new() -> Self {
        Self {
            receivers: HashMap::new(),
        }
    }

    /// Add a receiver for an agent
    pub fn add_receiver(&mut self, agent_id: AgentId, receiver: Receiver<ProviderEvent>) {
        self.receivers.insert(agent_id, receiver);
    }

    /// Remove a receiver for an agent
    ///
    /// Returns the removed receiver if it existed.
    pub fn remove_receiver(&mut self, agent_id: &AgentId) -> Option<Receiver<ProviderEvent>> {
        self.receivers.remove(agent_id)
    }

    /// Check if an agent has a registered receiver
    pub fn has_receiver(&self, agent_id: &AgentId) -> bool {
        self.receivers.contains_key(agent_id)
    }

    /// Get the number of registered receivers
    pub fn receiver_count(&self) -> usize {
        self.receivers.len()
    }

    /// Poll all channels without blocking
    ///
    /// Returns all available events from all channels.
    pub fn poll_all(&self) -> PollResult {
        let mut events = Vec::new();
        let mut empty_channels = Vec::new();
        let mut disconnected_channels = Vec::new();

        for (agent_id, receiver) in &self.receivers {
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        events.push(AgentEvent::from_provider(agent_id.clone(), event));
                    }
                    Err(TryRecvError::Empty) => {
                        empty_channels.push(agent_id.clone());
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        disconnected_channels.push(agent_id.clone());
                        break;
                    }
                }
            }
        }

        PollResult {
            events,
            empty_channels,
            disconnected_channels,
        }
    }

    /// Poll all channels with timeout
    ///
    /// Waits up to `timeout` duration for events from any channel.
    /// Uses a simple approach: poll all channels repeatedly until timeout.
    pub fn poll_with_timeout(&self, timeout: Duration) -> PollResult {
        let deadline = Instant::now() + timeout;
        let mut result = self.poll_all();

        // If we got events immediately, return them
        if result.has_events() {
            return result;
        }

        // Otherwise, keep polling until timeout
        while Instant::now() < deadline {
            let remaining = deadline - Instant::now();
            if remaining.is_zero() {
                break;
            }

            // Small sleep to avoid tight spin loop
            std::thread::sleep(Duration::from_millis(10));

            let next_result = self.poll_all();
            result.events.extend(next_result.events);
            result.disconnected_channels.extend(next_result.disconnected_channels);

            if result.has_events() {
                break;
            }
        }

        result
    }

    /// Poll only specific agents
    pub fn poll_agents(&self, agent_ids: &[AgentId]) -> PollResult {
        let mut events = Vec::new();
        let mut empty_channels = Vec::new();
        let mut disconnected_channels = Vec::new();

        for agent_id in agent_ids {
            if let Some(receiver) = self.receivers.get(agent_id) {
                loop {
                    match receiver.try_recv() {
                        Ok(event) => {
                            events.push(AgentEvent::from_provider(agent_id.clone(), event));
                        }
                        Err(TryRecvError::Empty) => {
                            empty_channels.push(agent_id.clone());
                            break;
                        }
                        Err(TryRecvError::Disconnected) => {
                            disconnected_channels.push(agent_id.clone());
                            break;
                        }
                    }
                }
            }
        }

        PollResult {
            events,
            empty_channels,
            disconnected_channels,
        }
    }

    /// Get all registered agent IDs
    pub fn registered_agents(&self) -> Vec<AgentId> {
        self.receivers.keys().cloned().collect()
    }

    /// Clear all receivers
    pub fn clear(&mut self) {
        self.receivers.clear();
    }
}

impl Default for EventAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn make_aggregator() -> EventAggregator {
        EventAggregator::new()
    }

    #[test]
    fn aggregator_new_is_empty() {
        let aggregator = make_aggregator();
        assert_eq!(aggregator.receiver_count(), 0);
    }

    #[test]
    fn add_receiver_increases_count() {
        let mut aggregator = make_aggregator();
        let (_tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id.clone(), rx);
        assert_eq!(aggregator.receiver_count(), 1);
        assert!(aggregator.has_receiver(&agent_id));
    }

    #[test]
    fn remove_receiver_decreases_count() {
        let mut aggregator = make_aggregator();
        let (_tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id.clone(), rx);
        let removed = aggregator.remove_receiver(&agent_id);
        assert!(removed.is_some());
        assert_eq!(aggregator.receiver_count(), 0);
        assert!(!aggregator.has_receiver(&agent_id));
    }

    #[test]
    fn poll_all_empty_returns_empty_result() {
        let mut aggregator = make_aggregator();
        let (tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id, rx);
        // Don't send anything, channel is empty
        drop(tx); // Disconnect sender
        let result = aggregator.poll_all();
        assert!(result.disconnected_channels.contains(&AgentId::new("agent_001")));
    }

    #[test]
    fn poll_all_collects_events() {
        let mut aggregator = make_aggregator();
        let (tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id.clone(), rx);

        tx.send(ProviderEvent::AssistantChunk("Hello".to_string())).unwrap();
        tx.send(ProviderEvent::Status("Running".to_string())).unwrap();
        drop(tx); // Disconnect after sending

        let result = aggregator.poll_all();
        assert_eq!(result.events.len(), 2);
        assert!(result.disconnected_channels.contains(&agent_id));
    }

    #[test]
    fn poll_all_multiple_agents() {
        let mut aggregator = make_aggregator();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let agent1 = AgentId::new("agent_001");
        let agent2 = AgentId::new("agent_002");
        aggregator.add_receiver(agent1.clone(), rx1);
        aggregator.add_receiver(agent2.clone(), rx2);

        tx1.send(ProviderEvent::AssistantChunk("From agent 1".to_string())).unwrap();
        tx2.send(ProviderEvent::ThinkingChunk("From agent 2".to_string())).unwrap();
        drop(tx1);
        drop(tx2);

        let result = aggregator.poll_all();
        assert_eq!(result.events.len(), 2);
        // Each event should be tagged with correct agent
        let agent_ids: Vec<_> = result.events.iter().map(|e| e.agent_id()).collect();
        assert!(agent_ids.contains(&&agent1));
        assert!(agent_ids.contains(&&agent2));
    }

    #[test]
    fn poll_result_has_events() {
        let result = PollResult {
            events: vec![AgentEvent::error(AgentId::new("agent_001"), "test".to_string())],
            empty_channels: vec![],
            disconnected_channels: vec![],
        };
        assert!(result.has_events());
    }

    #[test]
    fn poll_result_no_events() {
        let result = PollResult {
            events: vec![],
            empty_channels: vec![AgentId::new("agent_001")],
            disconnected_channels: vec![],
        };
        assert!(!result.has_events());
    }

    #[test]
    fn agent_event_agent_id() {
        let agent_id = AgentId::new("agent_001");
        let event = AgentEvent::from_provider(agent_id.clone(), ProviderEvent::Status("test".to_string()));
        assert_eq!(event.agent_id(), &agent_id);
    }

    #[test]
    fn agent_event_from_provider() {
        let agent_id = AgentId::new("agent_001");
        let event = AgentEvent::from_provider(agent_id.clone(), ProviderEvent::AssistantChunk("test".to_string()));
        assert!(matches!(event, AgentEvent::FromProvider { .. }));
    }

    #[test]
    fn agent_event_status_changed() {
        let agent_id = AgentId::new("agent_001");
        let event = AgentEvent::status_changed(
            agent_id,
            AgentSlotStatus::idle(),
            AgentSlotStatus::starting(),
        );
        assert!(matches!(event, AgentEvent::StatusChanged { .. }));
    }

    #[test]
    fn poll_with_timeout_returns_immediately_with_events() {
        let mut aggregator = make_aggregator();
        let (tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id, rx);

        tx.send(ProviderEvent::Status("test".to_string())).unwrap();
        drop(tx);

        let result = aggregator.poll_with_timeout(Duration::from_millis(100));
        assert_eq!(result.events.len(), 1);
    }

    #[test]
    fn poll_with_timeout_returns_empty_without_events() {
        let mut aggregator = make_aggregator();
        let (tx, rx) = channel();
        let agent_id = AgentId::new("agent_001");
        aggregator.add_receiver(agent_id, rx);
        drop(tx); // Disconnect without sending

        let result = aggregator.poll_with_timeout(Duration::from_millis(50));
        assert!(result.disconnected_channels.contains(&AgentId::new("agent_001")));
    }

    #[test]
    fn registered_agents() {
        let mut aggregator = make_aggregator();
        let (_, rx1) = channel();
        let (_, rx2) = channel();
        aggregator.add_receiver(AgentId::new("agent_001"), rx1);
        aggregator.add_receiver(AgentId::new("agent_002"), rx2);

        let agents = aggregator.registered_agents();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn clear_removes_all_receivers() {
        let mut aggregator = make_aggregator();
        let (_, rx1) = channel();
        let (_, rx2) = channel();
        aggregator.add_receiver(AgentId::new("agent_001"), rx1);
        aggregator.add_receiver(AgentId::new("agent_002"), rx2);
        aggregator.clear();
        assert_eq!(aggregator.receiver_count(), 0);
    }

    #[test]
    fn poll_agents_specific_agents() {
        let mut aggregator = make_aggregator();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let agent1 = AgentId::new("agent_001");
        let agent2 = AgentId::new("agent_002");
        aggregator.add_receiver(agent1.clone(), rx1);
        aggregator.add_receiver(agent2.clone(), rx2);

        tx1.send(ProviderEvent::Status("agent 1".to_string())).unwrap();
        tx2.send(ProviderEvent::Status("agent 2".to_string())).unwrap();

        // Poll only agent1
        let result = aggregator.poll_agents(&[agent1.clone()]);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].agent_id(), &agent1);
    }
}