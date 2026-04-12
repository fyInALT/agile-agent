use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

#[derive(Debug, Clone, Default)]
pub struct TranscriptOverlayState {
    pub scroll_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayCommand {
    Close,
}

impl TranscriptOverlayState {
    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
        page_height: usize,
    ) -> Option<OverlayCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(OverlayCommand::Close),
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                None
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(page_height.max(1));
                None
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_offset = self.scroll_offset.saturating_add(page_height.max(1));
                None
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                None
            }
            KeyCode::End => {
                self.scroll_offset = usize::MAX;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OverlayCommand;
    use super::TranscriptOverlayState;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn page_navigation_updates_scroll_offset() {
        let mut overlay = TranscriptOverlayState::default();

        overlay.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 10);
        assert_eq!(overlay.scroll_offset, 10);

        overlay.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), 10);
        assert_eq!(overlay.scroll_offset, 9);

        overlay.handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), 10);
        assert_eq!(overlay.scroll_offset, 0);
    }

    #[test]
    fn esc_closes_overlay() {
        let mut overlay = TranscriptOverlayState::default();
        let command = overlay.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);
        assert!(matches!(command, Some(OverlayCommand::Close)));
    }
}
