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
use crate::logging;
use crate::provider::{ProviderEvent, ProviderKind};
use crate::shared_state::SharedWorkplaceState;
use crate::shutdown_snapshot::ShutdownSnapshot;
use crate::skills::SkillRegistry;
use crate::workplace_store::WorkplaceStore;

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

    /// Bootstrap a new multi-agent session
    ///
    /// Creates a fresh session with one agent, or restores from shutdown snapshot if available.
    pub fn bootstrap(
        cwd: PathBuf,
        default_provider: ProviderKind,
        resume_snapshot: bool,
        max_agents: usize,
    ) -> Result<Self> {
        let workplace = WorkplaceStore::for_cwd(&cwd)?;
        workplace.ensure()?;

        if resume_snapshot && let Ok(Some(snapshot)) = workplace.load_shutdown_snapshot() {
            crate::logging::debug_event(
                "multi_agent.bootstrap.restore",
                "restoring from shutdown snapshot",
                serde_json::json!({
                    "agents_count": snapshot.agents.len(),
                    "reason": snapshot.shutdown_reason,
                }),
            );
            return Self::restore_from_snapshot(cwd, snapshot, default_provider, max_agents);
        }

        // Create fresh session with one agent
        let workplace_id = workplace.workplace_id().clone();
        let mut session = Self::new(cwd, workplace_id, default_provider, max_agents);
        if let Err(e) = session.spawn_agent(default_provider) {
            return Err(anyhow::anyhow!("Failed to spawn initial agent: {}", e));
        }

        crate::logging::debug_event(
            "multi_agent.bootstrap.fresh",
            "created fresh multi-agent session",
            serde_json::json!({
                "workplace_id": session.workplace.workplace_id.as_str(),
                "agents_count": session.agents.active_count(),
            }),
        );

        Ok(session)
    }

    /// Restore a multi-agent session from shutdown snapshot
    ///
    /// Restores all agents from the snapshot, not just the first one.
    pub fn restore_from_snapshot(
        cwd: PathBuf,
        snapshot: ShutdownSnapshot,
        default_provider: ProviderKind,
        max_agents: usize,
    ) -> Result<Self> {
        let workplace = WorkplaceStore::for_cwd(&cwd)?;
        let workplace_id = workplace.workplace_id().clone();

        let skills = SkillRegistry::discover(&cwd);
        let mut workplace_state =
            SharedWorkplaceState::with_backlog(workplace_id.clone(), snapshot.backlog.clone());
        workplace_state.skills = skills;

        let mut agents = AgentPool::new(workplace_id.clone(), max_agents);

        // Restore all agents from snapshot
        for agent_snapshot in &snapshot.agents {
            // Get provider from agent meta, fall back to default if not convertible
            let provider = agent_snapshot
                .meta
                .provider_type
                .to_provider_kind()
                .unwrap_or(default_provider);

            match agents.spawn_agent(provider) {
                Ok(_) => {
                    // Get the spawned slot and restore its state
                    let last_index = agents.active_count() - 1;
                    if let Some(slot) = agents.get_slot_mut(last_index) {
                        // Set session handle from provider_session_id if available
                        if let Some(session_id) = &agent_snapshot.meta.provider_session_id {
                            let handle = match agent_snapshot.meta.provider_type {
                                crate::agent_runtime::ProviderType::Claude => {
                                    crate::provider::SessionHandle::ClaudeSession {
                                        session_id: session_id.as_str().to_string(),
                                    }
                                }
                                crate::agent_runtime::ProviderType::Codex => {
                                    crate::provider::SessionHandle::CodexThread {
                                        thread_id: session_id.as_str().to_string(),
                                    }
                                }
                                _ => {
                                    // Mock or Opencode - no session handle needed
                                    crate::provider::SessionHandle::ClaudeSession {
                                        session_id: session_id.as_str().to_string(),
                                    }
                                }
                            };
                            slot.set_session_handle(handle);
                        }

                        // Set assigned task if agent had one
                        if let Some(task_id) = &agent_snapshot.assigned_task_id {
                            let _ = slot.assign_task(TaskId::new(task_id));
                        }

                        // Set status based on was_active
                        if agent_snapshot.was_active {
                            // Agent was interrupted, mark as stopped
                            let _ = slot.transition_to(AgentSlotStatus::stopped(
                                "interrupted during execution",
                            ));
                        }
                    }
                }
                Err(e) => {
                    crate::logging::debug_event(
                        "multi_agent.restore.spawn_failed",
                        "failed to spawn agent from snapshot",
                        serde_json::json!({
                            "error": e,
                        }),
                    );
                }
            }
        }

        // Clear the snapshot after successful restore
        workplace.clear_shutdown_snapshot()?;

        crate::logging::debug_event(
            "multi_agent.restore.complete",
            "restored multi-agent session from snapshot",
            serde_json::json!({
                "agents_count": agents.active_count(),
                "workplace_id": workplace_id.as_str(),
            }),
        );

        Ok(Self {
            workplace: workplace_state,
            agents,
            event_aggregator: EventAggregator::new(),
            cwd,
            default_provider,
            focused_index: 0,
        })
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
        logging::debug_event(
            "session.agent.spawn_default",
            "spawning agent with default provider",
            serde_json::json!({
                "default_provider": format!("{:?}", self.default_provider),
                "current_agents": self.agents.active_count(),
            }),
        );
        self.agents.spawn_agent(self.default_provider)
    }

    /// Spawn a new agent with a specific provider
    pub fn spawn_agent(&mut self, provider: ProviderKind) -> Result<AgentId, String> {
        logging::debug_event(
            "session.agent.spawn",
            "spawning agent with specific provider",
            serde_json::json!({
                "provider": format!("{:?}", provider),
                "current_agents": self.agents.active_count(),
            }),
        );
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
        let old_index = self.focused_index;
        let count = self.agents.active_count();
        if count > 0 {
            self.focused_index = (self.focused_index + 1) % count;
            logging::debug_event(
                "session.focus.next",
                "focus switched to next agent",
                serde_json::json!({
                    "old_index": old_index,
                    "new_index": self.focused_index,
                    "agent_count": count,
                }),
            );
        }
    }

    /// Switch focus to the previous agent
    pub fn focus_previous(&mut self) {
        let old_index = self.focused_index;
        let count = self.agents.active_count();
        if count > 0 {
            self.focused_index = if self.focused_index == 0 {
                count - 1
            } else {
                self.focused_index - 1
            };
            logging::debug_event(
                "session.focus.previous",
                "focus switched to previous agent",
                serde_json::json!({
                    "old_index": old_index,
                    "new_index": self.focused_index,
                    "agent_count": count,
                }),
            );
        }
    }

    /// Switch focus to a specific agent by index
    pub fn focus_agent(&mut self, index: usize) -> bool {
        let old_index = self.focused_index;
        if index < self.agents.active_count() {
            self.focused_index = index;
            logging::debug_event(
                "session.focus.index",
                "focus switched to agent by index",
                serde_json::json!({
                    "old_index": old_index,
                    "new_index": index,
                    "success": true,
                }),
            );
            true
        } else {
            logging::debug_event(
                "session.focus.index",
                "focus switch by index failed - out of bounds",
                serde_json::json!({
                    "requested_index": index,
                    "agent_count": self.agents.active_count(),
                    "success": false,
                }),
            );
            false
        }
    }

    /// Switch focus to a specific agent by ID
    pub fn focus_agent_by_id(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let old_index = self.focused_index;
        logging::debug_event(
            "session.focus.id",
            "focus switch requested by agent ID",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "old_index": old_index,
            }),
        );
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
        logging::debug_event(
            "session.event.process",
            "processing agent event",
            serde_json::json!({
                "event_type": format!("{:?}", event),
                "agent_id": event.agent_id().as_str(),
            }),
        );

        match event {
            AgentEvent::FromProvider { agent_id, event } => {
                logging::debug_event(
                    "session.event.from_provider",
                    "processing provider event",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "provider_event": format!("{:?}", event),
                    }),
                );
                self.process_provider_event(&agent_id, event)?;
                Ok(true)
            }
            AgentEvent::StatusChanged {
                agent_id,
                new_status,
                ..
            } => {
                logging::debug_event(
                    "session.event.status_changed",
                    "agent status changed",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "new_status": new_status.label(),
                    }),
                );
                if let Some(slot) = self.agents.get_slot_mut_by_id(&agent_id) {
                    slot.transition_to(new_status)?;
                }
                Ok(true)
            }
            AgentEvent::TaskCompleted {
                agent_id,
                task_id,
                result,
            } => {
                logging::debug_event(
                    "session.event.task_completed",
                    "agent task completed",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                        "result": format!("{:?}", result),
                    }),
                );
                self.complete_task(&agent_id, &task_id, result)?;
                Ok(true)
            }
            AgentEvent::AgentError { agent_id, error } => {
                logging::debug_event(
                    "session.event.agent_error",
                    "agent encountered error",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": error,
                    }),
                );
                if let Some(slot) = self.agents.get_slot_mut_by_id(&agent_id) {
                    slot.transition_to(AgentSlotStatus::error(error))?;
                }
                Ok(true)
            }
            AgentEvent::ThreadFinished { agent_id, outcome } => {
                logging::debug_event(
                    "session.event.thread_finished",
                    "agent thread finished",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "outcome": format!("{:?}", outcome),
                    }),
                );
                self.handle_thread_finished(&agent_id, outcome)?;
                Ok(true)
            }
        }
    }

    /// Process a provider event for a specific agent
    fn process_provider_event(
        &mut self,
        agent_id: &AgentId,
        event: ProviderEvent,
    ) -> Result<(), String> {
        let slot = self
            .agents
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found", agent_id.as_str()))?;

        match event {
            ProviderEvent::SessionHandle(handle) => {
                logging::debug_event(
                    "agent.session_handle",
                    "session handle received",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "handle_type": format!("{:?}", handle),
                    }),
                );
                slot.set_session_handle(handle);
                Ok(())
            }
            ProviderEvent::Status(text) => {
                logging::debug_event(
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
                logging::debug_event(
                    "agent.finished",
                    "agent finished processing",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                    }),
                );
                slot.transition_to(AgentSlotStatus::finishing())?;
                Ok(())
            }
            ProviderEvent::Error(error) => {
                logging::debug_event(
                    "agent.error",
                    "agent encountered error",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": error,
                    }),
                );
                slot.append_transcript(crate::app::TranscriptEntry::Error(error.clone()));
                slot.transition_to(AgentSlotStatus::error(error))?;
                Ok(())
            }
            // Tool events - log for debugging
            ProviderEvent::ExecCommandStarted {
                call_id,
                input_preview,
                source,
            } => {
                logging::debug_event(
                    "tool.exec.started",
                    "exec command started",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "input_preview": input_preview,
                        "source": source,
                    }),
                );
                Ok(())
            }
            ProviderEvent::ExecCommandFinished {
                call_id,
                output_preview: _,
                status,
                exit_code,
                duration_ms,
                source,
            } => {
                logging::debug_event(
                    "tool.exec.finished",
                    "exec command finished",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "exit_code": exit_code,
                        "duration_ms": duration_ms,
                        "status": format!("{:?}", status),
                        "source": source,
                    }),
                );
                Ok(())
            }
            ProviderEvent::GenericToolCallStarted {
                name,
                call_id,
                input_preview: _,
            } => {
                logging::debug_event(
                    "tool.generic.started",
                    "generic tool call started",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "tool_name": name,
                        "call_id": call_id,
                    }),
                );
                Ok(())
            }
            ProviderEvent::GenericToolCallFinished {
                name,
                call_id,
                output_preview: _,
                success,
                exit_code,
                duration_ms,
            } => {
                logging::debug_event(
                    "tool.generic.finished",
                    "generic tool call finished",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "tool_name": name,
                        "call_id": call_id,
                        "success": success,
                        "exit_code": exit_code,
                        "duration_ms": duration_ms,
                    }),
                );
                Ok(())
            }
            ProviderEvent::McpToolCallStarted {
                call_id,
                invocation,
            } => {
                logging::debug_event(
                    "tool.mcp.started",
                    "MCP tool call started",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "tool_name": invocation.tool,
                        "server": invocation.server,
                    }),
                );
                Ok(())
            }
            ProviderEvent::McpToolCallFinished {
                call_id,
                invocation,
                result_blocks: _,
                error: _,
                status,
                is_error,
            } => {
                logging::debug_event(
                    "tool.mcp.finished",
                    "MCP tool call finished",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "tool_name": invocation.tool,
                        "server": invocation.server,
                        "is_error": is_error,
                        "status": format!("{:?}", status),
                    }),
                );
                Ok(())
            }
            ProviderEvent::WebSearchStarted { call_id, query } => {
                logging::debug_event(
                    "tool.websearch.started",
                    "web search started",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "query": query,
                    }),
                );
                Ok(())
            }
            ProviderEvent::WebSearchFinished {
                call_id,
                query,
                action: _,
            } => {
                logging::debug_event(
                    "tool.websearch.finished",
                    "web search finished",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "call_id": call_id,
                        "query": query,
                    }),
                );
                Ok(())
            }
            // Chunk and tool events handled by TuiState (streaming buffers)
            _ => Ok(()),
        }
    }

    /// Complete a task for an agent
    fn complete_task(
        &mut self,
        agent_id: &AgentId,
        _task_id: &TaskId,
        result: TaskCompletionResult,
    ) -> Result<(), String> {
        self.agents
            .complete_task_with_backlog(agent_id, result, &mut self.workplace.backlog)?;
        Ok(())
    }

    /// Handle thread finished event
    fn handle_thread_finished(
        &mut self,
        agent_id: &AgentId,
        outcome: crate::agent_slot::ThreadOutcome,
    ) -> Result<(), String> {
        let slot = self
            .agents
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found", agent_id.as_str()))?;

        logging::debug_event(
            "thread.finished.handling",
            "handling thread finish with outcome",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "outcome": format!("{:?}", outcome),
            }),
        );

        slot.clear_provider_thread();

        match outcome {
            crate::agent_slot::ThreadOutcome::NormalExit => {
                logging::debug_event(
                    "thread.finished.normal",
                    "provider thread exited normally",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                    }),
                );
                slot.transition_to(AgentSlotStatus::idle())?;
            }
            crate::agent_slot::ThreadOutcome::ErrorExit { error } => {
                logging::debug_event(
                    "thread.finished.error",
                    "provider thread exited with error",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": error,
                    }),
                );
                slot.transition_to(AgentSlotStatus::error(error))?;
            }
            crate::agent_slot::ThreadOutcome::Cancelled => {
                logging::debug_event(
                    "thread.finished.cancelled",
                    "provider thread was cancelled",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                    }),
                );
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

    #[test]
    fn bootstrap_creates_fresh_session_with_one_agent() {
        use tempfile::TempDir;

        let temp = TempDir::new().expect("tempdir");
        let session = MultiAgentSession::bootstrap(
            temp.path().to_path_buf(),
            ProviderKind::Mock,
            false, // No resume
            10,
        )
        .expect("bootstrap");

        assert_eq!(session.agents.active_count(), 1);
        assert!(session.agents.focused_slot().is_some());
    }

    #[test]
    fn restore_from_snapshot_restores_all_agents() {
        use crate::agent_runtime::{AgentCodename, AgentId, AgentMeta, AgentStatus, ProviderType};
        use crate::shutdown_snapshot::{AgentShutdownSnapshot, ShutdownReason, ShutdownSnapshot};
        use tempfile::TempDir;

        let temp = TempDir::new().expect("tempdir");

        // Create snapshot with multiple agents
        let agent1 = AgentShutdownSnapshot {
            meta: AgentMeta {
                agent_id: AgentId::new("agent_001"),
                codename: AgentCodename::new("alpha"),
                workplace_id: WorkplaceId::new("test"),
                provider_type: ProviderType::Mock,
                provider_session_id: None,
                created_at: "2026-04-14T00:00:00Z".to_string(),
                updated_at: "2026-04-14T00:00:00Z".to_string(),
                status: AgentStatus::Idle,
            },
            assigned_task_id: Some("task-001".to_string()),
            was_active: false,
            had_error: false,
            provider_thread_state: None,
            captured_at: "2026-04-14T00:00:00Z".to_string(),
        };

        let agent2 = AgentShutdownSnapshot {
            meta: AgentMeta {
                agent_id: AgentId::new("agent_002"),
                codename: AgentCodename::new("bravo"),
                workplace_id: WorkplaceId::new("test"),
                provider_type: ProviderType::Claude,
                provider_session_id: Some(crate::agent_runtime::ProviderSessionId::new("sess-123")),
                created_at: "2026-04-14T00:00:00Z".to_string(),
                updated_at: "2026-04-14T00:00:00Z".to_string(),
                status: AgentStatus::Running,
            },
            assigned_task_id: None,
            was_active: true,
            had_error: false,
            provider_thread_state: None,
            captured_at: "2026-04-14T00:00:00Z".to_string(),
        };

        let snapshot = ShutdownSnapshot {
            workplace_id: "test".to_string(),
            backlog: crate::backlog::BacklogState::default(),
            agents: vec![agent1, agent2],
            pending_mail: vec![], // no pending mail in test
            shutdown_reason: ShutdownReason::UserQuit,
            shutdown_at: "2026-04-14T00:00:00Z".to_string(),
        };

        // Restore session from snapshot
        let session = MultiAgentSession::restore_from_snapshot(
            temp.path().to_path_buf(),
            snapshot,
            ProviderKind::Mock,
            10,
        )
        .expect("restore");

        // Should have restored both agents
        assert_eq!(session.agents.active_count(), 2);

        // Check agent states
        let statuses = session.agents.agent_statuses();
        assert_eq!(statuses[0].codename.as_str(), "alpha");
        assert_eq!(statuses[0].provider_type, ProviderType::Mock);
        assert!(statuses[0].assigned_task_id.is_some());

        assert_eq!(statuses[1].codename.as_str(), "bravo");
        assert_eq!(statuses[1].provider_type, ProviderType::Claude);
    }
}
