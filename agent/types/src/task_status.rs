use serde::{Deserialize, Serialize};

/// Status of a todo item in backlog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Candidate,
    Ready,
    InProgress,
    Blocked,
    Done,
    Dropped,
}

/// Status of a task in execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    Draft,
    Ready,
    Running,
    Verifying,
    #[serde(alias = "Completed")]
    Done,
    Blocked,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_status_serialization() {
        let status = TodoStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"InProgress\"");
    }

    #[test]
    fn task_status_serialization() {
        let status = TaskStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Running\"");
    }

    #[test]
    fn task_status_done_alias() {
        // Test that "Completed" alias deserializes to Done
        let status: TaskStatus = serde_json::from_str("\"Completed\"").unwrap();
        assert_eq!(status, TaskStatus::Done);
    }

    #[test]
    fn todo_status_all_values_roundtrip() {
        let all_statuses = [
            TodoStatus::Candidate,
            TodoStatus::Ready,
            TodoStatus::InProgress,
            TodoStatus::Blocked,
            TodoStatus::Done,
            TodoStatus::Dropped,
        ];
        for status in all_statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: TodoStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn task_status_all_values_roundtrip() {
        let all_statuses = [
            TaskStatus::Draft,
            TaskStatus::Ready,
            TaskStatus::Running,
            TaskStatus::Verifying,
            TaskStatus::Done,
            TaskStatus::Blocked,
            TaskStatus::Failed,
        ];
        for status in all_statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn task_status_invalid_value() {
        let result: Result<TaskStatus, _> = serde_json::from_str("\"Invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn todo_status_invalid_value() {
        let result: Result<TodoStatus, _> = serde_json::from_str("\"Invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn task_status_hash_consistency() {
        use std::collections::HashSet;

        let status1 = TaskStatus::Running;
        let status2 = TaskStatus::Running;
        let status3 = TaskStatus::Done;

        let set: HashSet<TaskStatus> = [status1, status2, status3].into_iter().collect();
        assert_eq!(set.len(), 2);
    }
}