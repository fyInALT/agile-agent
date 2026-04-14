use agent_core::app::AppStatus;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

use crate::ui_state::TuiState;

const TRANSCRIPT_SCROLL_STEP: usize = 3;

pub enum InputOutcome {
    None,
    Submit(String),
    ToggleProvider,
    ScrollTranscriptUp(usize),
    ScrollTranscriptDown(usize),
    ScrollTranscriptHome,
    ScrollTranscriptEnd,
    OpenSkills,
    CloseSkills,
    SkillUp,
    SkillDown,
    ToggleSelectedSkill,
    OpenTranscript,
    FocusNextAgent,
    FocusPreviousAgent,
    FocusAgent(usize),
    SpawnAgent,
    StopFocusedAgent,
    Quit,
    /// Switch to specific view mode (1-5)
    SwitchViewMode(u8),
    /// Cycle to next view mode
    NextViewMode,
    /// Cycle to previous view mode
    PrevViewMode,
    /// Split view: focus left side
    SplitFocusLeft,
    /// Split view: focus right side
    SplitFocusRight,
    /// Split view: swap left/right agents
    SplitSwap,
    /// Split view: equal split ratio
    SplitEqual,
    /// Dashboard: select next card
    DashboardNext,
    /// Dashboard: select previous card
    DashboardPrev,
    /// Dashboard: select card by number
    DashboardSelect(u8),
    /// Mail: select next mail
    MailNext,
    /// Mail: select previous mail
    MailPrev,
    /// Mail: mark selected as read
    MailMarkRead,
    /// Mail: start compose
    MailComposeStart,
    /// Mail: cancel compose
    MailComposeCancel,
    /// Mail: send composed mail
    MailComposeSend(String),
}

pub fn handle_paste_event(state: &mut TuiState, pasted_text: &str) {
    if pasted_text.is_empty() || state.app().skill_browser_open || state.is_overlay_open() {
        return;
    }

    state.composer.insert_text(pasted_text);
    state.sync_app_input_from_composer();
}

