//! Decision agent coordinator for managing decision layer state
//!
//! Provides DecisionAgentCoordinator struct that manages decision agent
//! slots, mail senders, and decision layer components.

use std::collections::HashMap;

use crate::agent_runtime::AgentId;
use crate::decision_agent_slot::{DecisionAgentSlot, DecisionAgentStatus};
use crate::decision_mail::DecisionMailSender;
use agent_decision::initializer::DecisionLayerComponents;

/// Stats about decision agent activity
#[derive(Debug, Clone, Default)]
pub struct DecisionAgentStats {
    /// Total number of decision agents
    pub total_agents: usize,
    /// Total decisions made across all agents
    pub total_decisions: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Number of idle decision agents
    pub idle_agents: usize,
    /// Number of thinking decision agents
    pub thinking_agents: usize,
    /// Number of responding decision agents
    pub responding_agents: usize,
    /// Number of decision agents with errors
    pub error_agents: usize,
    /// Number of stopped decision agents
    pub stopped_agents: usize,
}

/// Coordinator for decision agent management
///
/// Manages decision agent slots, mail senders, and decision layer components.
/// Used as a delegate within AgentPool for decision agent state operations.
pub struct DecisionAgentCoordinator {
    /// Decision agent slots keyed by work agent ID
    agents: HashMap<AgentId, DecisionAgentSlot>,
    /// Decision mail senders keyed by work agent ID
    mail_senders: HashMap<AgentId, DecisionMailSender>,
    /// Decision layer components (classifiers, actions, etc.)
    components: DecisionLayerComponents,
}

impl std::fmt::Debug for DecisionAgentCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionAgentCoordinator")
            .field("agents_count", &self.agents.len())
            .field("mail_senders_count", &self.mail_senders.len())
            .field("components", &"<DecisionLayerComponents>")
            .finish()
    }
}

