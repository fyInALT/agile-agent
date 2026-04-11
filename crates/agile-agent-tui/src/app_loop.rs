use agile_agent_core::app::AppState;
use agile_agent_core::app::AppStatus;
use agile_agent_core::provider;
use agile_agent_core::provider::ProviderEvent;
use agile_agent_core::session_store;
use agile_agent_core::skills::SkillRegistry;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use std::env;
use std::sync::mpsc;
use std::time::Duration;

use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::render::render_app;
use crate::terminal::AppTerminal;

pub fn run(terminal: &mut AppTerminal, resume_last: bool) -> Result<AppState> {
    let cwd = env::current_dir()?;
    let mut state =
        AppState::with_skills(provider::default_provider(), SkillRegistry::discover(&cwd));
    if resume_last {
        if let Ok(session) = session_store::load_recent_session() {
            session.apply_to_app_state(&mut state);
            state.push_status_message("restored recent session");
        } else {
            state.push_error_message("failed to restore recent session");
        }
    }
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
                    InputOutcome::OpenSkills => state.open_skill_browser(),
                    InputOutcome::CloseSkills => state.close_skill_browser(),
                    InputOutcome::SkillUp => state.move_skill_selection_up(),
                    InputOutcome::SkillDown => state.move_skill_selection_down(),
                    InputOutcome::ToggleSelectedSkill => state.toggle_selected_skill(),
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
                        let session_handle = state.current_session_handle();
                        let augmented_prompt = state.skills.build_injected_prompt(&user_input);
                        if let Err(err) = provider::start_provider(
                            provider_kind,
                            augmented_prompt,
                            session_handle,
                            event_tx,
                        ) {
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
                    ProviderEvent::ThinkingChunk(chunk) => state.append_thinking_chunk(&chunk),
                    ProviderEvent::ToolCallStarted {
                        name,
                        call_id,
                        input_preview,
                    } => state.push_tool_call_started(name, call_id, input_preview),
                    ProviderEvent::ToolCallFinished {
                        name,
                        call_id,
                        output_preview,
                        success,
                    } => state.push_tool_call_finished(name, call_id, output_preview, success),
                    ProviderEvent::SessionHandle(handle) => state.apply_session_handle(handle),
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

    session_store::save_recent_session(&state, &cwd)?;
    Ok(state)
}
