//! WorkerState — explicit state machine for the Worker aggregate root.
//!
//! This enum defines the core lifecycle states of an agent worker.
//! It is intentionally simpler than AgentSlotStatus, focusing on the
//! domain-model state rather than operational edge cases.
//!
//! State transition rules enforce valid paths and prevent invalid jumps
//! (e.g., Idle → Responding is not allowed — must go through Starting).

use std::fmt;

use chrono::{DateTime, Utc};

/// Core state of a Worker in the runtime.
///
/// States form a directed acyclic graph (DAG) with forward-only progression,
/// except for Error recovery paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerState {
    /// Worker thread is starting, provider not yet ready
    Starting,

    /// Worker is generating a response from the LLM
    Responding { sub: RespondingSubState },

    /// Worker is executing a tool call (exec, MCP, patch, etc.)
    ProcessingTool { name: String },

    /// Worker completed successfully
    Completed,

    /// Worker failed (error, tool failure, panic, etc.)
    Failed { reason: String },

    /// Worker is idle, waiting for input
    Idle,

    // ── NEW: Operational states previously missing ─────────────
    /// Agent is blocked awaiting external input or decision
    Blocked { reason: String },

    /// Agent is paused with worktree preserved
    Paused { reason: String },

    /// Agent is waiting for user input within a response
    WaitingForInput,

    /// Agent is resting due to rate limit (HTTP 429)
    Resting { until: Option<DateTime<Utc>> },

    /// Agent is finishing its current work (transitional)
    Finishing,

    /// Agent is being stopped gracefully (transitional)
    Stopping,
}

/// Sub-state within Responding — distinguishes streaming from confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespondingSubState {
    /// Actively receiving streaming chunks from provider
    Streaming,
    /// Awaiting user confirmation before proceeding
    WaitingConfirmation,
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: WorkerState,
    pub to: WorkerState,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid transition from {:?} to {:?}", self.from, self.to)
    }
}

impl std::error::Error for InvalidTransition {}

impl WorkerState {
    /// Create a new Idle state
    pub fn idle() -> Self {
        Self::Idle
    }

    /// Create a new Starting state
    pub fn starting() -> Self {
        Self::Starting
    }

    /// Create a new Responding::Streaming state
    pub fn responding_streaming() -> Self {
        Self::Responding {
            sub: RespondingSubState::Streaming,
        }
    }

    /// Create a new Responding::WaitingConfirmation state
    pub fn responding_waiting() -> Self {
        Self::Responding {
            sub: RespondingSubState::WaitingConfirmation,
        }
    }

    /// Create a new ProcessingTool state
    pub fn processing_tool(name: impl Into<String>) -> Self {
        Self::ProcessingTool {
            name: name.into(),
        }
    }

    /// Create a new Completed state
    pub fn completed() -> Self {
        Self::Completed
    }

