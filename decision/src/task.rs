//! Task entity for decision layer (Sprint 09)
//!
//! Provides Task entity with lifecycle tracking for decision workflows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}