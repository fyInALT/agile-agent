//! Task panel widget for TUI dashboard (Sprint 14)
//!
//! Displays active tasks with status and counts.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

// Import types from agent-decision
use agent_decision::task::{Task, TaskId, TaskStatus};

/// Task information for display
#[derive(Debug, Clone)]
pub struct TaskInfo {
    /// Task ID
    pub id: TaskId,
    /// Task description
    pub description: String,
    /// Current status
    pub status: TaskStatus,
    /// Reflection count
    pub reflection_count: usize,
    /// Confirmation count
    pub confirmation_count: usize,
}

impl From<Task> for TaskInfo {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            description: task.description,
            status: task.status,
            reflection_count: task.reflection_count,
            confirmation_count: task.confirmation_count,
        }
    }
}

/// Task panel state
#[derive(Debug, Clone)]
pub struct TaskPanel {
    /// Tasks to display
    tasks: Vec<TaskInfo>,
    /// Currently selected index
    selected_index: usize,
}

/// Command returned from key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskPanelCommand {
    /// No action
    None,
    /// Select a task for detail view
    SelectTask { id: TaskId },
    /// Request refresh
    Refresh,
    /// Force reflection
    ForceReflect { id: TaskId },
    /// Force confirmation
    ForceConfirm { id: TaskId },
    /// Cancel task
    CancelTask { id: TaskId },
}