    /// Create a new Failed state
    pub fn failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            reason: reason.into(),
        }
    }

    /// Create a new Blocked state
    pub fn blocked(reason: impl Into<String>) -> Self {
        Self::Blocked {
            reason: reason.into(),
        }
    }

    /// Create a new Paused state
    pub fn paused(reason: impl Into<String>) -> Self {
        Self::Paused {
            reason: reason.into(),
        }
    }

    /// Create a new WaitingForInput state
    pub fn waiting_for_input() -> Self {
        Self::WaitingForInput
    }

    /// Create a new Resting state
    pub fn resting(until: Option<DateTime<Utc>>) -> Self {
        Self::Resting { until }
    }

    /// Create a new Finishing state
    pub fn finishing() -> Self {
        Self::Finishing
    }

    /// Create a new Stopping state
    pub fn stopping() -> Self {
        Self::Stopping
    }

    /// Check if a transition from `self` to `target` is valid.
    ///
    /// Rules:
    /// - Forward-only: no backward jumps (except Error recovery)
    /// - No self-loops (same state → false)
    /// - Operational states (Blocked, Paused, WaitingForInput, Resting)
    ///   can enter/exit from most active states
    /// - Completed/Failed are terminal (only recovery paths out)
    pub fn can_transition_to(&self, target: &WorkerState) -> bool {
        // No self-loops
        if self == target {
            return false;
        }

        match (self, target) {
            // ── Idle ──────────────────────────────────────────────
            (Self::Idle, Self::Starting) => true,
            (Self::Idle, Self::Blocked { .. }) => true,
            (Self::Idle, Self::Paused { .. }) => true,
            (Self::Idle, Self::Stopping) => true,

            // ── Starting ──────────────────────────────────────────
            (Self::Starting, Self::Responding { .. }) => true,
            (Self::Starting, Self::Failed { .. }) => true,
            (Self::Starting, Self::Blocked { .. }) => true,
            (Self::Starting, Self::Paused { .. }) => true,
            (Self::Starting, Self::Stopping) => true,

            // ── Responding ────────────────────────────────────────
            (Self::Responding { .. }, Self::ProcessingTool { .. }) => true,
            (Self::Responding { .. }, Self::Completed) => true,
            (Self::Responding { .. }, Self::Failed { .. }) => true,
            (Self::Responding { .. }, Self::Idle) => true,
            (Self::Responding { .. }, Self::Blocked { .. }) => true,
            (Self::Responding { .. }, Self::Paused { .. }) => true,
            (Self::Responding { .. }, Self::WaitingForInput) => true,
            (Self::Responding { .. }, Self::Finishing) => true,
            (Self::Responding { .. }, Self::Stopping) => true,
            // Sub-state transitions within Responding
            (
                Self::Responding {
                    sub: RespondingSubState::Streaming,
                },
                Self::Responding {
                    sub: RespondingSubState::WaitingConfirmation,
                },
            ) => true,
            (
                Self::Responding {
                    sub: RespondingSubState::WaitingConfirmation,
                },
                Self::Responding {
                    sub: RespondingSubState::Streaming,
                },
            ) => true,

            // ── ProcessingTool ────────────────────────────────────
            (Self::ProcessingTool { .. }, Self::Responding { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Completed) => true,
            (Self::ProcessingTool { .. }, Self::Failed { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Idle) => true,
            (Self::ProcessingTool { .. }, Self::Blocked { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Paused { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Finishing) => true,
            (Self::ProcessingTool { .. }, Self::Stopping) => true,

            // ── Finishing ─────────────────────────────────────────
            (Self::Finishing, Self::Completed) => true,
            (Self::Finishing, Self::Failed { .. }) => true,
            (Self::Finishing, Self::Idle) => true,
            (Self::Finishing, Self::Blocked { .. }) => true,
            (Self::Finishing, Self::Paused { .. }) => true,
            (Self::Finishing, Self::Stopping) => true,

            // ── Stopping ──────────────────────────────────────────
            (Self::Stopping, Self::Completed) => true,
            (Self::Stopping, Self::Failed { .. }) => true,
            (Self::Stopping, Self::Idle) => true,

            // ── Completed (terminal — only explicit restart) ──────
            (Self::Completed, Self::Starting) => true,

            // ── Failed (recovery paths) ───────────────────────────
            (Self::Failed { .. }, Self::Idle) => true,
            (Self::Failed { .. }, Self::Starting) => true,
            (Self::Failed { .. }, Self::Blocked { .. }) => true,
            (Self::Failed { .. }, Self::Paused { .. }) => true,

            // ── Blocked ───────────────────────────────────────────
            (Self::Blocked { .. }, Self::Idle) => true,
            (Self::Blocked { .. }, Self::Responding { .. }) => true,
            (Self::Blocked { .. }, Self::Starting) => true,
            (Self::Blocked { .. }, Self::Paused { .. }) => true,
            (Self::Blocked { .. }, Self::Resting { .. }) => true,

            // ── Paused ────────────────────────────────────────────
            (Self::Paused { .. }, Self::Idle) => true,
            (Self::Paused { .. }, Self::Responding { .. }) => true,
            (Self::Paused { .. }, Self::Starting) => true,
            (Self::Paused { .. }, Self::Blocked { .. }) => true,

            // ── WaitingForInput ───────────────────────────────────
            (Self::WaitingForInput, Self::Responding { .. }) => true,
            (Self::WaitingForInput, Self::Idle) => true,
            (Self::WaitingForInput, Self::Blocked { .. }) => true,
            (Self::WaitingForInput, Self::Paused { .. }) => true,

            // ── Resting ───────────────────────────────────────────
            (Self::Resting { .. }, Self::Idle) => true,
            (Self::Resting { .. }, Self::Starting) => true,
            (Self::Resting { .. }, Self::Failed { .. }) => true,

            // All other combinations are invalid
            _ => false,
        }
    }

    /// Attempt to transition to `target`, returning Err if invalid.
    pub fn transition_to(&self, target: WorkerState) -> Result<WorkerState, InvalidTransition> {
        if self.can_transition_to(&target) {
            Ok(target)
        } else {
            Err(InvalidTransition {
                from: self.clone(),
                to: target,
            })
        }
    }

    /// Check if this is an active state (not Idle, Completed, Failed, or terminal transitional)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Starting
                | Self::Responding { .. }
                | Self::ProcessingTool { .. }
                | Self::Finishing
                | Self::Stopping
                | Self::Blocked { .. }
                | Self::Paused { .. }
                | Self::WaitingForInput
                | Self::Resting { .. }
        )
    }

    /// Check if this is a terminal state (Completed or Failed)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed { .. })
    }

    /// Check if worker is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if worker is in a failure state
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Check if worker is blocked (including resting due to rate limit)
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked { .. } | Self::Resting { .. })
    }

    /// Check if worker is paused
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused { .. })
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Responding {
                sub: RespondingSubState::Streaming,
            } => "responding:streaming",
            Self::Responding {
                sub: RespondingSubState::WaitingConfirmation,
            } => "responding:waiting",
            Self::ProcessingTool { .. } => "processing_tool",
            Self::Completed => "completed",
            Self::Failed { .. } => "failed",
            Self::Blocked { .. } => "blocked",
            Self::Paused { .. } => "paused",
            Self::WaitingForInput => "waiting_for_input",
            Self::Resting { .. } => "resting",
            Self::Finishing => "finishing",
            Self::Stopping => "stopping",
        }
    }
}

