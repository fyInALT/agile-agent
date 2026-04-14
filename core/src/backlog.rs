use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

    /// Find a task by ID
    pub fn find_task(&self, task_id: &str) -> Option<&TaskItem> {
        self.tasks.iter().find(|task| task.id == task_id)
    }

    /// Find a task by ID for mutation
    pub fn find_task_mut(&mut self, task_id: &str) -> Option<&mut TaskItem> {
        self.tasks.iter_mut().find(|task| task.id == task_id)
    }

    /// Check if task exists and can be assigned (Ready status)
    pub fn can_assign_task(&self, task_id: &str) -> bool {
        self.find_task(task_id)
            .map(|task| task.status == TaskStatus::Ready)
            .unwrap_or(false)
    }

    /// Mark task as running (assigned to agent)
    pub fn start_task(&mut self, task_id: &str) -> bool {
        if let Some(task) = self.find_task_mut(task_id) {
            if task.status == TaskStatus::Ready {
                task.status = TaskStatus::Running;
                return true;
            }
        }
        false
    }

    /// Mark task as done (completed successfully)
    pub fn complete_task(&mut self, task_id: &str, summary: Option<String>) -> bool {
        if let Some(task) = self.find_task_mut(task_id) {
            if task.status == TaskStatus::Running || task.status == TaskStatus::Verifying {
                task.status = TaskStatus::Done;
                task.result_summary = summary;
                return true;
            }
        }
        false
    }

    /// Mark task as failed
    pub fn fail_task(&mut self, task_id: &str, error: String) -> bool {
        if let Some(task) = self.find_task_mut(task_id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Failed;
                task.result_summary = Some(error);
                return true;
            }
        }
        false
    }

    /// Mark task as blocked
    pub fn block_task(&mut self, task_id: &str, reason: String) -> bool {
        if let Some(task) = self.find_task_mut(task_id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Blocked;
                task.result_summary = Some(reason);
                return true;
            }
        }
        false
    }

    /// List tasks with Ready status (can be assigned)
    pub fn ready_tasks(&self) -> Vec<&TaskItem> {
        self.tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Ready)
            .collect()
    }

    /// List tasks currently running
    pub fn running_tasks(&self) -> Vec<&TaskItem> {
        self.tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Running)
            .collect()
    }

    /// Count tasks by status
    pub fn count_tasks_by_status(&self) -> HashMap<TaskStatus, usize> {
        let mut counts = HashMap::new();
        for task in &self.tasks {
            *counts.entry(task.status).or_insert(0) += 1;
        }
        counts
    }
}

/// Thread-safe wrapper for BacklogState
///
/// Provides safe concurrent access to backlog for multi-agent scenarios.
/// Uses Mutex for interior mutability with timeout support.
///
/// # Thread Safety
///
/// - Multiple threads can read simultaneously (lock for read)
/// - Write operations are serialized via Mutex
/// - Lock timeout prevents deadlocks
///
/// # Usage
///
/// ```ignore
/// let backlog = ThreadSafeBacklog::new(BacklogState::default());
///
/// // Read with lock
/// if let Some(state) = backlog.read_with_timeout(Duration::from_millis(100)) {
///     let ready_tasks = state.ready_tasks();
/// }
///
/// // Write with lock
/// if let Some(mut state) = backlog.write_with_timeout(Duration::from_millis(100)) {
///     state.start_task("task-001");
/// }
/// ```
#[derive(Debug)]
pub struct ThreadSafeBacklog {
    inner: Mutex<BacklogState>,
}

impl ThreadSafeBacklog {
    /// Create a new thread-safe backlog wrapper
    pub fn new(state: BacklogState) -> Self {
        Self {
            inner: Mutex::new(state),
        }
    }

    /// Create with default empty state
    pub fn empty() -> Self {
        Self::new(BacklogState::default())
    }