impl Default for TaskPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskPanel {
    /// Create a new empty task panel
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            selected_index: 0,
        }
    }

    /// Create panel with tasks
    pub fn with_tasks(tasks: Vec<TaskInfo>) -> Self {
        Self {
            tasks,
            selected_index: 0,
        }
    }

    /// Update tasks list
    pub fn update_tasks(&mut self, tasks: Vec<TaskInfo>) {
        self.tasks = tasks;
        // Reset selection if out of bounds
        if self.selected_index >= self.tasks.len() && self.tasks.len() > 0 {
            self.selected_index = self.tasks.len() - 1;
        }
    }

    /// Get task count
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get selected task ID
    pub fn selected_task_id(&self) -> Option<TaskId> {
        self.tasks.get(self.selected_index).map(|t| t.id.clone())
    }

    /// Get selected task info
    pub fn selected_task(&self) -> Option<&TaskInfo> {
        self.tasks.get(self.selected_index)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index < self.tasks.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Get status display text
    pub fn status_text(status: &TaskStatus) -> &'static str {
        match status {
            TaskStatus::Pending => "Pending",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Reflecting => "Reflecting",
            TaskStatus::PendingConfirmation => "Confirming",
            TaskStatus::NeedsHumanDecision => "Needs Decision",
            TaskStatus::Paused => "Paused",
            TaskStatus::Completed => "Completed",
            TaskStatus::Cancelled => "Cancelled",
        }
    }

    /// Get status indicator symbol
    pub fn status_symbol(status: &TaskStatus) -> &'static str {
        match status {
            TaskStatus::Pending => "○",
            TaskStatus::InProgress => "◐",
            TaskStatus::Reflecting => "↻",
            TaskStatus::PendingConfirmation => "⏳",
            TaskStatus::NeedsHumanDecision => "⚠",
            TaskStatus::Paused => "⏸",
            TaskStatus::Completed => "●",
            TaskStatus::Cancelled => "✕",
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> TaskPanelCommand {
        if key_event.kind != KeyEventKind::Press {
            return TaskPanelCommand::None;
        }

        match key_event.code {
            KeyCode::Up => {
                self.move_up();
                TaskPanelCommand::None
            }
            KeyCode::Down => {
                self.move_down();
                TaskPanelCommand::None
            }
            KeyCode::Enter => {
                if let Some(id) = self.selected_task_id() {
                    TaskPanelCommand::SelectTask { id }
                } else {
                    TaskPanelCommand::None
                }
            }
            KeyCode::Char('r') => {
                TaskPanelCommand::Refresh
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.selected_task_id() {
                    TaskPanelCommand::SelectTask { id }
                } else {
                    TaskPanelCommand::None
                }
            }
            _ => TaskPanelCommand::None,
        }
    }

    /// Format task for display
    pub fn format_task_line(&self, index: usize, selected: bool) -> Option<String> {
        let task = self.tasks.get(index)?;
        let symbol = Self::status_symbol(&task.status);
        let status = Self::status_text(&task.status);
        let selector = if selected { ">" } else { " " };

        Some(format!(
            "{} {} {} [R:{} C:{}] {}",
            selector,
            symbol,
            status,
            task.reflection_count,
            task.confirmation_count,
            task.description.chars().take(30).collect::<String>()
        ))
    }

    /// Render lines for display
    pub fn render_lines(&self, max_lines: usize) -> Vec<String> {
        let start = if self.selected_index >= max_lines {
            self.selected_index - max_lines + 1
        } else {
            0
        };

        let end = std::cmp::min(start + max_lines, self.tasks.len());

        (start..end)
            .map(|i| {
                let selected = i == self.selected_index;
                self.format_task_line(i, selected).unwrap_or_default()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::Task;

    fn create_test_task(description: &str) -> TaskInfo {
        let task = Task::new(description.to_string(), vec![]);
        TaskInfo::from(task)
    }

    fn create_test_task_with_status(description: &str, status: TaskStatus) -> TaskInfo {
        let mut task = Task::new(description.to_string(), vec![]);
        if status != TaskStatus::Pending {
            let _ = task.transition_to(TaskStatus::InProgress);
            if status != TaskStatus::InProgress {
                let _ = task.transition_to(status);
            }
        }
        TaskInfo::from(task)
    }

    // Story 14.1 Tests: Task Panel Widget

    #[test]
    fn t14_1_t1_panel_renders_with_tasks() {
        let tasks = vec![
            create_test_task("Task 1"),
            create_test_task("Task 2"),
        ];
        let panel = TaskPanel::with_tasks(tasks);

        assert_eq!(panel.task_count(), 2);
    }

    #[test]
    fn t14_1_t2_status_displayed_correctly() {
        let task = create_test_task_with_status("Test", TaskStatus::InProgress);
        let panel = TaskPanel::with_tasks(vec![task]);

        let status = TaskPanel::status_text(&panel.tasks[0].status);
        assert_eq!(status, "In Progress");
    }

    #[test]
    fn t14_1_t3_reflection_count_shown() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.reflection_count = 3;
        let info = TaskInfo::from(task);

        let panel = TaskPanel::with_tasks(vec![info]);

        assert_eq!(panel.tasks[0].reflection_count, 3);
    }

    #[test]
    fn t14_1_t4_keyboard_navigation_works() {
        let tasks = vec![
            create_test_task("Task 1"),
            create_test_task("Task 2"),
            create_test_task("Task 3"),
        ];
        let mut panel = TaskPanel::with_tasks(tasks);

        // Move down
        panel.move_down();
        assert_eq!(panel.selected_index(), 1);

        // Move up
        panel.move_up();
        assert_eq!(panel.selected_index(), 0);

        // Can't move up from first
        panel.move_up();
        assert_eq!(panel.selected_index(), 0);
    }

    #[test]
    fn test_status_symbols() {
        assert_eq!(TaskPanel::status_symbol(&TaskStatus::Pending), "○");
        assert_eq!(TaskPanel::status_symbol(&TaskStatus::InProgress), "◐");
        assert_eq!(TaskPanel::status_symbol(&TaskStatus::Completed), "●");
        assert_eq!(TaskPanel::status_symbol(&TaskStatus::Cancelled), "✕");
    }

    #[test]
    fn test_render_lines() {
        let tasks = vec![
            create_test_task("Short task"),
            create_test_task("Another task"),
        ];
        let panel = TaskPanel::with_tasks(tasks);

        let lines = panel.render_lines(10);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with(">")); // Selected
        assert!(lines[1].starts_with(" ")); // Not selected
    }

    #[test]
    fn test_select_task_command() {
        let tasks = vec![create_test_task("Task 1")];
        let mut panel = TaskPanel::with_tasks(tasks);

        let cmd = panel.handle_key_event(KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE));
        assert!(matches!(cmd, TaskPanelCommand::SelectTask { .. }));
    }

    #[test]
    fn test_refresh_command() {
        let tasks = vec![create_test_task("Task 1")];
        let mut panel = TaskPanel::with_tasks(tasks);

        let cmd = panel.handle_key_event(KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE));
        assert_eq!(cmd, TaskPanelCommand::Refresh);
    }
}