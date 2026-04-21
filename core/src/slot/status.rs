//! AgentSlotStatus - operational status for agent slots
//!
//! Defines all possible states an agent can be in during runtime,
//! with transition validation and helper methods.

use std::time::Instant;

use chrono::{DateTime, Utc};
use agent_decision::{BlockedState, BlockingReason};

/// Status of an agent slot in the multi-agent runtime
#[derive(Debug, Clone)]
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
    /// Agent is blocked with simple reason (backward compatible)
    Blocked { reason: String },
    /// Agent is blocked with rich BlockedState from decision layer
    BlockedForDecision { blocked_state: BlockedState },
    /// Agent is paused with worktree preservation
    Paused { reason: String },
    /// Agent is waiting for user input (idle within Responding state)
    WaitingForInput { started_at: Instant },
    /// Agent is resting due to rate limit (💤), waiting for quota to recover
    Resting {
        /// When first 429 occurred
        started_at: DateTime<Utc>,
        /// Reference to decision layer's blocked state
        blocked_state: BlockedState,
        /// If true, attempt recovery immediately on snapshot restore
        on_resume: bool,
    },
}

impl PartialEq for AgentSlotStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Idle, Self::Idle) => true,
            (Self::Starting, Self::Starting) => true,
            (Self::Responding { started_at: a }, Self::Responding { started_at: b }) => a == b,
            (Self::ToolExecuting { tool_name: a }, Self::ToolExecuting { tool_name: b }) => a == b,
            (Self::Finishing, Self::Finishing) => true,
            (Self::Stopping, Self::Stopping) => true,
            (Self::Stopped { reason: a }, Self::Stopped { reason: b }) => a == b,
            (Self::Error { message: a }, Self::Error { message: b }) => a == b,
            (Self::Blocked { reason: a }, Self::Blocked { reason: b }) => a == b,
            // BlockedForDecision compares by reason_type only
            (
                Self::BlockedForDecision { blocked_state: a },
                Self::BlockedForDecision { blocked_state: b },
            ) => a.reason().reason_type() == b.reason().reason_type(),
            (Self::Paused { reason: a }, Self::Paused { reason: b }) => a == b,
            (Self::WaitingForInput { started_at: a }, Self::WaitingForInput { started_at: b }) => {
                a == b
            }
            // Resting compares by started_at only
            (Self::Resting { started_at: a, .. }, Self::Resting { started_at: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Eq for AgentSlotStatus {}

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
        Self::Responding {
            started_at: Instant::now(),
        }
    }

    /// Create a new ToolExecuting status
    pub fn tool_executing(tool_name: impl Into<String>) -> Self {
        Self::ToolExecuting {
            tool_name: tool_name.into(),
        }
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
        Self::Stopped {
            reason: reason.into(),
        }
    }

    /// Create a new Error status
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Create a new Blocked status
    pub fn blocked(reason: impl Into<String>) -> Self {
        Self::Blocked {
            reason: reason.into(),
        }
    }

    /// Create a new Paused status (worktree preserved)
    pub fn paused(reason: impl Into<String>) -> Self {
        Self::Paused {
            reason: reason.into(),
        }
    }

    /// Create a new BlockedForDecision status with rich BlockedState
    pub fn blocked_for_decision(blocked_state: BlockedState) -> Self {
        Self::BlockedForDecision { blocked_state }
    }

    /// Create a new WaitingForInput status
    pub fn waiting_for_input() -> Self {
        Self::WaitingForInput {
            started_at: Instant::now(),
        }
    }

    /// Create a new Resting status (rate limited)
    pub fn resting(blocked_state: BlockedState) -> Self {
        Self::Resting {
            started_at: chrono::Utc::now(),
            blocked_state,
            on_resume: false,
        }
    }

    /// Create a new Resting status with on_resume flag (for resume scenarios)
    pub fn resting_with_on_resume(blocked_state: BlockedState, on_resume: bool) -> Self {
        Self::Resting {
            started_at: chrono::Utc::now(),
            blocked_state,
            on_resume,
        }
    }

    /// Check if agent can transition to a new status
    pub fn can_transition_to(&self, target: &AgentSlotStatus) -> bool {
        match (self, target) {
            // Idle can go to Starting, Blocked, BlockedForDecision, Stopped, or Paused
            (Self::Idle, Self::Starting) => true,
            (Self::Idle, Self::Blocked { .. }) => true,
            (Self::Idle, Self::BlockedForDecision { .. }) => true,
            (Self::Idle, Self::Stopped { .. }) => true,
            (Self::Idle, Self::Paused { .. }) => true,
            // Starting can go to Idle, Responding, Stopping, Blocked, BlockedForDecision, Error, Paused, or Stopped
            (Self::Starting, Self::Idle) => true,
            (Self::Starting, Self::Responding { .. }) => true,
            (Self::Starting, Self::Stopping) => true,
            (Self::Starting, Self::Blocked { .. }) => true,
            (Self::Starting, Self::BlockedForDecision { .. }) => true,
            (Self::Starting, Self::Error { .. }) => true,
            (Self::Starting, Self::Paused { .. }) => true,
            (Self::Starting, Self::Stopped { .. }) => true,
            // Responding can go to Idle, ToolExecuting, Finishing, Stopping, Blocked, BlockedForDecision, Error, Paused, WaitingForInput, or Stopped
            (Self::Responding { .. }, Self::Idle) => true,
            (Self::Responding { .. }, Self::ToolExecuting { .. }) => true,
            (Self::Responding { .. }, Self::Finishing) => true,
            (Self::Responding { .. }, Self::Stopping) => true,
            (Self::Responding { .. }, Self::Blocked { .. }) => true,
            (Self::Responding { .. }, Self::BlockedForDecision { .. }) => true,
            (Self::Responding { .. }, Self::Error { .. }) => true,
            (Self::Responding { .. }, Self::Paused { .. }) => true,
            (Self::Responding { .. }, Self::WaitingForInput { .. }) => true,
            (Self::Responding { .. }, Self::Stopped { .. }) => true,
            // ToolExecuting can go back to Responding or to Idle/Finishing/Stopping/Blocked/BlockedForDecision/Error/Paused/WaitingForInput/Stopped
            (Self::ToolExecuting { .. }, Self::Idle) => true,
            (Self::ToolExecuting { .. }, Self::Responding { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Finishing) => true,
            (Self::ToolExecuting { .. }, Self::Stopping) => true,
            (Self::ToolExecuting { .. }, Self::Blocked { .. }) => true,
            (Self::ToolExecuting { .. }, Self::BlockedForDecision { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Error { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Paused { .. }) => true,
            (Self::ToolExecuting { .. }, Self::WaitingForInput { .. }) => true,
            (Self::ToolExecuting { .. }, Self::Stopped { .. }) => true,
            // Finishing can go to Idle, Stopping, Blocked, BlockedForDecision, Error, Paused, or Stopped
            (Self::Finishing, Self::Idle) => true,
            (Self::Finishing, Self::Stopping) => true,
            (Self::Finishing, Self::Blocked { .. }) => true,
            (Self::Finishing, Self::BlockedForDecision { .. }) => true,
            (Self::Finishing, Self::Error { .. }) => true,
            (Self::Finishing, Self::Paused { .. }) => true,
            (Self::Finishing, Self::Stopped { .. }) => true,
            // Stopping can go to Stopped or Error
            (Self::Stopping, Self::Stopped { .. }) => true,
            (Self::Stopping, Self::Error { .. }) => true,
            // Stopped can go to Starting (restart)
            (Self::Stopped { .. }, Self::Starting) => true,
            // Error can go to Idle (recovery) or Stopped
            (Self::Error { .. }, Self::Idle) => true,
            (Self::Error { .. }, Self::Stopped { .. }) => true,
            // Blocked can go to Idle, Responding, Stopped, Paused, or BlockedForDecision (escalation)
            (Self::Blocked { .. }, Self::Idle) => true,
            (Self::Blocked { .. }, Self::Responding { .. }) => true,
            (Self::Blocked { .. }, Self::Stopped { .. }) => true,
            (Self::Blocked { .. }, Self::Paused { .. }) => true,
            (Self::Blocked { .. }, Self::BlockedForDecision { .. }) => true,
            // BlockedForDecision can go to Idle, Responding, Stopped, Paused, or Blocked (provider crash recovery)
            (Self::BlockedForDecision { .. }, Self::Idle) => true,
            (Self::BlockedForDecision { .. }, Self::Responding { .. }) => true,
            (Self::BlockedForDecision { .. }, Self::Stopped { .. }) => true,
            (Self::BlockedForDecision { .. }, Self::Paused { .. }) => true,
            (Self::BlockedForDecision { .. }, Self::Blocked { .. }) => true,
            // Paused can go to Idle (resume) or Stopped
            (Self::Paused { .. }, Self::Idle) => true,
            (Self::Paused { .. }, Self::Stopped { .. }) => true,
            // WaitingForInput can go to Responding, Idle, Stopping, Blocked, BlockedForDecision, or Stopped
            (Self::WaitingForInput { .. }, Self::Responding { .. }) => true,
            (Self::WaitingForInput { .. }, Self::Idle) => true,
            (Self::WaitingForInput { .. }, Self::Stopping) => true,
            (Self::WaitingForInput { .. }, Self::Blocked { .. }) => true,
            (Self::WaitingForInput { .. }, Self::BlockedForDecision { .. }) => true,
            (Self::WaitingForInput { .. }, Self::Stopped { .. }) => true,
            // BlockedForDecision can go to Resting (rate limit escalation)
            (Self::BlockedForDecision { .. }, Self::Resting { .. }) => true,
            // Resting can go to Idle (recovery), Error (unrecoverable), Stopped (user cancel), or stay Resting
            (Self::Resting { .. }, Self::Idle) => true,
            (Self::Resting { .. }, Self::Error { .. }) => true,
            (Self::Resting { .. }, Self::Stopped { .. }) => true,
            (Self::Resting { .. }, Self::Resting { .. }) => true,
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

    /// Check if agent is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if this is a terminal status (Stopped)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped { .. })
    }

    /// Check if agent is in stopping state (graceful shutdown in progress)
    pub fn is_stopping(&self) -> bool {
        matches!(self, Self::Stopping)
    }

    /// Check if agent is blocked (including rate-limit resting state)
    ///
    /// Includes `Resting` since it represents a rate-limit escalation,
    /// which is a form of blocking that requires recovery.
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked { .. } | Self::BlockedForDecision { .. } | Self::Resting { .. })
    }

    /// Check if agent is paused
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused { .. })
    }

    /// Check if agent is waiting for user input
    pub fn is_waiting_for_input(&self) -> bool {
        matches!(self, Self::WaitingForInput { .. })
    }

    /// Check if agent is resting (rate limited)
    pub fn is_resting(&self) -> bool {
        matches!(self, Self::Resting { .. })
    }

    /// Check if agent is blocked for human decision
    pub fn is_blocked_for_human(&self) -> bool {
        match self {
            Self::Blocked { reason } => reason.contains("human"),
            Self::BlockedForDecision { blocked_state } => {
                blocked_state.reason().reason_type() == "human_decision"
            }
            _ => false,
        }
    }

    /// Get blocking reason if blocked
    pub fn blocking_reason(&self) -> Option<&dyn BlockingReason> {
        match self {
            Self::BlockedForDecision { blocked_state } => Some(blocked_state.reason()),
            Self::Resting { blocked_state, .. } => Some(blocked_state.reason()),
            _ => None,
        }
    }

    /// Get elapsed time since blocked
    pub fn blocked_elapsed(&self) -> Option<std::time::Duration> {
        match self {
            Self::BlockedForDecision { blocked_state } => Some(blocked_state.elapsed()),
            Self::Resting { blocked_state, .. } => Some(blocked_state.elapsed()),
            _ => None,
        }
    }

    /// Get resting start time if resting
    pub fn resting_started_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        match self {
            Self::Resting { started_at, .. } => Some(*started_at),
            _ => None,
        }
    }

    /// Get blocking state if in BlockedForDecision or Resting
    pub fn blocked_state(&self) -> Option<&BlockedState> {
        match self {
            Self::BlockedForDecision { blocked_state } => Some(blocked_state),
            Self::Resting { blocked_state, .. } => Some(blocked_state),
            _ => None,
        }
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
            Self::Blocked { reason } => format!("blocked:{}", reason),
            Self::BlockedForDecision { blocked_state } => {
                format!("blocked:{}", blocked_state.reason().reason_type())
            },
            Self::Paused { reason } => format!("paused:{}", reason),
            Self::WaitingForInput { .. } => "waiting_for_input".to_string(),
            Self::Resting { started_at, .. } => {
                let mins = (Utc::now() - *started_at).num_minutes();
                format!("resting:{}min", mins)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::{
        HumanDecisionBlocking,
        builtin_situations::WaitingForChoiceSituation,
    };

    /// Helper to create a BlockedState for tests
    fn test_blocked_state() -> BlockedState {
        let blocking = HumanDecisionBlocking::new(
            "test-req",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        BlockedState::new(Box::new(blocking))
    }

    #[test]
    fn status_idle_can_transition_to_starting() {
        assert!(AgentSlotStatus::idle().can_transition_to(&AgentSlotStatus::starting()));
    }

    #[test]
    fn status_idle_cannot_transition_to_responding() {
        assert!(!AgentSlotStatus::idle().can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_is_active() {
        assert!(AgentSlotStatus::starting().is_active());
        assert!(AgentSlotStatus::responding_now().is_active());
        assert!(!AgentSlotStatus::idle().is_active());
        assert!(!AgentSlotStatus::stopped("test").is_active());
    }

    #[test]
    fn status_blocked_is_blocked() {
        // Blocked status is blocked
        assert!(AgentSlotStatus::blocked("test").is_blocked());

        // BlockedForDecision is blocked
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::blocked_for_decision(blocked_state).is_blocked());

        // Resting is blocked (rate limit escalation)
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::resting(blocked_state).is_blocked());

        // Other statuses are not blocked
        assert!(!AgentSlotStatus::idle().is_blocked());
        assert!(!AgentSlotStatus::starting().is_blocked());
        assert!(!AgentSlotStatus::stopped("test").is_blocked());
    }

    #[test]
    fn status_blocked_label() {
        let status = AgentSlotStatus::blocked("human_intervention");
        assert!(status.label().contains("blocked"));
    }

    #[test]
    fn status_blocked_for_decision_can_transition_to_blocked() {
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::blocked_for_decision(blocked_state)
            .can_transition_to(&AgentSlotStatus::blocked("provider_crash")));
    }

    #[test]
    fn status_stopping_is_stopping() {
        assert!(AgentSlotStatus::stopping().is_stopping());
        assert!(!AgentSlotStatus::idle().is_stopping());
    }

    #[test]
    fn status_label_includes_stopping() {
        let status = AgentSlotStatus::stopping();
        assert!(status.label().contains("stopping"));
    }

    #[test]
    fn status_resting_can_transition_to_idle() {
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::resting(blocked_state).can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_resting_to_error() {
        let blocked_state = test_blocked_state();
        assert!(
            AgentSlotStatus::resting(blocked_state).can_transition_to(&AgentSlotStatus::error("unrecoverable"))
        );
    }

    #[test]
    fn status_resting_to_stopped() {
        let blocked_state = test_blocked_state();
        assert!(
            AgentSlotStatus::resting(blocked_state).can_transition_to(&AgentSlotStatus::stopped("user_cancel"))
        );
    }

    #[test]
    fn status_resting_is_resting() {
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::resting(blocked_state).is_resting());
        assert!(!AgentSlotStatus::idle().is_resting());
    }

    #[test]
    fn status_resting_label_shows_minutes() {
        let blocked_state = test_blocked_state();
        let label = AgentSlotStatus::resting(blocked_state).label();
        assert!(label.contains("resting"));
        assert!(label.contains("min"));
    }

    #[test]
    fn status_blocked_can_transition_to_idle() {
        assert!(AgentSlotStatus::blocked("test").can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_blocked_can_transition_to_responding() {
        assert!(
            AgentSlotStatus::blocked("test").can_transition_to(&AgentSlotStatus::responding_now())
        );
    }

    #[test]
    fn status_blocked_can_transition_to_stopped() {
        assert!(AgentSlotStatus::blocked("test").can_transition_to(&AgentSlotStatus::stopped("manual")));
    }

    #[test]
    fn status_blocked_can_transition_to_blocked_for_decision() {
        let blocked_state = test_blocked_state();
        assert!(AgentSlotStatus::blocked("test")
            .can_transition_to(&AgentSlotStatus::blocked_for_decision(blocked_state)));
    }

    #[test]
    fn status_blocked_is_not_active() {
        assert!(!AgentSlotStatus::blocked("test").is_active());
    }

    #[test]
    fn status_waiting_for_input_can_transition_to_responding() {
        assert!(
            AgentSlotStatus::waiting_for_input().can_transition_to(&AgentSlotStatus::responding_now())
        );
    }

    #[test]
    fn status_waiting_for_input_can_transition_to_idle() {
        assert!(AgentSlotStatus::waiting_for_input().can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_waiting_for_input_is_not_active() {
        assert!(!AgentSlotStatus::waiting_for_input().is_active());
    }

    #[test]
    fn status_waiting_for_input_label() {
        let label = AgentSlotStatus::waiting_for_input().label();
        assert!(label.contains("waiting_for_input"));
    }

    #[test]
    fn status_other_is_not_blocked() {
        assert!(!AgentSlotStatus::idle().is_blocked());
        assert!(!AgentSlotStatus::responding_now().is_blocked());
        assert!(!AgentSlotStatus::stopped("test").is_blocked());
    }
}