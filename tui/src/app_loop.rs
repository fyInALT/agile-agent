use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::commands::LocalCommand;
use agent_core::commands::parse_local_command;
use agent_core::probe;
use agent_core::provider;
use agent_core::provider::ProviderEvent;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
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
    let launch_cwd = env::current_dir()?;
    let mut state = AppState::with_skills(
        provider::default_provider(),
        launch_cwd.clone(),
        SkillRegistry::discover(&launch_cwd),
    );
    if resume_last {
        if let Ok(session) = session_store::load_recent_session() {
            let restored_cwd = std::path::PathBuf::from(&session.cwd);
            let effective_cwd = if restored_cwd.is_dir() {
                restored_cwd
            } else {
                launch_cwd.clone()
            };
            state.cwd = effective_cwd.clone();
            state.skills = SkillRegistry::discover(&effective_cwd);
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
                        if let Some(command_result) = parse_local_command(&user_input) {
                            match command_result {
                                Ok(command) => handle_local_command(&mut state, command),
                                Err(error) => state.push_error_message(error),
                            }
                            continue;
                        }

                        let (event_tx, event_rx) = mpsc::channel();
                        let provider_kind = state.selected_provider;
                        let session_handle = state.current_session_handle();
                        let augmented_prompt = state.skills.build_injected_prompt(&user_input);
                        if let Err(err) = provider::start_provider(
                            provider_kind,
                            augmented_prompt,
                            state.cwd.clone(),
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

    session_store::save_recent_session(&state)?;
    Ok(state)
}

fn handle_local_command(state: &mut AppState, command: LocalCommand) {
    match command {
        LocalCommand::Help => {
            for line in [
                "available commands:",
                "/help",
                "/provider",
                "/skills",
                "/doctor",
                "/quit",
            ] {
                state.push_status_message(line);
            }
        }
        LocalCommand::Provider => {
            state.push_status_message(format!(
                "current provider: {} (tab switches providers)",
                state.selected_provider.label()
            ));
        }
        LocalCommand::Skills => {
            state.open_skill_browser();
        }
        LocalCommand::Doctor => {
            let report = probe::probe_report();
            for line in probe::render_doctor_text(&report).lines() {
                if !line.trim().is_empty() {
                    state.push_status_message(line);
                }
            }
        }
        LocalCommand::Quit => state.request_quit(),
    }
}
