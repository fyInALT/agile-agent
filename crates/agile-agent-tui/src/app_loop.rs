use agile_agent_core::app::AppState;
use agile_agent_core::app::AppStatus;
use agile_agent_core::provider;
use agile_agent_core::provider::ProviderEvent;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use std::sync::mpsc;
use std::time::Duration;

use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::render::render_app;
use crate::terminal::AppTerminal;

pub fn run(terminal: &mut AppTerminal) -> Result<()> {
    let mut state = AppState::new(provider::default_provider());
    let mut provider_rx: Option<mpsc::Receiver<ProviderEvent>> = None;

    loop {
        terminal.draw(|frame| render_app(frame, &state))?;

        if state.should_quit {
            break;
        }

        let poll_timeout = if provider_rx.is_some() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(250)
        };

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => match handle_key_event(&mut state, key_event) {
                    InputOutcome::None => {}
                    InputOutcome::ToggleProvider => {
                        if state.status == AppStatus::Idle {
                            state.toggle_provider();
                            state.push_status_message(format!(
                                "selected provider: {}",
                                state.selected_provider.label()
                            ));
                        }
                    }
                    InputOutcome::Quit => state.request_quit(),
                    InputOutcome::Submit(user_input) => {
                        let (event_tx, event_rx) = mpsc::channel();
                        let provider_kind = state.selected_provider;
                        if let Err(err) =
                            provider::start_provider(provider_kind, user_input, event_tx)
                        {
                            state.push_error_message(format!("failed to start provider: {err}"));
                        } else {
                            state.begin_provider_response();
                            provider_rx = Some(event_rx);
                        }
                    }
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let mut should_clear_provider_rx = false;
        if let Some(rx) = provider_rx.as_ref() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ProviderEvent::Status(text) => state.push_status_message(text),
                    ProviderEvent::AssistantChunk(chunk) => state.append_assistant_chunk(&chunk),
                    ProviderEvent::Error(error) => state.push_error_message(error),
                    ProviderEvent::Finished => {
                        state.finish_provider_response();
                        should_clear_provider_rx = true;
                        break;
                    }
                }
            }
        }

        if should_clear_provider_rx {
            provider_rx = None;
        }
    }

    Ok(())
}