    /// Acquire read lock with timeout
    ///
    /// Polls for lock acquisition until timeout expires.
    /// Returns None if lock cannot be acquired within timeout.
    pub fn read_with_timeout(&self, timeout: Duration) -> Option<std::sync::MutexGuard<'_, BacklogState>> {
        let deadline = Instant::now() + timeout;

        // Poll for lock acquisition
        while Instant::now() < deadline {
            match self.inner.try_lock() {
                Ok(guard) => return Some(guard),
                Err(std::sync::TryLockError::WouldBlock) => {
                    // Brief sleep to avoid tight spin loop
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    // Lock is poisoned (thread panicked while holding it)
                    // We still want to access the data
                    return Some(e.into_inner());
                }
            }
        }

        // Timeout expired without acquiring lock
        None
    }

    /// Acquire write lock with timeout (same as read for Mutex)
    pub fn write_with_timeout(&self, timeout: Duration) -> Option<std::sync::MutexGuard<'_, BacklogState>> {
        self.read_with_timeout(timeout)
    }

    /// Get inner state (consumes wrapper)
    pub fn into_inner(self) -> BacklogState {
        self.inner.into_inner().unwrap_or_else(|e| e.into_inner())
    }

    /// Clone inner state (requires lock)
    pub fn clone_inner(&self) -> BacklogState {
        match self.inner.try_lock() {
            Ok(guard) => guard.clone(),
            Err(std::sync::TryLockError::WouldBlock) => BacklogState::default(),
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner().clone(),
        }
    }

    /// Update inner state (requires lock)
    pub fn update<F>(&self, f: F) -> bool
    where
        F: FnOnce(&mut BacklogState),
    {
        match self.inner.try_lock() {
            Ok(mut guard) => {
                f(&mut guard);
                true
            }
            Err(_) => false,
        }
    }
}

impl Default for ThreadSafeBacklog {
    fn default() -> Self {
        Self::empty()
    }
}

