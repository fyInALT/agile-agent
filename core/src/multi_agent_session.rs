//! Multi-Agent Session for parallel agent runtime
//!
//! Provides the session structure for running multiple agents concurrently.
//! Each agent has its own transcript, session handle, and state, while sharing
//! the workplace's backlog, skills, and loop control.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::agent_pool::AgentPool;
use crate::agent_runtime::{AgentId, WorkplaceId};
use crate::agent_slot::{AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::event_aggregator::{AgentEvent, EventAggregator, PollResult};
use crate::provider::{ProviderEvent, ProviderKind};
use crate::shared_state::SharedWorkplaceState;
use crate::skills::SkillRegistry;

/// Multi-agent runtime session
///
/// Contains the shared workplace state and agent pool for managing
/// multiple concurrent agents.
pub struct MultiAgentSession {
    /// Shared workplace state (backlog, skills, loop control)
    pub workplace: SharedWorkplaceState,
    /// Agent pool managing all agent slots
    pub agents: AgentPool,
    /// Event aggregator for polling all agent channels
    pub event_aggregator: EventAggregator,
    /// Launch working directory
    pub cwd: PathBuf,
    /// Default provider for new agents
    pub default_provider: ProviderKind,
    /// Index of the focused agent (for TUI)
    pub focused_index: usize,
}

impl MultiAgentSession {
    /// Create a new multi-agent session
    pub fn new(
        cwd: PathBuf,
        workplace_id: WorkplaceId,
        default_provider: ProviderKind,
        max_agents: usize,
    ) -> Self {
        let skills = SkillRegistry::discover(&cwd);
        let workplace = SharedWorkplaceState::with_skills(workplace_id, skills);
        let agents = AgentPool::new(workplace.workplace_id.clone(), max_agents);
        let event_aggregator = EventAggregator::new();

        Self {
            workplace,
            agents,
            event_aggregator,
            cwd,
            default_provider,
            focused_index: 0,
        }
    }

    /// Get workplace reference
    pub fn workplace(&self) -> &SharedWorkplaceState {
        &self.workplace
    }

    /// Get workplace mutable reference
    pub fn workplace_mut(&mut self) -> &mut SharedWorkplaceState {
        &mut self.workplace
    }

    /// Get agent pool reference
    pub fn agents(&self) -> &AgentPool {
        &self.agents
    }

    /// Get agent pool mutable reference
    pub fn agents_mut(&mut self) -> &mut AgentPool {
        &mut self.agents
    }

    /// Get event aggregator reference
    pub fn event_aggregator(&self) -> &EventAggregator {
        &self.event_aggregator
    }

    /// Get event aggregator mutable reference
    pub fn event_aggregator_mut(&mut self) -> &mut EventAggregator {
        &mut self.event_aggregator
    }

    /// Spawn a new agent with the default provider
    pub fn spawn_default_agent(&mut self) -> Result<AgentId, String> {
        self.agents.spawn_agent(self.default_provider)
    }

    /// Spawn a new agent with a specific provider
    pub fn spawn_agent(&mut self, provider: ProviderKind) -> Result<AgentId, String> {
        self.agents.spawn_agent(provider)
    }

    /// Get the focused agent ID
    pub fn focused_agent_id(&self) -> Option<AgentId> {
        self.agents
            .agent_statuses()
            .get(self.focused_index)
            .map(|s| s.agent_id.clone())
    }

    /// Get focused agent slot
    pub fn focused_slot(&self) -> Option<&crate::agent_slot::AgentSlot> {
        self.agents.focused_slot()
    }

    /// Get focused agent slot (mutable)
    pub fn focused_slot_mut(&mut self) -> Option<&mut crate::agent_slot::AgentSlot> {
        self.agents.focused_slot_mut()
    }

    /// Get focused agent's transcript
    pub fn focused_transcript(&self) -> Option<&[crate::app::TranscriptEntry]> {
        self.agents.focused_slot().map(|s| s.transcript())
    }

    /// Switch focus to the next agent
    pub fn focus_next(&mut self) {
        let count = self.agents.active_count();
        if count > 0 {
            self.focused_index = (self.focused_index + 1) % count;
        }
    }

    /// Switch focus to the previous agent
    pub fn focus_previous(&mut self) {
        let count = self.agents.active_count();
        if count > 0 {
            self.focused_index = if self.focused_index == 0 {
                count - 1
            } else {
                self.focused_index - 1
            };
        }
    }

    /// Switch focus to a specific agent by index
    pub fn focus_agent(&mut self, index: usize) -> bool {
        if index < self.agents.active_count() {
            self.focused_index = index;
            true
        } else {
            false
        }
    }

    /// Switch focus to a specific agent by ID
    pub fn focus_agent_by_id(&mut self, agent_id: &AgentId) -> Result<(), String> {
        self.agents.focus_agent(agent_id)
    }

    /// Get the focused agent index
    pub fn focused_index(&self) -> usize {
        self.focused_index
    }

    /// Check if any agent is busy (not idle)
    pub fn any_busy(&self) -> bool {
        self.agents
            .agent_statuses()
            .iter()
            .any(|s| !s.status.is_idle())
    }

    /// Check if any agent is active (responding or executing)
    pub fn any_active(&self) -> bool {
        self.agents.has_active_agents()
    }

    /// Poll all agent events without blocking
    pub fn poll_events(&self) -> PollResult {
        self.event_aggregator.poll_all()
    }

    /// Poll all agent events with timeout
    pub fn poll_events_with_timeout(&self, timeout: Duration) -> PollResult {
        self.event_aggregator.poll_with_timeout(timeout)
    }

    /// Process an agent event, routing to the correct slot
    ///
    /// Returns true if the event was processed successfully.
    pub fn process_event(&mut self, event: AgentEvent) -> Result<bool, String> {
        match event {
            AgentEvent::FromProvider { agent_id, event } => {
                self.process_provider_event(&agent_id, event)?;
                Ok(true)
            }
            AgentEvent::StatusChanged { agent_id, new_status, .. } => {
                if let Some(slot) = self.agents.get_slot_mut_by_id(&agent_id) {
                    slot.transition_to(new_status)?;
                }
                Ok(true)
            }
            AgentEvent::TaskCompleted { agent_id, task_id, result } => {
                self.complete_task(&agent_id, &task_id, result)?;
                Ok(true)
            }
            AgentEvent::AgentError { agent_id, error } => {
                if let Some(slot) = self.agents.get_slot_mut_by_id(&agent_id) {
                    slot.transition_to(AgentSlotStatus::error(error))?;
                }
                Ok(true)
            }
            AgentEvent::ThreadFinished { agent_id, outcome } => {
                self.handle_thread_finished(&agent_id, outcome)?;
                Ok(true)
            }
        }
    }

    /// Process a provider event for a specific agent
    fn process_provider_event(&mut self, agent_id: &AgentId, event: ProviderEvent) -> Result<(), String> {
        let slot = self.agents.get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found", agent_id.as_str()))?;

        match event {
            ProviderEvent::SessionHandle(handle) => {
                slot.set_session_handle(handle);
                Ok(())
            }
            ProviderEvent::Status(text) => {
                // Log status, don't add to transcript
                crate::logging::debug_event(
                    "agent.status",
                    "agent status message",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "status": text,
                    }),
                );
                Ok(())
            }
            ProviderEvent::Finished => {
                slot.transition_to(AgentSlotStatus::finishing())?;
                Ok(())
            }
            ProviderEvent::Error(error) => {
                slot.append_transcript(crate::app::TranscriptEntry::Error(error.clone()));
                slot.transition_to(AgentSlotStatus::error(error))?;
                Ok(())
            }
            // Chunk and tool events handled by TuiState (streaming buffers)
            _ => Ok(()),
        }
    }

    /// Complete a task for an agent
    fn complete_task(&mut self, agent_id: &AgentId, _task_id: &TaskId, result: TaskCompletionResult) -> Result<(), String> {
        self.agents.complete_task_with_backlog(agent_id, result, &mut self.workplace.backlog)?;
        Ok(())
    }

    /// Handle thread finished event
    fn handle_thread_finished(&mut self, agent_id: &AgentId, outcome: crate::agent_slot::ThreadOutcome) -> Result<(), String> {
        let slot = self.agents.get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found", agent_id.as_str()))?;

        slot.clear_provider_thread();

        match outcome {
            crate::agent_slot::ThreadOutcome::NormalExit => {
                slot.transition_to(AgentSlotStatus::idle())?;
            }
            crate::agent_slot::ThreadOutcome::ErrorExit { error } => {
                slot.transition_to(AgentSlotStatus::error(error))?;
            }
            crate::agent_slot::ThreadOutcome::Cancelled => {
                slot.transition_to(AgentSlotStatus::stopped("cancelled"))?;
            }
        }

        // Remove from event aggregator
        self.event_aggregator.remove_receiver(agent_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::WorkplaceId;

    #[test]
    fn new_session_has_empty_agent_pool() {
        let session = MultiAgentSession::new(
            PathBuf::from("."),
            WorkplaceId::new("test"),
            ProviderKind::Mock,
            5,
        );
        assert_eq!(session.agents.active_count(), 0);
        assert!(session.agents.can_spawn());
    }

    #[test]
    fn spawn_default_agent_adds_to_pool() {
        let mut session = MultiAgentSession::new(
            PathBuf::from("."),
            WorkplaceId::new("test"),
            ProviderKind::Mock,
            5,
        );
        let _agent_id = session.spawn_default_agent().expect("spawn");
        assert_eq!(session.agents.active_count(), 1);
        assert!(session.focused_agent_id().is_some());
    }

    #[test]
    fn focus_next_cycles_agents() {
        let mut session = MultiAgentSession::new(
            PathBuf::from("."),
            WorkplaceId::new("test"),
            ProviderKind::Mock,
            5,
        );
        session.spawn_agent(ProviderKind::Mock).expect("spawn");
        session.spawn_agent(ProviderKind::Claude).expect("spawn");
        session.spawn_agent(ProviderKind::Codex).expect("spawn");

        assert_eq!(session.focused_index(), 0);
        session.focus_next();
        assert_eq!(session.focused_index(), 1);
        session.focus_next();
        assert_eq!(session.focused_index(), 2);
        session.focus_next();
        assert_eq!(session.focused_index(), 0);
    }

    #[test]
    fn focus_previous_cycles_agents() {
        let mut session = MultiAgentSession::new(
            PathBuf::from("."),
            WorkplaceId::new("test"),
            ProviderKind::Mock,
            5,
        );
        session.spawn_agent(ProviderKind::Mock).expect("spawn");
        session.spawn_agent(ProviderKind::Claude).expect("spawn");
        session.spawn_agent(ProviderKind::Codex).expect("spawn");

        assert_eq!(session.focused_index(), 0);
        session.focus_previous();
        assert_eq!(session.focused_index(), 2);
        session.focus_previous();
        assert_eq!(session.focused_index(), 1);
    }

    #[test]
    fn focus_agent_by_index() {
        let mut session = MultiAgentSession::new(
            PathBuf::from("."),
            WorkplaceId::new("test"),
            ProviderKind::Mock,
            5,
        );
        session.spawn_agent(ProviderKind::Mock).expect("spawn");
        session.spawn_agent(ProviderKind::Claude).expect("spawn");

        assert!(session.focus_agent(1));
        assert_eq!(session.focused_index(), 1);
        assert!(!session.focus_agent(5)); // Out of bounds
    }
}