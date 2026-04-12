use agent_core::agent_runtime::AgentRuntime;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use anyhow::Result;
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

    pub fn switch_to_new_agent(
        &mut self,
        provider_kind: agent_core::provider::ProviderKind,
    ) -> Result<String> {
        self.sync_app_input_from_composer();
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        self.agent_runtime.persist()?;

        let next_runtime = self.agent_runtime.create_sibling(provider_kind)?;
        let cwd = self.app.cwd.clone();
        let backlog = self.app.backlog.clone();
        let mut skills = agent_core::skills::SkillRegistry::discover(&cwd);
        skills.enabled_names = self.app.skills.enabled_names.clone();

        let mut next_app = AppState::with_skills(provider_kind, cwd, skills);
        next_app.backlog = backlog;
        for warning in next_runtime.apply_to_app_state(&mut next_app) {
            next_app.push_error_message(warning);
        }
        let summary = next_runtime.summary();
        next_app.push_status_message(format!("created agent: {summary}"));

        self.app = next_app;
        self.agent_runtime = next_runtime;
        self.composer = TextArea::new();
        self.composer_state = TextAreaState::default();
        self.transcript_overlay = None;
        self.transcript_scroll_offset = 0;
        self.transcript_follow_tail = true;
        self.busy_started_at = None;

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::TuiState;
    use agent_core::agent_runtime::AgentRuntime;
    use agent_core::app::AppState;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
    use agent_core::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    #[test]
    fn switching_provider_creates_new_agent_runtime() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Claude);
        runtime.persist().expect("persist");
        let mut app = AppState::new(ProviderKind::Claude);
        app.push_status_message("existing transcript");

        let mut state = TuiState::from_app(app, runtime);
        let previous_agent_id = state.agent_runtime.agent_id().as_str().to_string();

        let summary = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert_ne!(state.agent_runtime.agent_id().as_str(), previous_agent_id);
        assert_eq!(state.app.selected_provider, ProviderKind::Codex);
        assert!(summary.contains("agent_"));
        assert!(matches!(
            state.app.transcript.first(),
            Some(TranscriptEntry::Status(text)) if text.contains("created agent:")
        ));
    }
}