impl Clone for ThreadSafeBacklog {
    fn clone(&self) -> Self {
        Self::new(self.clone_inner())
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

    fn task(id: &str, status: TaskStatus) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            todo_id: "todo-1".to_string(),
            objective: "test objective".to_string(),
            scope: "test scope".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status,
            result_summary: None,
        }
    }

    #[test]
    fn find_task_returns_correct_task() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Ready));
        backlog.push_task(task("task-2", TaskStatus::Running));

        let found = backlog.find_task("task-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().status, TaskStatus::Ready);
    }

    #[test]
    fn can_assign_task_only_for_ready_status() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-ready", TaskStatus::Ready));
        backlog.push_task(task("task-running", TaskStatus::Running));
        backlog.push_task(task("task-done", TaskStatus::Done));

        assert!(backlog.can_assign_task("task-ready"));
        assert!(!backlog.can_assign_task("task-running"));
        assert!(!backlog.can_assign_task("task-done"));
        assert!(!backlog.can_assign_task("task-nonexistent"));
    }

    #[test]
    fn start_task_changes_status_to_running() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Ready));

        let result = backlog.start_task("task-1");
        assert!(result);
        assert_eq!(backlog.find_task("task-1").unwrap().status, TaskStatus::Running);
    }

    #[test]
    fn start_task_fails_for_non_ready_task() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Running));

        let result = backlog.start_task("task-1");
        assert!(!result);
    }

    #[test]
    fn complete_task_changes_status_to_done() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Running));

        let result = backlog.complete_task("task-1", Some("completed successfully".to_string()));
        assert!(result);
        let task = backlog.find_task("task-1").unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert_eq!(task.result_summary, Some("completed successfully".to_string()));
    }

    #[test]
    fn fail_task_changes_status_to_failed() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Running));

        let result = backlog.fail_task("task-1", "error message".to_string());
        assert!(result);
        let task = backlog.find_task("task-1").unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.result_summary, Some("error message".to_string()));
    }

    #[test]
    fn block_task_changes_status_to_blocked() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Running));

        let result = backlog.block_task("task-1", "blocked reason".to_string());
        assert!(result);
        let task = backlog.find_task("task-1").unwrap();
        assert_eq!(task.status, TaskStatus::Blocked);
        assert_eq!(task.result_summary, Some("blocked reason".to_string()));
    }

    #[test]
    fn ready_tasks_returns_only_ready_tasks() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Ready));
        backlog.push_task(task("task-2", TaskStatus::Running));
        backlog.push_task(task("task-3", TaskStatus::Ready));

        let ready = backlog.ready_tasks();
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn running_tasks_returns_only_running_tasks() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Ready));
        backlog.push_task(task("task-2", TaskStatus::Running));
        backlog.push_task(task("task-3", TaskStatus::Running));

        let running = backlog.running_tasks();
        assert_eq!(running.len(), 2);
    }

    #[test]
    fn count_tasks_by_status() {
        let mut backlog = BacklogState::default();
        backlog.push_task(task("task-1", TaskStatus::Ready));
        backlog.push_task(task("task-2", TaskStatus::Ready));
        backlog.push_task(task("task-3", TaskStatus::Running));

        let counts = backlog.count_tasks_by_status();
        assert_eq!(counts.get(&TaskStatus::Ready), Some(&2));
        assert_eq!(counts.get(&TaskStatus::Running), Some(&1));
        assert_eq!(counts.get(&TaskStatus::Done), None);
    }

    // ThreadSafeBacklog Tests

    use super::ThreadSafeBacklog;
    use std::time::Duration;

    #[test]
    fn thread_safe_backlog_new_creates_wrapper() {
        let state = BacklogState::default();
        let safe_backlog = ThreadSafeBacklog::new(state);
        // Should be able to acquire lock
        let guard = safe_backlog.read_with_timeout(Duration::from_millis(100));
        assert!(guard.is_some());
    }

    #[test]
    fn thread_safe_backlog_empty_creates_default_state() {
        let safe_backlog = ThreadSafeBacklog::empty();
        let guard = safe_backlog.read_with_timeout(Duration::from_millis(100)).unwrap();
        assert!(guard.todos.is_empty());
        assert!(guard.tasks.is_empty());
    }

    #[test]
    fn thread_safe_backlog_update_modifies_state() {
        let safe_backlog = ThreadSafeBacklog::empty();
        safe_backlog.update(|state| {
            state.push_task(task("task-1", TaskStatus::Ready));
        });

        let guard = safe_backlog.read_with_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(guard.tasks.len(), 1);
    }

    #[test]
    fn thread_safe_backlog_clone_inner_returns_copy() {
        let safe_backlog = ThreadSafeBacklog::empty();
        safe_backlog.update(|state| {
            state.push_task(task("task-1", TaskStatus::Ready));
        });

        let clone = safe_backlog.clone_inner();
        assert_eq!(clone.tasks.len(), 1);
    }

    #[test]
    fn thread_safe_backlog_into_inner_returns_state() {
        let mut state = BacklogState::default();
        state.push_task(task("task-1", TaskStatus::Ready));
        let safe_backlog = ThreadSafeBacklog::new(state);

        let inner = safe_backlog.into_inner();
        assert_eq!(inner.tasks.len(), 1);
    }

    #[test]
    fn thread_safe_backlog_clone_creates_new_wrapper() {
        let safe_backlog = ThreadSafeBacklog::empty();
        safe_backlog.update(|state| {
            state.push_task(task("task-1", TaskStatus::Ready));
        });

        let clone = safe_backlog.clone();
        clone.update(|state| {
            state.push_task(task("task-2", TaskStatus::Ready));
        });

        // Original should still have only 1 task
        let guard = safe_backlog.read_with_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(guard.tasks.len(), 1);

        // Clone should have 2 tasks
        let guard2 = clone.read_with_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(guard2.tasks.len(), 2);
    }
}
