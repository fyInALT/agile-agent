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
}