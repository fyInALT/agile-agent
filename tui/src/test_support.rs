use agent_core::agent_runtime::AgentRuntime;
use agent_core::agent_runtime::WorkplaceId;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::provider::ProviderKind;
use agent_core::runtime_session::RuntimeSession;
use agent_core::shared_state::SharedWorkplaceState;
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

        let mut app = AppState::new(provider);
        app.cwd = workdir.path().to_path_buf();
        for warning in runtime.apply_to_app_state(&mut app) {
            app.push_error_message(warning);
        }

        let workplace_state = SharedWorkplaceState::with_skills(
            WorkplaceId::new(workplace.workplace_id().as_str()),
            SkillRegistry::discover(workdir.path()),
        );

        let session = RuntimeSession {
            app,
            agent_runtime: runtime,
            workplace: workplace_state,
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
            InputOutcome::FocusNextAgent => {
                if let Some(status) = self.state.focus_next_agent() {
                    self.state.app_mut().push_status_message(format!(
                        "focused {} ({})",
                        status.codename.as_str(),
                        status.status.label()
                    ));
                } else {
                    self.state
                        .app_mut()
                        .push_status_message("no agents to switch (press Ctrl+N to spawn)");
                }
            }
            InputOutcome::FocusPreviousAgent => {
                if let Some(status) = self.state.focus_previous_agent() {
                    self.state.app_mut().push_status_message(format!(
                        "focused {} ({})",
                        status.codename.as_str(),
                        status.status.label()
                    ));
                } else {
                    self.state
                        .app_mut()
                        .push_status_message("no agents to switch (press Ctrl+N to spawn)");
                }
            }
            InputOutcome::FocusAgent(index) => {
                if let Some(status) = self.state.focus_agent_by_index(index) {
                    self.state.app_mut().push_status_message(format!(
                        "focused {} ({})",
                        status.codename.as_str(),
                        status.status.label()
                    ));
                } else {
                    self.state
                        .app_mut()
                        .push_status_message(format!("no agent at index {}", index + 1));
                }
            }
            InputOutcome::SpawnAgent => {
                self.state.open_provider_overlay();
            }
            InputOutcome::StopFocusedAgent => {
                let codename = self.state.focused_agent_codename().to_string();
                self.state.open_stop_confirmation(&codename);
            }
            InputOutcome::Submit(text) => {
                self.state.app_mut().push_user_message(text);
            }
            InputOutcome::Quit => self.state.workplace_mut().loop_control.signal_quit(),
            InputOutcome::SwitchViewMode(n) => {
                self.state.view_state.switch_by_number(n);
            }
            InputOutcome::NextViewMode => {
                self.state.view_state.next_mode();
            }
            InputOutcome::PrevViewMode => {
                self.state.view_state.prev_mode();
            }
            InputOutcome::SplitFocusLeft => {
                self.state.view_state.split.focus_left();
            }
            InputOutcome::SplitFocusRight => {
                self.state.view_state.split.focus_right();
            }
            InputOutcome::SplitSwap => {
                self.state.view_state.split.swap();
            }
            InputOutcome::SplitEqual => {
                self.state.view_state.split.equal_split();
            }
            InputOutcome::DashboardNext => {
                let count = self.state.agent_statuses().len();
                self.state.view_state.dashboard.select_next(count);
            }
            InputOutcome::DashboardPrev => {
                self.state.view_state.dashboard.select_prev();
            }
            InputOutcome::DashboardSelect(n) => {
                let count = self.state.agent_statuses().len();
                self.state.view_state.dashboard.select_by_number(n, count);
            }
            InputOutcome::MailNext => {
                self.state.view_state.mail.select_next(0);
            }
            InputOutcome::MailPrev => {
                self.state.view_state.mail.select_prev();
            }
            InputOutcome::MailMarkRead => {}
            InputOutcome::MailComposeStart => {
                self.state.view_state.mail.start_compose();
            }
            InputOutcome::MailComposeCancel => {
                self.state.view_state.mail.cancel_compose();
            }
            InputOutcome::MailComposeNextField => {
                self.state.view_state.mail.next_compose_field();
            }
            InputOutcome::MailComposePrevField => {
                self.state.view_state.mail.prev_compose_field();
            }
            InputOutcome::MailComposeSend(_, _, _) => {}
            InputOutcome::OverviewFilterBlocked => {
                self.state.view_state.overview.filter =
                    crate::overview_state::OverviewFilter::BlockedOnly;
            }
            InputOutcome::OverviewFilterRunning => {
                self.state.view_state.overview.filter =
                    crate::overview_state::OverviewFilter::RunningOnly;
            }
            InputOutcome::OverviewFilterAll => {
                self.state.view_state.overview.filter = crate::overview_state::OverviewFilter::All;
            }
            InputOutcome::OverviewPageUp => {
                self.state.view_state.overview.page_up(1);
            }
            InputOutcome::OverviewPageDown => {
                self.state.view_state.overview.page_down(1);
            }
            InputOutcome::OverviewSearchStart => {
                self.state.view_state.overview.search_active = true;
                self.state.view_state.overview.search_query.clear();
            }
            InputOutcome::OverviewSearchCancel => {
                self.state.view_state.overview.search_active = false;
                self.state.view_state.overview.search_query.clear();
            }
            InputOutcome::OverviewSearchSelect(agent_name) => {
                let statuses = self.state.agent_statuses();
                if let Some(index) = statuses
                    .iter()
                    .position(|s| s.codename.as_str() == agent_name)
                {
                    self.state.view_state.overview.focused_agent_index = index;
                    self.state.view_state.overview.search_active = false;
                    self.state.view_state.overview.search_query.clear();
                }
            }
        }
    }
}
