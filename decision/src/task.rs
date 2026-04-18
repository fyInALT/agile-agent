//! Task entity for decision layer (Sprint 09)
//!
//! Provides Task entity with lifecycle tracking for decision workflows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Forward reference to ExecutionRecord defined in persistence module
// This is resolved at compile time after both modules are processed
use crate::persistence::ExecutionRecord;

/// Unique task identifier (UUID-based)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// Generate a new unique TaskId
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Task status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting to start
    Pending,
    /// Task is actively being executed
    InProgress,
    /// Task is in reflection cycle
    Reflecting,
    /// Task passed verification, waiting for confirmation
    PendingConfirmation,
    /// Task blocked, requires human decision
    NeedsHumanDecision,
    /// Task paused (timeout, system error)
    Paused,
    /// Task completed
    Completed,
    /// Task cancelled
    Cancelled,
}

impl TaskStatus {
    /// Get display text for TUI rendering
    pub fn display(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::InProgress => "In Progress",
            Self::Reflecting => "Reflecting",
            Self::PendingConfirmation => "Awaiting Confirmation",
            Self::NeedsHumanDecision => "Needs Decision",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display())
    }
}

/// Error type for invalid task status transitions
#[derive(Debug, Clone, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },
}

/// Task entity with lifecycle tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,
    /// Task description from Sprint Backlog
    pub description: String,
    /// Task boundary constraints
    pub constraints: Vec<String>,
    /// Current task status
    pub status: TaskStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Number of reflection cycles executed
    pub reflection_count: usize,
    /// Number of completion confirmation attempts
    pub confirmation_count: usize,
    /// Maximum allowed reflection rounds
    pub max_reflection_rounds: usize,
    /// Retry count for error recovery
    pub retry_count: usize,
    /// Execution history records
    pub execution_history: Vec<ExecutionRecord>,
}

