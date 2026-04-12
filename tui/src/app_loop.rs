use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::autonomy;
use agent_core::autonomy::CompletionDecision;
use agent_core::backlog_store;
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
    state.backlog = backlog_store::load_backlog()?;
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
                                Ok(command) => {
                                    if let Some(prompt) = handle_local_command(&mut state, command)
                                    {
                                        start_provider_request(
                                            &mut state,
                                            prompt,
                                            &mut provider_rx,
                                        );
                                    }
                                }
                                Err(error) => state.push_error_message(error),
                            }
                            continue;
                        }

                        let augmented_prompt = state.skills.build_injected_prompt(&user_input);
                        start_provider_request(&mut state, augmented_prompt, &mut provider_rx);
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
                    ProviderEvent::Error(error) => {
                        state.mark_active_task_error();
                        state.push_error_message(error);
                    }
                    ProviderEvent::Finished => {
                        let summary = state.active_task_summary();
                        if state.active_task_id.is_some() {
                            if state.active_task_had_error {
                                state.block_active_task("provider execution failed");
                            } else if let Some(summary_text) = summary.clone() {
                                if let Some(next_prompt) =
                                    autonomy::continuation_prompt(&summary_text)
                                {
                                    if state.continuation_attempts < 3 {
                                        state.continuation_attempts += 1;
                                        state.push_status_message(format!(
                                            "continuing active task automatically (attempt {})",
                                            state.continuation_attempts
                                        ));
                                        start_provider_request(
                                            &mut state,
                                            next_prompt,
                                            &mut provider_rx,
                                        );
                                        should_clear_provider_rx = false;
                                        break;
                                    } else {
                                        state.block_active_task("continuation limit reached");
                                    }
                                } else {
                                    match autonomy::judge_completion(&summary_text) {
                                        CompletionDecision::Complete => {
                                            state.complete_active_task(summary.clone());
                                        }
                                        CompletionDecision::Incomplete { reason } => {
                                            state.block_active_task(reason);
                                        }
                                    }
                                }
                            } else {
                                state.block_active_task("no assistant summary available");
                            }
                        }
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

    backlog_store::save_backlog(&state.backlog)?;
    session_store::save_recent_session(&state)?;
    Ok(state)
}

fn handle_local_command(state: &mut AppState, command: LocalCommand) -> Option<String> {
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
            None
        }
        LocalCommand::Provider => {
            state.push_status_message(format!(
                "current provider: {} (tab switches providers)",
                state.selected_provider.label()
            ));
            None
        }
        LocalCommand::Skills => {
            state.open_skill_browser();
            None
        }
        LocalCommand::Doctor => {
            let report = probe::probe_report();
            for line in probe::render_doctor_text(&report).lines() {
                if !line.trim().is_empty() {
                    state.push_status_message(line);
                }
            }
            None
        }
        LocalCommand::Backlog => {
            for line in state.render_backlog_lines() {
                state.push_status_message(line);
            }
            None
        }
        LocalCommand::TodoAdd(title) => {
            let todo_id = state.add_todo(title.clone());
            state.push_status_message(format!("added todo: {} ({})", todo_id, title));
            None
        }
        LocalCommand::RunOnce => {
            let Some(todo_id) = state.next_ready_todo_id() else {
                state.push_status_message("no ready todo available");
                return None;
            };

            let Some(task) = state.begin_task_from_todo(&todo_id) else {
                state.push_error_message(format!("failed to start task from todo: {todo_id}"));
                return None;
            };

            state.push_status_message(format!("running task: {}", task.id));
            Some(build_task_prompt(&task))
        }
        LocalCommand::Quit => {
            state.request_quit();
            None
        }
    }
}

fn start_provider_request(
    state: &mut AppState,
    prompt: String,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) {
    let (event_tx, event_rx) = mpsc::channel();
    let provider_kind = state.selected_provider;
    let session_handle = state.current_session_handle();
    if let Err(err) = provider::start_provider(
        provider_kind,
        prompt,
        state.cwd.clone(),
        session_handle,
        event_tx,
    ) {
        state.push_error_message(format!("failed to start provider: {err}"));
    } else {
        state.begin_provider_response();
        *provider_rx = Some(event_rx);
    }
}

fn build_task_prompt(task: &agent_core::backlog::TaskItem) -> String {
    let mut prompt = format!(
        "Execute the following task.\n\nObjective: {}\nScope: {}\n",
        task.objective, task.scope
    );

    if !task.constraints.is_empty() {
        prompt.push_str("Constraints:\n");
        for constraint in &task.constraints {
            prompt.push_str(&format!("- {}\n", constraint));
        }
    }

    if !task.verification_plan.is_empty() {
        prompt.push_str("\nVerification plan:\n");
        for item in &task.verification_plan {
            prompt.push_str(&format!("- {}\n", item));
        }
    }

    prompt
}
