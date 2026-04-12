use agent_core::app::AppStatus;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

use crate::ui_state::TuiState;

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
    Quit,
}

pub fn handle_paste_event(state: &mut TuiState, pasted_text: &str) {
    if pasted_text.is_empty() || state.app.skill_browser_open || state.is_overlay_open() {
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
        && !state.app.skill_browser_open
        && !state.is_overlay_open()
    {
        return InputOutcome::Quit;
    }

    if state.app.skill_browser_open {
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
        KeyEvent {
            code: KeyCode::Tab, ..
        } if state.app.status == AppStatus::Idle => InputOutcome::ToggleProvider,
        KeyEvent {
            code: KeyCode::Char('p'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && state.app.status == AppStatus::Idle => {
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
        } if state.composer.is_empty() => InputOutcome::ScrollTranscriptUp(1),
        KeyEvent {
            code: KeyCode::Down,
            ..
        } if state.composer.is_empty() => InputOutcome::ScrollTranscriptDown(1),
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
            if state.app.status == AppStatus::Responding {
                return InputOutcome::None;
            }
            let Some(submitted) = state.composer.take_submission() else {
                return InputOutcome::None;
            };
            state.sync_app_input_from_composer();
            state.app.push_user_message(submitted.clone());
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
    use agent_core::skills::SkillRegistry;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn enter_submits_user_input() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);
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
    fn tab_requests_provider_toggle() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ToggleProvider));
    }

    #[test]
    fn empty_composer_up_scrolls_transcript() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ScrollTranscriptUp(1)));
    }

    #[test]
    fn ctrl_t_opens_transcript_overlay() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);

        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        );

        assert!(matches!(outcome, InputOutcome::OpenTranscript));
    }

    #[test]
    fn paste_appends_multiline_text_when_idle() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);

        handle_paste_event(&mut state, "hello\nworld");

        assert_eq!(state.composer.text(), "hello\nworld");
    }

    #[test]
    fn paste_is_ignored_when_overlay_is_open() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);
        state.open_transcript_overlay();

        handle_paste_event(&mut state, "hello");

        assert!(state.composer.text().is_empty());
    }

    #[test]
    fn submit_is_blocked_while_responding() {
        let app = AppState::new(ProviderKind::Mock);
        let mut state = TuiState::from_app(app);
        state.app.status = AppStatus::Responding;
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
        let mut state = TuiState::from_app(app);
        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        );
        assert!(matches!(outcome, InputOutcome::OpenSkills));
    }
}
