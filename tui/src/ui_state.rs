use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::logging;
use agent_core::runtime_session::RuntimeSession;
use anyhow::Result;
use std::time::Instant;

use crate::composer::textarea::TextArea;
use crate::composer::textarea::TextAreaState;
use crate::transcript::overlay::TranscriptOverlayState;

#[derive(Debug)]
pub struct TuiState {
    pub session: RuntimeSession,
    pub composer: TextArea,
    pub composer_state: TextAreaState,
    pub transcript_overlay: Option<TranscriptOverlayState>,
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
    pub transcript_scroll_offset: usize,
    pub transcript_max_scroll: usize,
    pub transcript_follow_tail: bool,
    pub transcript_rendered_lines: Vec<String>,
    pub transcript_last_cell_range: Option<(usize, usize)>,
    pub busy_started_at: Option<Instant>,
}

impl TuiState {
    pub fn from_session(session: RuntimeSession) -> Self {
        let composer = TextArea::from_text(session.app.input.clone());
        Self {
            session,
            composer,
            composer_state: TextAreaState::default(),
            transcript_overlay: None,
            composer_width: 80,
            transcript_viewport_height: 1,
            transcript_scroll_offset: 0,
            transcript_max_scroll: 0,
            transcript_follow_tail: true,
            transcript_rendered_lines: Vec::new(),
            transcript_last_cell_range: None,
            busy_started_at: None,
        }
    }

    pub fn app(&self) -> &AppState {
        &self.session.app
    }

    pub fn app_mut(&mut self) -> &mut AppState {
        &mut self.session.app
    }

    pub fn sync_app_input_from_composer(&mut self) {
        self.session.app.input = self.composer.text().to_string();
    }

    pub fn into_app_state(mut self) -> AppState {
        self.sync_app_input_from_composer();
        self.session.app
    }

    pub fn persist_if_changed(&mut self) -> Result<()> {
        self.session.persist_if_changed()
    }

    pub fn is_overlay_open(&self) -> bool {
        self.transcript_overlay.is_some()
    }

    pub fn open_transcript_overlay(&mut self) {
        if self.transcript_overlay.is_none() {
            self.transcript_overlay = Some(TranscriptOverlayState::default());
        }
    }

    pub fn close_transcript_overlay(&mut self) {
        self.transcript_overlay = None;
    }

    pub fn scroll_transcript_up(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_sub(rows);
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_down(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_add(rows);
        if rows > 0 {
            self.transcript_follow_tail =
                self.transcript_scroll_offset >= self.transcript_max_scroll;
        }
    }

    pub fn scroll_transcript_home(&mut self) {
        self.transcript_scroll_offset = 0;
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_end(&mut self) {
        self.transcript_follow_tail = true;
    }

    pub fn sync_busy_started_at(&mut self) {
        if self.is_busy() {
            if self.busy_started_at.is_none() {
                self.busy_started_at = Some(Instant::now());
            }
        } else {
            self.busy_started_at = None;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.session.app.status == AppStatus::Responding
            || !matches!(self.session.app.loop_phase, LoopPhase::Idle)
    }

    pub fn switch_to_new_agent(
        &mut self,
        provider_kind: agent_core::provider::ProviderKind,
    ) -> Result<String> {
        self.sync_app_input_from_composer();
        let summary = self.session.switch_agent(provider_kind)?;
        logging::debug_event(
            "tui.provider_switch",
            "switched to sibling agent from TUI state",
            serde_json::json!({
                "provider": provider_kind.label(),
                "summary": summary,
            }),
        );
        self.composer = TextArea::new();
        self.composer_state = TextAreaState::default();
        self.transcript_overlay = None;
        self.transcript_scroll_offset = 0;
        self.transcript_max_scroll = 0;
        self.transcript_follow_tail = true;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        self.busy_started_at = None;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::TuiState;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use tempfile::TempDir;

    #[test]
    fn switching_provider_creates_new_agent_runtime() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
                .expect("bootstrap");
        session.app.push_status_message("existing transcript");

        let mut state = TuiState::from_session(session);
        let previous_agent_id = state.session.agent_runtime.agent_id().as_str().to_string();

        let summary = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert_ne!(
            state.session.agent_runtime.agent_id().as_str(),
            previous_agent_id
        );
        assert_eq!(state.session.app.selected_provider, ProviderKind::Codex);
        assert!(summary.contains("agent_"));
        assert!(matches!(
            state.session.app.transcript.first(),
            Some(TranscriptEntry::Status(text)) if text.contains("created agent:")
        ));
    }

    #[test]
    fn scrolling_down_to_known_tail_restores_follow_mode() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_scroll_offset = 6;
        state.transcript_max_scroll = 6;
        state.transcript_follow_tail = false;

        state.scroll_transcript_up(2);
        assert!(!state.transcript_follow_tail);

        state.scroll_transcript_down(2);

        assert_eq!(state.transcript_scroll_offset, 6);
        assert!(state.transcript_follow_tail);
    }
}
