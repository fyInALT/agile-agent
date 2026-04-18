//! Task detail view widget (Sprint 14)
//!
//! Displays detailed task information with execution history.
//!
//! NOTE: This widget is designed for future integration with the app loop.
//! Currently not connected to the runtime - suppress dead_code and unused warnings.

#![allow(dead_code)]
#![allow(unused_imports)]

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_decision::persistence::ExecutionRecord;
use agent_decision::task::{Task, TaskStatus};
use agent_decision::workflow::{StageId, WorkflowAction};
use crate::task_panel::TaskPanel;

/// Task detail view state
#[derive(Debug, Clone)]
pub struct TaskDetailView {
    /// The task being displayed
    task: Task,
    /// Scroll position in history
    history_scroll: usize,
    /// Whether viewing history
    viewing_history: bool,
}

/// Command returned from key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDetailCommand {
    /// No action
    None,
    /// Close detail view
    Close,
    /// Scroll up
    ScrollUp,
    /// Scroll down
    ScrollDown,
    /// Toggle history view
    ToggleHistory,
    /// Request reflection
    RequestReflect,
    /// Request confirmation
    RequestConfirm,
}

impl TaskDetailView {
    /// Create new detail view for a task
    pub fn new(task: Task) -> Self {
        Self {
            task,
            history_scroll: 0,
            viewing_history: false,
        }
    }

    /// Get task
    pub fn task(&self) -> &Task {
        &self.task
    }

    /// Get history scroll position
    pub fn history_scroll(&self) -> usize {
        self.history_scroll
    }

    /// Check if viewing history
    pub fn is_viewing_history(&self) -> bool {
        self.viewing_history
    }

    /// Toggle history view mode
    pub fn toggle_history(&mut self) {
        self.viewing_history = !self.viewing_history;
        self.history_scroll = 0;
    }

    /// Scroll history up
    pub fn scroll_up(&mut self) {
        if self.history_scroll > 0 {
            self.history_scroll -= 1;
        }
    }

    /// Scroll history down
    pub fn scroll_down(&mut self) {
        let max_scroll = self.task.execution_history.len().saturating_sub(1);
        if self.history_scroll < max_scroll {
            self.history_scroll += 1;
        }
    }

    /// Get history count
    pub fn history_count(&self) -> usize {
        self.task.execution_history.len()
    }

    /// Get visible history records
    pub fn visible_history(&self, max_lines: usize) -> Vec<&ExecutionRecord> {
        let start = self.history_scroll;
        let end = std::cmp::min(start + max_lines, self.task.execution_history.len());

        self.task.execution_history[start..end].iter().collect()
    }

    /// Format task info section
    pub fn format_task_info(&self) -> Vec<String> {
        let status_text = TaskPanel::status_text(&self.task.status);
        let status_symbol = TaskPanel::status_symbol(&self.task.status);

        vec![
            format!("Task: {}", self.task.description),
            format!("Status: {} {}", status_symbol, status_text),
            format!("Reflections: {} / {}", self.task.reflection_count, self.task.max_reflection_rounds),
            format!("Confirmations: {}", self.task.confirmation_count),
            format!("Constraints: {}", self.task.constraints.join(", ")),
            format!("History records: {}", self.task.execution_history.len()),
        ]
    }

