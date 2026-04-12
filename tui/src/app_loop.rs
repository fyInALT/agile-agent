use agent_core::agent_runtime::AgentBootstrapKind;
use agent_core::agent_runtime::AgentRuntime;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::backlog_store;
use agent_core::commands::LocalCommand;
use agent_core::commands::parse_local_command;
use agent_core::probe;
use agent_core::provider;
use agent_core::provider::ProviderEvent;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
use agent_core::task_engine;
use agent_core::task_engine::ExecutionGuardrails;
use agent_core::task_engine::TurnResolution;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use std::env;
use std::sync::mpsc;
use std::time::Duration;

use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::input::handle_paste_event;
use crate::render::render_app;
use crate::terminal::AppTerminal;
use crate::transcript::overlay::OverlayCommand;
use crate::ui_state::TuiState;

pub fn run(terminal: &mut AppTerminal, resume_last: bool) -> Result<AppState> {
    let launch_cwd = env::current_dir()?;
    let bootstrap = AgentRuntime::bootstrap_for_cwd(&launch_cwd, provider::default_provider())?;
    let mut app = AppState::with_skills(
        provider::default_provider(),
        launch_cwd.clone(),
        SkillRegistry::discover(&launch_cwd),
    );
    app.backlog = backlog_store::load_backlog_for_workplace(bootstrap.runtime.workplace())?;
    for warning in bootstrap.runtime.apply_to_app_state(&mut app) {
        app.push_error_message(warning);
    }
    match &bootstrap.kind {
        AgentBootstrapKind::Created => {
            app.push_status_message(format!("created agent: {}", bootstrap.runtime.summary()));
        }
        AgentBootstrapKind::Restored => {
            app.push_status_message(format!("restored agent: {}", bootstrap.runtime.summary()));
        }
        AgentBootstrapKind::RecreatedAfterError { error } => {
            app.push_error_message(format!("failed to restore agent runtime: {error}"));
            app.push_status_message(format!(
                "created replacement agent: {}",
                bootstrap.runtime.summary()
            ));
        }
    }
    if resume_last {
        match bootstrap.runtime.restore_state(&mut app) {
            Ok(restored) => {
                app.push_status_message("restored recent agent state");
                for warning in restored.warnings {
                    app.push_error_message(warning);
                }
                for warning in bootstrap.runtime.apply_to_app_state(&mut app) {
                    app.push_error_message(warning);
                }
            }
            Err(err) => match session_store::restore_recent_session_for_workplace(
                &mut app,
                &launch_cwd,
                bootstrap.runtime.workplace(),
            ) {
                Ok(restored) => {
                    app.push_status_message("restored recent session");
                    for warning in restored.warnings {
                        app.push_error_message(warning);
                    }
                    for warning in bootstrap.runtime.apply_to_app_state(&mut app) {
                        app.push_error_message(warning);
                    }
                }
                Err(_) => {
                    app.push_error_message(format!("failed to restore recent agent state: {err}"))
                }
            },
        }
    }

    let mut state = TuiState::from_app(app, bootstrap.runtime);
    if state.sync_agent_runtime_from_app() {
        state.agent_runtime.persist()?;
    }
    let mut provider_rx: Option<mpsc::Receiver<ProviderEvent>> = None;

    loop {
        terminal.draw(|frame| render_app(frame, &mut state))?;

        if state.app.should_quit {
            break;
        }

        if state.app.loop_run_active && provider_rx.is_none() && state.app.status == AppStatus::Idle
        {
            if state.app.remaining_loop_iterations == 0 {
                state.app.set_loop_phase(LoopPhase::Idle);
                state
                    .app
                    .stop_loop_run("loop guardrail reached: max iterations");
            } else if let Some((prompt, started_new_task)) = next_loop_prompt(&mut state) {
                if started_new_task {
                    state.app.remaining_loop_iterations =
                        state.app.remaining_loop_iterations.saturating_sub(1);
                }
                start_provider_request(&mut state, prompt, &mut provider_rx);
            } else {
                state.app.set_loop_phase(LoopPhase::Idle);
                state.app.stop_loop_run("no ready todo available");
            }
        }

        let poll_timeout = if provider_rx.is_some() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(250)
        };

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    if state.is_overlay_open() {
                        if key_event.code == KeyCode::Char('t')
                            && key_event.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            state.close_transcript_overlay();
                            continue;
                        }
                        let page_height = terminal.size()?.height.saturating_sub(2) as usize;
                        if let Some(overlay) = state.transcript_overlay.as_mut() {
                            if let Some(command) = overlay.handle_key_event(key_event, page_height)
                            {
                                if matches!(command, OverlayCommand::Close) {
                                    state.close_transcript_overlay();
                                }
                            }
                        }
                        continue;
                    }

                    match handle_key_event(&mut state, key_event) {
                        InputOutcome::None => {}
                        InputOutcome::ScrollTranscriptUp(rows) => state.scroll_transcript_up(rows),
                        InputOutcome::ScrollTranscriptDown(rows) => {
                            state.scroll_transcript_down(rows)
                        }
                        InputOutcome::ScrollTranscriptHome => state.scroll_transcript_home(),
                        InputOutcome::ScrollTranscriptEnd => state.scroll_transcript_end(),
                        InputOutcome::OpenSkills => state.app.open_skill_browser(),
                        InputOutcome::CloseSkills => state.app.close_skill_browser(),
                        InputOutcome::SkillUp => state.app.move_skill_selection_up(),
                        InputOutcome::SkillDown => state.app.move_skill_selection_down(),
                        InputOutcome::ToggleSelectedSkill => state.app.toggle_selected_skill(),
                        InputOutcome::ToggleProvider => {
                            if state.app.status == AppStatus::Idle {
                                let next_provider = state.app.selected_provider.next();
                                let summary = state.switch_to_new_agent(next_provider)?;
                                state.app.push_status_message(format!(
                                    "switched to agent {summary} on {}",
                                    state.app.selected_provider.label()
                                ));
                            }
                        }
                        InputOutcome::OpenTranscript => state.open_transcript_overlay(),
                        InputOutcome::Quit => state.app.request_quit(),
                        InputOutcome::Submit(user_input) => {
                            if let Some(command_result) = parse_local_command(&user_input) {
                                match command_result {
                                    Ok(command) => {
                                        if let Some(prompt) =
                                            handle_local_command(&mut state, command)
                                        {
                                            start_provider_request(
                                                &mut state,
                                                prompt,
                                                &mut provider_rx,
                                            );
                                        }
                                    }
                                    Err(error) => state.app.push_error_message(error),
                                }
                                continue;
                            }

                            let augmented_prompt =
                                state.app.skills.build_injected_prompt(&user_input);
                            state.app.set_loop_phase(LoopPhase::Executing);
                            start_provider_request(&mut state, augmented_prompt, &mut provider_rx);
                        }
                    }
                }
                Event::Paste(text) => handle_paste_event(&mut state, &text),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let mut should_clear_provider_rx = false;
        if let Some(rx) = provider_rx.as_ref() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ProviderEvent::Status(text) => state.app.push_status_message(text),
                    ProviderEvent::AssistantChunk(chunk) => {
                        state.app.append_assistant_chunk(&chunk)
                    }
                    ProviderEvent::ThinkingChunk(chunk) => state.app.append_thinking_chunk(&chunk),
                    ProviderEvent::ToolCallStarted {
                        name,
                        call_id,
                        input_preview,
                    } => state
                        .app
                        .push_tool_call_started(name, call_id, input_preview),
                    ProviderEvent::ToolCallFinished {
                        name,
                        call_id,
                        output_preview,
                        success,
                    } => state
                        .app
                        .push_tool_call_finished(name, call_id, output_preview, success),
                    ProviderEvent::SessionHandle(handle) => {
                        state.app.apply_session_handle(handle);
                        persist_agent_runtime_if_changed(&mut state)?;
                    }
                    ProviderEvent::Error(error) => {
                        state.app.mark_active_task_error();
                        state.app.push_error_message(error);
                        persist_agent_runtime_if_changed(&mut state)?;
                    }
                    ProviderEvent::Finished => {
                        state.app.finish_provider_response();
                        if state.app.active_task_id.is_some() {
                            match task_engine::resolve_active_task_after_turn(
                                &mut state.app,
                                ExecutionGuardrails::default(),
                            )? {
                                TurnResolution::Continue { prompt } => {
                                    start_provider_request(&mut state, prompt, &mut provider_rx);
                                    should_clear_provider_rx = false;
                                    break;
                                }
                                TurnResolution::Completed
                                | TurnResolution::Failed { .. }
                                | TurnResolution::Escalated
                                | TurnResolution::Idle => {}
                            }
                        }
                        if state.app.active_task_id.is_none()
                            && state.app.loop_phase != LoopPhase::Escalating
                        {
                            state.app.set_loop_phase(LoopPhase::Idle);
                        }
                        persist_agent_runtime_if_changed(&mut state)?;
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

    state.sync_app_input_from_composer();
    state.agent_runtime.sync_from_app_state(&state.app);
    state.agent_runtime.mark_stopped();
    state.agent_runtime.persist()?;
    state.agent_runtime.persist_state(&state.app)?;
    backlog_store::save_backlog_for_workplace(&state.app.backlog, state.agent_runtime.workplace())?;
    session_store::save_recent_session_for_workplace(&state.app, state.agent_runtime.workplace())?;
    Ok(state.into_app_state())
}

