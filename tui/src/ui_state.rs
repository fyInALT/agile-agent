use agent_core::agent_runtime::AgentRuntime;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use std::time::Instant;

use crate::composer::textarea::TextArea;
use crate::composer::textarea::TextAreaState;
use crate::transcript::overlay::TranscriptOverlayState;

#[derive(Debug)]
pub struct TuiState {
    pub app: AppState,
    pub agent_runtime: AgentRuntime,
    pub composer: TextArea,
    pub composer_state: TextAreaState,
    pub transcript_overlay: Option<TranscriptOverlayState>,
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
    pub transcript_scroll_offset: usize,
    pub transcript_follow_tail: bool,
    pub busy_started_at: Option<Instant>,
}

impl TuiState {
    pub fn from_app(app: AppState, agent_runtime: AgentRuntime) -> Self {
        let composer = TextArea::from_text(app.input.clone());
        Self {
            app,
            agent_runtime,
            composer,
            composer_state: TextAreaState::default(),
            transcript_overlay: None,
            composer_width: 80,
            transcript_viewport_height: 1,
            transcript_scroll_offset: 0,
            transcript_follow_tail: true,
            busy_started_at: None,
        }
    }

    pub fn sync_app_input_from_composer(&mut self) {
        self.app.input = self.composer.text().to_string();
    }

    pub fn into_app_state(mut self) -> AppState {
        self.sync_app_input_from_composer();
        self.app
    }

    pub fn sync_agent_runtime_from_app(&mut self) -> bool {
        self.agent_runtime.sync_from_app_state(&self.app)
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
            self.transcript_follow_tail = false;
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
        self.app.status == AppStatus::Responding || !matches!(self.app.loop_phase, LoopPhase::Idle)
    }
}