impl DecisionAgentCoordinator {
    /// Create a new coordinator with initialized decision layer
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            mail_senders: HashMap::new(),
            components: agent_decision::initializer::initialize_decision_layer(),
        }
    }

    /// Get decision components reference
    pub fn components(&self) -> &DecisionLayerComponents {
        &self.components
    }

    /// Check if decision agent exists for work agent
    pub fn has_agent(&self, work_agent_id: &AgentId) -> bool {
        self.agents.contains_key(work_agent_id)
    }

    /// Get decision agent for work agent
    pub fn agent_for(&self, work_agent_id: &AgentId) -> Option<&DecisionAgentSlot> {
        self.agents.get(work_agent_id)
    }

    /// Get mutable decision agent for work agent
    pub fn agent_mut_for(&mut self, work_agent_id: &AgentId) -> Option<&mut DecisionAgentSlot> {
        self.agents.get_mut(work_agent_id)
    }

    /// Get mail sender for work agent
    pub fn mail_sender_for(&self, work_agent_id: &AgentId) -> Option<&DecisionMailSender> {
        self.mail_senders.get(work_agent_id)
    }

    /// Insert a decision agent for work agent
    pub fn insert_agent(&mut self, work_agent_id: AgentId, agent: DecisionAgentSlot) {
        self.agents.insert(work_agent_id.clone(), agent);
    }

    /// Insert a mail sender for work agent
    pub fn insert_mail_sender(&mut self, work_agent_id: AgentId, sender: DecisionMailSender) {
        self.mail_senders.insert(work_agent_id, sender);
    }

    /// Remove decision agent and mail sender for work agent
    pub fn remove_agent(&mut self, work_agent_id: &AgentId) -> Option<DecisionAgentSlot> {
        self.mail_senders.remove(work_agent_id);
        self.agents.remove(work_agent_id)
    }

    /// Iterate over all decision agents
    pub fn agents_iter(&self) -> impl Iterator<Item = (&AgentId, &DecisionAgentSlot)> {
        self.agents.iter()
    }

    /// Iterate mutably over all decision agents
    pub fn agents_iter_mut(&mut self) -> impl Iterator<Item = (&AgentId, &mut DecisionAgentSlot)> {
        self.agents.iter_mut()
    }

    /// Iterate over all mail senders references
    pub fn mail_senders_iter(&self) -> impl Iterator<Item = (&AgentId, &DecisionMailSender)> {
        self.mail_senders.iter()
    }

    /// Get mutable mail sender for work agent
    pub fn mail_sender_mut_for(&mut self, work_agent_id: &AgentId) -> Option<&mut DecisionMailSender> {
        self.mail_senders.get_mut(work_agent_id)
    }

    /// Get all work agent IDs that have decision agents
    pub fn work_agent_ids(&self) -> Vec<AgentId> {
        self.agents.keys().cloned().collect()
    }

    /// Process each decision agent with its mail sender
    ///
    /// This method allows safe access to both mutable agent and immutable mail sender
    /// by processing one agent at a time.
    pub fn for_each_agent_with_mail_sender<F>(&mut self, mut f: F)
    where
        F: FnMut(&AgentId, &mut DecisionAgentSlot, Option<&DecisionMailSender>),
    {
        // Collect keys first to avoid borrow conflicts
        let keys: Vec<AgentId> = self.agents.keys().cloned().collect();

        for key in keys {
            if let Some(agent) = self.agents.get_mut(&key) {
                let sender = self.mail_senders.get(&key);
                f(&key, agent, sender);
            }
        }
    }

    /// Process each decision agent mutably and then cleanup
    pub fn for_each_agent_mut<F>(&mut self, f: F)
    where
        F: Fn(&AgentId, &mut DecisionAgentSlot),
    {
        for (id, agent) in self.agents.iter_mut() {
            f(id, agent);
        }
    }

    /// Count decision agents
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Compute decision agent stats
    pub fn stats(&self) -> DecisionAgentStats {
        let mut stats = DecisionAgentStats::default();
        for (_, agent) in &self.agents {
            stats.total_agents += 1;
            stats.total_decisions += agent.decision_count();
            stats.total_errors += agent.error_count();
            match agent.status() {
                DecisionAgentStatus::Idle => stats.idle_agents += 1,
                DecisionAgentStatus::Thinking { .. } => stats.thinking_agents += 1,
                DecisionAgentStatus::Responding => stats.responding_agents += 1,
                DecisionAgentStatus::Error { .. } => stats.error_agents += 1,
                DecisionAgentStatus::Stopped { .. } => stats.stopped_agents += 1,
            }
        }
        stats
    }

    /// Find agents with pending decisions (for timeout tracking)
    pub fn agents_with_pending_decisions(&self) -> Vec<(AgentId, std::time::Instant)> {
        self.agents
            .iter()
            .filter_map(|(work_agent_id, agent)| {
                if agent.status().is_thinking() {
                    agent.last_decision_started_at()
                        .map(|started_at| (work_agent_id.clone(), started_at))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find agents approaching timeout
    pub fn agents_approaching_timeout(&self, threshold_ms: u64) -> Vec<(AgentId, std::time::Instant)> {
        self.agents
            .iter()
            .filter_map(|(work_agent_id, agent)| {
                if let Some(started_at) = agent.last_decision_started_at() {
                    let elapsed = started_at.elapsed().as_millis() as u64;
                    if elapsed >= threshold_ms {
                        return Some((work_agent_id.clone(), started_at));
                    }
                }
                None
            })
            .collect()
    }

    /// Reset all error state decision agents
    pub fn reset_all_errors(&mut self) {
        for (_, agent) in self.agents.iter_mut() {
            if agent.status().has_error() {
                agent.reset_error();
            }
        }
    }

    // ===== Test helpers (pub for testing) =====

    /// Get mail sender for work agent (pub for test access)
    pub fn mail_sender_for_test(&self, work_agent_id: &AgentId) -> Option<&DecisionMailSender> {
        self.mail_senders.get(work_agent_id)
    }

    /// Get mutable decision agent for work agent (pub for test access)
    pub fn agent_mut_for_test(&mut self, work_agent_id: &AgentId) -> Option<&mut DecisionAgentSlot> {
        self.agents.get_mut(work_agent_id)
    }

    /// Get decision agent for work agent (pub for test access)
    pub fn agent_for_test(&self, work_agent_id: &AgentId) -> Option<&DecisionAgentSlot> {
        self.agents.get(work_agent_id)
    }
}

impl Default for DecisionAgentCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision_mail::DecisionMail;
    use crate::ProviderKind;

    fn make_test_agent(work_agent_id: &str) -> DecisionAgentSlot {
        let mail = DecisionMail::new();
        let (_, receiver) = mail.split();
        DecisionAgentSlot::new(
            work_agent_id.to_string(),
            ProviderKind::Mock,
            receiver,
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            &agent_decision::initializer::initialize_decision_layer(),
        )
    }

    #[test]
    fn coordinator_new_is_empty() {
        let coord = DecisionAgentCoordinator::new();
        assert_eq!(coord.agent_count(), 0);
        assert!(!coord.has_agent(&AgentId::new("test")));
    }

    #[test]
    fn insert_and_get_agent() {
        let mut coord = DecisionAgentCoordinator::new();
        let agent_id = AgentId::new("work-001");
        let agent = make_test_agent("work-001");

        coord.insert_agent(agent_id.clone(), agent);
        assert!(coord.has_agent(&agent_id));
        assert!(coord.agent_for(&agent_id).is_some());
    }

    #[test]
    fn insert_and_get_mail_sender() {
        let mut coord = DecisionAgentCoordinator::new();
        let agent_id = AgentId::new("work-001");
        let mail = DecisionMail::new();
        let (sender, _) = mail.split();

        coord.insert_mail_sender(agent_id.clone(), sender);
        assert!(coord.mail_sender_for(&agent_id).is_some());
    }

    #[test]
    fn remove_agent() {
        let mut coord = DecisionAgentCoordinator::new();
        let agent_id = AgentId::new("work-001");
        let agent = make_test_agent("work-001");
        let mail = DecisionMail::new();
        let (sender, _) = mail.split();

        coord.insert_agent(agent_id.clone(), agent);
        coord.insert_mail_sender(agent_id.clone(), sender);
        assert!(coord.has_agent(&agent_id));

        coord.remove_agent(&agent_id);
        assert!(!coord.has_agent(&agent_id));
        assert!(coord.mail_sender_for(&agent_id).is_none());
    }

    #[test]
    fn stats_empty() {
        let coord = DecisionAgentCoordinator::new();
        let stats = coord.stats();
        assert_eq!(stats.total_decisions, 0);
        assert_eq!(stats.idle_agents, 0);
    }

    #[test]
    fn stats_after_insert() {
        let mut coord = DecisionAgentCoordinator::new();
        coord.insert_agent(AgentId::new("work-001"), make_test_agent("work-001"));
        let stats = coord.stats();
        assert_eq!(stats.idle_agents, 1);
    }

    #[test]
    fn agents_with_pending_decisions_empty_initially() {
        let coord = DecisionAgentCoordinator::new();
        let pending = coord.agents_with_pending_decisions();
        assert!(pending.is_empty());
    }

    #[test]
    fn reset_all_errors_no_effect_on_idle() {
        let mut coord = DecisionAgentCoordinator::new();
        coord.insert_agent(AgentId::new("work-001"), make_test_agent("work-001"));
        coord.reset_all_errors();
        // Should not panic, idle agents stay idle
        let agent = coord.agent_for(&AgentId::new("work-001")).unwrap();
        assert!(agent.status().is_idle());
    }
}