fn handle_local_command(state: &mut TuiState, command: LocalCommand) -> Option<String> {
    match command {
        LocalCommand::Help => {
            for line in [
                "available commands:",
                "/help",
                "/provider",
                "/skills",
                "/doctor",
                "/backlog",
                "/todo-add <title>",
                "/quit",
                "/run-once",
                "/run-loop",
            ] {
                state.app.push_status_message(line);
            }
            None
        }
        LocalCommand::Provider => {
            state.app.push_status_message(format!(
                "current agent: {} · provider: {} (tab creates a new agent on the next provider)",
                state.agent_runtime.summary(),
                state.app.selected_provider.label(),
            ));
            None
        }
        LocalCommand::Skills => {
            state.app.open_skill_browser();
            None
        }
        LocalCommand::Doctor => {
            let report = probe::probe_report();
            for line in probe::render_doctor_text(&report).lines() {
                if !line.trim().is_empty() {
                    state.app.push_status_message(line);
                }
            }
            None
        }
        LocalCommand::Backlog => {
            for line in state.app.render_backlog_lines() {
                state.app.push_status_message(line);
            }
            None
        }
        LocalCommand::TodoAdd(title) => {
            let todo_id = state.app.add_todo(title.clone());
            state
                .app
                .push_status_message(format!("added todo: {} ({})", todo_id, title));
            None
        }
        LocalCommand::RunOnce => {
            let Some(todo_id) = state.app.next_ready_todo_id() else {
                state.app.push_status_message("no ready todo available");
                return None;
            };

            let Some(task) = state.app.begin_task_from_todo(&todo_id) else {
                state
                    .app
                    .push_error_message(format!("failed to start task from todo: {todo_id}"));
                return None;
            };

            state
                .app
                .push_status_message(format!("running task: {}", task.id));
            Some(task_engine::build_task_prompt(&task))
        }
        LocalCommand::RunLoop => {
            state.app.start_loop_run(5);
            state.app.set_loop_phase(LoopPhase::Planning);
            state
                .app
                .push_status_message("starting autonomous run-loop");
            None
        }
        LocalCommand::Quit => {
            state.app.request_quit();
            None
        }
    }
}

