use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Candidate,
    Ready,
    InProgress,
    Blocked,
    Done,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklogState {
    pub todos: Vec<TodoItem>,
    pub tasks: Vec<TaskItem>,
}

impl BacklogState {
    pub fn ready_todos(&self) -> Vec<&TodoItem> {
        self.todos
            .iter()
            .filter(|todo| todo.status == TodoStatus::Ready)
            .collect()
    }

    pub fn find_todo_mut(&mut self, todo_id: &str) -> Option<&mut TodoItem> {
        self.todos.iter_mut().find(|todo| todo.id == todo_id)
    }

    pub fn push_todo(&mut self, todo: TodoItem) {
        self.todos.push(todo);
        self.todos
            .sort_by(|a, b| a.priority.cmp(&b.priority).then(a.title.cmp(&b.title)));
    }

    pub fn push_task(&mut self, task: TaskItem) {
        self.tasks.push(task);
    }
}

#[cfg(test)]
mod tests {
    use super::BacklogState;
    use super::TaskItem;
    use super::TaskStatus;
    use super::TodoItem;
    use super::TodoStatus;

    fn todo(id: &str, title: &str, priority: u8, status: TodoStatus) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            title: title.to_string(),
            description: String::new(),
            priority,
            status,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn ready_todos_only_returns_ready_items() {
        let mut backlog = BacklogState::default();
        backlog.push_todo(todo("1", "ready", 1, TodoStatus::Ready));
        backlog.push_todo(todo("2", "blocked", 2, TodoStatus::Blocked));

        let ready = backlog.ready_todos();

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "1");
    }

    #[test]
    fn push_todo_keeps_priority_order() {
        let mut backlog = BacklogState::default();
        backlog.push_todo(todo("2", "later", 2, TodoStatus::Ready));
        backlog.push_todo(todo("1", "sooner", 1, TodoStatus::Ready));

        assert_eq!(backlog.todos[0].id, "1");
        assert_eq!(backlog.todos[1].id, "2");
    }

    #[test]
    fn push_task_adds_task() {
        let mut backlog = BacklogState::default();
        backlog.push_task(TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "do thing".to_string(),
            scope: "current repo".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Ready,
            result_summary: None,
        });

        assert_eq!(backlog.tasks.len(), 1);
        assert_eq!(backlog.tasks[0].id, "task-1");
    }
}
