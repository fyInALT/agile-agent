//! Worker aggregate root — single authority over all mutable state for one agent.
//!
//! The Worker encapsulates the domain model for a single agent's lifecycle.
//! It is intentionally separate from thread/IO concerns (which stay in AgentSlot).
//!
//! All state changes go through `apply(event)`, which validates transitions
//! and maintains invariants. This makes the handwritten event loop honest:
//! instead of ad-hoc mutations scattered across SessionManager, there is
//! a single method with a clear contract.

use std::path::PathBuf;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId};
use crate::transcript_journal::{JournalEntry, TranscriptJournal};
use crate::worker_state::{InvalidTransition, WorkerState};
use agent_events::DomainEvent;
use agent_types::TaskId;

/// Error when applying an event to a Worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerError {
    /// State transition is not allowed
    InvalidTransition(InvalidTransition),
    /// Event is not applicable in the current state
    InvalidEventForState {
        event: &'static str,
        state: WorkerState,
    },
    /// Invariant violated (should never happen in production)
    InvariantViolation(String),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerError::InvalidTransition(e) => write!(f, "{}", e),
            WorkerError::InvalidEventForState { event, state } => {
                write!(f, "event {} not valid in state {:?}", event, state)
            }
            WorkerError::InvariantViolation(msg) => write!(f, "invariant violation: {}", msg),
        }
    }
}

impl std::error::Error for WorkerError {}

impl From<InvalidTransition> for WorkerError {
    fn from(e: InvalidTransition) -> Self {
        WorkerError::InvalidTransition(e)
    }
}

/// Worker aggregate root — encapsulates all mutable domain state for one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worker {
    /// Unique agent identifier
    agent_id: AgentId,
    /// Agent display codename
    codename: AgentCodename,
    /// Agent role
    role: AgentRole,
    /// Current domain state
    state: WorkerState,
    /// Structured event transcript
    transcript: TranscriptJournal,
    /// Currently assigned task (if any)
    assigned_task_id: Option<TaskId>,
    /// Worktree path (if using independent worktree)
    worktree_path: Option<PathBuf>,
    /// Worktree branch name
    worktree_branch: Option<String>,
}

impl Worker {
    /// Create a new Worker in Idle state.
    pub fn new(agent_id: AgentId, codename: AgentCodename, role: AgentRole) -> Self {
        Self {
            agent_id,
            codename,
            role,
            state: WorkerState::idle(),
            transcript: TranscriptJournal::new(),
            assigned_task_id: None,
            worktree_path: None,
            worktree_branch: None,
        }
    }

