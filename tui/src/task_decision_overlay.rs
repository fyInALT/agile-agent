//! Task decision overlay for task concept
//!
//! Provides modal for task-specific human decisions.
//!
//! NOTE: This widget is designed for future integration with the app loop.
//! Currently not connected to the runtime - suppress dead_code warnings.

#![allow(dead_code)]

use agent_decision::task::{DecisionTaskId, DecisionTaskStatus};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

/// Task decision request for overlay
#[derive(Debug, Clone)]
pub struct TaskDecisionRequest {
    /// Task ID
    pub task_id: DecisionTaskId,
    /// Task description
    pub description: String,
    /// Current status
    pub status: DecisionTaskStatus,
    /// Decision question
    pub question: String,
    /// Available options
    pub options: Vec<TaskDecisionOption>,
    /// Recommended option (if any)
    pub recommendation: Option<usize>,
}

/// Task decision option
#[derive(Debug, Clone)]
pub struct TaskDecisionOption {
    /// Option ID
    pub id: String,
    /// Option label
    pub label: String,
    /// Option description
    pub description: String,
}

impl TaskDecisionOption {
    /// Create a new option
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
        }
    }
}

impl TaskDecisionRequest {
    /// Create a new task decision request
    pub fn new(
        task_id: DecisionTaskId,
        description: String,
        status: DecisionTaskStatus,
        question: String,
        options: Vec<TaskDecisionOption>,
    ) -> Self {
        Self {
            task_id,
            description,
            status,
            question,
            options,
            recommendation: None,
        }
    }

    /// Create a standard completion confirmation request
    pub fn completion_confirmation(task_id: DecisionTaskId, description: String) -> Self {
        Self::new(
            task_id,
            description,
            DecisionTaskStatus::PendingConfirmation,
            "Task appears complete. Confirm completion?".to_string(),
            vec![
                TaskDecisionOption::new("approve", "Approve", "Mark task as completed"),
                TaskDecisionOption::new("reflect", "Reflect", "Request more reflection"),
                TaskDecisionOption::new("cancel", "Cancel", "Cancel the task"),
            ],
        )
    }

    /// Create a human intervention request
    pub fn human_intervention(task_id: DecisionTaskId, description: String, reason: String) -> Self {
        Self::new(
            task_id,
            description,
            DecisionTaskStatus::NeedsHumanDecision,
            reason,
            vec![
                TaskDecisionOption::new("approve", "Approve", "Continue with current approach"),
                TaskDecisionOption::new("deny", "Deny", "Reject current approach"),
                TaskDecisionOption::new("custom", "Custom", "Provide custom feedback"),
                TaskDecisionOption::new("cancel", "Cancel", "Cancel the task"),
            ],
        )
    }
}

/// Task decision overlay state
#[derive(Debug, Clone)]
pub struct TaskDecisionOverlay {
    /// The pending decision request
    pub request: TaskDecisionRequest,
    /// Currently selected option index (0-based)
    pub selected_index: usize,
    /// Custom feedback input (if in custom mode)
    pub custom_input: String,
    /// Whether in custom input mode
    pub custom_mode: bool,
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskDecisionCommand {
    /// Close overlay without action
    Cancel,
    /// Select option and submit
    Select { option_id: String },
    /// Submit custom feedback
    CustomFeedback { feedback: String },
    /// No action taken
    None,
}

impl TaskDecisionOverlay {
    /// Create new task decision overlay from request
    pub fn new(request: TaskDecisionRequest) -> Self {
        Self {
            request,
            selected_index: 0,
            custom_input: String::new(),
            custom_mode: false,
        }
    }

    /// Get status display text
    pub fn status_text(&self) -> &'static str {
        match self.request.status {
            DecisionTaskStatus::Pending => "Pending",
            DecisionTaskStatus::InProgress => "In Progress",
            DecisionTaskStatus::Reflecting => "Reflecting",
            DecisionTaskStatus::PendingConfirmation => "Confirming",
            DecisionTaskStatus::NeedsHumanDecision => "Needs Decision",
            DecisionTaskStatus::Paused => "Paused",
            DecisionTaskStatus::Completed => "Completed",
            DecisionTaskStatus::Cancelled => "Cancelled",
        }
    }

