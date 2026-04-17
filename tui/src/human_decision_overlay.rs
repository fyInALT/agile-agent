//! Human decision overlay for decision layer
//!
//! Provides modal for human decision requests from the decision layer.

use agent_decision::{HumanDecisionRequest, HumanSelection, UrgencyLevel};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

/// Human decision overlay state
#[derive(Debug, Clone)]
pub struct HumanDecisionOverlay {
    /// The pending decision request
    pub request: HumanDecisionRequest,
    /// Currently selected option index (0-based)
    pub selected_index: usize,
    /// Custom instruction input (if in custom mode)
    pub custom_input: String,
    /// Whether in custom input mode
    pub custom_mode: bool,
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanDecisionCommand {
    /// Close overlay without action
    Cancel,
    /// Select option and submit
    Select { option_id: String },
    /// Accept recommendation
    AcceptRecommendation,
    /// Submit custom instruction
    CustomInstruction { instruction: String },
    /// Skip the decision
    Skip,
    /// No action taken
    None,
}

impl HumanDecisionOverlay {
    /// Create new human decision overlay from request
    pub fn new(request: HumanDecisionRequest) -> Self {
        Self {
            request,
            selected_index: 0,
            custom_input: String::new(),
            custom_mode: false,
        }
    }

    /// Get urgency display text
    pub fn urgency_text(&self) -> &'static str {
        match self.request.urgency {
            UrgencyLevel::Critical => "[CRITICAL]",
            UrgencyLevel::High => "[HIGH]",
            UrgencyLevel::Medium => "[MEDIUM]",
            UrgencyLevel::Low => "[LOW]",
        }
    }

