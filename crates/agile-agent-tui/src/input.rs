use agile_agent_core::app::AppState;
use agile_agent_core::app::AppStatus;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

pub enum InputOutcome {
    None,
    Submit(String),
    Quit,
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

    if state.status == AppStatus::Responding {
        return InputOutcome::None;
    }

    match key_event.code {
        KeyCode::Char('q') if state.input.is_empty() => InputOutcome::Quit,
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
    use agile_agent_core::app::AppState;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn enter_submits_user_input() {
        let mut state = AppState::default();
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
}