    /// Format history record for display
    pub fn format_history_record(record: &ExecutionRecord, index: usize) -> String {
        let action_text = format!("{:?}", record.action);
        let timestamp = record.timestamp.format("%H:%M:%S");

        let human_flag = if record.human_requested { " [HUMAN]" } else { "" };
        let auto_check = record.auto_check_result.as_ref()
            .map(|r| format!(" [{}]", r))
            .unwrap_or_default();

        format!(
            "{}. {} @ {} {}{} | Stage: {}",
            index + 1,
            action_text.chars().take(20).collect::<String>(),
            timestamp,
            human_flag,
            auto_check,
            record.stage.as_str()
        )
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> TaskDetailCommand {
        if key_event.kind != KeyEventKind::Press {
            return TaskDetailCommand::None;
        }

        match key_event.code {
            KeyCode::Esc => TaskDetailCommand::Close,
            KeyCode::Up => {
                self.scroll_up();
                TaskDetailCommand::ScrollUp
            }
            KeyCode::Down => {
                self.scroll_down();
                TaskDetailCommand::ScrollDown
            }
            KeyCode::Char('h') => {
                self.toggle_history();
                TaskDetailCommand::ToggleHistory
            }
            KeyCode::Char('r') if self.viewing_history => TaskDetailCommand::RequestReflect,
            KeyCode::Char('c') if self.viewing_history => TaskDetailCommand::RequestConfirm,
            _ => TaskDetailCommand::None,
        }
    }

    /// Render lines for display
    pub fn render_lines(&self, max_lines: usize) -> Vec<String> {
        if self.viewing_history {
            self.visible_history(max_lines)
                .iter()
                .enumerate()
                .map(|(i, r)| Self::format_history_record(r, i + self.history_scroll))
                .collect()
        } else {
            self.format_task_info()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::{StageId, WorkflowAction};
    use crate::task_panel::TaskInfo;

    fn create_test_task_with_history() -> Task {
        let mut task = Task::new("Test task with history".to_string(), vec!["constraint1".to_string()]);
        let _ = task.transition_to(TaskStatus::InProgress);

        // Add some history records
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Reflect { reason: "syntax error".to_string() },
            StageId::new("reflecting"),
        ));
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));

        task
    }

    // Story 14.2 Tests: Task Detail View

    #[test]
    fn t14_2_t1_detail_view_renders_task_info() {
        let task = create_test_task_with_history();
        let view = TaskDetailView::new(task);

        let info = view.format_task_info();

        assert!(info.iter().any(|s| s.contains("Test task with history")));
        assert!(info.iter().any(|s| s.contains("Status")));
    }

    #[test]
    fn t14_2_t2_history_shown_as_timeline() {
        let task = create_test_task_with_history();
        let view = TaskDetailView::new(task);

        assert_eq!(view.history_count(), 3);
    }

    #[test]
    fn t14_2_t3_scroll_works_for_long_history() {
        let mut task = create_test_task_with_history();
        // Add more history
        for i in 0..10 {
            task.execution_history.push(ExecutionRecord::new(
                WorkflowAction::Continue,
                StageId::new("developing"),
            ));
        }

        let mut view = TaskDetailView::new(task);
        view.toggle_history();

        // Scroll down
        view.scroll_down();
        assert_eq!(view.history_scroll(), 1);

        // Scroll up
        view.scroll_up();
        assert_eq!(view.history_scroll(), 0);
    }

    #[test]
    fn t14_2_t4_actions_timestamped_correctly() {
        let task = create_test_task_with_history();
        let view = TaskDetailView::new(task);

        let record = &view.task.execution_history[0];
        let formatted = TaskDetailView::format_history_record(record, 0);

        // Should contain timestamp format HH:MM:SS
        assert!(formatted.contains("@"));
    }

    #[test]
    fn test_toggle_history() {
        let task = create_test_task_with_history();
        let mut view = TaskDetailView::new(task);

        assert!(!view.is_viewing_history());

        view.toggle_history();
        assert!(view.is_viewing_history());

        view.toggle_history();
        assert!(!view.is_viewing_history());
    }

    #[test]
    fn test_handle_key_navigation() {
        let task = create_test_task_with_history();
        let mut view = TaskDetailView::new(task);
        view.toggle_history();

        let cmd = view.handle_key_event(KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE));
        assert_eq!(cmd, TaskDetailCommand::ScrollDown);

        let cmd = view.handle_key_event(KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE));
        assert_eq!(cmd, TaskDetailCommand::Close);
    }

    #[test]
    fn test_visible_history_limit() {
        let mut task = create_test_task_with_history();
        for _ in 0..20 {
            task.execution_history.push(ExecutionRecord::new(
                WorkflowAction::Continue,
                StageId::new("developing"),
            ));
        }

        let mut view = TaskDetailView::new(task);
        view.toggle_history();
        view.history_scroll = 5;

        let visible = view.visible_history(10);
        assert_eq!(visible.len(), 10);
    }
}