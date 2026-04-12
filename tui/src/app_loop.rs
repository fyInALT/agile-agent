use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::autonomy;
use agent_core::autonomy::CompletionDecision;
use agent_core::backlog_store;
use agent_core::commands::LocalCommand;
use agent_core::commands::parse_local_command;
use agent_core::escalation;
use agent_core::escalation::EscalationRecord;
use agent_core::probe;
use agent_core::provider;
use agent_core::provider::ProviderEvent;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
use agent_core::verification;
use agent_core::verification::VerificationOutcome;
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

        if state.loop_run_active
            && provider_rx.is_none()
            && state.status == AppStatus::Idle
            && state.active_task_id.is_none()
        {
            if state.remaining_loop_iterations == 0 {
                state.set_loop_phase(LoopPhase::Idle);
                state.stop_loop_run("loop guardrail reached: max iterations");
            } else if let Some(prompt) = start_next_loop_iteration(&mut state) {
                state.remaining_loop_iterations = state.remaining_loop_iterations.saturating_sub(1);
                start_provider_request(&mut state, prompt, &mut provider_rx);
            } else {
                state.set_loop_phase(LoopPhase::Idle);
                state.stop_loop_run("no ready todo available");
            }
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
                        state.set_loop_phase(LoopPhase::Executing);
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
                                state.set_loop_phase(LoopPhase::Escalating);
                                escalate_active_task(&mut state, "provider execution failed");
                            } else if let Some(summary_text) = summary.clone() {
                                if let Some(next_prompt) =
                                    autonomy::continuation_prompt(&summary_text)
                                {
                                    if state.continuation_attempts < 3 {
                                        state.continuation_attempts += 1;
                                        state.set_loop_phase(LoopPhase::Executing);
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
                                        state.set_loop_phase(LoopPhase::Escalating);
                                        escalate_active_task(
                                            &mut state,
                                            "continuation limit reached",
                                        );
                                    }
                                } else {
                                    match autonomy::judge_completion(&summary_text) {
                                        CompletionDecision::Complete => {
                                            state.set_loop_phase(LoopPhase::Verifying);
                                            let verification_task = state
                                                .active_task_id
                                                .as_ref()
                                                .and_then(|task_id| {
                                                    state
                                                        .backlog
                                                        .tasks
                                                        .iter()
                                                        .find(|task| &task.id == task_id)
                                                })
                                                .cloned();
                                            if let Some(task) = verification_task {
                                                let plan = verification::build_verification_plan(
                                                    &state.cwd, &task,
                                                );
                                                let result = verification::execute_verification(
                                                    &plan,
                                                    &state.cwd,
                                                    Some(&summary_text),
                                                );
                                                state.push_status_message(result.summary.clone());
                                                for evidence in result.evidence {
                                                    state.push_status_message(format!(
                                                        "evidence: {}",
                                                        evidence
                                                    ));
                                                }
                                                match result.outcome {
                                                    VerificationOutcome::Passed => {
                                                        state.complete_active_task(summary.clone());
                                                        state.set_loop_phase(LoopPhase::Idle);
                                                    }
                                                    VerificationOutcome::Failed
                                                    | VerificationOutcome::NotRunnable => {
                                                        state.set_loop_phase(LoopPhase::Escalating);
                                                        escalate_active_task(
                                                            &mut state,
                                                            "verification failed",
                                                        );
                                                    }
                                                }
                                            } else {
                                                state.complete_active_task(summary.clone());
                                                state.set_loop_phase(LoopPhase::Idle);
                                            }
                                        }
                                        CompletionDecision::Incomplete { reason } => {
                                            state.set_loop_phase(LoopPhase::Escalating);
                                            escalate_active_task(&mut state, reason);
                                        }
                                    }
                                }
                            } else {
                                state.set_loop_phase(LoopPhase::Escalating);
                                escalate_active_task(&mut state, "no assistant summary available");
                            }
                        }
                        state.finish_provider_response();
                        if state.active_task_id.is_none()
                            && state.loop_phase != LoopPhase::Escalating
                        {
                            state.set_loop_phase(LoopPhase::Idle);
                        }
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

fn escalate_active_task(state: &mut AppState, reason: impl Into<String>) {
    let reason = reason.into();
    let task_id = state
        .active_task_id
        .clone()
        .unwrap_or_else(|| "unknown-task".to_string());
    let context_summary = state
        .active_task_summary()
        .unwrap_or_else(|| "no assistant summary".to_string());

    let record = EscalationRecord {
        task_id: task_id.clone(),
        reason: reason.clone(),
        context_summary,
        recommended_actions: vec![
            "inspect task output".to_string(),
            "review verification evidence".to_string(),
        ],
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let artifact_path = escalation::save_escalation(&record).ok();
    state.block_active_task(reason.clone());
    state.push_error_message(format!("escalated task: {} ({})", task_id, reason));
    if let Some(path) = artifact_path {
        state.push_status_message(format!("escalation artifact: {}", path.display()));
    }
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
                "/backlog",
                "/todo-add <title>",
                "/quit",
                "/run-once",
                "/run-loop",
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
        LocalCommand::RunLoop => {
            state.start_loop_run(5);
            state.set_loop_phase(LoopPhase::Planning);
            state.push_status_message("starting autonomous run-loop");
            None
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

fn start_next_loop_iteration(state: &mut AppState) -> Option<String> {
    let Some(todo_id) = state.next_ready_todo_id() else {
        return None;
    };

    state.set_loop_phase(LoopPhase::Planning);
    let Some(task) = state.begin_task_from_todo(&todo_id) else {
        state.push_error_message(format!("failed to start task from todo: {todo_id}"));
        return None;
    };

    state.push_status_message(format!("running task: {}", task.id));
    Some(build_task_prompt(&task))
}
