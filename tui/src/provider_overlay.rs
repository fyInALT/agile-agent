//! Provider selection overlay for agent creation
//!
//! Provides UI for selecting a provider when spawning a new agent.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_core::provider::ProviderKind;

/// Provider selection overlay state
#[derive(Debug, Clone, Default)]
pub struct ProviderSelectionOverlay {
    /// Currently selected provider index
    pub selected_index: usize,
    /// Available providers
    pub providers: Vec<ProviderKind>,
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSelectionCommand {
    /// Close the overlay without selecting
    Close,
    /// Select the provider and spawn agent
    Select(ProviderKind),
}

impl ProviderSelectionOverlay {
    /// Create a new overlay with all available providers
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            providers: vec![
                ProviderKind::Claude,
                ProviderKind::Codex,
                ProviderKind::Mock,
            ],
        }
    }

    /// Get the currently selected provider
    pub fn selected_provider(&self) -> ProviderKind {
        self.providers
            .get(self.selected_index)
            .copied()
            .unwrap_or(ProviderKind::Mock)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index < self.providers.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ProviderSelectionCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Esc => Some(ProviderSelectionCommand::Close),
            KeyCode::Up => {
                self.move_up();
                None
            }
            KeyCode::Down => {
                self.move_down();
                None
            }
            KeyCode::Enter => Some(ProviderSelectionCommand::Select(self.selected_provider())),
            KeyCode::Char('c')
                if key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                Some(ProviderSelectionCommand::Close)
            }
            _ => None,
        }
    }

    /// Get provider label for display
    pub fn provider_label(provider: ProviderKind) -> &'static str {
        match provider {
            ProviderKind::Claude => "Claude",
            ProviderKind::Codex => "Codex",
            ProviderKind::Mock => "Mock",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderSelectionCommand;
    use super::ProviderSelectionOverlay;
    use agent_core::provider::ProviderKind;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;

    #[test]
    fn new_overlay_has_first_provider_selected() {
        let overlay = ProviderSelectionOverlay::new();
        assert_eq!(overlay.selected_index, 0);
        assert_eq!(overlay.selected_provider(), ProviderKind::Claude);
    }

    #[test]
    fn move_down_selects_next_provider() {
        let mut overlay = ProviderSelectionOverlay::new();
        overlay.move_down();
        assert_eq!(overlay.selected_index, 1);
        assert_eq!(overlay.selected_provider(), ProviderKind::Codex);
    }

    #[test]
    fn move_up_at_first_stays_at_first() {
        let mut overlay = ProviderSelectionOverlay::new();
        overlay.move_up();
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn move_down_at_last_stays_at_last() {
        let mut overlay = ProviderSelectionOverlay::new();
        overlay.selected_index = overlay.providers.len() - 1;
        overlay.move_down();
        assert_eq!(overlay.selected_index, overlay.providers.len() - 1);
    }

    #[test]
    fn enter_returns_selected_provider() {
        let mut overlay = ProviderSelectionOverlay::new();
        overlay.selected_index = 1;
        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(
            result,
            Some(ProviderSelectionCommand::Select(ProviderKind::Codex))
        );
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = ProviderSelectionOverlay::new();
        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(result, Some(ProviderSelectionCommand::Close));
    }
}
