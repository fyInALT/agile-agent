//! Multi-Agent Session for parallel agent runtime
//!
//! Provides the session structure for running multiple agents concurrently.
//! Each agent has its own transcript, session handle, and state, while sharing
//! the workplace's backlog, skills, and loop control.

use std::path::PathBuf;

use anyhow::Result;

use crate::agent_pool::AgentPool;
use crate::agent_runtime::{AgentId, WorkplaceId};
use crate::provider::ProviderKind;
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

        Self {
            workplace,
            agents,
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

    /// Get the focused agent index
    pub fn focused_index(&self) -> usize {
        self.focused_index
    }

    /// Check if any agent is busy
    pub fn any_busy(&self) -> bool {
        self.agents
            .agent_statuses()
            .iter()
            .any(|s| !s.status.is_idle())
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
        let agent_id = session.spawn_default_agent().expect("spawn");
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