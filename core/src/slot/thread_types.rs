//! Thread completion types for agent slots
//!
//! Defines outcomes when agent threads finish execution.

/// Result of a task completion
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCompletionResult {
    /// Task completed successfully
    Success,
    /// Task failed with error
    Failure { error: String },
}

/// Outcome when agent thread finishes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadOutcome {
    /// Thread exited normally (task completed or stopped gracefully)
    NormalExit,
    /// Thread exited with error
    ErrorExit { error: String },
    /// Thread was cancelled (force stop)
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_completion_result_success() {
        let result = TaskCompletionResult::Success;
        assert_eq!(result, TaskCompletionResult::Success);
    }

    #[test]
    fn task_completion_result_failure() {
        let result = TaskCompletionResult::Failure { error: "test error".to_string() };
        assert!(result != TaskCompletionResult::Success);
    }

    #[test]
    fn thread_outcome_normal_exit() {
        let outcome = ThreadOutcome::NormalExit;
        assert_eq!(outcome, ThreadOutcome::NormalExit);
    }

    #[test]
    fn thread_outcome_error_exit() {
        let outcome = ThreadOutcome::ErrorExit { error: "crash".to_string() };
        assert!(outcome != ThreadOutcome::NormalExit);
    }

    #[test]
    fn thread_outcome_cancelled() {
        let outcome = ThreadOutcome::Cancelled;
        assert!(outcome != ThreadOutcome::NormalExit);
        assert!(outcome != ThreadOutcome::ErrorExit { error: "".to_string() });
    }
}