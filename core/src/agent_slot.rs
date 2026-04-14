//! AgentSlot and AgentSlotStatus for multi-agent runtime
//!
//! Represents a single agent's runtime slot in the agent pool.

use std::time::Instant;

use crate::agent_runtime::{AgentId, AgentCodename, ProviderType};
use crate::provider::SessionHandle;

/// Status of an agent slot in the multi-agent runtime
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSlotStatus {
    /// Agent is idle, waiting for task assignment
    Idle,
    /// Agent is starting up
    Starting,
    /// Agent is generating response (thinking/streaming)
    Responding { started_at: Instant },
    /// Agent is executing a tool call
    ToolExecuting { tool_name: String },
    /// Agent is finishing its current work
    Finishing,
    /// Agent has been stopped intentionally
    Stopped { reason: String },
    /// Agent encountered an error
    Error { message: String },
}

impl AgentSlotStatus {
    /// Create a new Idle status
    pub fn idle() -> Self {
        Self::Idle
    }

    /// Create a new Starting status
    pub fn starting() -> Self {
        Self::Starting
    }

    /// Create a new Responding status with current timestamp
    pub fn responding_now() -> Self {
        Self::Responding { started_at: Instant::now() }
    }

    /// Create a new ToolExecuting status
    pub fn tool_executing(tool_name: impl Into<String>) -> Self {
        Self::ToolExecuting { tool_name: tool_name.into() }
    }

    /// Create a new Finishing status
    pub fn finishing() -> Self {
        Self::Finishing
    }

    /// Create a new Stopped status
    pub fn stopped(reason: impl Into<String>) -> Self {
        Self::Stopped { reason: reason.into() }
    }

    /// Create a new Error status
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }

    /// Check if agent can transition to a new status
    pub fn can_transition_to(&self, target: &AgentSlotStatus) -> bool {
        match (self, target) {
            // Idle can go to Starting
            (Self::Idle, Self::Starting) => true,
            // Starting can go to Responding or Error
            (Self::Starting, Self::Responding { .. }) => true,
            (Self::Starting, Self::Error { .. }) => true,
            // Responding can go to ToolExecuting, Finishing, or Error
            (Self::Responding { .. }, Self::ToolExecuting { .. }) => true,
            (Self::Responding { .. }, Self::Finishing) => true,
            (Self::Responding { .. }, Self::Error { .. }) => true,
            // ToolExecuting can go back to Responding or to Finishing/Error
            (Self::ToolExecuting { .. }, Self::Responding { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Finishing) => true,
            (Self::ToolExecuting { .. }, Self::Error { .. }) => true,
            // Finishing can go to Idle or Error
            (Self::Finishing, Self::Idle) => true,
            (Self::Finishing, Self::Error { .. }) => true,
            // Stopped can go to Starting (restart)
            (Self::Stopped { .. }, Self::Starting) => true,
            // Error can go to Idle (recovery) or Stopped
            (Self::Error { .. }, Self::Idle) => true,
            (Self::Error { .. }, Self::Stopped { .. }) => true,
            // Same status is always valid
            (a, b) if a == b => true,
            // All other transitions are invalid
            _ => false,
        }
    }

    /// Check if this is an active status (not Idle, Stopped, or Error)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Responding { .. } | Self::ToolExecuting { .. } | Self::Finishing
        )
    }

    /// Check if this is a terminal status (Stopped)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped { .. })
    }

    /// Get a human-readable label for the status
    pub fn label(&self) -> String {
        match self {
            Self::Idle => "idle".to_string(),
            Self::Starting => "starting".to_string(),
            Self::Responding { .. } => "responding".to_string(),
            Self::ToolExecuting { tool_name } => format!("tool:{}", tool_name),
            Self::Finishing => "finishing".to_string(),
            Self::Stopped { reason } => format!("stopped:{}", reason),
            Self::Error { message } => format!("error:{}", message),
        }
    }
}

/// Unique identifier for a task assigned to an agent
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Result of a task completion
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCompletionResult {
    Success,
    Failure { error: String },
}

/// Outcome when agent thread finishes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadOutcome {
    NormalExit,
    ErrorExit { error: String },
    Cancelled,
}

/// A single agent's runtime slot
///
/// Contains all state for managing one agent's execution thread,
/// including provider session, transcript, and event channels.
pub struct AgentSlot {
    /// Unique agent identifier
    agent_id: AgentId,
    /// Agent display codename (alpha, bravo, etc.)
    codename: AgentCodename,
    /// Provider type binding
    provider_type: ProviderType,
    /// Current runtime status
    status: AgentSlotStatus,
    /// Provider session handle for multi-turn continuity
    session_handle: Option<SessionHandle>,
    /// Currently assigned task (if any)
    assigned_task_id: Option<TaskId>,
    /// Last activity timestamp
    last_activity: Instant,
}