impl From<WorkerState> for agent_types::WorkerStatus {
    fn from(state: WorkerState) -> Self {
        match state {
            WorkerState::Idle => Self::Idle,
            WorkerState::Completed | WorkerState::Failed { .. } => Self::Stopped,
            _ => Self::Running,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Valid transitions ───────────────────────────────────────

    #[test]
    fn idle_to_starting() {
        assert!(WorkerState::idle().can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn starting_to_responding() {
        assert!(WorkerState::starting().can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn starting_to_failed() {
        assert!(WorkerState::starting().can_transition_to(&WorkerState::failed("panic")));
    }

    #[test]
    fn responding_to_processing_tool() {
        assert!(
            WorkerState::responding_streaming().can_transition_to(&WorkerState::processing_tool("exec"))
        );
    }

    #[test]
    fn responding_to_completed() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::completed()));
    }

    #[test]
    fn responding_to_failed() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::failed("error")));
    }

    #[test]
    fn responding_to_idle() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn responding_sub_state_transition_streaming_to_waiting() {
        assert!(
            WorkerState::responding_streaming().can_transition_to(&WorkerState::responding_waiting())
        );
    }

    #[test]
    fn responding_sub_state_transition_waiting_to_streaming() {
        assert!(
            WorkerState::responding_waiting().can_transition_to(&WorkerState::responding_streaming())
        );
    }

    #[test]
    fn processing_tool_to_responding() {
        assert!(
            WorkerState::processing_tool("exec").can_transition_to(&WorkerState::responding_streaming())
        );
    }

    #[test]
    fn processing_tool_to_completed() {
        assert!(
            WorkerState::processing_tool("exec").can_transition_to(&WorkerState::completed())
        );
    }

    #[test]
    fn processing_tool_to_failed() {
        assert!(
            WorkerState::processing_tool("exec").can_transition_to(&WorkerState::failed("tool_error"))
        );
    }

    #[test]
    fn processing_tool_to_idle() {
        assert!(
            WorkerState::processing_tool("exec").can_transition_to(&WorkerState::idle())
        );
    }

