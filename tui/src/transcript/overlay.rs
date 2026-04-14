use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::text::Line;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveTailKey {
    width: u16,
    revision: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptOverlayState {
    pub scroll_offset: usize,
    max_scroll: usize,
    live_tail_key: Option<LiveTailKey>,
    live_tail_lines: Vec<Line<'static>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayCommand {
    Close,
}

impl TranscriptOverlayState {
    pub fn pinned_to_bottom() -> Self {
        Self {
            scroll_offset: usize::MAX,
            ..Self::default()
        }
    }

    pub fn set_max_scroll(&mut self, max_scroll: usize) {
        self.max_scroll = max_scroll;
        if self.scroll_offset != usize::MAX && self.scroll_offset > self.max_scroll {
            self.scroll_offset = self.max_scroll;
        }
    }

    pub fn render_scroll_offset(&self) -> usize {
        if self.scroll_offset == usize::MAX {
            self.max_scroll
        } else {
            self.scroll_offset.min(self.max_scroll)
        }
    }

    pub fn sync_live_tail(
        &mut self,
        width: u16,
        revision: Option<u64>,
        compute_lines: impl FnOnce() -> Vec<Line<'static>>,
    ) {
        let next_key = revision.map(|revision| LiveTailKey { width, revision });
        if self.live_tail_key == next_key {
            return;
        }

        self.live_tail_key = next_key;
        self.live_tail_lines = match next_key {
            Some(_) => compute_lines(),
            None => Vec::new(),
        };
    }

    pub fn live_tail_lines(&self) -> &[Line<'static>] {
        &self.live_tail_lines
    }

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
                self.scroll_offset = self.render_scroll_offset().saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = self.render_scroll_offset().saturating_add(1);
                self.scroll_offset = if next >= self.max_scroll {
                    usize::MAX
                } else {
                    next
                };
                None
            }
            KeyCode::PageUp => {
                self.scroll_offset = self
                    .render_scroll_offset()
                    .saturating_sub(page_height.max(1));
                None
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                let next = self
                    .render_scroll_offset()
                    .saturating_add(page_height.max(1));
                self.scroll_offset = if next >= self.max_scroll {
                    usize::MAX
                } else {
                    next
                };
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
    use ratatui::text::Line;
    use std::cell::Cell;

    #[test]
    fn page_navigation_updates_scroll_offset() {
        let mut overlay = TranscriptOverlayState::default();
        overlay.set_max_scroll(20);

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

    #[test]
    fn end_keeps_overlay_pinned_to_bottom_as_content_grows() {
        let mut overlay = TranscriptOverlayState::default();
        overlay.set_max_scroll(10);

        overlay.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), 10);
        assert_eq!(overlay.render_scroll_offset(), 10);

        overlay.set_max_scroll(25);
        assert_eq!(overlay.render_scroll_offset(), 25);
    }

    #[test]
    fn up_from_pinned_bottom_enters_manual_scroll_near_tail() {
        let mut overlay = TranscriptOverlayState::default();
        overlay.set_max_scroll(10);
        overlay.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), 10);

        overlay.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), 10);

        assert_eq!(overlay.render_scroll_offset(), 9);
    }

    #[test]
    fn sync_live_tail_is_noop_for_identical_key() {
        let mut overlay = TranscriptOverlayState::default();
        let computes = Cell::new(0);

        overlay.sync_live_tail(40, Some(1), || {
            computes.set(computes.get() + 1);
            vec![Line::from("tail")]
        });
        overlay.sync_live_tail(40, Some(1), || {
            computes.set(computes.get() + 1);
            vec![Line::from("changed")]
        });

        assert_eq!(computes.get(), 1);
        assert_eq!(
            overlay.live_tail_lines(),
            vec![Line::from("tail")].as_slice()
        );
    }

    #[test]
    fn sync_live_tail_recomputes_when_revision_changes_and_drops_when_cleared() {
        let mut overlay = TranscriptOverlayState::default();

        overlay.sync_live_tail(40, Some(1), || vec![Line::from("tail-1")]);
        overlay.sync_live_tail(40, Some(2), || vec![Line::from("tail-2")]);
        assert_eq!(
            overlay.live_tail_lines(),
            vec![Line::from("tail-2")].as_slice()
        );

        overlay.sync_live_tail(40, None, || vec![Line::from("ignored")]);
        assert!(overlay.live_tail_lines().is_empty());
    }
}
