//! AgentSlot and AgentSlotStatus for multi-agent runtime
//!
//! Represents a single agent's runtime slot in the agent pool.

use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use std::time::Instant;

use crate::agent_runtime::{AgentId, AgentCodename, ProviderType};
use crate::app::TranscriptEntry;
use crate::provider::{ProviderEvent, SessionHandle};

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
    /// Agent is being stopped gracefully (not yet joined)
    Stopping,
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

    /// Create a new Stopping status (graceful shutdown in progress)
    pub fn stopping() -> Self {
        Self::Stopping
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
            // Idle can go to Starting or Stopped (user can stop idle agent)
            (Self::Idle, Self::Starting) => true,
            (Self::Idle, Self::Stopped { .. }) => true,
            // Starting can go to Responding, Stopping, or Error
            (Self::Starting, Self::Responding { .. }) => true,
            (Self::Starting, Self::Stopping) => true,
            (Self::Starting, Self::Error { .. }) => true,
            // Responding can go to ToolExecuting, Finishing, Stopping, or Error
            (Self::Responding { .. }, Self::ToolExecuting { .. }) => true,
            (Self::Responding { .. }, Self::Finishing) => true,
            (Self::Responding { .. }, Self::Stopping) => true,
            (Self::Responding { .. }, Self::Error { .. }) => true,
            // ToolExecuting can go back to Responding or to Finishing/Stopping/Error
            (Self::ToolExecuting { .. }, Self::Responding { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Finishing) => true,
            (Self::ToolExecuting { .. }, Self::Stopping) => true,
            (Self::ToolExecuting { .. }, Self::Error { .. }) => true,
            // Finishing can go to Idle, Stopping, or Error
            (Self::Finishing, Self::Idle) => true,
            (Self::Finishing, Self::Stopping) => true,
            (Self::Finishing, Self::Error { .. }) => true,
            // Stopping can go to Stopped (after thread join) or Error (if join fails)
            (Self::Stopping, Self::Stopped { .. }) => true,
            (Self::Stopping, Self::Error { .. }) => true,
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

    /// Check if this is an active status (not Idle, Stopped, Stopping, or Error)
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

    /// Check if agent is in stopping state (graceful shutdown in progress)
    pub fn is_stopping(&self) -> bool {
        matches!(self, Self::Stopping)
    }

    /// Get a human-readable label for the status
    pub fn label(&self) -> String {
        match self {
            Self::Idle => "idle".to_string(),
            Self::Starting => "starting".to_string(),
            Self::Responding { .. } => "responding".to_string(),
            Self::ToolExecuting { tool_name } => format!("tool:{}", tool_name),
            Self::Finishing => "finishing".to_string(),
            Self::Stopping => "stopping".to_string(),
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
///
/// # Thread Safety
///
/// AgentSlot is owned by the main thread (TUI loop). The provider thread
/// sends events through the channel, and main thread receives via `event_rx`.
/// All mutations happen on main thread after receiving events.
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
    /// Per-agent transcript (copy for TUI rendering)
    transcript: Vec<TranscriptEntry>,
    /// Currently assigned task (if any)
    assigned_task_id: Option<TaskId>,
    /// Event channel receiver from provider thread
    event_rx: Option<Receiver<ProviderEvent>>,
    /// Thread handle for join/cancel operations
    thread_handle: Option<JoinHandle<()>>,
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
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: None,
            thread_handle: None,
            last_activity: Instant::now(),
        }
    }

    /// Create a new agent slot ready for provider thread
    pub fn with_thread(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        event_rx: Receiver<ProviderEvent>,
        thread_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            status: AgentSlotStatus::starting(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: Some(event_rx),
            thread_handle: Some(thread_handle),
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

    /// Get the transcript entries
    pub fn transcript(&self) -> &[TranscriptEntry] {
        &self.transcript
    }

    /// Get mutable reference to transcript
    pub fn transcript_mut(&mut self) -> &mut Vec<TranscriptEntry> {
        &mut self.transcript
    }

    /// Get the event receiver (if provider thread is running)
    pub fn event_rx(&self) -> Option<&Receiver<ProviderEvent>> {
        self.event_rx.as_ref()
    }

    /// Get the thread handle (if provider thread is running)
    pub fn thread_handle(&self) -> Option<&JoinHandle<()> >  {
        self.thread_handle.as_ref()
    }

    /// Take the thread handle (for joining)
    pub fn take_thread_handle(&mut self) -> Option<JoinHandle<()> >  {
        self.thread_handle.take()
    }

    /// Get the last activity timestamp
    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    /// Check if this slot has an active provider thread
    pub fn has_provider_thread(&self) -> bool {
        self.event_rx.is_some() && self.thread_handle.is_some()
    }

    /// Set provider thread components
    pub fn set_provider_thread(
        &mut self,
        event_rx: Receiver<ProviderEvent>,
        thread_handle: JoinHandle<()>,
    ) {
        self.event_rx = Some(event_rx);
        self.thread_handle = Some(thread_handle);
        self.status = AgentSlotStatus::starting();
        self.last_activity = Instant::now();
    }

    /// Clear provider thread components (after join)
    pub fn clear_provider_thread(&mut self) {
        self.event_rx = None;
        self.thread_handle = None;
    }

    /// Append entry to transcript
    pub fn append_transcript(&mut self, entry: TranscriptEntry) {
        self.transcript.push(entry);
        self.last_activity = Instant::now();
    }

    /// Clear transcript
    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
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
        assert!(slot.transcript().is_empty());
        assert!(!slot.has_provider_thread());
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
    fn status_starting_can_transition_to_stopping() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_responding_can_transition_to_tool_executing() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::tool_executing("read_file")));
    }

    #[test]
    fn status_responding_can_transition_to_stopping() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_tool_executing_can_transition_to_responding() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_tool_executing_can_transition_to_stopping() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_finishing_can_transition_to_idle() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_finishing_can_transition_to_stopping() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_stopping_can_transition_to_stopped() {
        let status = AgentSlotStatus::stopping();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_stopping_can_transition_to_error() {
        let status = AgentSlotStatus::stopping();
        assert!(status.can_transition_to(&AgentSlotStatus::error("thread panic")));
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
    fn status_idle_can_transition_to_stopped() {
        let status = AgentSlotStatus::idle();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
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
    fn slot_assign_task_to_non_idle_fails() {
        let mut slot = make_slot();
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        let result = slot.assign_task(TaskId::new("task-001"));
        assert!(result.is_err());
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn slot_clear_task() {
        let mut slot = make_slot();
        slot.assign_task(TaskId::new("task-001")).unwrap();
        slot.clear_task();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn slot_transcript_operations() {
        let mut slot = make_slot();
        assert!(slot.transcript().is_empty());

        // Append entry (using placeholder since TranscriptEntry is complex)
        // Note: In actual implementation, we'd create real TranscriptEntry
        // For this test, we verify the transcript_mut accessor works
        slot.transcript_mut().clear();
        assert!(slot.transcript().is_empty());
    }

    #[test]
    fn status_is_active() {
        assert!(!AgentSlotStatus::idle().is_active());
        assert!(AgentSlotStatus::starting().is_active());
        assert!(AgentSlotStatus::responding_now().is_active());
        assert!(AgentSlotStatus::tool_executing("test").is_active());
        assert!(AgentSlotStatus::finishing().is_active());
        assert!(!AgentSlotStatus::stopping().is_active());
        assert!(!AgentSlotStatus::stopped("test").is_active());
        assert!(!AgentSlotStatus::error("test").is_active());
    }

    #[test]
    fn status_is_stopping() {
        assert!(!AgentSlotStatus::idle().is_stopping());
        assert!(AgentSlotStatus::stopping().is_stopping());
        assert!(!AgentSlotStatus::stopped("test").is_stopping());
    }

    #[test]
    fn status_label_includes_stopping() {
        assert_eq!(AgentSlotStatus::stopping().label(), "stopping");
    }

    #[test]
    fn slot_with_thread_creates_slot_with_provider_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        let slot = AgentSlot::with_thread(
            AgentId::new("agent_001"),
            AgentCodename::new("alpha"),
            ProviderType::Mock,
            rx,
            handle,
        );

        assert!(slot.has_provider_thread());
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);
        assert!(slot.event_rx().is_some());
        assert!(slot.thread_handle().is_some());

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_set_provider_thread() {
        let mut slot = make_slot();
        assert!(!slot.has_provider_thread());

        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);

        assert!(slot.has_provider_thread());
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_clear_provider_thread() {
        let mut slot = make_slot();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);
        assert!(slot.has_provider_thread());

        slot.clear_provider_thread();
        assert!(!slot.has_provider_thread());
        assert!(slot.event_rx().is_none());
        assert!(slot.thread_handle().is_none());

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_take_thread_handle() {
        let mut slot = make_slot();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);
        assert!(slot.thread_handle().is_some());

        let taken = slot.take_thread_handle();
        assert!(taken.is_some());
        assert!(slot.thread_handle().is_none());

        // Cleanup - join the taken handle
        taken.unwrap().join().unwrap();
        drop(tx);
    }
}