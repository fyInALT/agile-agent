use serde::{Deserialize, Serialize};

use super::task_status::{TaskStatus, TodoStatus};

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A todo item in the backlog
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: u8,
    pub status: TodoStatus,
    pub acceptance_criteria: Vec<String>,
    pub dependencies: Vec<String>,
    pub source: String,
}

/// A task derived from a todo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub todo_id: String,
    pub objective: String,
    pub scope: String,
    pub constraints: Vec<String>,
    pub verification_plan: Vec<String>,
    pub status: TaskStatus,
    pub result_summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_creation() {
        let id = TaskId::new("task-001");
        assert_eq!(id.as_str(), "task-001");
    }

    #[test]
    fn todo_item_serialization() {
        let todo = TodoItem {
            id: "todo-1".to_string(),
            title: "Test todo".to_string(),
            description: "Description".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["criteria".to_string()],
            dependencies: vec![],
            source: "user".to_string(),
        };
        let json = serde_json::to_string(&todo).unwrap();
        assert!(json.contains("\"title\":\"Test todo\""));
    }

    #[test]
    fn task_item_serialization() {
        let task = TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "Implement feature".to_string(),
            scope: "Full implementation".to_string(),
            constraints: vec!["No external deps".to_string()],
            verification_plan: vec!["Run tests".to_string()],
            status: TaskStatus::Ready,
            result_summary: None,
        };
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"objective\":\"Implement feature\""));
    }
}