    /// Get current selection label
    pub fn current_selection_label(&self) -> Option<&str> {
        self.request
            .options
            .get(self.selected_index)
            .map(|o| o.label.as_str())
    }

    /// Move selection up (previous option)
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down (next option)
    pub fn move_down(&mut self) {
        if self.selected_index < self.request.options.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Select option by letter (A, B, C, D...)
    pub fn select_by_letter(&mut self, letter: char) -> bool {
        let index = (letter as usize) - ('A' as usize);
        if index < self.request.options.len() {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    /// Enter custom input mode
    pub fn enter_custom_mode(&mut self) {
        self.custom_mode = true;
        self.custom_input.clear();
    }

    /// Exit custom input mode
    pub fn exit_custom_mode(&mut self) {
        self.custom_mode = false;
        self.custom_input.clear();
    }

    /// Add character to custom input
    pub fn add_char(&mut self, c: char) {
        if self.custom_mode {
            self.custom_input.push(c);
        }
    }

    /// Remove last character from custom input
    pub fn remove_char(&mut self) {
        if self.custom_mode {
            self.custom_input.pop();
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> TaskDecisionCommand {
        if key_event.kind != KeyEventKind::Press {
            return TaskDecisionCommand::None;
        }

        // In custom input mode, handle text input
        if self.custom_mode {
            match key_event.code {
                KeyCode::Esc => {
                    self.exit_custom_mode();
                    TaskDecisionCommand::None
                }
                KeyCode::Enter => {
                    if self.custom_input.is_empty() {
                        self.exit_custom_mode();
                        TaskDecisionCommand::None
                    } else {
                        let feedback = self.custom_input.clone();
                        self.exit_custom_mode();
                        TaskDecisionCommand::CustomFeedback { feedback }
                    }
                }
                KeyCode::Backspace => {
                    self.remove_char();
                    TaskDecisionCommand::None
                }
                KeyCode::Char(c) => {
                    self.add_char(c);
                    TaskDecisionCommand::None
                }
                _ => TaskDecisionCommand::None,
            }
        } else {
            // Normal selection mode
            match key_event.code {
                KeyCode::Esc => TaskDecisionCommand::Cancel,
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    TaskDecisionCommand::Cancel
                }
                KeyCode::Up => {
                    self.move_up();
                    TaskDecisionCommand::None
                }
                KeyCode::Down => {
                    self.move_down();
                    TaskDecisionCommand::None
                }
                KeyCode::Enter => {
                    if let Some(option) = self.request.options.get(self.selected_index) {
                        // If selecting "Custom" option, enter custom mode
                        if option.id == "custom" {
                            self.enter_custom_mode();
                            TaskDecisionCommand::None
                        } else {
                            TaskDecisionCommand::Select {
                                option_id: option.id.clone(),
                            }
                        }
                    } else {
                        TaskDecisionCommand::Cancel
                    }
                }
                // Ctrl+I for custom input
                KeyCode::Char('i') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.enter_custom_mode();
                    TaskDecisionCommand::None
                }
                // Letter selection (A, B, C, D...)
                KeyCode::Char(letter)
                    if letter.is_ascii_uppercase()
                        && letter >= 'A'
                        && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if self.select_by_letter(letter) {
                        if let Some(option) = self.request.options.get(self.selected_index) {
                            if option.id == "custom" {
                                self.enter_custom_mode();
                                TaskDecisionCommand::None
                            } else {
                                TaskDecisionCommand::Select {
                                    option_id: option.id.clone(),
                                }
                            }
                        } else {
                            TaskDecisionCommand::None
                        }
                    } else {
                        TaskDecisionCommand::None
                    }
                }
                // Also accept lowercase letters
                KeyCode::Char(letter)
                    if letter.is_ascii_lowercase()
                        && letter >= 'a'
                        && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    let uppercase = letter.to_ascii_uppercase();
                    if self.select_by_letter(uppercase) {
                        if let Some(option) = self.request.options.get(self.selected_index) {
                            if option.id == "custom" {
                                self.enter_custom_mode();
                                TaskDecisionCommand::None
                            } else {
                                TaskDecisionCommand::Select {
                                    option_id: option.id.clone(),
                                }
                            }
                        } else {
                            TaskDecisionCommand::None
                        }
                    } else {
                        TaskDecisionCommand::None
                    }
                }
                _ => TaskDecisionCommand::None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::Task;

    fn create_test_task() -> Task {
        Task::new(
            "Test task description".to_string(),
            vec!["constraint1".to_string()],
        )
    }

    fn create_completion_request() -> TaskDecisionRequest {
        let task = create_test_task();
        TaskDecisionRequest::completion_confirmation(task.id, task.description)
    }

    fn create_intervention_request() -> TaskDecisionRequest {
        let task = create_test_task();
        TaskDecisionRequest::human_intervention(
            task.id,
            task.description,
            "Maximum reflection rounds reached".to_string(),
        )
    }

    // Story 14.3 Tests: Task Decision Overlay

    #[test]
    fn t14_3_t1_overlay_created_with_request() {
        let request = create_completion_request();
        let overlay = TaskDecisionOverlay::new(request);

        assert_eq!(overlay.selected_index, 0);
        assert!(!overlay.custom_mode);
        assert_eq!(overlay.request.options.len(), 3);
    }

    #[test]
    fn t14_3_t2_keyboard_navigation_works() {
        let request = create_intervention_request();
        let mut overlay = TaskDecisionOverlay::new(request);

        // Move down
        overlay.move_down();
        assert_eq!(overlay.selected_index, 1);

        // Move up
        overlay.move_up();
        assert_eq!(overlay.selected_index, 0);

        // Can't move up from first
        overlay.move_up();
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn t14_3_t3_custom_feedback_mode() {
        let request = create_intervention_request();
        let mut overlay = TaskDecisionOverlay::new(request);

        // Select "Custom" option (index 2)
        overlay.selected_index = 2;
        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Should enter custom mode, not select
        assert_eq!(cmd, TaskDecisionCommand::None);
        assert!(overlay.custom_mode);

        // Type custom feedback
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        // Submit with Enter
        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            cmd,
            TaskDecisionCommand::CustomFeedback {
                feedback: "test".to_string()
            }
        );
        assert!(!overlay.custom_mode);
    }

    #[test]
    fn t14_3_t4_decision_command_returned() {
        let request = create_completion_request();
        let mut overlay = TaskDecisionOverlay::new(request);

        // Select "Approve" (Enter on first option)
        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            cmd,
            TaskDecisionCommand::Select {
                option_id: "approve".to_string()
            }
        );

        // Select "Reflect" with letter B
        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
        assert_eq!(
            cmd,
            TaskDecisionCommand::Select {
                option_id: "reflect".to_string()
            }
        );
    }

    #[test]
    fn test_ctrl_i_custom_mode() {
        let request = create_completion_request();
        let mut overlay = TaskDecisionOverlay::new(request);

        // Ctrl+I enters custom mode
        let cmd =
            overlay.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL));
        assert_eq!(cmd, TaskDecisionCommand::None);
        assert!(overlay.custom_mode);
    }

    #[test]
    fn test_cancel_with_escape() {
        let request = create_completion_request();
        let mut overlay = TaskDecisionOverlay::new(request);

        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(cmd, TaskDecisionCommand::Cancel);
    }

    #[test]
    fn test_completion_confirmation_options() {
        let task = create_test_task();
        let request = TaskDecisionRequest::completion_confirmation(task.id, task.description);

        assert_eq!(request.options.len(), 3);
        assert_eq!(request.options[0].id, "approve");
        assert_eq!(request.options[1].id, "reflect");
        assert_eq!(request.options[2].id, "cancel");
    }

    #[test]
    fn test_human_intervention_options() {
        let task = create_test_task();
        let request = TaskDecisionRequest::human_intervention(
            task.id,
            task.description,
            "Needs decision".to_string(),
        );

        assert_eq!(request.options.len(), 4);
        assert_eq!(request.options[0].id, "approve");
        assert_eq!(request.options[1].id, "deny");
        assert_eq!(request.options[2].id, "custom");
        assert_eq!(request.options[3].id, "cancel");
    }
}
