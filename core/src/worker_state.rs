//! WorkerState — explicit state machine for the Worker aggregate root.
//!
//! This enum defines the core lifecycle states of an agent worker.
//! It is intentionally simpler than AgentSlotStatus, focusing on the
//! domain-model state rather than operational edge cases.
//!
//! State transition rules enforce valid paths and prevent invalid jumps
//! (e.g., Idle → Responding is not allowed — must go through Starting).

use std::fmt;

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

    /// Check if a transition from `self` to `target` is valid.
    ///
    /// Rules:
    /// - Forward-only: no backward jumps (except Error → Idle recovery)
    /// - No self-loops (same state → false)
    /// - Starting must precede Responding
    /// - Responding can go to ProcessingTool or terminal states
    /// - ProcessingTool can go back to Responding or terminal states
    /// - Completed/Failed are terminal (only Idle or Starting from Failed)
    pub fn can_transition_to(&self, target: &WorkerState) -> bool {
        // No self-loops
        if self == target {
            return false;
        }

        match (self, target) {
            // Idle can only go to Starting
            (Self::Idle, Self::Starting) => true,

            // Starting can go to Responding or Failed
            (Self::Starting, Self::Responding { .. }) => true,
            (Self::Starting, Self::Failed { .. }) => true,

            // Responding can go to ProcessingTool, terminal states, or sub-state changes
            (Self::Responding { .. }, Self::ProcessingTool { .. }) => true,
            (Self::Responding { .. }, Self::Completed) => true,
            (Self::Responding { .. }, Self::Failed { .. }) => true,
            (Self::Responding { .. }, Self::Idle) => true,
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

            // ProcessingTool can go back to Responding or to terminal states
            (Self::ProcessingTool { .. }, Self::Responding { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Completed) => true,
            (Self::ProcessingTool { .. }, Self::Failed { .. }) => true,
            (Self::ProcessingTool { .. }, Self::Idle) => true,

            // Failed can recover to Idle (for retry) or Starting (for restart)
            (Self::Failed { .. }, Self::Idle) => true,
            (Self::Failed { .. }, Self::Starting) => true,

            // Completed is terminal — no transitions out
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

    /// Check if this is an active state (not Idle, Completed, or Failed)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Responding { .. } | Self::ProcessingTool { .. }
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

    // ── Invalid transitions ─────────────────────────────────────

    #[test]
    fn no_self_loops() {
        assert!(!WorkerState::idle().can_transition_to(&WorkerState::idle()));
        assert!(!WorkerState::starting().can_transition_to(&WorkerState::starting()));
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::completed()));
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
    fn completed_is_terminal() {
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::idle()));
        assert!(!WorkerState::completed().can_transition_to(&WorkerState::starting()));
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
    fn labels() {
        assert_eq!(WorkerState::idle().label(), "idle");
        assert_eq!(WorkerState::starting().label(), "starting");
        assert_eq!(WorkerState::responding_streaming().label(), "responding:streaming");
        assert_eq!(WorkerState::responding_waiting().label(), "responding:waiting");
        assert_eq!(WorkerState::completed().label(), "completed");
        assert_eq!(WorkerState::failed("e").label(), "failed");
    }
}
