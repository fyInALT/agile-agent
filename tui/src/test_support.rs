use agent_core::agent_runtime::AgentRuntime;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::provider::ProviderKind;
use agent_core::runtime_session::RuntimeSession;
use agent_core::skills::SkillRegistry;
use agent_core::workplace_store::WorkplaceStore;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::input::handle_paste_event;
use crate::render::render_app;
use crate::ui_state::TuiState;

pub(crate) struct ShellHarness {
    pub(crate) state: TuiState,
    _workdir: TempDir,
    _root: TempDir,
}

impl ShellHarness {
    pub(crate) fn new(provider: ProviderKind) -> Self {
        let workdir = TempDir::new().expect("temp workdir");
        let root = TempDir::new().expect("temp root");
        let workplace = WorkplaceStore::for_root(
            workdir.path(),
            root.path().join(".agile-agent").join("workplaces"),
        )
        .expect("workplace");
        workplace.ensure().expect("ensure workplace");

        let runtime = AgentRuntime::new(&workplace, 1, provider);
        runtime.persist().expect("persist runtime");

        let mut app = AppState::with_skills(
            provider,
            workdir.path().to_path_buf(),
            SkillRegistry::discover(workdir.path()),
        );
        for warning in runtime.apply_to_app_state(&mut app) {
            app.push_error_message(warning);
        }

        let session = RuntimeSession {
            app,
            agent_runtime: runtime,
        };

        Self {
            state: TuiState::from_session(session),
            _workdir: workdir,
            _root: root,
        }
    }

    pub(crate) fn render_to_string(&mut self, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_app(frame, &mut self.state))
            .expect("draw");
        let buf = terminal.backend().buffer();
        let mut rendered = String::new();
        for y in 0..height {
            for x in 0..width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        rendered
    }

    pub(crate) fn press(&mut self, key_code: KeyCode, modifiers: KeyModifiers) {
        let outcome = handle_key_event(&mut self.state, KeyEvent::new(key_code, modifiers));
        self.apply_input_outcome(outcome);
    }

    pub(crate) fn paste(&mut self, text: &str) {
        handle_paste_event(&mut self.state, text);
    }

    fn apply_input_outcome(&mut self, outcome: InputOutcome) {
        match outcome {
            InputOutcome::None => {}
            InputOutcome::ToggleProvider if self.state.app().status == AppStatus::Idle => {
                let next_provider = self.state.app().selected_provider.next();
                let _ = self.state.switch_to_new_agent(next_provider);
            }
            InputOutcome::ToggleProvider => {}
            InputOutcome::OpenTranscript => self.state.open_transcript_overlay(),
            InputOutcome::CloseSkills => self.state.app_mut().close_skill_browser(),
            InputOutcome::OpenSkills => self.state.app_mut().open_skill_browser(),
            InputOutcome::SkillUp => self.state.app_mut().move_skill_selection_up(),
            InputOutcome::SkillDown => self.state.app_mut().move_skill_selection_down(),
            InputOutcome::ToggleSelectedSkill => self.state.app_mut().toggle_selected_skill(),
            InputOutcome::ScrollTranscriptUp(rows) => self.state.scroll_transcript_up(rows),
            InputOutcome::ScrollTranscriptDown(rows) => self.state.scroll_transcript_down(rows),
            InputOutcome::ScrollTranscriptHome => self.state.scroll_transcript_home(),
            InputOutcome::ScrollTranscriptEnd => self.state.scroll_transcript_end(),
            InputOutcome::Submit(text) => {
                self.state.app_mut().push_user_message(text);
            }
            InputOutcome::Quit => self.state.app_mut().request_quit(),
        }
    }
}
