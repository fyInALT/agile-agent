//! Blocked handler for managing blocked agents
//!
//! Provides BlockedHandler struct and related traits.
//! Note: The actual blocked handling logic remains in AgentPool
//! due to deep coupling with pool internals.

use std::sync::Arc;

use crate::agent_runtime::AgentId;
use crate::pool::types::{
    BlockedHandlingConfig, BlockedHistoryEntry,
};

// Re-export types from pool::types for backward compatibility
pub use crate::pool::types::{
    AgentBlockedEvent, AgentBlockedNotifier, NoOpAgentBlockedNotifier,
};

/// Handler for blocked agent management
///
/// Manages blocked agent detection, notification, and history recording.
/// Used as a delegate within AgentPool.
pub struct BlockedHandler {
    /// Blocked handling configuration
    config: BlockedHandlingConfig,
    /// History of blocked events
    history: Vec<BlockedHistoryEntry>,
    /// Notifier for blocked events
    notifier: Arc<dyn AgentBlockedNotifier>,
}

impl std::fmt::Debug for BlockedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockedHandler")
            .field("config", &self.config)
            .field("history", &self.history)
            .field("notifier", &"<dyn AgentBlockedNotifier>")
            .finish()
    }
}

impl BlockedHandler {
    /// Create a new blocked handler with default config
    pub fn new() -> Self {
        Self {
            config: BlockedHandlingConfig::default(),
            history: Vec::new(),
            notifier: Arc::new(NoOpAgentBlockedNotifier),
        }
    }

    /// Create a blocked handler with custom config
    pub fn with_config(config: BlockedHandlingConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
            notifier: Arc::new(NoOpAgentBlockedNotifier),
        }
    }

    /// Set custom notifier
    pub fn set_notifier(&mut self, notifier: Arc<dyn AgentBlockedNotifier>) {
        self.notifier = notifier;
    }

    /// Get config reference
    pub fn config(&self) -> &BlockedHandlingConfig {
        &self.config
    }

    /// Get history reference
    pub fn history(&self) -> &[BlockedHistoryEntry] {
        &self.history
    }

    /// Record a blocked event in history
    pub fn record_blocked(&mut self, entry: BlockedHistoryEntry) {
        if self.config.record_history {
            self.history.push(entry);
            self.prune_history();
        }
    }

    /// Record resolution in history
    pub fn record_resolution(&mut self, agent_id: &AgentId, resolution: String) {
        for entry in self.history.iter_mut().rev() {
            if &entry.agent_id == agent_id && !entry.resolved {
                entry.resolved = true;
                entry.resolution = Some(resolution);
                return;
            }
        }
    }

    /// Notify others about blocked agent
    pub fn notify_blocked(&self, event: AgentBlockedEvent) {
        if self.config.notify_others {
            self.notifier.on_agent_blocked(event);
        }
    }

    /// Prune history to max entries
    fn prune_history(&mut self) {
        let max = self.config.max_history_entries;
        if max == 0 {
            return; // Unlimited
        }
        while self.history.len() > max {
            // Remove resolved entries first
            if let Some(pos) = self.history.iter().position(|e| e.resolved) {
                self.history.remove(pos);
            } else {
                // Remove oldest entry
                self.history.remove(0);
            }
        }
    }

    /// Count blocked entries in history
    pub fn count_active(&self) -> usize {
        self.history.iter().filter(|e| !e.resolved).count()
    }
}

impl Default for BlockedHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::types::{BlockedTaskPolicy};
    use agent_decision::HumanDecisionTimeoutConfig;

    #[test]
    fn blocked_handler_new() {
        let handler = BlockedHandler::new();
        assert!(handler.history().is_empty());
        assert!(handler.config().notify_others);
    }

    #[test]
    fn blocked_handler_with_config() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::KeepAssigned,
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: false,
            record_history: true,
            max_history_entries: 50,
        };
        let handler = BlockedHandler::with_config(config);
        assert!(!handler.config().notify_others);
        assert_eq!(handler.config().max_history_entries, 50);
    }

    #[test]
    fn blocked_handler_record_blocked() {
        let mut handler = BlockedHandler::new();
        let entry = BlockedHistoryEntry {
            agent_id: AgentId::new("agent-001"),
            reason_type: "human_decision".to_string(),
            description: "test".to_string(),
            duration_ms: 1000,
            resolved: false,
            resolution: None,
        };
        handler.record_blocked(entry);
        assert_eq!(handler.history().len(), 1);
    }

    #[test]
    fn blocked_handler_record_resolution() {
        let mut handler = BlockedHandler::new();
        handler.record_blocked(BlockedHistoryEntry {
            agent_id: AgentId::new("agent-001"),
            reason_type: "test".to_string(),
            description: "test".to_string(),
            duration_ms: 1000,
            resolved: false,
            resolution: None,
        });
        handler.record_resolution(&AgentId::new("agent-001"), "resolved".to_string());
        assert!(handler.history()[0].resolved);
    }

    #[test]
    fn blocked_handler_prune_history() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::default(),
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: true,
            record_history: true,
            max_history_entries: 3,
        };
        let mut handler = BlockedHandler::with_config(config);

        for i in 0..5 {
            handler.record_blocked(BlockedHistoryEntry {
                agent_id: AgentId::new(format!("agent-{}", i)),
                reason_type: "test".to_string(),
                description: "test".to_string(),
                duration_ms: i as u64 * 1000,
                resolved: i < 2, // First 2 are resolved
                resolution: None,
            });
        }

        // Should prune resolved entries first, keeping 3 unresolved
        assert_eq!(handler.history().len(), 3);
        // Remaining entries should all be unresolved (index 2, 3, 4)
        assert!(handler.history().iter().all(|e| !e.resolved));
    }

    #[test]
    fn blocked_handler_count_active() {
        let mut handler = BlockedHandler::new();
        handler.record_blocked(BlockedHistoryEntry {
            agent_id: AgentId::new("agent-001"),
            reason_type: "test".to_string(),
            description: "test".to_string(),
            duration_ms: 1000,
            resolved: false,
            resolution: None,
        });
        handler.record_blocked(BlockedHistoryEntry {
            agent_id: AgentId::new("agent-002"),
            reason_type: "test".to_string(),
            description: "test".to_string(),
            duration_ms: 1000,
            resolved: true,
            resolution: Some("resolved".to_string()),
        });
        assert_eq!(handler.count_active(), 1);
    }
}