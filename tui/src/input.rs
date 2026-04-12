use agent_core::app::AppState;
use agent_core::app::AppStatus;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

pub enum InputOutcome {
    None,
    Submit(String),
    ToggleProvider,
    OpenSkills,
    CloseSkills,
    SkillUp,
    SkillDown,
    ToggleSelectedSkill,
    Quit,
}

pub fn handle_paste_event(state: &mut AppState, pasted_text: &str) {
    if pasted_text.is_empty() || state.skill_browser_open || state.status == AppStatus::Responding {
        return;
    }

    state.insert_text(pasted_text);
}

pub fn handle_key_event(state: &mut AppState, key_event: KeyEvent) -> InputOutcome {
    if key_event.kind != KeyEventKind::Press {
        return InputOutcome::None;
    }

    if matches!(key_event.code, KeyCode::Esc) {
        return InputOutcome::Quit;
    }

    if key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c'))
    {
        return InputOutcome::Quit;
    }

    if matches!(key_event.code, KeyCode::Char('q')) && state.input.is_empty() {
        return InputOutcome::Quit;
    }

    if state.skill_browser_open {
        return match key_event.code {
            KeyCode::Esc => InputOutcome::CloseSkills,
            KeyCode::Up => InputOutcome::SkillUp,
            KeyCode::Down => InputOutcome::SkillDown,
            KeyCode::Enter | KeyCode::Char(' ') => InputOutcome::ToggleSelectedSkill,
            _ => InputOutcome::None,
        };
    }

    if state.status == AppStatus::Responding {
        return InputOutcome::None;
    }

    match key_event.code {
        KeyCode::Tab => InputOutcome::ToggleProvider,
        KeyCode::Char('$') if state.input.is_empty() => InputOutcome::OpenSkills,
        KeyCode::Char(ch) if !has_non_shift_modifiers(key_event.modifiers) => {
            state.insert_char(ch);
            InputOutcome::None
        }
        KeyCode::Backspace => {
            state.backspace();
            InputOutcome::None
        }
        KeyCode::Enter => {
            let Some(submitted) = state.take_input() else {
                return InputOutcome::None;
            };
            state.push_user_message(submitted.clone());
            InputOutcome::Submit(submitted)
        }
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
    use agent_core::app::AppState;
    use agent_core::app::AppStatus;
    use agent_core::provider::ProviderKind;
    use agent_core::skills::SkillRegistry;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn enter_submits_user_input() {
        let mut state = AppState::new(ProviderKind::Mock);
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
        let mut state = AppState::new(ProviderKind::Mock);

        let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(matches!(outcome, InputOutcome::ToggleProvider));
    }

    #[test]
    fn dollar_opens_skill_browser_when_input_is_empty() {
        let mut state =
            AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        let outcome = handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        );
        assert!(matches!(outcome, InputOutcome::OpenSkills));
    }

    #[test]
    fn paste_appends_multiline_text_when_idle() {
        let mut state = AppState::new(ProviderKind::Mock);

        handle_paste_event(&mut state, "hello\nworld");

        assert_eq!(state.input, "hello\nworld");
    }

    #[test]
    fn paste_is_ignored_while_responding() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.status = AppStatus::Responding;

        handle_paste_event(&mut state, "hello");

        assert!(state.input.is_empty());
    }

    #[test]
    fn paste_is_ignored_when_skill_browser_is_open() {
        let mut state =
            AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        state.skill_browser_open = true;

        handle_paste_event(&mut state, "hello");

        assert!(state.input.is_empty());
    }
}