impl AgentSlot {
    /// Create a new agent slot with given identity
    pub fn new(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            status: AgentSlotStatus::idle(),
            session_handle: None,
            assigned_task_id: None,
            last_activity: Instant::now(),
        }
    }

    /// Get the agent's unique identifier
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get the agent's codename
    pub fn codename(&self) -> &AgentCodename {
        &self.codename
    }

    /// Get the provider type
    pub fn provider_type(&self) -> ProviderType {
        self.provider_type
    }

    /// Get the current status
    pub fn status(&self) -> &AgentSlotStatus {
        &self.status
    }

    /// Get the session handle
    pub fn session_handle(&self) -> Option<&SessionHandle> {
        self.session_handle.as_ref()
    }

    /// Get the assigned task ID
    pub fn assigned_task_id(&self) -> Option<&TaskId> {
        self.assigned_task_id.as_ref()
    }

    /// Get the last activity timestamp
    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    /// Transition to a new status
    ///
    /// Returns Ok(()) if transition is valid, Err if invalid.
    pub fn transition_to(&mut self, new_status: AgentSlotStatus) -> Result<(), String> {
        if !self.status.can_transition_to(&new_status) {
            return Err(format!(
                "Invalid transition from {} to {}",
                self.status.label(),
                new_status.label()
            ));
        }
        self.status = new_status;
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Set the session handle
    pub fn set_session_handle(&mut self, handle: SessionHandle) {
        self.session_handle = Some(handle);
        self.last_activity = Instant::now();
    }

    /// Clear the session handle
    pub fn clear_session_handle(&mut self) {
        self.session_handle = None;
    }

    /// Assign a task to this agent
    pub fn assign_task(&mut self, task_id: TaskId) -> Result<(), String> {
        if self.status != AgentSlotStatus::Idle {
            return Err(format!(
                "Cannot assign task to agent with status {}",
                self.status.label()
            ));
        }
        self.assigned_task_id = Some(task_id);
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Clear the assigned task
    pub fn clear_task(&mut self) {
        self.assigned_task_id = None;
    }

    /// Summary string for display
    pub fn summary(&self) -> String {
        format!(
            "{} ({}) [{}]",
            self.codename.as_str(),
            self.agent_id.as_str(),
            self.status.label()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_slot() -> AgentSlot {
        AgentSlot::new(
            AgentId::new("agent_001"),
            AgentCodename::new("alpha"),
            ProviderType::Mock,
        )
    }

    #[test]
    fn slot_new_creates_idle_slot() {
        let slot = make_slot();
        assert_eq!(slot.status(), &AgentSlotStatus::Idle);
        assert!(slot.session_handle().is_none());
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn status_idle_can_transition_to_starting() {
        let status = AgentSlotStatus::idle();
        assert!(status.can_transition_to(&AgentSlotStatus::starting()));
    }

    #[test]
    fn status_idle_cannot_transition_to_responding() {
        let status = AgentSlotStatus::idle();
        assert!(!status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_starting_can_transition_to_responding() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_responding_can_transition_to_tool_executing() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::tool_executing("read_file")));
    }

    #[test]
    fn status_tool_executing_can_transition_to_responding() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_finishing_can_transition_to_idle() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_error_can_transition_to_idle() {
        let status = AgentSlotStatus::error("something went wrong");
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_stopped_can_transition_to_starting() {
        let status = AgentSlotStatus::stopped("user requested");
        assert!(status.can_transition_to(&AgentSlotStatus::starting()));
    }

    #[test]
    fn slot_transition_valid_succeeds() {
        let mut slot = make_slot();
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);
    }

    #[test]
    fn slot_transition_invalid_fails() {
        let mut slot = make_slot();
        let result = slot.transition_to(AgentSlotStatus::responding_now());
        assert!(result.is_err());
        assert_eq!(slot.status(), &AgentSlotStatus::Idle);
    }

    #[test]
    fn slot_assign_task_to_idle_succeeds() {
        let mut slot = make_slot();
        slot.assign_task(TaskId::new("task-001")).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn slot_assign_task_to_active_fails() {
        let mut slot = make_slot();
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        let result = slot.assign_task(TaskId::new("task-001"));
        assert!(result.is_err());
    }

    #[test]
    fn status_is_active() {
        assert!(!AgentSlotStatus::idle().is_active());
        assert!(AgentSlotStatus::starting().is_active());
        assert!(AgentSlotStatus::responding_now().is_active());
        assert!(AgentSlotStatus::tool_executing("test").is_active());
        assert!(!AgentSlotStatus::stopped("test").is_active());
        assert!(!AgentSlotStatus::error("test").is_active());
    }

    #[test]
    fn status_is_terminal() {
        assert!(!AgentSlotStatus::idle().is_terminal());
        assert!(AgentSlotStatus::stopped("test").is_terminal());
        assert!(!AgentSlotStatus::error("test").is_terminal());
    }

    #[test]
    fn status_label() {
        assert_eq!(AgentSlotStatus::idle().label(), "idle");
        assert_eq!(AgentSlotStatus::starting().label(), "starting");
        assert_eq!(AgentSlotStatus::tool_executing("bash").label(), "tool:bash");
        assert_eq!(AgentSlotStatus::stopped("user").label(), "stopped:user");
    }
}