    /// Get urgency color (for styling)
    pub fn urgency_color(&self) -> &'static str {
        match self.request.urgency {
            UrgencyLevel::Critical => "red",
            UrgencyLevel::High => "yellow",
            UrgencyLevel::Medium => "cyan",
            UrgencyLevel::Low => "gray",
        }
    }

    /// Get remaining time display
    pub fn remaining_time_text(&self) -> String {
        let remaining = self.request.remaining_seconds();
        if remaining <= 0 {
            "EXPIRED".to_string()
        } else if remaining < 60 {
            format!("{}s", remaining)
        } else if remaining < 3600 {
            format!("{}m", remaining / 60)
        } else {
            format!("{}h {}m", remaining / 3600, (remaining % 3600) / 60)
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
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> HumanDecisionCommand {
        if key_event.kind != KeyEventKind::Press {
            return HumanDecisionCommand::None;
        }

        // In custom input mode, handle text input
        if self.custom_mode {
            match key_event.code {
                KeyCode::Esc => {
                    self.exit_custom_mode();
                    HumanDecisionCommand::None
                }
                KeyCode::Enter => {
                    if self.custom_input.is_empty() {
                        self.exit_custom_mode();
                        HumanDecisionCommand::None
                    } else {
                        let instruction = self.custom_input.clone();
                        self.exit_custom_mode();
                        HumanDecisionCommand::CustomInstruction { instruction }
                    }
                }
                KeyCode::Backspace => {
                    self.remove_char();
                    HumanDecisionCommand::None
                }
                KeyCode::Char(c) => {
                    self.add_char(c);
                    HumanDecisionCommand::None
                }
                _ => HumanDecisionCommand::None,
            }
        } else {
            // Normal selection mode
            match key_event.code {
                KeyCode::Esc => HumanDecisionCommand::Cancel,
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    HumanDecisionCommand::Cancel
                }
                KeyCode::Up => {
                    self.move_up();
                    HumanDecisionCommand::None
                }
                KeyCode::Down => {
                    self.move_down();
                    HumanDecisionCommand::None
                }
                KeyCode::Enter => {
                    if let Some(option) = self.request.options.get(self.selected_index) {
                        HumanDecisionCommand::Select {
                            option_id: option.id.clone(),
                        }
                    } else {
                        HumanDecisionCommand::Cancel
                    }
                }
                // Ctrl+R for recommendation
                KeyCode::Char('r') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.request.recommendation.is_some() {
                        HumanDecisionCommand::AcceptRecommendation
                    } else {
                        HumanDecisionCommand::None
                    }
                }
                // Ctrl+I for custom instruction
                KeyCode::Char('i') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.enter_custom_mode();
                    HumanDecisionCommand::None
                }
                // Ctrl+S for skip
                KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    HumanDecisionCommand::Skip
                }
                // Letter selection (A, B, C, D...) - no modifier required
                KeyCode::Char(letter)
                    if letter.is_ascii_uppercase()
                        && letter >= 'A'
                        && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if self.select_by_letter(letter) {
                        if let Some(option) = self.request.options.get(self.selected_index) {
                            HumanDecisionCommand::Select {
                                option_id: option.id.clone(),
                            }
                        } else {
                            HumanDecisionCommand::None
                        }
                    } else {
                        HumanDecisionCommand::None
                    }
                }
                // Also accept lowercase letters - no modifier required
                KeyCode::Char(letter)
                    if letter.is_ascii_lowercase()
                        && letter >= 'a'
                        && !key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    let uppercase = letter.to_ascii_uppercase();
                    if self.select_by_letter(uppercase) {
                        if let Some(option) = self.request.options.get(self.selected_index) {
                            HumanDecisionCommand::Select {
                                option_id: option.id.clone(),
                            }
                        } else {
                            HumanDecisionCommand::None
                        }
                    } else {
                        HumanDecisionCommand::None
                    }
                }
                _ => HumanDecisionCommand::None,
            }
        }
    }

    /// Convert command to HumanSelection
    pub fn command_to_selection(cmd: &HumanDecisionCommand) -> Option<HumanSelection> {
        match cmd {
            HumanDecisionCommand::Select { option_id } => Some(HumanSelection::selected(option_id)),
            HumanDecisionCommand::AcceptRecommendation => {
                Some(HumanSelection::accept_recommendation())
            }
            HumanDecisionCommand::CustomInstruction { instruction } => {
                Some(HumanSelection::custom(instruction))
            }
            HumanDecisionCommand::Skip => Some(HumanSelection::skip()),
            HumanDecisionCommand::Cancel => Some(HumanSelection::cancel()),
            HumanDecisionCommand::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::{ChoiceOption, SituationType};
    use chrono::Utc;

    fn make_test_request() -> HumanDecisionRequest {
        HumanDecisionRequest::new(
            "req-001",
            "agent-alpha",
            SituationType::new("waiting_for_choice"),
            vec![
                ChoiceOption::new("A", "Approve"),
                ChoiceOption::new("B", "Approve for session"),
                ChoiceOption::new("C", "Deny"),
                ChoiceOption::new("D", "Abort"),
            ],
            UrgencyLevel::High,
            1800000,
        )
    }

    #[test]
    fn test_overlay_new() {
        let request = make_test_request();
        let overlay = HumanDecisionOverlay::new(request);
        assert_eq!(overlay.selected_index, 0);
        assert!(!overlay.custom_mode);
    }

    #[test]
    fn test_urgency_text() {
        let request = make_test_request();
        let overlay = HumanDecisionOverlay::new(request);
        assert_eq!(overlay.urgency_text(), "[HIGH]");
    }

    #[test]
    fn test_move_selection() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        overlay.move_down();
        assert_eq!(overlay.selected_index, 1);

        overlay.move_up();
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn test_select_by_letter() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        assert!(overlay.select_by_letter('C'));
        assert_eq!(overlay.selected_index, 2);

        // Invalid letter
        assert!(!overlay.select_by_letter('Z'));
    }

    #[test]
    fn test_enter_exit_custom_mode() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        overlay.enter_custom_mode();
        assert!(overlay.custom_mode);
        assert!(overlay.custom_input.is_empty());

        overlay.add_char('t');
        overlay.add_char('e');
        overlay.add_char('s');
        overlay.add_char('t');
        assert_eq!(overlay.custom_input, "test");

        overlay.exit_custom_mode();
        assert!(!overlay.custom_mode);
        assert!(overlay.custom_input.is_empty());
    }

    #[test]
    fn test_handle_key_enter() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            cmd,
            HumanDecisionCommand::Select {
                option_id: "A".to_string()
            }
        );
    }

    #[test]
    fn test_handle_key_letter() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
        assert_eq!(
            cmd,
            HumanDecisionCommand::Select {
                option_id: "C".to_string()
            }
        );
    }

    #[test]
    fn test_handle_key_skip() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        let cmd =
            overlay.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(cmd, HumanDecisionCommand::Skip);
    }

    #[test]
    fn test_handle_key_custom_mode() {
        let request = make_test_request();
        let mut overlay = HumanDecisionOverlay::new(request);

        // Enter custom mode
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL));
        assert!(overlay.custom_mode);

        // Type text
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        overlay.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(overlay.custom_input, "test");

        // Submit with Enter
        let cmd = overlay.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            cmd,
            HumanDecisionCommand::CustomInstruction {
                instruction: "test".to_string()
            }
        );
        assert!(!overlay.custom_mode);
    }

    #[test]
    fn test_command_to_selection() {
        let select = HumanDecisionCommand::Select {
            option_id: "A".to_string(),
        };
        assert_eq!(
            HumanDecisionOverlay::command_to_selection(&select),
            Some(HumanSelection::selected("A"))
        );

        let skip = HumanDecisionCommand::Skip;
        assert_eq!(
            HumanDecisionOverlay::command_to_selection(&skip),
            Some(HumanSelection::skip())
        );
    }
}