pub fn handle_key_event(state: &mut TuiState, key_event: KeyEvent) -> InputOutcome {
    if key_event.kind != KeyEventKind::Press {
        return InputOutcome::None;
    }

    if key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c'))
    {
        return InputOutcome::Quit;
    }

    if matches!(key_event.code, KeyCode::Char('q'))
        && state.composer.is_empty()
        && !state.app().skill_browser_open
        && !state.is_overlay_open()
    {
        return InputOutcome::Quit;
    }

    if state.app().skill_browser_open {
        return match key_event.code {
            KeyCode::Esc => InputOutcome::CloseSkills,
            KeyCode::Up => InputOutcome::SkillUp,
            KeyCode::Down => InputOutcome::SkillDown,
            KeyCode::Enter | KeyCode::Char(' ') => InputOutcome::ToggleSelectedSkill,
            _ => InputOutcome::None,
        };
    }

    if state.is_overlay_open() {
        return InputOutcome::None;
    }

    match key_event {
        KeyEvent {
            code: KeyCode::Char('t'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::OpenTranscript,
        // View mode switching (Ctrl+V 1-5)
        KeyEvent {
            code: KeyCode::Char('1'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(1),
        KeyEvent {
            code: KeyCode::Char('2'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(2),
        KeyEvent {
            code: KeyCode::Char('3'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(3),
        KeyEvent {
            code: KeyCode::Char('4'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(4),
        KeyEvent {
            code: KeyCode::Char('5'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(5),
        // Alt+V to cycle view modes
        KeyEvent {
            code: KeyCode::Char('v'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::ALT) => InputOutcome::NextViewMode,

        // View-specific key handling (when composer is empty and in specific mode)
        // Split view: arrow keys for side selection
        KeyEvent {
            code: KeyCode::Left,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Split => InputOutcome::SplitFocusLeft,
        KeyEvent {
            code: KeyCode::Right,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Split => InputOutcome::SplitFocusRight,
        KeyEvent {
            code: KeyCode::Char('s'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Split => InputOutcome::SplitSwap,
        KeyEvent {
            code: KeyCode::Char('e'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Split => InputOutcome::SplitEqual,

        // Dashboard view: arrow keys and number selection
        KeyEvent {
            code: KeyCode::Up,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Dashboard => InputOutcome::DashboardPrev,
        KeyEvent {
            code: KeyCode::Down,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Dashboard => InputOutcome::DashboardNext,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Dashboard
            && c >= '1' && c <= '9' => InputOutcome::DashboardSelect(c as u8),

        // Mail view: arrow keys, c compose, r reply, m mark read
        KeyEvent {
            code: KeyCode::Up,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Mail => InputOutcome::MailPrev,
        KeyEvent {
            code: KeyCode::Down,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Mail => InputOutcome::MailNext,
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Mail
            && !state.view_state.mail.composing => InputOutcome::MailComposeStart,
        KeyEvent {
            code: KeyCode::Esc,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE)
            && state.view_state.mode == crate::view_mode::ViewMode::Mail
            && state.view_state.mail.composing => InputOutcome::MailComposeCancel,
        KeyEvent {
            code: KeyCode::Enter,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE)
            && state.view_state.mode == crate::view_mode::ViewMode::Mail
            && state.view_state.mail.composing => {
                InputOutcome::MailComposeSend(state.view_state.mail.compose_buffer.clone())
            },
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE)
            && state.view_state.mode == crate::view_mode::ViewMode::Mail
            && state.view_state.mail.composing => {
                state.view_state.mail.append_char(c);
                InputOutcome::None
            },
        KeyEvent {
            code: KeyCode::Backspace,
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE)
            && state.view_state.mode == crate::view_mode::ViewMode::Mail
            && state.view_state.mail.composing => {
                state.view_state.mail.remove_char();
                InputOutcome::None
            },
        KeyEvent {
            code: KeyCode::Char('m'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
            && state.view_state.mode == crate::view_mode::ViewMode::Mail => InputOutcome::MailMarkRead,

        // Agent focus switching (Ctrl+1-9 for direct selection)
        KeyEvent {
            code: KeyCode::Char('1'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(0),
        KeyEvent {
            code: KeyCode::Char('2'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(1),
        KeyEvent {
            code: KeyCode::Char('3'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(2),
        KeyEvent {
            code: KeyCode::Char('4'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(3),
        KeyEvent {
            code: KeyCode::Char('5'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(4),
        KeyEvent {
            code: KeyCode::Char('6'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(5),
        KeyEvent {
            code: KeyCode::Char('7'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(6),
        KeyEvent {
            code: KeyCode::Char('8'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(7),
        KeyEvent {
            code: KeyCode::Char('9'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => InputOutcome::FocusAgent(8),
        // Tab for next agent, Shift+Tab for previous (when idle)
        KeyEvent {
            code: KeyCode::Tab, ..
        } if state.app().status == AppStatus::Idle => InputOutcome::FocusNextAgent,
        KeyEvent {
            code: KeyCode::BackTab, ..
        } if state.app().status == AppStatus::Idle => InputOutcome::FocusPreviousAgent,
        // Ctrl+N to spawn new agent
        KeyEvent {
            code: KeyCode::Char('n'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && state.app().status == AppStatus::Idle => {
            InputOutcome::SpawnAgent
        }
        // Ctrl+X to stop focused agent
        KeyEvent {
            code: KeyCode::Char('x'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && state.app().status == AppStatus::Idle => {
            InputOutcome::StopFocusedAgent
        }
        KeyEvent {
            code: KeyCode::Tab, ..
        } => InputOutcome::ToggleProvider,
        KeyEvent {
            code: KeyCode::Char('p'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && state.app().status == AppStatus::Idle => {
            InputOutcome::ToggleProvider
        }
        KeyEvent {
            code: KeyCode::Char('j'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => {
            state.composer.insert_newline();
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Char('$'),
            modifiers: KeyModifiers::NONE,
            ..
        } if state.composer.is_empty() => InputOutcome::OpenSkills,
        KeyEvent {
            code: KeyCode::Left,
            ..
        } => {
            state.composer.move_left();
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Right,
            ..
        } => {
            state.composer.move_right();
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Up, ..
        } if state.composer.is_empty() => InputOutcome::ScrollTranscriptUp(TRANSCRIPT_SCROLL_STEP),
        KeyEvent {
            code: KeyCode::Down,
            ..
        } if state.composer.is_empty() => {
            InputOutcome::ScrollTranscriptDown(TRANSCRIPT_SCROLL_STEP)
        }
        KeyEvent {
            code: KeyCode::PageUp,
            ..
        } => InputOutcome::ScrollTranscriptUp(state.transcript_viewport_height.max(1) as usize),
        KeyEvent {
            code: KeyCode::PageDown,
            ..
        } => InputOutcome::ScrollTranscriptDown(state.transcript_viewport_height.max(1) as usize),
        KeyEvent {
            code: KeyCode::Home,
            ..
        } if state.composer.is_empty() => InputOutcome::ScrollTranscriptHome,
        KeyEvent {
            code: KeyCode::End, ..
        } if state.composer.is_empty() => InputOutcome::ScrollTranscriptEnd,
        KeyEvent {
            code: KeyCode::Up, ..
        } => {
            state.composer.move_up(state.composer_width);
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Down,
            ..
        } => {
            state.composer.move_down(state.composer_width);
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Home,
            ..
        } => {
            state.composer.move_home(state.composer_width);
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::End, ..
        } => {
            state.composer.move_end(state.composer_width);
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Backspace,
            ..
        } => {
            state.composer.backspace();
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Enter,
            ..
        } => {
            if state.app().status == AppStatus::Responding {
                return InputOutcome::None;
            }
            let Some(submitted) = state.composer.take_submission() else {
                return InputOutcome::None;
            };
            state.sync_app_input_from_composer();
            state.app_mut().push_user_message(submitted.clone());
            InputOutcome::Submit(submitted)
        }
        KeyEvent {
            code: KeyCode::Char(ch),
            modifiers,
            ..
        } if !has_non_shift_modifiers(modifiers) => {
            state.composer.insert_char(ch);
            state.sync_app_input_from_composer();
            InputOutcome::None
        }
        KeyEvent {
            code: KeyCode::Esc, ..
        } => InputOutcome::Quit,
        _ => InputOutcome::None,
    }
}

fn has_non_shift_modifiers(modifiers: KeyModifiers) -> bool {
    modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use super::InputOutcome;
    use super::handle_key_event;
    use super::handle_paste_event;
    use crate::ui_state::TuiState;
    use agent_core::app::AppState;
    use agent_core::app::AppStatus;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use agent_core::skills::SkillRegistry;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use tempfile::TempDir;

    fn state_from_app(app: AppState) -> TuiState {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), app.selected_provider, false)
            .expect("bootstrap");
        let mut session = session;
        session.app = app;
        TuiState::from_session(session)
    }

    #[test]
    fn enter_submits_user_input() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        );
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
        );

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::Submit(text) if text == "hi"));
    }

    #[test]
    fn empty_composer_up_scrolls_transcript_faster() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ScrollTranscriptUp(3)));
    }

    #[test]
    fn tab_requests_focus_next_when_idle() {
        let mut app = AppState::new(ProviderKind::Mock);
        app.status = AppStatus::Idle;
        let mut state = state_from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::FocusNextAgent));
    }

    #[test]
    fn tab_requests_provider_toggle_when_not_idle() {
        let mut app = AppState::new(ProviderKind::Mock);
        app.status = AppStatus::Responding;
        let mut state = state_from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ToggleProvider));
    }

    #[test]
    fn backtab_requests_focus_previous_when_idle() {
        let mut app = AppState::new(ProviderKind::Mock);
        app.status = AppStatus::Idle;
        let mut state = state_from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::FocusPreviousAgent));
    }

    #[test]
    fn empty_composer_up_scrolls_transcript() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ScrollTranscriptUp(3)));
    }

    #[test]
    fn empty_composer_down_scrolls_transcript_in_steps() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome =
            handle_key_event(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ScrollTranscriptDown(3)));
    }

    #[test]
    fn ctrl_t_opens_transcript_overlay() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        );

        assert!(matches!(outcome, InputOutcome::OpenTranscript));
    }

    #[test]
    fn paste_appends_multiline_text_when_idle() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        handle_paste_event(&mut state, "hello\nworld");

        assert_eq!(state.composer.text(), "hello\nworld");
    }

    #[test]
    fn paste_is_ignored_when_overlay_is_open() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.open_transcript_overlay();

        handle_paste_event(&mut state, "hello");

        assert!(state.composer.text().is_empty());
    }

    #[test]
    fn submit_is_blocked_while_responding() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.app_mut().status = AppStatus::Responding;
        state.composer.insert_text("hello");

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::None));
        assert_eq!(state.composer.text(), "hello");
    }

    #[test]
    fn dollar_opens_skill_browser_when_composer_is_empty() {
        let app = AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        let mut state = state_from_app(app);
        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        );
        assert!(matches!(outcome, InputOutcome::OpenSkills));
    }

    #[test]
    fn alt_1_switches_to_focused_view() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        // First switch to a different mode
        state.view_state.switch_by_number(2);

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT),
        );

        assert!(matches!(outcome, InputOutcome::SwitchViewMode(1)));
    }

    #[test]
    fn alt_2_switches_to_split_view() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT),
        );

        assert!(matches!(outcome, InputOutcome::SwitchViewMode(2)));
    }

    #[test]
    fn alt_v_cycles_view_modes() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
        );

        assert!(matches!(outcome, InputOutcome::NextViewMode));
    }

    #[test]
    fn split_view_left_arrow_focuses_left() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(2); // Split mode
        state.view_state.split.focused_side = 1; // Start on right

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::SplitFocusLeft));
    }

    #[test]
    fn split_view_right_arrow_focuses_right() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(2); // Split mode
        state.view_state.split.focused_side = 0; // Start on left

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::SplitFocusRight));
    }

    #[test]
    fn split_view_s_swaps_agents() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(2); // Split mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::SplitSwap));
    }

    #[test]
    fn split_view_e_equal_split() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(2); // Split mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::SplitEqual));
    }

    #[test]
    fn dashboard_down_selects_next() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(3); // Dashboard mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::DashboardNext));
    }

    #[test]
    fn dashboard_up_selects_prev() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(3); // Dashboard mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::DashboardPrev));
    }

    #[test]
    fn dashboard_number_selects_card() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(3); // Dashboard mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::DashboardSelect(51))); // '3' as u8 = 51
    }

    #[test]
    fn mail_down_selects_next() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(4); // Mail mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::MailNext));
    }

    #[test]
    fn mail_up_selects_prev() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(4); // Mail mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::MailPrev));
    }

    #[test]
    fn mail_c_starts_compose() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(4); // Mail mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::MailComposeStart));
    }

    #[test]
    fn mail_m_marks_read() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(4); // Mail mode

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
        );

        assert!(matches!(outcome, InputOutcome::MailMarkRead));
    }

    #[test]
    fn view_keys_blocked_when_composer_not_empty() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        state.view_state.switch_by_number(2); // Split mode
        state.composer.insert_text("text");

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        );

        // Should add 't' to composer, not navigate
        assert!(matches!(outcome, InputOutcome::None));
    }

    #[test]
    fn view_keys_blocked_in_wrong_mode() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = state_from_app(app);
        // Stay in Focused mode (default)
        state.composer.insert_text("text");

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        );

        // Left with text in composer moves cursor, not split navigation
        assert!(matches!(outcome, InputOutcome::None));
    }
}