    /// Apply a domain event to this worker.
    ///
    /// This is the single entry point for all state mutations.
    /// Rules:
    /// - Events that imply state changes trigger validated transitions
    /// - Events that only append to transcript do not change state
    /// - Invalid transitions return `WorkerError::InvalidTransition`
    /// - Events that don't make sense in the current state return
    ///   `WorkerError::InvalidEventForState`
    pub fn apply(&mut self, event: DomainEvent) -> Result<(), WorkerError> {
        // Record the event in transcript first (always succeeds)
        self.transcript.append(event.clone());

        // Then handle state transitions based on event type
        match &event {
            // ── Lifecycle events ──────────────────────────────────
            DomainEvent::Status(text) if text == "worker started" => {
                self.state = self.state.transition_to(WorkerState::starting())?;
            }
            DomainEvent::Finished => {
                self.state = self.state.transition_to(WorkerState::completed())?;
            }
            DomainEvent::Error(msg) => {
                self.state = self
                    .state
                    .transition_to(WorkerState::failed(msg.clone()))?;
            }

            // ── Streaming events ──────────────────────────────────
            DomainEvent::AssistantChunk(_) | DomainEvent::ThinkingChunk(_) => {
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }
            DomainEvent::Status(_) => {
                // Generic status updates don't change state
            }

            // ── Tool execution ────────────────────────────────────
            DomainEvent::ExecCommandStarted { .. } => {
                self.state = self
                    .state
                    .transition_to(WorkerState::processing_tool("exec"))?;
            }
            DomainEvent::ExecCommandFinished { .. } => {
                // Return to responding after tool finishes
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }
            DomainEvent::ExecCommandOutputDelta { .. } => {
                // Delta events don't change state
            }

            // ── Generic tool calls ────────────────────────────────
            DomainEvent::GenericToolCallStarted { name, .. } => {
                self.state = self
                    .state
                    .transition_to(WorkerState::processing_tool(name.clone()))?;
            }
            DomainEvent::GenericToolCallFinished { .. } => {
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }

            // ── Web search ────────────────────────────────────────
            DomainEvent::WebSearchStarted { .. } => {
                self.state = self
                    .state
                    .transition_to(WorkerState::processing_tool("websearch"))?;
            }
            DomainEvent::WebSearchFinished { .. } => {
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }

            // ── Images ────────────────────────────────────────────
            DomainEvent::ViewImage { .. } | DomainEvent::ImageGenerationFinished { .. } => {
                // Image events don't change state
            }

            // ── MCP ───────────────────────────────────────────────
            DomainEvent::McpToolCallStarted { .. } => {
                self.state = self
                    .state
                    .transition_to(WorkerState::processing_tool("mcp"))?;
            }
            DomainEvent::McpToolCallFinished { .. } => {
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }

            // ── Patch apply ───────────────────────────────────────
            DomainEvent::PatchApplyStarted { .. } => {
                self.state = self
                    .state
                    .transition_to(WorkerState::processing_tool("patch"))?;
            }
            DomainEvent::PatchApplyFinished { .. } => {
                if !matches!(self.state, WorkerState::Responding { .. }) {
                    self.state = self.state.transition_to(WorkerState::responding_streaming())?;
                }
            }
            DomainEvent::PatchApplyOutputDelta { .. } => {
                // Delta events don't change state
            }

            // ── Session handle ────────────────────────────────────
            DomainEvent::SessionHandle(_) => {
                // Session acquisition doesn't change worker state
            }
        }

        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> &WorkerState {
        &self.state
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get codename
    pub fn codename(&self) -> &AgentCodename {
        &self.codename
    }

    /// Get role
    pub fn role(&self) -> &AgentRole {
        &self.role
    }

    /// Get assigned task ID
    pub fn assigned_task_id(&self) -> Option<&TaskId> {
        self.assigned_task_id.as_ref()
    }

    /// Assign a task (only valid when idle)
    pub fn assign_task(&mut self, task_id: TaskId) -> Result<(), WorkerError> {
        if !self.state.is_idle() {
            return Err(WorkerError::InvariantViolation(
                "cannot assign task to non-idle worker".to_string(),
            ));
        }
        self.assigned_task_id = Some(task_id);
        Ok(())
    }

    /// Clear assigned task
    pub fn clear_task(&mut self) {
        self.assigned_task_id = None;
    }

    /// Get worktree path
    pub fn worktree_path(&self) -> Option<&PathBuf> {
        self.worktree_path.as_ref()
    }

    /// Set worktree path
    pub fn set_worktree_path(&mut self, path: PathBuf) {
        self.worktree_path = Some(path);
    }

    /// Get worktree branch
    pub fn worktree_branch(&self) -> Option<&str> {
        self.worktree_branch.as_deref()
    }

    /// Set worktree branch
    pub fn set_worktree_branch(&mut self, branch: String) {
        self.worktree_branch = Some(branch);
    }

    /// Get transcript entries
    pub fn transcript(&self) -> &TranscriptJournal {
        &self.transcript
    }

    /// Get last n transcript entries
    pub fn last_n_entries(&self, n: usize) -> &[JournalEntry] {
        self.transcript.last_n(n)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::AgentId;
    use agent_events::DomainEvent;

    fn test_worker() -> Worker {
        Worker::new(
            AgentId::new("test-agent"),
            AgentCodename::new("alpha"),
            AgentRole::Developer,
        )
    }

    // ── Construction ────────────────────────────────────────────

    #[test]
    fn worker_starts_idle() {
        let w = test_worker();
        assert!(w.state().is_idle());
        assert_eq!(w.transcript().len(), 0);
    }

    #[test]
    fn worker_fields_set_correctly() {
        let w = test_worker();
        assert_eq!(w.agent_id().as_str(), "test-agent");
        assert_eq!(w.codename().as_str(), "alpha");
        assert_eq!(*w.role(), AgentRole::Developer);
    }

    // ── Task assignment ─────────────────────────────────────────

    #[test]
    fn assign_task_when_idle() {
        let mut w = test_worker();
        assert!(w.assign_task(TaskId::new("task-1")).is_ok());
        assert_eq!(w.assigned_task_id().unwrap().as_str(), "task-1");
    }

    #[test]
    fn assign_task_when_not_idle_fails() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        let result = w.assign_task(TaskId::new("task-1"));
        assert!(result.is_err());
    }

    // ── Apply: lifecycle events ─────────────────────────────────

    #[test]
    fn apply_status_worker_started_transitions_to_starting() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        assert!(matches!(w.state(), WorkerState::Starting));
    }

    #[test]
    fn apply_finished_transitions_to_completed() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("done".to_string()))
            .unwrap();
        w.apply(DomainEvent::Finished).unwrap();
        assert!(w.state().is_terminal());
        assert!(matches!(w.state(), WorkerState::Completed));
    }

    #[test]
    fn apply_error_transitions_to_failed() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::Error("panic".to_string())).unwrap();
        assert!(w.state().is_failed());
    }

    // ── Apply: streaming events ─────────────────────────────────

    #[test]
    fn apply_assistant_chunk_from_starting() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("hello".to_string()))
            .unwrap();
        assert!(matches!(w.state(), WorkerState::Responding { .. }));
    }

    #[test]
    fn apply_thinking_chunk_keeps_responding() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("hi".to_string()))
            .unwrap();
        w.apply(DomainEvent::ThinkingChunk("thinking...".to_string()))
            .unwrap();
        assert!(matches!(w.state(), WorkerState::Responding { .. }));
    }

    // ── Apply: tool events ──────────────────────────────────────

    #[test]
    fn apply_exec_command_started_then_finished() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("hi".to_string()))
            .unwrap();
        w.apply(DomainEvent::ExecCommandStarted {
            call_id: None,
            input_preview: None,
            source: None,
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::ProcessingTool { .. }));

        w.apply(DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: None,
            status: agent_events::ExecCommandStatus::Completed,
            exit_code: None,
            duration_ms: None,
            source: None,
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::Responding { .. }));
    }

    #[test]
    fn apply_generic_tool_call() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("let me read".to_string()))
            .unwrap();
        w.apply(DomainEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: None,
            input_preview: None,
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::ProcessingTool { name } if name == "read_file"));
    }

    #[test]
    fn apply_mcp_tool_call() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("using mcp".to_string()))
            .unwrap();
        w.apply(DomainEvent::McpToolCallStarted {
            call_id: None,
            invocation: agent_events::McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::ProcessingTool { name } if name == "mcp"));
    }

    #[test]
    fn apply_patch_apply() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("applying patch".to_string()))
            .unwrap();
        w.apply(DomainEvent::PatchApplyStarted {
            call_id: None,
            changes: vec![],
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::ProcessingTool { name } if name == "patch"));
    }

    // ── Apply: web search ───────────────────────────────────────

    #[test]
    fn apply_web_search() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("searching".to_string()))
            .unwrap();
        w.apply(DomainEvent::WebSearchStarted {
            call_id: None,
            query: "rust".to_string(),
        })
        .unwrap();
        assert!(matches!(w.state(), WorkerState::ProcessingTool { name } if name == "websearch"));
    }

    // ── Apply: session handle ───────────────────────────────────

    #[test]
    fn apply_session_handle_does_not_change_state() {
        let mut w = test_worker();
        w.apply(DomainEvent::SessionHandle(agent_events::SessionHandle::ClaudeSession {
            session_id: "s1".to_string(),
        }))
        .unwrap();
        assert!(w.state().is_idle());
    }

    // ── Transcript ──────────────────────────────────────────────

    #[test]
    fn transcript_records_all_events() {
        let mut w = test_worker();
        w.apply(DomainEvent::Status("worker started".to_string()))
            .unwrap();
        w.apply(DomainEvent::AssistantChunk("hi".to_string()))
            .unwrap();
        w.apply(DomainEvent::Finished).unwrap();

        assert_eq!(w.transcript().len(), 3);
    }

    #[test]
    fn last_n_entries() {
        let mut w = test_worker();
        for i in 0..5 {
            w.apply(DomainEvent::Status(format!("event-{}", i))).unwrap();
        }
        let last_2 = w.last_n_entries(2);
        assert_eq!(last_2.len(), 2);
    }

    // ── Worktree ────────────────────────────────────────────────

    #[test]
    fn worktree_setters_and_getters() {
        let mut w = test_worker();
        w.set_worktree_path(PathBuf::from("/tmp/wt"));
        w.set_worktree_branch("feature-1".to_string());
        assert_eq!(w.worktree_path(), Some(&PathBuf::from("/tmp/wt")));
        assert_eq!(w.worktree_branch(), Some("feature-1"));
    }

    // ── Invariant violations ────────────────────────────────────

    #[test]
    fn apply_finished_from_idle_is_invalid() {
        let mut w = test_worker();
        let result = w.apply(DomainEvent::Finished);
        assert!(result.is_err());
    }

    #[test]
    fn apply_error_from_idle_is_invalid() {
        let mut w = test_worker();
        let result = w.apply(DomainEvent::Error("e".to_string()));
        assert!(result.is_err());
    }
}