fn start_provider_request(
    state: &mut TuiState,
    prompt: String,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) {
    let (event_tx, event_rx) = mpsc::channel();
    let provider_kind = state.app.selected_provider;
    let session_handle = state.app.current_session_handle();
    state.app.mark_active_task_running();
    if let Err(err) = provider::start_provider(
        provider_kind,
        prompt,
        state.app.cwd.clone(),
        session_handle,
        event_tx,
    ) {
        task_engine::handle_provider_start_failure(&mut state.app, err.to_string());
    } else {
        state.app.begin_provider_response();
        let _ = state.sync_agent_runtime_from_app();
        let _ = state.agent_runtime.persist();
        *provider_rx = Some(event_rx);
    }
}

fn next_loop_prompt(state: &mut TuiState) -> Option<(String, bool)> {
    if let Some(active_task_id) = state.app.active_task_id.clone() {
        let task = state
            .app
            .backlog
            .tasks
            .iter()
            .find(|task| task.id == active_task_id)
            .cloned()?;
        state.app.set_loop_phase(LoopPhase::Executing);
        state
            .app
            .push_status_message(format!("resuming task: {}", task.id));
        return Some((task_engine::build_task_prompt(&task), false));
    }

    let Some(todo_id) = state.app.next_ready_todo_id() else {
        return None;
    };

    state.app.set_loop_phase(LoopPhase::Planning);
    let Some(task) = state.app.begin_task_from_todo(&todo_id) else {
        state
            .app
            .push_error_message(format!("failed to start task from todo: {todo_id}"));
        return None;
    };

    state
        .app
        .push_status_message(format!("running task: {}", task.id));
    Some((task_engine::build_task_prompt(&task), true))
}

fn persist_agent_runtime_if_changed(state: &mut TuiState) -> Result<()> {
    if state.sync_agent_runtime_from_app() {
        state.agent_runtime.persist()?;
        state.agent_runtime.persist_state(&state.app)?;
    }
    Ok(())
}