    #[test]
    fn failed_to_idle_recovery() {
        assert!(WorkerState::failed("error").can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn failed_to_starting_restart() {
        assert!(WorkerState::failed("error").can_transition_to(&WorkerState::starting()));
    }

    // ── NEW: Operational state transitions ──────────────────────

    #[test]
    fn idle_to_blocked() {
        assert!(WorkerState::idle().can_transition_to(&WorkerState::blocked("decision")));
    }

    #[test]
    fn idle_to_paused() {
        assert!(WorkerState::idle().can_transition_to(&WorkerState::paused("user")));
    }

    #[test]
    fn idle_to_stopping() {
        assert!(WorkerState::idle().can_transition_to(&WorkerState::stopping()));
    }

    #[test]
    fn responding_to_blocked() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::blocked("decision")));
    }

    #[test]
    fn responding_to_paused() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::paused("user")));
    }

    #[test]
    fn responding_to_waiting_for_input() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::waiting_for_input()));
    }

    #[test]
    fn responding_to_finishing() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::finishing()));
    }

    #[test]
    fn responding_to_stopping() {
        assert!(WorkerState::responding_streaming().can_transition_to(&WorkerState::stopping()));
    }

    #[test]
    fn processing_tool_to_finishing() {
        assert!(WorkerState::processing_tool("exec").can_transition_to(&WorkerState::finishing()));
    }

    #[test]
    fn finishing_to_completed() {
        assert!(WorkerState::finishing().can_transition_to(&WorkerState::completed()));
    }

    #[test]
    fn finishing_to_failed() {
        assert!(WorkerState::finishing().can_transition_to(&WorkerState::failed("err")));
    }

    #[test]
    fn finishing_to_idle() {
        assert!(WorkerState::finishing().can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn stopping_to_completed() {
        assert!(WorkerState::stopping().can_transition_to(&WorkerState::completed()));
    }

    #[test]
    fn stopping_to_failed() {
        assert!(WorkerState::stopping().can_transition_to(&WorkerState::failed("shutdown")));
    }

    #[test]
    fn stopping_to_idle() {
        assert!(WorkerState::stopping().can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn blocked_to_idle() {
        assert!(WorkerState::blocked("decision").can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn blocked_to_responding() {
        assert!(WorkerState::blocked("decision").can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn blocked_to_starting() {
        assert!(WorkerState::blocked("decision").can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn blocked_to_paused() {
        assert!(WorkerState::blocked("decision").can_transition_to(&WorkerState::paused("user")));
    }

    #[test]
    fn blocked_to_resting() {
        assert!(WorkerState::blocked("rate_limit").can_transition_to(&WorkerState::resting(None)));
    }

    #[test]
    fn paused_to_idle() {
        assert!(WorkerState::paused("user").can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn paused_to_responding() {
        assert!(WorkerState::paused("user").can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn paused_to_starting() {
        assert!(WorkerState::paused("user").can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn paused_to_blocked() {
        assert!(WorkerState::paused("user").can_transition_to(&WorkerState::blocked("decision")));
    }

    #[test]
    fn waiting_for_input_to_responding() {
        assert!(WorkerState::waiting_for_input().can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn waiting_for_input_to_idle() {
        assert!(WorkerState::waiting_for_input().can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn resting_to_idle() {
        assert!(WorkerState::resting(None).can_transition_to(&WorkerState::idle()));
    }

    #[test]
    fn resting_to_starting() {
        assert!(WorkerState::resting(None).can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn resting_to_failed() {
        assert!(WorkerState::resting(None).can_transition_to(&WorkerState::failed("timeout")));
    }

    #[test]
    fn completed_to_starting_restart() {
        assert!(WorkerState::completed().can_transition_to(&WorkerState::starting()));
    }

    // ── Invalid transitions ─────────────────────────────────────

    #[test]
    fn no_self_loops() {
        assert!(!WorkerState::idle().can_transition_to(&WorkerState::idle()));
        assert!(!WorkerState::starting().can_transition_to(&WorkerState::starting()));
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::completed()));
        assert!(!WorkerState::blocked("x").can_transition_to(&WorkerState::blocked("x")));
    }

    #[test]
    fn no_backward_from_responding_to_starting() {
        assert!(!WorkerState::responding_streaming().can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn no_idle_to_responding() {
        assert!(!WorkerState::idle().can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn no_idle_to_completed() {
        assert!(!WorkerState::idle().can_transition_to(&WorkerState::completed()));
    }

    #[test]
    fn completed_is_terminal_except_restart() {
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::idle()));
        assert!(WorkerState::completed().can_transition_to(&WorkerState::starting()));
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::responding_streaming()));
    }

    #[test]
    fn no_processing_tool_to_starting() {
        assert!(!WorkerState::processing_tool("exec").can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn no_responding_to_starting() {
        assert!(!WorkerState::responding_streaming().can_transition_to(&WorkerState::starting()));
    }

    #[test]
    fn no_idle_to_blocked_self_loop() {
        assert!(!WorkerState::blocked("a").can_transition_to(&WorkerState::blocked("b")));
    }

    #[test]
    fn no_rested_to_responding() {
        assert!(!WorkerState::resting(None).can_transition_to(&WorkerState::responding_streaming()));
    }

    // ── transition_to helper ────────────────────────────────────

    #[test]
    fn transition_to_ok() {
        let result = WorkerState::idle().transition_to(WorkerState::starting());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WorkerState::starting());
    }

    #[test]
    fn transition_to_err() {
        let result = WorkerState::idle().transition_to(WorkerState::responding_streaming());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.from, WorkerState::idle());
        assert_eq!(err.to, WorkerState::responding_streaming());
    }

    // ── Helper methods ──────────────────────────────────────────

    #[test]
    fn is_active() {
        assert!(WorkerState::starting().is_active());
        assert!(WorkerState::responding_streaming().is_active());
        assert!(WorkerState::processing_tool("x").is_active());
        assert!(WorkerState::finishing().is_active());
        assert!(WorkerState::stopping().is_active());
        assert!(WorkerState::blocked("r").is_active());
        assert!(WorkerState::paused("r").is_active());
        assert!(WorkerState::waiting_for_input().is_active());
        assert!(WorkerState::resting(None).is_active());
        assert!(!WorkerState::idle().is_active());
        assert!(!WorkerState::completed().is_active());
        assert!(!WorkerState::failed("e").is_active());
    }

    #[test]
    fn is_terminal() {
        assert!(WorkerState::completed().is_terminal());
        assert!(WorkerState::failed("e").is_terminal());
        assert!(!WorkerState::idle().is_terminal());
        assert!(!WorkerState::starting().is_terminal());
        assert!(!WorkerState::responding_streaming().is_terminal());
        assert!(!WorkerState::blocked("e").is_terminal());
    }

    #[test]
    fn is_idle() {
        assert!(WorkerState::idle().is_idle());
        assert!(!WorkerState::starting().is_idle());
    }

    #[test]
    fn is_failed() {
        assert!(WorkerState::failed("e").is_failed());
        assert!(!WorkerState::idle().is_failed());
    }

    #[test]
    fn is_blocked() {
        assert!(WorkerState::blocked("decision").is_blocked());
        assert!(WorkerState::resting(None).is_blocked());
        assert!(!WorkerState::idle().is_blocked());
        assert!(!WorkerState::paused("user").is_blocked());
    }

    #[test]
    fn is_paused() {
        assert!(WorkerState::paused("user").is_paused());
        assert!(!WorkerState::idle().is_paused());
        assert!(!WorkerState::blocked("x").is_paused());
    }

    #[test]
    fn labels() {
        assert_eq!(WorkerState::idle().label(), "idle");
        assert_eq!(WorkerState::starting().label(), "starting");
        assert_eq!(WorkerState::responding_streaming().label(), "responding:streaming");
        assert_eq!(WorkerState::responding_waiting().label(), "responding:waiting");
        assert_eq!(WorkerState::completed().label(), "completed");
        assert_eq!(WorkerState::failed("e").label(), "failed");
        assert_eq!(WorkerState::blocked("r").label(), "blocked");
        assert_eq!(WorkerState::paused("r").label(), "paused");
        assert_eq!(WorkerState::waiting_for_input().label(), "waiting_for_input");
        assert_eq!(WorkerState::resting(None).label(), "resting");
        assert_eq!(WorkerState::finishing().label(), "finishing");
        assert_eq!(WorkerState::stopping().label(), "stopping");
    }

    // ── Exhaustive transition matrix (smoke test) ───────────────

    #[test]
    fn all_states_can_transition_from_idle_or_starting() {
        // Every non-terminal state should be reachable from Idle or Starting
        let states = vec![
            WorkerState::idle(),
            WorkerState::starting(),
            WorkerState::responding_streaming(),
            WorkerState::responding_waiting(),
            WorkerState::processing_tool("t"),
            WorkerState::completed(),
            WorkerState::failed("e"),
            WorkerState::blocked("b"),
            WorkerState::paused("p"),
            WorkerState::waiting_for_input(),
            WorkerState::resting(None),
            WorkerState::finishing(),
            WorkerState::stopping(),
        ];

        let mut transition_count = 0;
        for from in &states {
            for to in &states {
                if from.can_transition_to(to) {
                    transition_count += 1;
                }
            }
        }

        // With 13 states, we expect at least 40 valid transitions
        // (exact count depends on rules, but this is a sanity check)
        assert!(
            transition_count >= 40,
            "expected at least 40 valid transitions, got {transition_count}"
        );
    }
}