impl Task {
    /// Create a new task with given description and constraints
    pub fn new(description: String, constraints: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: TaskId::generate(),
            description,
            constraints,
            status: TaskStatus::Pending,
            created_at: now,
            updated_at: now,
            reflection_count: 0,
            confirmation_count: 0,
            max_reflection_rounds: 2,
            retry_count: 0,
            execution_history: Vec::new(),
        }
    }

    /// Transition to a new status
    ///
    /// Returns error if the transition is invalid.
    /// Updates `updated_at` timestamp on successful transition.
    pub fn transition_to(&mut self, new_status: TaskStatus) -> Result<(), TransitionError> {
        if !self.is_valid_transition(new_status) {
            return Err(TransitionError::InvalidTransition {
                from: self.status,
                to: new_status,
            });
        }

        self.status = new_status;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Check if transition to new status is valid
    fn is_valid_transition(&self, new_status: TaskStatus) -> bool {
        match (self.status, new_status) {
            // Pending → InProgress
            (TaskStatus::Pending, TaskStatus::InProgress) => true,

            // InProgress → Reflecting, PendingConfirmation, Paused, Cancelled
            (TaskStatus::InProgress, TaskStatus::Reflecting) => true,
            (TaskStatus::InProgress, TaskStatus::PendingConfirmation) => true,
            (TaskStatus::InProgress, TaskStatus::Paused) => true,
            (TaskStatus::InProgress, TaskStatus::Cancelled) => true,

            // Reflecting → InProgress, NeedsHumanDecision, Paused
            (TaskStatus::Reflecting, TaskStatus::InProgress) => true,
            (TaskStatus::Reflecting, TaskStatus::NeedsHumanDecision) => true,
            (TaskStatus::Reflecting, TaskStatus::Paused) => true, // Recovery transition

            // PendingConfirmation → Completed, Reflecting
            (TaskStatus::PendingConfirmation, TaskStatus::Completed) => true,
            (TaskStatus::PendingConfirmation, TaskStatus::Reflecting) => true,

            // NeedsHumanDecision → InProgress, Cancelled
            (TaskStatus::NeedsHumanDecision, TaskStatus::InProgress) => true,
            (TaskStatus::NeedsHumanDecision, TaskStatus::Cancelled) => true,

            // Paused → InProgress, Cancelled
            (TaskStatus::Paused, TaskStatus::InProgress) => true,
            (TaskStatus::Paused, TaskStatus::Cancelled) => true,

            // Any → Cancelled (except Completed which is terminal)
            (_, TaskStatus::Cancelled) => self.status != TaskStatus::Completed,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Check if task is actively executing
    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress | TaskStatus::Reflecting)
    }

    /// Check if task is completed
    pub fn is_complete(&self) -> bool {
        self.status == TaskStatus::Completed
    }

    /// Check if more reflection rounds are available
    pub fn needs_reflection(&self) -> bool {
        self.reflection_count < self.max_reflection_rounds
    }

    /// Check if task can continue execution
    pub fn can_continue(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress | TaskStatus::Reflecting)
    }

    /// Check if task is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.status == TaskStatus::Cancelled
    }

    /// Check if task needs human decision
    pub fn needs_human(&self) -> bool {
        self.status == TaskStatus::NeedsHumanDecision
    }

    /// Check if task is paused
    pub fn is_paused(&self) -> bool {
        self.status == TaskStatus::Paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 9.1 Tests

    #[test]
    fn t9_1_t1_task_id_generation_is_unique() {
        let id1 = TaskId::generate();
        let id2 = TaskId::generate();
        let id3 = TaskId::generate();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn t9_1_t2_task_created_with_correct_defaults() {
        let task = Task::new(
            "Fix login bug".to_string(),
            vec!["Do not modify auth.rs".to_string()],
        );

        assert!(!task.id.to_string().is_empty());
        assert_eq!(task.description, "Fix login bug");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.reflection_count, 0);
        assert_eq!(task.confirmation_count, 0);
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_reflection_rounds, 2);
    }

    #[test]
    fn t9_1_t3_task_serialization_deserialization_works() {
        let original = Task::new(
            "Add logout feature".to_string(),
            vec!["Only modify logout.rs".to_string(), "Add tests".to_string()],
        );

        let json = serde_json::to_string(&original).expect("Should serialize");
        let deserialized: Task = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.description, original.description);
        assert_eq!(deserialized.constraints, original.constraints);
        assert_eq!(deserialized.status, original.status);
        assert_eq!(deserialized.reflection_count, original.reflection_count);
        assert_eq!(deserialized.confirmation_count, original.confirmation_count);
    }

    #[test]
    fn t9_1_t4_task_constraints_stored_correctly() {
        let constraints = vec![
            "Do not modify auth.rs".to_string(),
            "Add unit tests".to_string(),
            "Follow existing code style".to_string(),
        ];

        let task = Task::new("Fix login bug".to_string(), constraints.clone());

        assert_eq!(task.constraints.len(), 3);
        assert_eq!(task.constraints[0], "Do not modify auth.rs");
        assert_eq!(task.constraints[1], "Add unit tests");
        assert_eq!(task.constraints[2], "Follow existing code style");
    }

    #[test]
    fn t9_1_t5_timestamps_set_on_creation() {
        let before = Utc::now();
        let task = Task::new("Test task".to_string(), vec![]);
        let after = Utc::now();

        assert!(task.created_at >= before);
        assert!(task.created_at <= after);
        assert_eq!(task.created_at, task.updated_at);
    }

    #[test]
    fn t9_1_t6_task_id_serializable() {
        let id = TaskId::generate();
        let json = serde_json::to_string(&id).expect("Should serialize");
        let deserialized: TaskId = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(id, deserialized);
    }

    // Story 9.2 Tests: Task Status Enumeration

    #[test]
    fn t9_2_t1_all_status_variants_defined() {
        // Verify all 8 status variants exist
        let statuses = [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Reflecting,
            TaskStatus::PendingConfirmation,
            TaskStatus::NeedsHumanDecision,
            TaskStatus::Paused,
            TaskStatus::Completed,
            TaskStatus::Cancelled,
        ];

        // Each should be distinct
        for (i, s1) in statuses.iter().enumerate() {
            for (j, s2) in statuses.iter().enumerate() {
                if i != j {
                    assert_ne!(s1, s2, "Status variants should be distinct");
                }
            }
        }
    }

    #[test]
    fn t9_2_t2_status_display_returns_readable_text() {
        assert_eq!(TaskStatus::Pending.display(), "Pending");
        assert_eq!(TaskStatus::InProgress.display(), "In Progress");
        assert_eq!(TaskStatus::Reflecting.display(), "Reflecting");
        assert_eq!(TaskStatus::PendingConfirmation.display(), "Awaiting Confirmation");
        assert_eq!(TaskStatus::NeedsHumanDecision.display(), "Needs Decision");
        assert_eq!(TaskStatus::Paused.display(), "Paused");
        assert_eq!(TaskStatus::Completed.display(), "Completed");
        assert_eq!(TaskStatus::Cancelled.display(), "Cancelled");
    }

    #[test]
    fn t9_2_t3_status_serialization_works() {
        // Test serialization of each status
        for status in [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Reflecting,
            TaskStatus::PendingConfirmation,
            TaskStatus::NeedsHumanDecision,
            TaskStatus::Paused,
            TaskStatus::Completed,
            TaskStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).expect("Should serialize");
            let deserialized: TaskStatus =
                serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(status, deserialized, "Status should roundtrip correctly");
        }
    }

    // Task Status Transitions Tests

    #[test]
    fn t9_3_t1_pending_to_inprogress_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        assert_eq!(task.status, TaskStatus::Pending);

        task.transition_to(TaskStatus::InProgress).expect("Should work");

        assert_eq!(task.status, TaskStatus::InProgress);
    }

    #[test]
    fn t9_3_t2_inprogress_to_reflecting_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();

        task.transition_to(TaskStatus::Reflecting).expect("Should work");

        assert_eq!(task.status, TaskStatus::Reflecting);
    }

    #[test]
    fn t9_3_t3_inprogress_to_pending_confirmation_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();

        task.transition_to(TaskStatus::PendingConfirmation).expect("Should work");

        assert_eq!(task.status, TaskStatus::PendingConfirmation);
    }

    #[test]
    fn t9_3_t4_reflecting_to_inprogress_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::Reflecting).unwrap();

        task.transition_to(TaskStatus::InProgress).expect("Should work");

        assert_eq!(task.status, TaskStatus::InProgress);
    }

    #[test]
    fn t9_3_t5_reflecting_to_needs_human_decision_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::Reflecting).unwrap();

        task.transition_to(TaskStatus::NeedsHumanDecision).expect("Should work");

        assert_eq!(task.status, TaskStatus::NeedsHumanDecision);
    }

    #[test]
    fn t9_3_t6_pending_confirmation_to_completed_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::PendingConfirmation).unwrap();

        task.transition_to(TaskStatus::Completed).expect("Should work");

        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[test]
    fn t9_3_t7_pending_confirmation_to_reflecting_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::PendingConfirmation).unwrap();

        task.transition_to(TaskStatus::Reflecting).expect("Should work");

        assert_eq!(task.status, TaskStatus::Reflecting);
    }

    #[test]
    fn t9_3_t8_invalid_transition_returns_error() {
        let mut task = Task::new("Test".to_string(), vec![]);
        // Pending -> Completed is invalid (must go through InProgress)

        let result = task.transition_to(TaskStatus::Completed);

        assert!(result.is_err(), "Pending -> Completed should be invalid");
    }

    #[test]
    fn t9_3_t9_transition_updates_timestamp() {
        let mut task = Task::new("Test".to_string(), vec![]);
        let original_updated_at = task.updated_at;

        // Wait a tiny bit to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(1));
        task.transition_to(TaskStatus::InProgress).unwrap();

        assert!(task.updated_at > original_updated_at, "updated_at should be updated");
    }

    #[test]
    fn t9_3_t10_any_to_cancelled_works() {
        // Any status can transition to Cancelled
        for initial_status in [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Reflecting,
            TaskStatus::PendingConfirmation,
            TaskStatus::NeedsHumanDecision,
            TaskStatus::Paused,
        ] {
            let mut task = Task::new("Test".to_string(), vec![]);
            // Manually set status for testing
            task.status = initial_status;

            task.transition_to(TaskStatus::Cancelled).expect(&format!(
                "{} -> Cancelled should work",
                initial_status.display()
            ));

            assert_eq!(task.status, TaskStatus::Cancelled);
        }
    }

    #[test]
    fn t9_3_t11_paused_to_inprogress_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.status = TaskStatus::Paused;

        task.transition_to(TaskStatus::InProgress).expect("Paused -> InProgress should work");

        assert_eq!(task.status, TaskStatus::InProgress);
    }

    #[test]
    fn t9_3_t12_paused_to_cancelled_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.status = TaskStatus::Paused;

        task.transition_to(TaskStatus::Cancelled).expect("Paused -> Cancelled should work");

        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    fn t9_3_t13_needs_human_to_inprogress_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.status = TaskStatus::NeedsHumanDecision;

        task.transition_to(TaskStatus::InProgress).expect("NeedsHumanDecision -> InProgress should work");

        assert_eq!(task.status, TaskStatus::InProgress);
    }

    #[test]
    fn t9_3_t14_needs_human_to_cancelled_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.status = TaskStatus::NeedsHumanDecision;

        task.transition_to(TaskStatus::Cancelled).expect("NeedsHumanDecision -> Cancelled should work");

        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    fn t9_3_t15_inprogress_to_paused_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();

        task.transition_to(TaskStatus::Paused).expect("InProgress -> Paused should work");

        assert_eq!(task.status, TaskStatus::Paused);
    }

    #[test]
    fn t9_3_t16_inprogress_to_cancelled_works() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).unwrap();

        task.transition_to(TaskStatus::Cancelled).expect("InProgress -> Cancelled should work");

        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    // Task Helper Methods Tests

    #[test]
    fn t9_4_t1_is_active_true_for_inprogress_and_reflecting() {
        let mut task = Task::new("Test".to_string(), vec![]);

        // Pending - not active
        assert!(!task.is_active());

        // InProgress - active
        task.transition_to(TaskStatus::InProgress).unwrap();
        assert!(task.is_active());

        // Reflecting - active
        task.transition_to(TaskStatus::Reflecting).unwrap();
        assert!(task.is_active());

        // PendingConfirmation - not active
        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::PendingConfirmation).unwrap();
        assert!(!task.is_active());
    }

    #[test]
    fn t9_4_t2_is_complete_true_for_completed() {
        let mut task = Task::new("Test".to_string(), vec![]);
        assert!(!task.is_complete());

        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::PendingConfirmation).unwrap();
        task.transition_to(TaskStatus::Completed).unwrap();
        assert!(task.is_complete());
    }

    #[test]
    fn t9_4_t3_needs_reflection_checks_count_vs_limit() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.max_reflection_rounds = 2;

        // 0 < 2 - needs reflection
        assert!(task.needs_reflection());

        task.reflection_count = 1;
        // 1 < 2 - needs reflection
        assert!(task.needs_reflection());

        task.reflection_count = 2;
        // 2 >= 2 - does not need reflection
        assert!(!task.needs_reflection());
    }

    #[test]
    fn t9_4_t4_can_continue_checks_appropriate_states() {
        let mut task = Task::new("Test".to_string(), vec![]);

        // Pending - cannot continue
        assert!(!task.can_continue());

        // InProgress - can continue
        task.transition_to(TaskStatus::InProgress).unwrap();
        assert!(task.can_continue());

        // Reflecting - can continue
        task.transition_to(TaskStatus::Reflecting).unwrap();
        assert!(task.can_continue());

        // NeedsHumanDecision - cannot continue
        task.status = TaskStatus::NeedsHumanDecision;
        assert!(!task.can_continue());

        // Paused - cannot continue
        task.status = TaskStatus::Paused;
        assert!(!task.can_continue());
    }

    #[test]
    fn t9_4_t5_is_cancelled_true_for_cancelled() {
        let mut task = Task::new("Test".to_string(), vec![]);
        assert!(!task.is_cancelled());

        task.transition_to(TaskStatus::Cancelled).unwrap();
        assert!(task.is_cancelled());
    }

    #[test]
    fn t9_4_t6_needs_human_true_for_needs_human_decision() {
        let mut task = Task::new("Test".to_string(), vec![]);
        assert!(!task.needs_human());

        task.status = TaskStatus::NeedsHumanDecision;
        assert!(task.needs_human());
    }

    #[test]
    fn t9_4_t7_is_paused_true_for_paused() {
        let mut task = Task::new("Test".to_string(), vec![]);
        assert!(!task.is_paused());

        task.transition_to(TaskStatus::InProgress).unwrap();
        task.transition_to(TaskStatus::Paused).unwrap();
        assert!(task.is_paused());
    }
}