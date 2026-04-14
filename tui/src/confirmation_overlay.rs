//! Confirmation overlay for agent stop
//!
//! Provides confirmation dialog before stopping an agent.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

/// Confirmation overlay state for stopping an agent
#[derive(Debug, Clone, Default)]
pub struct ConfirmationOverlay {
    /// The action being confirmed
    pub action: String,
    /// Currently selected option (0 = confirm, 1 = cancel)
    pub selected_index: usize,
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationCommand {
    /// Close the overlay without confirming
    Cancel,
    /// Confirm the action
    Confirm,
}

impl ConfirmationOverlay {
    /// Create a new confirmation overlay for stopping an agent
    pub fn for_stop_agent(agent_name: &str) -> Self {
        Self {
            action: format!("Stop agent {}?", agent_name),
            selected_index: 1, // Default to cancel (safer)
        }
    }

    /// Get the currently selected option label
    pub fn selected_label(&self) -> &'static str {
        if self.selected_index == 0 {
            "Confirm"
        } else {
            "Cancel"
        }
    }

    /// Move selection left (towards confirm)
    pub fn move_left(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection right (towards cancel)
    pub fn move_right(&mut self) {
        if self.selected_index < 1 {
            self.selected_index += 1;
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ConfirmationCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Esc => Some(ConfirmationCommand::Cancel),
            KeyCode::Left => {
                self.move_left();
                None
            }
            KeyCode::Right => {
                self.move_right();
                None
            }
            KeyCode::Enter => {
                if self.selected_index == 0 {
                    Some(ConfirmationCommand::Confirm)
                } else {
                    Some(ConfirmationCommand::Cancel)
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                Some(ConfirmationCommand::Cancel)
            }
            KeyCode::Char('y') => Some(ConfirmationCommand::Confirm),
            KeyCode::Char('n') => Some(ConfirmationCommand::Cancel),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConfirmationOverlay;
    use super::ConfirmationCommand;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyCode;

    #[test]
    fn new_overlay_defaults_to_cancel() {
        let overlay = ConfirmationOverlay::for_stop_agent("alpha");
        assert_eq!(overlay.selected_index, 1);
        assert_eq!(overlay.selected_label(), "Cancel");
    }

    #[test]
    fn move_left_selects_confirm() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        overlay.move_left();
        assert_eq!(overlay.selected_index, 0);
        assert_eq!(overlay.selected_label(), "Confirm");
    }

    #[test]
    fn move_right_from_confirm_selects_cancel() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        overlay.selected_index = 0;
        overlay.move_right();
        assert_eq!(overlay.selected_index, 1);
    }

    #[test]
    fn enter_on_confirm_returns_confirm() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        overlay.selected_index = 0;
        let result = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE));
        assert_eq!(result, Some(ConfirmationCommand::Confirm));
    }

    #[test]
    fn enter_on_cancel_returns_cancel() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        overlay.selected_index = 1;
        let result = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE));
        assert_eq!(result, Some(ConfirmationCommand::Cancel));
    }

    #[test]
    fn y_key_confirms() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        let result = overlay.handle_key_event(KeyEvent::new(KeyCode::Char('y'), crossterm::event::KeyModifiers::NONE));
        assert_eq!(result, Some(ConfirmationCommand::Confirm));
    }

    #[test]
    fn n_key_cancels() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        let result = overlay.handle_key_event(KeyEvent::new(KeyCode::Char('n'), crossterm::event::KeyModifiers::NONE));
        assert_eq!(result, Some(ConfirmationCommand::Cancel));
    }

    #[test]
    fn esc_cancels() {
        let mut overlay = ConfirmationOverlay::for_stop_agent("alpha");
        let result = overlay.handle_key_event(KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE));
        assert_eq!(result, Some(ConfirmationCommand::Cancel));
    }
}