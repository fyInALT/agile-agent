use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::app::TranscriptEntry;
use agent_core::command_bus::model::{
    CommandInvocation, CommandNamespace, CommandTargetSpec, ParsedSlashCommand,
};
use agent_core::command_bus::parse::parse_slash_command;
use agent_core::commands::LocalCommand;
use agent_core::commands::parse_legacy_alias;
use agent_core::logging;
use agent_core::probe;
use agent_core::ProviderEvent;
use agent_core::runtime_command::RuntimeCommand;
use agent_core::runtime_session::RuntimeSession;
use agent_core::task_engine;
use agent_core::task_engine::ExecutionGuardrails;
use agent_core::task_engine::TurnResolution;
use anyhow::Result;
use chrono::Timelike;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use std::env;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;
use std::time::Instant;

use crate::confirmation_overlay::ConfirmationCommand;
use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::input::handle_paste_event;
use crate::launch_config_overlay::LaunchConfigOverlayCommand;
use crate::provider_overlay::ProviderSelectionCommand;
use crate::profile_selection_overlay::ProfileSelectionCommand;
use crate::render::render_app;
use crate::resume_overlay::{ResumeCommand, ResumeOverlay};
use crate::terminal::AppTerminal;
use crate::transcript::overlay::OverlayCommand;
use crate::tui_snapshot::clear_resume_snapshot;
use crate::tui_snapshot::load_resume_snapshot;
use crate::tui_snapshot::save_resume_snapshot;
use crate::ui_state::AtCommandResult;
use crate::ui_state::TuiState;
use crate::ui_state::parse_at_command;

/// Interval for periodic persistence flush
const PERSISTENCE_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Interval for decision agent polling
const DECISION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Idle timeout for detecting agents waiting for user input
/// An agent is transitioned to WaitingForInput if no events received for this duration
/// This should match the desired decision trigger timeout (60s) to avoid premature intervention
const RESPONDING_IDLE_TIMEOUT_SECS: u64 = 60;

/// Idle timeout for triggering decision layer intervention
/// If an agent is idle for this duration, decision layer will check if there are pending tasks
const IDLE_DECISION_TRIGGER_SECS: u64 = 60;

/// Outcome of a decision execution — either pure path (RuntimeCommands)
/// or legacy path (DecisionExecutionResults).
enum DecisionOutcome {
    /// Pure effect path: interpreter successfully mapped all DecisionCommands
    Pure(Vec<RuntimeCommand>),
    /// Legacy path: one or more commands could not be interpreted
    Legacy(Vec<agent_core::agent_pool::DecisionExecutionResult>),
}

/// Decision output info for transcript display
///
/// Captures decision details for showing in TUI with special formatting.
struct DecisionOutputInfo {
    situation_type: String,
    action_type: String,
    reasoning: String,
    confidence: f64,
    tier: String,
}

pub fn run(terminal: &mut AppTerminal, resume_last: bool) -> Result<AppState> {
    let launch_cwd = env::current_dir()?;

    // Check for shutdown snapshot and show resume dialog if exists
    let effective_resume_last = if resume_last {
        check_resume_snapshot(terminal, &launch_cwd)?
    } else {
        false
    };

    let workplace = agent_core::workplace_store::WorkplaceStore::for_cwd(&launch_cwd)?;
    let tui_resume_snapshot = if effective_resume_last {
        load_resume_snapshot(&workplace)?
    } else {
        None
    };

    let session = RuntimeSession::bootstrap(
        launch_cwd.clone(),
        agent_core::default_provider(),
        effective_resume_last && tui_resume_snapshot.is_none(),
    )?;
    let mut state = TuiState::from_session(session);

    if let Some(snapshot) = tui_resume_snapshot {
        state.restore_from_resume_snapshot(snapshot)?;
        workplace.clear_shutdown_snapshot()?;
        clear_resume_snapshot(&workplace)?;
    } else {
        // Ensure OVERVIEW agent exists on startup (always at index 0)
        state.ensure_overview_agent();
    }

    // Load provider profiles (global + workplace merged) after agent_pool is created
    {
        use agent_core::global_config::GlobalConfigStore;
        use agent_core::provider_profile::ProfilePersistence;

        if let Ok(config_store) = GlobalConfigStore::new()
            && let Ok(profile_store) = config_store.load_profile_store() {
                // Merge with workplace profiles if available
                let persistence = ProfilePersistence::for_paths(
                    config_store.path().clone(),
                    Some(workplace.path().to_path_buf()),
                );
                let merged_store = persistence.load_merged().unwrap_or(profile_store);

                if let Some(pool) = state.agent_pool.as_mut() {
                    pool.set_profile_store(merged_store);
                    logging::debug_event(
                        "app_loop.load_profiles",
                        "loaded provider profiles into agent pool",
                        serde_json::json!({
                            "profile_count": pool.profile_store().map(|s| s.profile_count()).unwrap_or(0),
                        }),
                    );
                }
            }
    }

    let mut provider_rx: Option<mpsc::Receiver<ProviderEvent>> = None;
    let mut last_flush = Instant::now();
    let mut last_decision_poll = Instant::now();

    loop {
        terminal.draw(|frame| render_app(frame, &mut state))?;

        if state.workplace().loop_control.should_quit {
            break;
        }

        // Periodic persistence flush
        if last_flush.elapsed() >= PERSISTENCE_FLUSH_INTERVAL {
            state.persist_if_changed()?;
            last_flush = Instant::now();
        }

        // Decision agent polling - process decision requests and responses
        if last_decision_poll.elapsed() >= DECISION_POLL_INTERVAL {
            // First, check for pending decisions and show animated status
            if let Some(pool) = state.agent_pool.as_ref() {
                let pending_decisions = pool.agents_with_pending_decisions();
                if !pending_decisions.is_empty() {
                    // Show spinner animation for each pending decision
                    for (agent_id, started_at) in pending_decisions {
                        let elapsed_ms = started_at.elapsed().as_millis();
                        let spinner = match (elapsed_ms / 400) % 4 {
                            0 => "⠋",
                            1 => "⠙",
                            2 => "⠹",
                            3 => "⠸",
                            _ => "⠋",
                        };
                        let elapsed_secs = started_at.elapsed().as_secs();
                        state.app_mut().push_status_message(format!(
                            "🧠 {}: {} Analyzing... ({:.0}s)",
                            agent_id.as_str(),
                            spinner,
                            elapsed_secs as f32
                        ));
                    }
                }
            }

            // Collect responses first, preserving output details for transcript
            let decision_results: Vec<(
                agent_core::agent_runtime::AgentId,
                DecisionOutcome,
                Option<DecisionOutputInfo>,
            )> = {
                if let Some(pool) = state.agent_pool.as_mut() {
                    let responses = pool.poll_decision_agents();
                    responses
                        .into_iter()
                        .filter_map(|(agent_id, response)| {
                            if response.is_success() && response.output().is_some() {
                                let output = response.output().unwrap();
                                let action_name = output
                                    .actions
                                    .first()
                                    .map(|a| a.action_type().name.clone())
                                    .unwrap_or_else(|| "none".to_string());

                                // Extract decision output info for transcript display
                                let output_info = DecisionOutputInfo {
                                    situation_type: "auto_detected".to_string(), // TODO: get from request
                                    action_type: action_name.clone(),
                                    reasoning: output.reasoning.clone(),
                                    confidence: output.confidence,
                                    tier: "auto".to_string(), // TODO: get from engine
                                };

                                logging::debug_event(
                                    "app_loop.decision_response",
                                    "received decision response",
                                    serde_json::json!({
                                        "agent_id": agent_id.as_str(),
                                        "action": action_name,
                                        "reasoning": output.reasoning,
                                        "confidence": output.confidence,
                                    }),
                                );

                                // NEW: Try pure path first (translate + interpreter)
                                use agent_core::pool::DecisionExecutor;
                                use agent_core::pool::DecisionCommandInterpreter;
                                let interpreter = DecisionCommandInterpreter::new();
                                let commands = DecisionExecutor::translate(&agent_id, output);
                                let mut all_interpreted = true;
                                let mut runtime_cmds = Vec::new();
                                for cmd in &commands {
                                    match interpreter.interpret(&agent_id, cmd) {
                                        Some(mut cmds) => runtime_cmds.append(&mut cmds),
                                        None => {
                                            all_interpreted = false;
                                            break;
                                        }
                                    }
                                }

                                if all_interpreted && !commands.is_empty() {
                                    tracing::info!(
                                        agent_id = %agent_id.as_str(),
                                        commands = commands.len(),
                                        "TUI decision pure path"
                                    );
                                    Some((agent_id, DecisionOutcome::Pure(runtime_cmds), Some(output_info)))
                                } else {
                                    // Legacy fallback
                                    tracing::info!(
                                        agent_id = %agent_id.as_str(),
                                        "TUI decision legacy fallback"
                                    );
                                    let result = pool.execute_decision_action(&agent_id, output);
                                    Some((agent_id, DecisionOutcome::Legacy(result), Some(output_info)))
                                }
                            } else if response.is_error() {
                                logging::warn_event(
                                    "app_loop.decision_error",
                                    "decision agent error",
                                    serde_json::json!({
                                        "agent_id": agent_id.as_str(),
                                        "error": response.error_message().unwrap_or("unknown"),
                                    }),
                                );
                                None
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            };

            // Process results after releasing pool borrow
            for (agent_id, outcome, output_info) in decision_results {
                match outcome {
                    DecisionOutcome::Pure(cmds) => {
                        // Pure path: dispatch RuntimeCommands via effect handler
                        if let Err(e) = crate::effect_handler::dispatch_runtime_commands(&cmds, &mut state) {
                            tracing::warn!(
                                agent_id = %agent_id.as_str(),
                                error = ?e,
                                "TUI pure path: effect dispatch failed"
                            );
                        }
                    }
                    DecisionOutcome::Legacy(results) => {
                        for result in results {
                            match result {
                                agent_core::agent_pool::DecisionExecutionResult::Executed { option_id } => {
                            // Push decision entry to transcript for detailed display
                            if let Some(ref info) = output_info {
                                state.app_mut().push_decision(
                                    agent_id.as_str().to_string(),
                                    info.situation_type.clone(),
                                    format!("{} → {}", info.action_type, option_id),
                                    info.reasoning.clone(),
                                    info.confidence,
                                    info.tier.clone(),
                                );
                                // Set decision status in status bar (max 15 chars)
                                state.set_decision_status(Some(option_id.clone()));
                            }
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: decision executed ({})",
                                agent_id.as_str(),
                                option_id
                            ));
                        }
                        agent_core::agent_pool::DecisionExecutionResult::AcceptedRecommendation => {
                            if let Some(ref info) = output_info {
                                state.app_mut().push_decision(
                                    agent_id.as_str().to_string(),
                                    info.situation_type.clone(),
                                    info.action_type.clone(),
                                    info.reasoning.clone(),
                                    info.confidence,
                                    info.tier.clone(),
                                );
                                // Set decision status in status bar (max 15 chars)
                                state.set_decision_status(Some(info.action_type.clone()));
                            }
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: recommendation accepted",
                                agent_id.as_str()
                            ));
                        }
                        agent_core::agent_pool::DecisionExecutionResult::CustomInstruction {
                            instruction,
                        } => {
                            if let Some(ref info) = output_info {
                                state.app_mut().push_decision(
                                    agent_id.as_str().to_string(),
                                    info.situation_type.clone(),
                                    "custom_instruction".to_string(),
                                    info.reasoning.clone(),
                                    info.confidence,
                                    info.tier.clone(),
                                );
                            }
                            // Set decision status in status bar (max 15 chars)
                            state.set_decision_status(Some("custom".to_string()));
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: custom instruction sent",
                                agent_id.as_str()
                            ));

                            // Trigger new provider request with the instruction
                            // Use start_raw_provider_for_agent to avoid duplicate transcript entry
                            // since instruction was already added in execute_decision_action
                            logging::debug_event(
                                "decision_layer.custom_instruction_trigger",
                                "triggering provider request for custom instruction",
                                serde_json::json!({
                                    "agent_id": agent_id.as_str(),
                                }),
                            );

                            // Check if agent slot is in a valid state for new provider request
                            // Valid states include:
                            // - "responding": Already ready (rare case)
                            // - "starting": Just transitioned from idle/blocked/waiting
                            // - "idle": Agent ready for new work (will be transitioned in start_provider_for_agent_with_mode)
                            // - "blocked": Decision triggered work (will be transitioned)
                            // - "waiting_for_input": Idle timeout recovery (will be transitioned)
                            //
                            // Invalid states: "stopping", "stopped", "tool_executing", "finishing"
                            let slot_status = state
                                .agent_pool
                                .as_ref()
                                .and_then(|pool| pool.get_slot_by_id(&agent_id))
                                .map(|slot| slot.status().label());

                            if let Some(status) = slot_status {
                                let is_valid_for_request = status == "responding"
                                    || status == "starting"
                                    || status == "idle"
                                    || status.starts_with("blocked:")
                                    || status == "waiting_for_input";

                                if is_valid_for_request {
                                    // Agent is ready to process the instruction
                                    // Start provider request without injecting mail (instruction is the prompt)
                                    let _started = start_multi_agent_provider_request_for_agent(
                                        &mut state,
                                        agent_id.clone(),
                                        instruction.clone(),
                                        false, // Don't inject mail, instruction is the prompt
                                    );
                                } else {
                                    logging::warn_event(
                                        "decision_layer.custom_instruction_skip",
                                        "agent not in valid state for provider request",
                                        serde_json::json!({
                                            "agent_id": agent_id.as_str(),
                                            "status": status,
                                            "valid_states": ["responding", "starting", "idle", "blocked:*", "waiting_for_input"],
                                        }),
                                    );
                                }
                            }
                        }
                        agent_core::agent_pool::DecisionExecutionResult::Skipped => {
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: decision skipped",
                                agent_id.as_str()
                            ));
                        }
                        agent_core::agent_pool::DecisionExecutionResult::Cancelled => {
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: decision cancelled",
                                agent_id.as_str()
                            ));
                        }
                        agent_core::agent_pool::DecisionExecutionResult::AgentNotFound
                        | agent_core::agent_pool::DecisionExecutionResult::NotBlocked => {}
                        agent_core::agent_pool::DecisionExecutionResult::TaskPrepared {
                            branch,
                            worktree_path: _,
                        } => {
                            state.set_decision_status(Some("prepared".to_string()));
                            state.app_mut().push_status_message(format!(
                                "🧠 {}: task prepared (branch: {})",
                                agent_id.as_str(),
                                branch
                            ));
                        }
                        agent_core::agent_pool::DecisionExecutionResult::PreparationFailed { reason } => {
                            state.set_decision_status(Some("prep-fail".to_string()));
                            state.app_mut().push_status_message(format!(
                                "⚠️ {}: preparation failed ({})",
                                agent_id.as_str(),
                                reason
                            ));
                        }
                    }
                }
            }
        }
    }
            last_decision_poll = Instant::now();
        }

        if state.workplace().loop_control.loop_run_active
            && provider_rx.is_none()
            && state.app().status == AppStatus::Idle
        {
            if state.workplace().loop_control.remaining_iterations() == 0 {
                state.app_mut().set_loop_phase(LoopPhase::Idle);
                state
                    .app_mut()
                    .push_status_message("loop guardrail reached: max iterations");
                state.workplace_mut().loop_control.stop_loop();
            } else if let Some((prompt, started_new_task)) = next_loop_prompt(&mut state) {
                if started_new_task {
                    state.workplace_mut().loop_control.increment_iteration();
                }
                start_provider_request(&mut state, prompt, &mut provider_rx);
            } else {
                state.app_mut().set_loop_phase(LoopPhase::Idle);
                state
                    .app_mut()
                    .push_status_message("no ready todo available");
                state.workplace_mut().loop_control.stop_loop();
            }
        }

        let poll_timeout = event_poll_timeout(&state, provider_rx.is_some());

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    // Handle profile selection overlay (primary agent creation)
                    if state.is_profile_selection_overlay_open() {
                        if let Some(overlay) = state.profile_selection_overlay.as_mut()
                            && let Some(command) = overlay.handle_key_event(key_event)
                        {
                            match command {
                                ProfileSelectionCommand::Close => {
                                    state.close_profile_selection_overlay();
                                }
                                ProfileSelectionCommand::Select {
                                    work_profile_id,
                                    decision_profile_id,
                                } => {
                                    state.close_profile_selection_overlay();
                                    if let Some(agent_id) =
                                        state.spawn_agent_with_profiles(&work_profile_id, &decision_profile_id)
                                    {
                                        state.app_mut().push_status_message(format!(
                                            "spawned {} with work={} decision={}",
                                            agent_id.as_str(),
                                            work_profile_id,
                                            decision_profile_id
                                        ));
                                    } else {
                                        state.app_mut().push_error_message(
                                            "failed to spawn agent with profile (pool full or profile invalid)",
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Handle provider selection overlay (legacy/fallback)
                    if state.is_provider_overlay_open() {
                        if let Some(overlay) = state.provider_overlay.as_mut()
                            && let Some(command) = overlay.handle_key_event(key_event)
                        {
                            match command {
                                ProviderSelectionCommand::Close => {
                                    state.close_provider_overlay();
                                }
                                ProviderSelectionCommand::Select(provider) => {
                                    state.close_provider_overlay();
                                    // Mock provider skips config overlay (Story 3.9)
                                    if provider == agent_core::ProviderKind::Mock {
                                        if let Some(agent_id) = state.spawn_agent(provider) {
                                            state.app_mut().push_status_message(format!(
                                                "spawned {} with {}",
                                                agent_id.as_str(),
                                                provider.label()
                                            ));
                                        } else {
                                            state.app_mut().push_error_message(
                                                "failed to spawn agent (pool full)",
                                            );
                                        }
                                    } else {
                                        // Claude/Codex: open launch config overlay
                                        state.open_launch_config_overlay(provider);
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Handle launch config overlay
                    if state.is_launch_config_overlay_open() {
                        if let Some(overlay) = state.launch_config_overlay.as_mut()
                            && let Some(command) = overlay.handle_key_event(key_event)
                        {
                            match command {
                                LaunchConfigOverlayCommand::Close => {
                                    state.close_launch_config_overlay();
                                }
                                LaunchConfigOverlayCommand::Confirm {
                                    work_config,
                                    decision_config,
                                } => {
                                    let provider = overlay.provider;
                                    state.close_launch_config_overlay();
                                    if let Some(agent_id) = state.spawn_agent_with_launch_config(
                                        provider,
                                        &work_config,
                                        &decision_config,
                                    ) {
                                        state.app_mut().push_status_message(format!(
                                            "spawned {} with {}",
                                            agent_id.as_str(),
                                            provider.label()
                                        ));
                                    } else {
                                        state.app_mut().push_error_message(
                                            "failed to spawn agent (pool full)",
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Handle confirmation overlay
                    if state.is_confirmation_overlay_open() {
                        if let Some(overlay) = state.confirmation_overlay.as_mut()
                            && let Some(command) = overlay.handle_key_event(key_event)
                        {
                            match command {
                                ConfirmationCommand::Cancel => {
                                    state.close_confirmation_overlay();
                                }
                                ConfirmationCommand::Confirm => {
                                    state.close_confirmation_overlay();
                                    if let Some(agent_id) = state.stop_focused_agent() {
                                        state.app_mut().push_status_message(format!(
                                            "stopped agent {}",
                                            agent_id
                                        ));
                                    } else {
                                        state.app_mut().push_status_message("no agent to stop");
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Handle transcript overlay
                    if state.is_overlay_open() {
                        if key_event.code == KeyCode::Char('t')
                            && key_event.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            state.close_transcript_overlay();
                            continue;
                        }
                        let page_height = terminal.size()?.height.saturating_sub(2) as usize;
                        if let Some(overlay) = state.transcript_overlay.as_mut()
                            && let Some(command) = overlay.handle_key_event(key_event, page_height)
                            && matches!(command, OverlayCommand::Close)
                        {
                            state.close_transcript_overlay();
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
                        InputOutcome::OpenSkills => state.app_mut().open_skill_browser(),
                        InputOutcome::CloseSkills => state.app_mut().close_skill_browser(),
                        InputOutcome::SkillUp => state.app_mut().move_skill_selection_up(),
                        InputOutcome::SkillDown => state.app_mut().move_skill_selection_down(),
                        InputOutcome::ToggleSelectedSkill => {
                            state.app_mut().toggle_selected_skill()
                        }
                        InputOutcome::ToggleProvider => {
                            if state.app().status == AppStatus::Idle {
                                let next_provider = state.app().selected_provider.next();
                                let summary = state.switch_to_new_agent(next_provider)?;
                                let provider_label =
                                    state.app().selected_provider.label().to_string();
                                state.app_mut().push_status_message(format!(
                                    "switched to agent {summary} on {}",
                                    provider_label
                                ));
                            }
                        }
                        InputOutcome::OpenTranscript => state.open_transcript_overlay(),
                        InputOutcome::FocusNextAgent => {
                            if let Some(status) = state.focus_next_agent() {
                                if state.view_state.mode == crate::view_mode::ViewMode::Overview {
                                    state.sync_overview_page_to_focus();
                                }
                                state.app_mut().push_status_message(format!(
                                    "focused {} ({})",
                                    status.codename.as_str(),
                                    status.status.label()
                                ));
                            } else {
                                // Single-agent mode, no pool yet
                                state.app_mut().push_status_message(
                                    "no agents to switch (press Ctrl+N to spawn)",
                                );
                            }
                        }
                        InputOutcome::FocusPreviousAgent => {
                            if let Some(status) = state.focus_previous_agent() {
                                if state.view_state.mode == crate::view_mode::ViewMode::Overview {
                                    state.sync_overview_page_to_focus();
                                }
                                state.app_mut().push_status_message(format!(
                                    "focused {} ({})",
                                    status.codename.as_str(),
                                    status.status.label()
                                ));
                            } else {
                                state.app_mut().push_status_message(
                                    "no agents to switch (press Ctrl+N to spawn)",
                                );
                            }
                        }
                        InputOutcome::FocusAgent(index) => {
                            if let Some(status) = state.focus_agent_by_index(index) {
                                if state.view_state.mode == crate::view_mode::ViewMode::Overview {
                                    state.sync_overview_page_to_focus();
                                }
                                state.app_mut().push_status_message(format!(
                                    "focused {} ({})",
                                    status.codename.as_str(),
                                    status.status.label()
                                ));
                            } else {
                                state.app_mut().push_status_message(format!(
                                    "no agent at index {}",
                                    index + 1
                                ));
                            }
                        }
                        InputOutcome::SpawnAgent => {
                            // Primary: open profile selection overlay
                            state.open_profile_selection_overlay();
                            // Fallback to provider overlay if no profiles available
                            if state.profile_selection_overlay.is_none() {
                                state.open_provider_overlay();
                            }
                        }
                        InputOutcome::StopFocusedAgent => {
                            let codename = state.focused_agent_codename().to_string();
                            state.open_stop_confirmation(&codename);
                        }
                        InputOutcome::PauseFocusedAgent => {
                            if state.pause_focused_agent().is_some() {
                                state.app_mut().push_status_message(
                                    "Agent paused with worktree preserved".to_string(),
                                );
                            } else {
                                state
                                    .app_mut()
                                    .push_status_message("Failed to pause agent".to_string());
                            }
                        }
                        InputOutcome::ResumeFocusedAgent => {
                            if state.resume_focused_agent().is_some() {
                                state
                                    .app_mut()
                                    .push_status_message("Agent resumed".to_string());
                            } else {
                                state.app_mut().push_status_message(
                                    "Failed to resume agent (not paused?)".to_string(),
                                );
                            }
                        }
                        InputOutcome::SwitchViewMode(n) => {
                            state.view_state.switch_by_number(n);
                            let label = state.view_state.mode.label();
                            state
                                .app_mut()
                                .push_status_message(format!("switched to {} view", label));
                        }
                        InputOutcome::NextViewMode => {
                            state.view_state.next_mode();
                            let label = state.view_state.mode.label();
                            state
                                .app_mut()
                                .push_status_message(format!("switched to {} view", label));
                        }
                        InputOutcome::PrevViewMode => {
                            state.view_state.prev_mode();
                            let label = state.view_state.mode.label();
                            state
                                .app_mut()
                                .push_status_message(format!("switched to {} view", label));
                        }
                        // Split view input handling
                        InputOutcome::SplitFocusLeft => {
                            state.view_state.split.focus_left();
                        }
                        InputOutcome::SplitFocusRight => {
                            state.view_state.split.focus_right();
                        }
                        InputOutcome::SplitSwap => {
                            state.view_state.split.swap();
                            state.app_mut().push_status_message("swapped agents");
                        }
                        InputOutcome::SplitEqual => {
                            state.view_state.split.equal_split();
                            state.app_mut().push_status_message("equal split");
                        }
                        // Dashboard view input handling
                        InputOutcome::DashboardNext => {
                            let count = state.agent_statuses().len();
                            state.view_state.dashboard.select_next(count);
                        }
                        InputOutcome::DashboardPrev => {
                            state.view_state.dashboard.select_prev();
                        }
                        InputOutcome::DashboardSelect(n) => {
                            let count = state.agent_statuses().len();
                            state.view_state.dashboard.select_by_number(n, count);
                        }
                        // Mail view input handling
                        InputOutcome::MailNext => {
                            let focused_id = state.focused_agent_id();
                            let count = focused_id
                                .as_ref()
                                .and_then(|id| state.mailbox.inbox_for(id))
                                .map(|inbox| inbox.len())
                                .unwrap_or(0);
                            state.view_state.mail.select_next(count);
                        }
                        InputOutcome::MailPrev => {
                            state.view_state.mail.select_prev();
                        }
                        InputOutcome::MailMarkRead => {
                            // Mark selected mail as read
                            let focused_id = state.focused_agent_id();
                            if let Some(id) = focused_id {
                                let inbox = state.mailbox.inbox_for(&id);
                                if let Some(mails) = inbox {
                                    let idx = state.view_state.mail.selected_mail_index;
                                    if idx < mails.len() {
                                        let mail_id = mails[idx].mail_id.clone();
                                        state.mailbox.mark_read(&id, &mail_id);
                                        state.app_mut().push_status_message("mail marked as read");
                                    }
                                }
                            }
                        }
                        InputOutcome::MailComposeStart => {
                            state.view_state.mail.start_compose();
                        }
                        InputOutcome::MailComposeCancel => {
                            state.view_state.mail.cancel_compose();
                        }
                        InputOutcome::MailComposeNextField => {
                            state.view_state.mail.next_compose_field();
                        }
                        InputOutcome::MailComposePrevField => {
                            state.view_state.mail.prev_compose_field();
                        }
                        InputOutcome::MailComposeSend(to, subject, body) => {
                            use agent_core::agent_mail::{
                                AgentMail, MailBody, MailSubject, MailTarget,
                            };
                            use agent_core::agent_runtime::AgentId;

                            // Parse recipient from 'to' field (codename or agent_id)
                            let recipient = if to.is_empty() {
                                // Default: send to focused agent
                                state.focused_agent_id()
                            } else {
                                // Try to parse as agent_id or find by codename
                                state
                                    .agent_pool
                                    .as_ref()
                                    .and_then(|pool| {
                                        pool.agent_statuses()
                                            .iter()
                                            .find(|s| {
                                                s.codename.as_str() == to
                                                    || s.agent_id.as_str() == to
                                            })
                                            .map(|s| s.agent_id.clone())
                                    })
                                    .or_else(|| Some(AgentId::new(&to)))
                            };

                            if let Some(recipient_id) = recipient {
                                let mail_subject = if subject.is_empty() {
                                    MailSubject::Custom {
                                        label: "Note".to_string(),
                                    }
                                } else {
                                    MailSubject::Custom { label: subject }
                                };

                                let sender = state
                                    .focused_agent_id()
                                    .unwrap_or_else(|| AgentId::new("unknown"));
                                let mail = AgentMail::new(
                                    sender,
                                    MailTarget::Direct(recipient_id),
                                    mail_subject,
                                    MailBody::Text(body),
                                );
                                state.mailbox.send_mail(mail);
                                state.view_state.mail.cancel_compose();
                                state.app_mut().push_status_message("mail sent");
                            } else {
                                state
                                    .app_mut()
                                    .push_status_message("no recipient specified");
                            }
                        }
                        // Overview view input handling
                        InputOutcome::OverviewFilterBlocked => {
                            state.view_state.overview.filter =
                                crate::overview_state::OverviewFilter::BlockedOnly;
                            state.ensure_overview_focus_visible();
                            state
                                .app_mut()
                                .push_status_message("showing blocked agents only");
                        }
                        InputOutcome::OverviewFilterRunning => {
                            state.view_state.overview.filter =
                                crate::overview_state::OverviewFilter::RunningOnly;
                            state.ensure_overview_focus_visible();
                            state
                                .app_mut()
                                .push_status_message("showing running agents only");
                        }
                        InputOutcome::OverviewFilterAll => {
                            state.view_state.overview.filter =
                                crate::overview_state::OverviewFilter::All;
                            state.ensure_overview_focus_visible();
                            state.app_mut().push_status_message("showing all agents");
                        }
                        InputOutcome::OverviewPageUp => {
                            let total_pages = state.overview_total_pages();
                            state.view_state.overview.page_up(total_pages);
                        }
                        InputOutcome::OverviewPageDown => {
                            let total_pages = state.overview_total_pages();
                            state.view_state.overview.page_down(total_pages);
                        }
                        InputOutcome::OverviewSearchStart => {
                            state.view_state.overview.search_active = true;
                            state.view_state.overview.search_query.clear();
                            state
                                .app_mut()
                                .push_status_message("search: type agent name, Enter to select");
                        }
                        InputOutcome::OverviewSearchCancel => {
                            state.view_state.overview.search_active = false;
                            state.view_state.overview.search_query.clear();
                            state.app_mut().push_status_message("search cancelled");
                        }
                        InputOutcome::OverviewSearchSelect(agent_name) => {
                            if state
                                .focus_overview_agent_by_codename(&agent_name)
                                .is_some()
                            {
                                state.view_state.overview.search_active = false;
                                state.view_state.overview.search_query.clear();
                                state
                                    .app_mut()
                                    .push_status_message(format!("focused {}", agent_name));
                            } else {
                                state
                                    .app_mut()
                                    .push_status_message(format!("agent {} not found", agent_name));
                            }
                        }
                        InputOutcome::Quit => {
                            state.session.workplace_mut().loop_control.signal_quit();
                        }
                        InputOutcome::Submit(user_input) => {
                            if handle_command_submission(
                                &mut state,
                                user_input.clone(),
                                &mut provider_rx,
                            )? {
                                continue;
                            }

                            logging::debug_event(
                                "tui.submit",
                                "submitted prompt from TUI",
                                serde_json::json!({
                                    "provider": state.app().selected_provider.label(),
                                    "prompt": user_input,
                                    "active_task_id": state.app().active_task_id,
                                }),
                            );
                            if state.is_multi_agent_mode() {
                                handle_multi_agent_submission(&mut state, user_input);
                            } else {
                                let augmented_prompt =
                                    state.workplace().skills.build_injected_prompt(&user_input);
                                state.app_mut().set_loop_phase(LoopPhase::Executing);
                                start_provider_request(
                                    &mut state,
                                    augmented_prompt,
                                    &mut provider_rx,
                                );
                            }
                        }
                    }
                }
                Event::Paste(text) => {
                    // Handle paste in launch config overlay
                    if state.is_launch_config_overlay_open() {
                        if let Some(overlay) = state.launch_config_overlay.as_mut() {
                            overlay.handle_paste(&text);
                        }
                    } else {
                        handle_paste_event(&mut state, &text);
                    }
                }
                Event::Mouse(mouse_event)
                    // Handle mouse click in Overview mode for agent selection
                    if state.view_state.mode == crate::view_mode::ViewMode::Overview => {
                        use crossterm::event::MouseEventKind;
                        if let MouseEventKind::Down(crossterm::event::MouseButton::Left) =
                            mouse_event.kind
                        {
                            // Calculate which agent row was clicked
                            let agent_list_height =
                                state.view_state.overview.agent_list_rows as u16;
                            let click_row = mouse_event.row;

                            // Click within agent list area
                            if click_row < agent_list_height {
                                let visible = state.overview_visible_agent_indices();

                                // Select clicked agent
                                let clicked_index = click_row as usize;
                                if let Some(original_index) = visible.get(clicked_index).copied()
                                    && let Some(snapshot) =
                                        state.focus_agent_by_index(original_index)
                                {
                                    state.app_mut().push_status_message(format!(
                                        "focused {}",
                                        snapshot.codename.as_str()
                                    ));
                                }
                            }
                        }
                    }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let mut should_clear_provider_rx = false;
        if let Some(rx) = provider_rx.as_ref() {
            loop {
                let event = match rx.try_recv() {
                    Ok(event) => event,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        handle_provider_terminal_error(
                            &mut state,
                            "provider event stream disconnected".to_string(),
                        )?;
                        should_clear_provider_rx = true;
                        break;
                    }
                };

                match event {
                    ProviderEvent::Status(text) => state.app_mut().push_status_message(text),
                    ProviderEvent::AssistantChunk(chunk) => {
                        state.append_active_assistant_chunk(&chunk)
                    }
                    ProviderEvent::ThinkingChunk(chunk) => {
                        state.append_active_thinking_chunk(&chunk)
                    }
                    ProviderEvent::ExecCommandStarted {
                        call_id,
                        input_preview,
                        source,
                    } => state.push_active_exec_started(call_id, input_preview, source),
                    ProviderEvent::ExecCommandFinished {
                        call_id,
                        output_preview,
                        status,
                        exit_code,
                        duration_ms,
                        source,
                    } => state.finish_active_exec(
                        call_id,
                        output_preview,
                        status,
                        exit_code,
                        duration_ms,
                        source,
                    ),
                    ProviderEvent::ExecCommandOutputDelta { call_id, delta } => {
                        state.append_active_exec_output(call_id, &delta)
                    }
                    ProviderEvent::GenericToolCallStarted {
                        name,
                        call_id,
                        input_preview,
                    } => state.push_active_generic_tool_call_started(name, call_id, input_preview),
                    ProviderEvent::GenericToolCallFinished {
                        name,
                        call_id,
                        output_preview,
                        success,
                        exit_code,
                        duration_ms,
                    } => state.finish_active_generic_tool_call(
                        name,
                        call_id,
                        output_preview,
                        success,
                        exit_code,
                        duration_ms,
                    ),
                    ProviderEvent::WebSearchStarted { call_id, query } => {
                        state.push_active_web_search_started(call_id, query)
                    }
                    ProviderEvent::WebSearchFinished {
                        call_id,
                        query,
                        action,
                    } => state.finish_active_web_search(call_id, query, action),
                    ProviderEvent::ViewImage { call_id, path } => {
                        state.app_mut().push_view_image(call_id, path)
                    }
                    ProviderEvent::ImageGenerationFinished {
                        call_id,
                        revised_prompt,
                        result,
                        saved_path,
                    } => state.app_mut().push_image_generation(
                        call_id,
                        revised_prompt,
                        result,
                        saved_path,
                    ),
                    ProviderEvent::McpToolCallStarted {
                        call_id,
                        invocation,
                    } => state.push_active_mcp_tool_call_started(call_id, invocation),
                    ProviderEvent::McpToolCallFinished {
                        call_id,
                        invocation,
                        result_blocks,
                        error,
                        status,
                        is_error,
                    } => state.finish_active_mcp_tool_call(
                        call_id,
                        invocation,
                        result_blocks,
                        error,
                        status,
                        is_error,
                    ),
                    ProviderEvent::PatchApplyStarted { call_id, changes } => {
                        state.push_active_patch_apply_started(call_id, changes)
                    }
                    ProviderEvent::PatchApplyOutputDelta { call_id, delta } => {
                        state.append_active_patch_apply_output(call_id, &delta)
                    }
                    ProviderEvent::PatchApplyFinished {
                        call_id,
                        changes,
                        status,
                    } => state.finish_active_patch_apply(call_id, changes, status),
                    ProviderEvent::SessionHandle(handle) => {
                        state.app_mut().apply_session_handle(handle);
                        state.persist_if_changed()?;
                    }
                    ProviderEvent::Error(error) => {
                        handle_provider_terminal_error(&mut state, error)?;
                        should_clear_provider_rx = true;
                        break;
                    }
                    ProviderEvent::ProviderPid(_pid) => {
                        // PID is tracked for shutdown cleanup; TUI doesn't need to display it
                    }
                    ProviderEvent::Finished => {
                        state.flush_active_entries_to_transcript();
                        state.app_mut().finish_provider_response();
                        if state.app().active_task_id.is_some() {
                            match task_engine::resolve_active_task_after_turn(
                                state.app_mut(),
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
                        if state.app().active_task_id.is_none()
                            && state.app().loop_phase != LoopPhase::Escalating
                        {
                            state.app_mut().set_loop_phase(LoopPhase::Idle);
                        }
                        state.persist_if_changed()?;
                        should_clear_provider_rx = true;
                        break;
                    }
                }
            }
        }

        if should_clear_provider_rx {
            provider_rx = None;
        }

        // Poll multi-agent events from EventAggregator
        if state.agent_channel_count() > 0 {
            let poll_result = state.poll_agent_events();
            for event in poll_result.events {
                match event {
                    agent_core::event_aggregator::AgentEvent::FromProvider { agent_id, event } => {
                        handle_agent_provider_event(&mut state, agent_id, event);
                    }
                    agent_core::event_aggregator::AgentEvent::ThreadFinished {
                        agent_id,
                        outcome,
                    } => {
                        handle_agent_thread_finished(&mut state, agent_id, outcome);
                    }
                    _ => {}
                }
            }

            // Handle disconnected channels
            for disconnected_id in poll_result.disconnected_channels {
                handle_agent_channel_disconnect(&mut state, disconnected_id);
            }

            // Check for idle responding agents and transition to WaitingForInput
            // Also trigger decision layer intervention for long-idle agents
            check_for_idle_responding_agents(&mut state);

            // Check for idle agents that need decision layer intervention
            // Decision layer will determine if agent should continue or stop
            check_idle_agents_for_decision(&mut state);
        }

        // Process pending mail delivery
        if state.mailbox.pending_count() > 0 {
            let delivered_to = state.mailbox.process_pending();
            for agent_id in delivered_to {
                state
                    .app_mut()
                    .push_status_message(format!("📬 {} received mail", agent_id.as_str()));
            }
        }

        if state.drain_active_stream_commit_tick() {
            state.persist_if_changed()?;
        }
    }

    shutdown_tui_state(&mut state)?;
    Ok(state.into_app_state())
}

fn event_poll_timeout(state: &TuiState, provider_active: bool) -> Duration {
    if provider_active || state.has_pending_active_stream_commits() {
        Duration::from_millis(80)
    } else {
        Duration::from_millis(250)
    }
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
                state.app_mut().push_status_message(line);
            }
            None
        }
        LocalCommand::Provider => {
            let summary = state.session.agent_runtime.summary();
            let provider_label = state.app().selected_provider.label().to_string();
            state.app_mut().push_status_message(format!(
                "current agent: {} · provider: {} (tab creates a new agent on the next provider)",
                summary, provider_label,
            ));
            None
        }
        LocalCommand::Skills => {
            state.app_mut().open_skill_browser();
            None
        }
        LocalCommand::Doctor => {
            let report = probe::probe_report();
            for line in probe::render_doctor_text(&report).lines() {
                if !line.trim().is_empty() {
                    state.app_mut().push_status_message(line);
                }
            }
            None
        }
        LocalCommand::Backlog => {
            for line in state.app().render_backlog_lines() {
                state.app_mut().push_status_message(line);
            }
            None
        }
        LocalCommand::TodoAdd(title) => {
            let todo_id = state.app_mut().add_todo(title.clone());
            state
                .app_mut()
                .push_status_message(format!("added todo: {} ({})", todo_id, title));
            None
        }
        LocalCommand::RunOnce => {
            let Some(todo_id) = state.app().next_ready_todo_id() else {
                state
                    .app_mut()
                    .push_status_message("no ready todo available");
                return None;
            };

            let Some(task) = state.app_mut().begin_task_from_todo(&todo_id) else {
                state
                    .app_mut()
                    .push_error_message(format!("failed to start task from todo: {todo_id}"));
                return None;
            };

            // In multi-agent mode, assign task to an idle agent to trigger git flow preparation
            if state.is_multi_agent_mode() {
                let task_id = task.id.clone();
                // Use a scope to ensure only one mutable borrow at a time
                // by projecting the session reference directly
                {
                    let backlog_ref = &mut state.session.app.backlog;
                    if let Some(pool) = state.agent_pool.as_mut()
                        && let Some(idle_agent_id) = pool.find_idle_agent_id() {
                            // Assign task to agent via assign_task_with_backlog to trigger trigger_task_preparation
                            if let Err(e) = pool.assign_task_with_backlog(
                                &idle_agent_id,
                                agent_core::agent_slot::TaskId::new(&task_id),
                                backlog_ref,
                            ) {
                                logging::warn_event(
                                    "tui.task.assign_failed",
                                    "failed to assign task to agent",
                                    serde_json::json!({
                                        "task_id": task_id,
                                        "agent_id": idle_agent_id.as_str(),
                                        "error": e,
                                    }),
                                );
                            } else {
                                logging::debug_event(
                                    "tui.task.assigned",
                                    "task assigned to agent for git flow preparation",
                                    serde_json::json!({
                                        "task_id": task_id,
                                        "agent_id": idle_agent_id.as_str(),
                                    }),
                                );
                            }
                        }
                }
            }

            state
                .app_mut()
                .push_status_message(format!("running task: {}", task.id));
            Some(task_engine::build_task_prompt(&task))
        }
        LocalCommand::RunLoop => {
            state.workplace_mut().loop_control.start_loop(5);
            state.app_mut().set_loop_phase(LoopPhase::Planning);
            state
                .app_mut()
                .push_status_message("starting autonomous run-loop");
            logging::debug_event(
                "tui.loop_control",
                "started autonomous loop from TUI",
                serde_json::json!({
                    "remaining_iterations": state.workplace().loop_control.remaining_iterations(),
                }),
            );
            None
        }
        LocalCommand::Quit => {
            state.workplace_mut().loop_control.signal_quit();
            None
        }
    }
}

fn start_provider_request(
    state: &mut TuiState,
    prompt: String,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) {
    // Clear decision status when starting new provider request
    // (agent is starting new work, decision context no longer applies)
    state.set_decision_status(None);

    // Check if multi-agent mode is active (agent pool exists with agents)
    if state.is_multi_agent_mode() {
        start_multi_agent_provider_request(state, prompt);
        return;
    }

    // Single-agent mode: use existing flow
    let (event_tx, event_rx) = mpsc::channel();
    let provider_kind = state.app().selected_provider;
    let session_handle = state.app().current_session_handle();
    logging::debug_event(
        "tui.provider_request",
        "starting provider request from TUI (single-agent)",
        serde_json::json!({
            "provider": provider_kind.label(),
            "prompt": prompt,
            "session_handle": format!("{:?}", session_handle),
        }),
    );
    state.app_mut().mark_active_task_running();
    if let Err(err) = agent_core::start_provider(
        provider_kind,
        prompt,
        state.app().cwd.clone(),
        session_handle,
        event_tx,
    ) {
        task_engine::handle_provider_start_failure(state.app_mut(), err.to_string());
    } else {
        state.app_mut().begin_provider_response();
        let _ = state.persist_if_changed();
        *provider_rx = Some(event_rx);
    }
}

fn start_multi_agent_provider_request(state: &mut TuiState, prompt: String) {
    let focused_id = state.focused_agent_id();

    logging::debug_event(
        "tui.multi_agent_request",
        "starting provider request for focused agent",
        serde_json::json!({
            "focused_agent": focused_id.as_ref().map(|id| id.as_str()).unwrap_or("none"),
            "prompt": prompt,
        }),
    );

    if let Some(agent_id) = focused_id
        && !start_multi_agent_provider_request_for_agent(state, agent_id, prompt, true) {
            task_engine::handle_provider_start_failure(
                state.app_mut(),
                "failed to start provider for agent".to_string(),
            );
        }
}

fn start_multi_agent_provider_request_for_agent(
    state: &mut TuiState,
    agent_id: agent_core::agent_runtime::AgentId,
    prompt: String,
    inject_mail: bool,
) -> bool {
    let provider_label = state
        .agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(&agent_id))
        .map(|slot| slot.provider_type().label().to_string())
        .unwrap_or_else(|| state.app().selected_provider.label().to_string());

    let event_rx = if inject_mail {
        state.start_provider_for_agent(&agent_id, prompt)
    } else {
        state.start_raw_provider_for_agent(&agent_id, prompt)
    };
    if let Some(rx) = event_rx {
        state.register_agent_channel(agent_id.clone(), rx);
        state.app_mut().push_status_message(format!(
            "started {} ({})",
            agent_id.as_str(),
            provider_label
        ));
        true
    } else {
        false
    }
}

fn handle_command_submission(
    state: &mut TuiState,
    user_input: String,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) -> Result<bool> {
    if !user_input.trim_start().starts_with('/') {
        return Ok(false);
    }

    if let Some(invocation) = parse_legacy_alias(&user_input) {
        if let Err(error) = execute_invocation(state, invocation, provider_rx) {
            state.app_mut().push_error_message(error.to_string());
        }
        return Ok(true);
    }

    let parsed = match parse_slash_command(&user_input) {
        Ok(parsed) => parsed,
        Err(error) => {
            state.app_mut().push_error_message(error.to_string());
            return Ok(true);
        }
    };

    let ParsedSlashCommand::Invocation(invocation) = parsed;
    if let Err(error) = execute_invocation(state, invocation, provider_rx) {
        state.app_mut().push_error_message(error.to_string());
    }
    Ok(true)
}

fn execute_invocation(
    state: &mut TuiState,
    invocation: CommandInvocation,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) -> Result<()> {
    match invocation.namespace {
        CommandNamespace::Local => execute_local_invocation(state, invocation, provider_rx),
        CommandNamespace::Agent => execute_agent_invocation(state, invocation),
        CommandNamespace::Provider => execute_provider_invocation(state, invocation),
    }
}

fn execute_local_invocation(
    state: &mut TuiState,
    invocation: CommandInvocation,
    provider_rx: &mut Option<mpsc::Receiver<ProviderEvent>>,
) -> Result<()> {
    if let Some(command) = legacy_local_command_from_invocation(&invocation) {
        logging::debug_event(
            "tui.command.legacy",
            "executed legacy local alias",
            serde_json::json!({
                "command": format!("{:?}", command),
            }),
        );
        if let Some(prompt) = handle_local_command(state, command) {
            start_provider_request(state, prompt, provider_rx);
        }
        return Ok(());
    }

    let path = invocation
        .path
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let args = invocation
        .args
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let lines = crate::command_runtime::execute_local_command(state, &path, &args)?;
    for line in lines {
        state.app_mut().push_status_message(line);
    }
    Ok(())
}

fn execute_agent_invocation(state: &mut TuiState, invocation: CommandInvocation) -> Result<()> {
    let explicit_target = invocation.target.as_ref().map(|target| match target {
        CommandTargetSpec::AgentName(value) => value.as_str(),
    });
    let path = invocation
        .path
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let args = invocation
        .args
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let lines =
        crate::command_runtime::execute_agent_command(state, explicit_target, &path, &args)?;
    for line in lines {
        state.app_mut().push_status_message(line);
    }
    Ok(())
}

fn execute_provider_invocation(state: &mut TuiState, invocation: CommandInvocation) -> Result<()> {
    let explicit_target = invocation.target.as_ref().map(|target| match target {
        CommandTargetSpec::AgentName(value) => value.as_str(),
    });
    let raw_tail = invocation.raw_tail.as_deref().unwrap_or("");
    let request =
        crate::command_runtime::execute_provider_command(state, explicit_target, raw_tail)?;
    state.append_status_to_agent_transcript(
        &request.agent_id,
        format!("provider command: {}", request.raw_tail),
    );
    if !start_multi_agent_provider_request_for_agent(
        state,
        request.agent_id.clone(),
        request.raw_tail.clone(),
        false,
    ) {
        task_engine::handle_provider_start_failure(
            state.app_mut(),
            format!("failed to send provider command `{}`", request.raw_tail),
        );
    } else {
        state.app_mut().push_status_message(format!(
            "sent provider command `{}` to {}",
            request.raw_tail, request.codename
        ));
    }
    Ok(())
}

fn legacy_local_command_from_invocation(invocation: &CommandInvocation) -> Option<LocalCommand> {
    let path = invocation
        .path
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    match path.as_slice() {
        ["legacy", "provider"] => Some(LocalCommand::Provider),
        ["legacy", "skills"] => Some(LocalCommand::Skills),
        ["legacy", "doctor"] => Some(LocalCommand::Doctor),
        ["legacy", "run-once"] => Some(LocalCommand::RunOnce),
        ["legacy", "run-loop"] => Some(LocalCommand::RunLoop),
        ["legacy", "quit"] => Some(LocalCommand::Quit),
        ["legacy", "todo-add"] => invocation.args.first().cloned().map(LocalCommand::TodoAdd),
        _ => None,
    }
}

fn shutdown_tui_state(state: &mut TuiState) -> Result<()> {
    state.sync_app_input_from_composer();
    let reason = if state.session.was_interrupted() {
        agent_core::shutdown_snapshot::ShutdownReason::Interrupted
    } else {
        agent_core::shutdown_snapshot::ShutdownReason::UserQuit
    };

    let summary = state.create_shutdown_snapshot(reason.clone());
    state
        .session
        .agent_runtime
        .workplace()
        .save_shutdown_snapshot(&summary)?;

    let resume_snapshot = state.create_resume_snapshot(reason);
    save_resume_snapshot(state.session.agent_runtime.workplace(), &resume_snapshot)?;

    state.session.quick_shutdown()
}

fn resolve_agent_target_ids(
    state: &TuiState,
    agents: &[String],
) -> std::result::Result<Vec<agent_core::agent_runtime::AgentId>, String> {
    let statuses = state.agent_statuses();
    let mut resolved = Vec::with_capacity(agents.len());

    for agent in agents {
        let Some(status) = statuses
            .iter()
            .find(|status| status.codename.as_str() == agent || status.agent_id.as_str() == agent)
        else {
            return Err(format!("agent {} not found", agent));
        };
        resolved.push(status.agent_id.clone());
    }

    Ok(resolved)
}

fn handle_multi_agent_submission(state: &mut TuiState, user_input: String) -> bool {
    match parse_at_command(&user_input) {
        AtCommandResult::Invalid { error } => {
            state.app_mut().push_error_message(error);
            true
        }
        AtCommandResult::Normal(message) => {
            let augmented_prompt = state.workplace().skills.build_injected_prompt(&message);
            start_multi_agent_provider_request(state, augmented_prompt);
            true
        }
        AtCommandResult::Single { agent, message } => {
            let prompt = state.workplace().skills.build_injected_prompt(&message);
            match resolve_agent_target_ids(state, &[agent]) {
                Ok(agent_ids) => {
                    for agent_id in agent_ids {
                        start_multi_agent_provider_request_for_agent(
                            state,
                            agent_id,
                            prompt.clone(),
                            true,
                        );
                    }
                }
                Err(error) => state.app_mut().push_error_message(error),
            }
            true
        }
        AtCommandResult::Broadcast { agents, message } => {
            let prompt = state.workplace().skills.build_injected_prompt(&message);
            match resolve_agent_target_ids(state, &agents) {
                Ok(agent_ids) => {
                    for agent_id in agent_ids {
                        start_multi_agent_provider_request_for_agent(
                            state,
                            agent_id,
                            prompt.clone(),
                            true,
                        );
                    }
                }
                Err(error) => state.app_mut().push_error_message(error),
            }
            true
        }
    }
}

fn handle_provider_terminal_error(state: &mut TuiState, error: String) -> Result<()> {
    state.finalize_active_entries_after_failure(Some(&error));
    state.app_mut().mark_active_task_error();

    // Check if this is a session expiry error and clear the session handle
    if is_session_expired_error(&error) {
        state.app_mut().clear_session();
        state.app_mut().push_error_message(
            "session expired - starting fresh conversation. Please retry your request.",
        );
    } else {
        state.app_mut().push_error_message(error);
    }

    state.app_mut().finish_provider_response();
    if state.app().active_task_id.is_none() && state.app().loop_phase != LoopPhase::Escalating {
        state.app_mut().set_loop_phase(LoopPhase::Idle);
    }
    state.persist_if_changed()
}

/// Check if the error indicates an expired or invalid session/conversation
fn is_session_expired_error(error: &str) -> bool {
    // Claude CLI returns this when --resume session ID doesn't exist
    error.contains("No conversation found with session ID")
        || error.contains("No conversation found")
        // Codex may have similar error patterns
        || error.contains("thread not found")
        || error.contains("session not found")
}

fn next_loop_prompt(state: &mut TuiState) -> Option<(String, bool)> {
    if let Some(active_task_id) = state.app().active_task_id.clone() {
        let task = state
            .workplace()
            .backlog
            .tasks
            .iter()
            .find(|task| task.id == active_task_id)
            .cloned()?;
        state.app_mut().set_loop_phase(LoopPhase::Executing);
        state
            .app_mut()
            .push_status_message(format!("resuming task: {}", task.id));
        return Some((task_engine::build_task_prompt(&task), false));
    }

    let todo_id = state.app().next_ready_todo_id()?;

    state.app_mut().set_loop_phase(LoopPhase::Planning);
    let Some(task) = state.app_mut().begin_task_from_todo(&todo_id) else {
        state
            .app_mut()
            .push_error_message(format!("failed to start task from todo: {todo_id}"));
        return None;
    };

    state
        .app_mut()
        .push_status_message(format!("running task: {}", task.id));
    Some((task_engine::build_task_prompt(&task), true))
}

/// Check for shutdown snapshot and show resume dialog if exists
///
/// Returns true if user wants to resume, false if start fresh/clean.
fn check_resume_snapshot(terminal: &mut AppTerminal, launch_cwd: &Path) -> Result<bool> {
    use agent_core::workplace_store::WorkplaceStore;
    use crossterm::event::Event;

    let workplace = WorkplaceStore::for_cwd(launch_cwd)?;
    let snapshot = workplace.load_shutdown_snapshot()?;

    if snapshot.is_none() {
        // No snapshot, proceed with normal bootstrap
        return Ok(false);
    }

    let snapshot = snapshot.unwrap();
    logging::debug_event(
        "tui.resume.check",
        "found shutdown snapshot, showing resume dialog",
        serde_json::json!({
            "agents_count": snapshot.agents.len(),
            "reason": format!("{:?}", snapshot.shutdown_reason),
        }),
    );

    let mut overlay = ResumeOverlay::new(snapshot);

    // Run resume overlay loop
    loop {
        terminal.draw(|frame| {
            crate::render::render_resume_overlay(frame, &overlay);
        })?;

        // Poll for input with timeout
        if crossterm::event::poll(Duration::from_millis(100))?
            && let Event::Key(key_event) = crossterm::event::read()?
            && let Some(cmd) = overlay.handle_key_event(key_event)
        {
            match cmd {
                ResumeCommand::Resume => {
                    logging::debug_event(
                        "tui.resume.choice",
                        "user chose to resume",
                        serde_json::json!({}),
                    );
                    return Ok(true);
                }
                ResumeCommand::StartFresh => {
                    logging::debug_event(
                        "tui.resume.choice",
                        "user chose start fresh",
                        serde_json::json!({}),
                    );
                    // Clear snapshot for fresh start
                    workplace.clear_shutdown_snapshot()?;
                    clear_resume_snapshot(&workplace)?;
                    return Ok(false);
                }
                ResumeCommand::CancelRestore => {
                    logging::debug_event(
                        "tui.resume.choice",
                        "user chose cancel restore",
                        serde_json::json!({}),
                    );
                    // Clear snapshot and start clean
                    workplace.clear_shutdown_snapshot()?;
                    clear_resume_snapshot(&workplace)?;
                    return Ok(false);
                }
            }
        }
    }
}

/// Generate scroll log message from provider event for Overview mode
fn generate_overview_log_message(
    event: &ProviderEvent,
    agent_id: &agent_core::agent_runtime::AgentId,
    state: &TuiState,
) -> Option<crate::overview_state::OverviewLogMessage> {
    use crate::overview_state::{OverviewLogMessage, OverviewMessageType};

    let timestamp = current_time_as_u32();
    let codename = state
        .agent_pool
        .as_ref()
        .and_then(|p| p.get_slot_by_id(agent_id))
        .map(|s| s.codename().as_str().to_string())
        .unwrap_or_else(|| agent_id.as_str().to_string());

    match event {
        ProviderEvent::Status(text) => Some(OverviewLogMessage {
            timestamp,
            agent: codename,
            message_type: OverviewMessageType::Progress,
            content: text.clone(),
        }),
        ProviderEvent::Finished => Some(OverviewLogMessage {
            timestamp,
            agent: codename,
            message_type: OverviewMessageType::Complete,
            content: "Task complete".to_string(),
        }),
        ProviderEvent::Error(error) => Some(OverviewLogMessage {
            timestamp,
            agent: codename,
            message_type: OverviewMessageType::Blocked,
            content: format!("ERROR: {}", error),
        }),
        ProviderEvent::ExecCommandStarted { input_preview, .. } => {
            let preview = input_preview.clone().unwrap_or_else(|| "exec".to_string());
            Some(OverviewLogMessage {
                timestamp,
                agent: codename,
                message_type: OverviewMessageType::Progress,
                content: format!("Running: {}", preview),
            })
        }
        ProviderEvent::ExecCommandFinished { status, .. } => {
            let status_str = match status {
                agent_core::ExecCommandStatus::Completed => "Success",
                agent_core::ExecCommandStatus::Failed => "Failed",
                agent_core::ExecCommandStatus::Declined => "Declined",
                agent_core::ExecCommandStatus::InProgress => "In Progress",
            };
            Some(OverviewLogMessage {
                timestamp,
                agent: codename,
                message_type: OverviewMessageType::Progress,
                content: format!("Exec {}", status_str),
            })
        }
        ProviderEvent::GenericToolCallStarted { name, .. } => Some(OverviewLogMessage {
            timestamp,
            agent: codename,
            message_type: OverviewMessageType::Progress,
            content: format!("Tool: {}", name),
        }),
        ProviderEvent::WebSearchStarted { query, .. } => Some(OverviewLogMessage {
            timestamp,
            agent: codename,
            message_type: OverviewMessageType::Progress,
            content: format!("Searching: {}", query),
        }),
        _ => None,
    }
}

fn latest_assistant_summary_for_agent(
    state: &TuiState,
    agent_id: &agent_core::agent_runtime::AgentId,
) -> Option<String> {
    let text = state
        .agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(agent_id))
        .and_then(|slot| {
            slot.transcript()
                .iter()
                .rev()
                .find_map(|entry| match entry {
                    TranscriptEntry::Assistant(text) if !text.trim().is_empty() => {
                        Some(text.as_str())
                    }
                    _ => None,
                })
        })?;

    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(truncate_overview_log_text(&normalized, 160))
    }
}

fn truncate_overview_log_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn push_overview_assistant_summary(
    state: &mut TuiState,
    agent_id: &agent_core::agent_runtime::AgentId,
) {
    let Some(content) = latest_assistant_summary_for_agent(state, agent_id) else {
        return;
    };
    let agent = state
        .agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(agent_id))
        .map(|slot| slot.codename().as_str().to_string())
        .unwrap_or_else(|| agent_id.as_str().to_string());

    state
        .view_state
        .overview
        .push_log_message(crate::overview_state::OverviewLogMessage {
            timestamp: current_time_as_u32(),
            agent,
            message_type: crate::overview_state::OverviewMessageType::Progress,
            content,
        });
}

fn append_agent_transcript_chunk(
    state: &mut TuiState,
    agent_id: &agent_core::agent_runtime::AgentId,
    chunk: String,
    kind: crate::ui_state::StreamTextKind,
) {
    let Some(pool) = state.agent_pool.as_mut() else {
        return;
    };
    let Some(slot) = pool.get_slot_mut_by_id(agent_id) else {
        return;
    };

    match (slot.transcript_mut().last_mut(), kind) {
        (Some(TranscriptEntry::Assistant(text)), crate::ui_state::StreamTextKind::Assistant) => {
            text.push_str(&chunk);
        }
        (Some(TranscriptEntry::Thinking(text)), crate::ui_state::StreamTextKind::Thinking) => {
            text.push_str(&chunk);
        }
        (_, crate::ui_state::StreamTextKind::Assistant) => {
            slot.append_transcript(TranscriptEntry::Assistant(chunk));
        }
        (_, crate::ui_state::StreamTextKind::Thinking) => {
            slot.append_transcript(TranscriptEntry::Thinking(chunk));
        }
    }
}

fn handle_agent_provider_event(
    state: &mut TuiState,
    agent_id: agent_core::agent_runtime::AgentId,
    event: ProviderEvent,
) {
    // Clone event for decision layer classification before processing
    let event_for_classification = event.clone();

    // Update activity timestamp for this agent
    if let Some(pool) = state.agent_pool.as_mut()
        && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
    {
        slot.touch_activity();
        // If agent was waiting for input, transition back to responding
        if slot.status().is_waiting_for_input() {
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
        }
    }

    if let Some(msg) = generate_overview_log_message(&event, &agent_id, state) {
        state.view_state.overview.push_log_message(msg);
    }

    match event {
        ProviderEvent::Status(text) => {
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.append_transcript(TranscriptEntry::Status(text.clone()));
            }
            state
                .app_mut()
                .push_status_message(format!("{}: {}", agent_id.as_str(), text));
        }
        ProviderEvent::AssistantChunk(chunk) => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.append_active_assistant_chunk(&chunk);
            }
            append_agent_transcript_chunk(
                state,
                &agent_id,
                chunk,
                crate::ui_state::StreamTextKind::Assistant,
            );
        }
        ProviderEvent::ThinkingChunk(chunk) => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.append_active_thinking_chunk(&chunk);
            }
            append_agent_transcript_chunk(
                state,
                &agent_id,
                chunk,
                crate::ui_state::StreamTextKind::Thinking,
            );
        }
        ProviderEvent::Finished => {
            push_overview_assistant_summary(state, &agent_id);
            // Flush active entries when focused agent finishes
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.flush_active_entries_to_transcript();
            }
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
                && slot.status().is_active()
            {
                let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::idle());
            }
            state.unregister_agent_channel(&agent_id);
            state
                .app_mut()
                .push_status_message(format!("{} finished", agent_id.as_str()));
        }
        ProviderEvent::Error(error) => {
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.append_transcript(TranscriptEntry::Error(error.clone()));
                let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::blocked(
                    error.clone(),
                ));
            }
            // Finalize active entries when focused agent has error
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finalize_active_entries_after_failure(Some(&error));
            }
            state.unregister_agent_channel(&agent_id);
            state
                .app_mut()
                .push_error_message(format!("{} error: {}", agent_id.as_str(), error));
        }
        ProviderEvent::SessionHandle(handle) => {
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.set_session_handle(handle);
            }
        }
        // Tool call events - update active_entries when focused, always add to slot transcript
        ProviderEvent::ExecCommandStarted {
            call_id,
            input_preview,
            source,
        } => {
            // Add to slot transcript for all agents
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.append_transcript(TranscriptEntry::ExecCommand {
                    call_id: call_id.clone(),
                    source: source.clone(),
                    allow_exploring_group: true,
                    input_preview: input_preview.clone(),
                    output_preview: None,
                    status: agent_core::ExecCommandStatus::InProgress,
                    exit_code: None,
                    duration_ms: None,
                });
            }
            // Update active_entries only for focused agent
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.push_active_exec_started(call_id, input_preview, source);
            }
        }
        ProviderEvent::ExecCommandFinished {
            call_id,
            output_preview,
            status,
            exit_code,
            duration_ms,
            source,
        } => {
            // Update slot transcript for all agents by replacing the InProgress entry
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                // Replace the InProgress entry with completed one
                slot.update_last_exec_command(
                    call_id.clone(),
                    output_preview.clone(),
                    status,
                    exit_code,
                    duration_ms,
                );
            }
            // Update active_entries only for focused agent
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finish_active_exec(
                    call_id,
                    output_preview,
                    status,
                    exit_code,
                    duration_ms,
                    source,
                );
            }
        }
        ProviderEvent::ExecCommandOutputDelta { call_id, delta } => {
            // Update slot transcript output for all agents
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.append_exec_command_output_delta(call_id.clone(), &delta);
            }
            // Update active_entries only for focused agent
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.append_active_exec_output(call_id, &delta);
            }
        }
        ProviderEvent::GenericToolCallStarted {
            name,
            call_id,
            input_preview,
        } => {
            // Add to slot transcript for all agents
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.append_transcript(TranscriptEntry::GenericToolCall {
                    name: name.clone(),
                    call_id: call_id.clone(),
                    input_preview: input_preview.clone(),
                    output_preview: None,
                    success: true,
                    started: true,
                    exit_code: None,
                    duration_ms: None,
                });
            }
            // Update active_entries only for focused agent
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.push_active_generic_tool_call_started(name, call_id, input_preview);
            }
        }
        ProviderEvent::GenericToolCallFinished {
            name,
            call_id,
            output_preview,
            success,
            exit_code,
            duration_ms,
        } => {
            // Update slot transcript for all agents
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.update_last_generic_tool_call(
                    name.clone(),
                    call_id.clone(),
                    output_preview.clone(),
                    success,
                    exit_code,
                    duration_ms,
                );
            }
            // Update active_entries only for focused agent
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finish_active_generic_tool_call(
                    name,
                    call_id,
                    output_preview,
                    success,
                    exit_code,
                    duration_ms,
                );
            }
        }
        ProviderEvent::WebSearchStarted { call_id, query } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.push_active_web_search_started(call_id, query);
            }
        }
        ProviderEvent::WebSearchFinished {
            call_id,
            query,
            action,
        } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finish_active_web_search(call_id, query, action);
            }
        }
        ProviderEvent::McpToolCallStarted {
            call_id,
            invocation,
        } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.push_active_mcp_tool_call_started(call_id, invocation);
            }
        }
        ProviderEvent::McpToolCallFinished {
            call_id,
            invocation,
            result_blocks,
            error,
            status,
            is_error,
        } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finish_active_mcp_tool_call(
                    call_id,
                    invocation,
                    result_blocks,
                    error,
                    status,
                    is_error,
                );
            }
        }
        ProviderEvent::PatchApplyStarted { call_id, changes } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.push_active_patch_apply_started(call_id, changes);
            }
        }
        ProviderEvent::PatchApplyOutputDelta { call_id, delta } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.append_active_patch_apply_output(call_id, &delta);
            }
        }
        ProviderEvent::PatchApplyFinished {
            call_id,
            changes,
            status,
        } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.finish_active_patch_apply(call_id, changes, status);
            }
        }
        ProviderEvent::ViewImage { call_id, path } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state.app_mut().push_view_image(call_id, path);
            }
        }
        ProviderEvent::ImageGenerationFinished {
            call_id,
            revised_prompt,
            result,
            saved_path,
        } => {
            if state.focused_agent_id().as_ref() == Some(&agent_id) {
                state
                    .app_mut()
                    .push_image_generation(call_id, revised_prompt, result, saved_path);
            }
        }
        ProviderEvent::ProviderPid(pid) => {
            // Save PID for shutdown cleanup
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
            {
                slot.set_provider_pid(pid);
            }
        }
    }

    // Decision layer integration: classify event and send decision request if needed
    if let Some(pool) = state.agent_pool.as_ref() {
        let classify_result = pool.classify_event(&agent_id, &event_for_classification);
        if classify_result.is_needs_decision() {
            // Get situation from classify result, or build if not available
            let situation = classify_result
                .situation()
                .map(|s| s.clone_boxed())
                .unwrap_or_else(|| {
                    let components = agent_decision::initializer::initialize_decision_layer();
                    components
                        .situation_registry
                        .build(classify_result.situation_type().unwrap().clone())
                });

            let situation_type = classify_result.situation_type().unwrap();

            // Create decision context
            use agent_decision::context::DecisionContext;
            let context = DecisionContext::new(situation.clone_boxed(), agent_id.as_str());

            // Create decision request
            use agent_core::decision_mail::DecisionRequest;
            let request = DecisionRequest::new(agent_id.clone(), situation_type.clone(), context);

            // Send decision request to decision agent
            if let Some(pool) = state.agent_pool.as_mut() {
                if let Err(e) = pool.send_decision_request(&agent_id, request) {
                    logging::warn_event(
                        "app_loop.decision_request_failed",
                        "failed to send decision request",
                        serde_json::json!({
                            "agent_id": agent_id.as_str(),
                            "error": e,
                        }),
                    );
                } else {
                    // Request sent successfully - transition agent to blocked_for_decision status
                    use agent_decision::blocking::{BlockedState, HumanDecisionBlocking};

                    let decision_request_id = format!(
                        "req-{}-{}",
                        agent_id.as_str(),
                        chrono::Local::now().format("%H%M%S")
                    );
                    let blocking = HumanDecisionBlocking::new(
                        decision_request_id,
                        situation,
                        Vec::new(), // Options determined by decision engine
                    );
                    let blocked_state = BlockedState::new(Box::new(blocking));

                    if let Err(e) = pool.process_agent_blocked(&agent_id, blocked_state, None) {
                        logging::warn_event(
                            "app_loop.blocked_transition_failed",
                            "failed to transition agent to blocked_for_decision",
                            serde_json::json!({
                                "agent_id": agent_id.as_str(),
                                "error": e,
                            }),
                        );
                    }

                    logging::debug_event(
                        "app_loop.decision_request_sent",
                        "decision request sent, agent now blocked_for_decision",
                        serde_json::json!({
                            "agent_id": agent_id.as_str(),
                            "situation_type": situation_type.name,
                        }),
                    );
                }
            }
        }
    }
}

/// Check for agents that have been idle in Responding state for too long
/// and transition them to WaitingForInput status
fn check_for_idle_responding_agents(state: &mut TuiState) {
    // Collect agent IDs that need transition and decision trigger
    let agents_to_process: Vec<(agent_core::agent_runtime::AgentId, String)> = {
        if let Some(pool) = state.agent_pool.as_ref() {
            pool.slots()
                .iter()
                .filter_map(|slot| {
                    if slot.should_transition_to_waiting(RESPONDING_IDLE_TIMEOUT_SECS) {
                        Some((slot.agent_id().clone(), "idle_timeout".to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    // Transition each agent and trigger decision layer
    for (agent_id, trigger_reason) in agents_to_process {
        // Transition agent status
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&agent_id) {
                let _ = slot
                    .transition_to(agent_core::agent_slot::AgentSlotStatus::waiting_for_input());
                logging::debug_event(
                    "app_loop.agent_idle_timeout",
                    "agent transitioned to waiting_for_input due to idle timeout",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "idle_timeout_secs": RESPONDING_IDLE_TIMEOUT_SECS,
                    }),
                );
            }

        // Trigger decision layer for this agent to determine next action
        trigger_decision_for_idle_agent(state, &agent_id, &trigger_reason);
    }
}

/// Check for idle agents that need decision layer intervention
///
/// This function triggers the decision layer for agents that have been idle
/// (not blocked, not active) for a configured duration. The decision layer
/// will check if there are pending tasks and either continue or stop the agent.
fn check_idle_agents_for_decision(state: &mut TuiState) {
    // Guard: only trigger idle agents when backlog has ready tasks
    let has_ready_tasks = !state.app().backlog.ready_tasks().is_empty();
    if !has_ready_tasks {
        return;
    }

    if let Some(pool) = state.agent_pool.as_ref() {
        // Collect agent IDs that have been idle for too long
        let agents_to_check: Vec<agent_core::agent_runtime::AgentId> = pool
            .slots()
            .iter()
            .filter_map(|slot| {
                // Only check idle agents (not blocked, not active, not stopped)
                if slot.status().is_idle() {
                    // Check if idle for longer than IDLE_DECISION_TRIGGER_SECS
                    let idle_duration = slot.last_activity();
                    let elapsed = idle_duration.elapsed().as_secs();
                    // Cooldown: don't re-trigger if already triggered recently (300s)
                    let recently_triggered = slot.last_idle_trigger_at()
                        .map(|t| t.elapsed().as_secs() < 300)
                        .unwrap_or(false);
                    if elapsed >= IDLE_DECISION_TRIGGER_SECS && !recently_triggered {
                        Some(slot.agent_id().clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Trigger decision for each idle agent
        for agent_id in agents_to_check {
            trigger_decision_for_idle_agent(state, &agent_id, "idle_check");
        }
    }
}

/// Trigger decision layer for an idle agent
///
/// Creates a decision request for the agent to determine whether to:
/// 1. Continue working on pending tasks
/// 2. Stop if all tasks are complete
fn trigger_decision_for_idle_agent(
    state: &mut TuiState,
    agent_id: &agent_core::agent_runtime::AgentId,
    trigger_reason: &str,
) {
    // Guard: skip if decision agent is already processing a request
    let decision_agent_busy = state.agent_pool.as_ref().and_then(|pool| {
        pool.decision_agent_for(agent_id).map(|da| !da.is_idle())
    }).unwrap_or(false);
    if decision_agent_busy {
        logging::debug_event(
            "decision_layer.idle_trigger_skipped",
            "decision agent already processing, skipping duplicate trigger",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "trigger_reason": trigger_reason,
            }),
        );
        return;
    }

    use agent_decision::context::DecisionContext;
    use agent_decision::initializer::initialize_decision_layer;
    use agent_decision::types::SituationType;

    // Create agent_idle situation type
    let situation_type = SituationType::new("agent_idle");

    // Get decision layer components
    let components = initialize_decision_layer();

    // Build the situation
    let situation = components.situation_registry.build(situation_type.clone());

    // Create decision context with agent state info
    let context = DecisionContext::new(situation, agent_id.as_str());

    // Create decision request
    use agent_core::decision_mail::DecisionRequest;
    let request = DecisionRequest::new(agent_id.clone(), situation_type.clone(), context);

    // Send decision request
    if let Some(pool) = state.agent_pool.as_mut() {
        {
            let slot = pool.get_slot_mut_by_id(agent_id);
            if let Some(s) = slot {
                s.set_last_idle_trigger_at(std::time::Instant::now());
            }
        }
        if let Err(e) = pool.send_decision_request(agent_id, request) {
            logging::warn_event(
                "decision_layer.idle_trigger_failed",
                "failed to trigger decision for idle agent",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "trigger_reason": trigger_reason,
                    "error": e,
                }),
            );
            state.app_mut().push_error_message(format!(
                "Decision trigger failed for {}: {}",
                agent_id.as_str(),
                e
            ));
        } else {
            // Show immediate status message to user that decision is being processed
            state.app_mut().push_status_message(format!(
                "🧠 {}: ⏳ Decision triggered, analyzing situation...",
                agent_id.as_str()
            ));
            logging::debug_event(
                "decision_layer.idle_triggered",
                "decision layer triggered for idle agent",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "trigger_reason": trigger_reason,
                    "situation_type": "agent_idle",
                }),
            );
        }
    }
}

fn handle_agent_thread_finished(
    state: &mut TuiState,
    agent_id: agent_core::agent_runtime::AgentId,
    outcome: agent_core::agent_slot::ThreadOutcome,
) {
    let should_flush_summary = state
        .agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(&agent_id))
        .is_some_and(|slot| {
            slot.status().is_active()
                && matches!(outcome, agent_core::agent_slot::ThreadOutcome::NormalExit)
        });
    if should_flush_summary {
        push_overview_assistant_summary(state, &agent_id);
    }

    if let Some(pool) = state.agent_pool.as_mut()
        && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
    {
        slot.clear_provider_thread();
        match outcome {
            agent_core::agent_slot::ThreadOutcome::NormalExit => {
                if slot.status().is_active() {
                    let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::idle());
                }
            }
            agent_core::agent_slot::ThreadOutcome::ErrorExit { error } => {
                if !slot.status().is_blocked() {
                    let _ =
                        slot.transition_to(agent_core::agent_slot::AgentSlotStatus::blocked(error));
                }
            }
            agent_core::agent_slot::ThreadOutcome::Cancelled => {
                let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::stopped(
                    "cancelled",
                ));
            }
        }
    }
    state.unregister_agent_channel(&agent_id);
}

fn handle_agent_channel_disconnect(
    state: &mut TuiState,
    agent_id: agent_core::agent_runtime::AgentId,
) {
    let should_flush_summary = state
        .agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(&agent_id))
        .is_some_and(|slot| slot.status().is_active());
    if should_flush_summary {
        push_overview_assistant_summary(state, &agent_id);
    }

    if let Some(pool) = state.agent_pool.as_mut()
        && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
    {
        slot.clear_provider_thread();
        if slot.status().is_active() {
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::idle());
        }
    }
    state.unregister_agent_channel(&agent_id);
    state
        .app_mut()
        .push_status_message(format!("{} disconnected", agent_id.as_str()));
}

/// Get current time as HH:MM:SS packed into u32
fn current_time_as_u32() -> u32 {
    let now = chrono::Local::now();
    (now.hour() * 10000) + (now.minute() * 100) + now.second()
}

#[cfg(test)]
mod tests {
    use super::event_poll_timeout;
    use super::execute_provider_invocation;
    use super::handle_agent_channel_disconnect;
    use super::handle_agent_provider_event;
    use super::handle_command_submission;
    use super::handle_multi_agent_submission;
    use super::handle_provider_terminal_error;
    use super::is_session_expired_error;
    use super::shutdown_tui_state;
    use crate::ui_state::TuiState;
    use agent_core::app::AppStatus;
    use agent_core::app::TranscriptEntry;
    use agent_core::command_bus::model::{CommandInvocation, CommandNamespace, CommandTargetSpec};
    use agent_core::ProviderKind;
    use agent_core::SessionHandle;
    use agent_core::runtime_session::RuntimeSession;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn provider_terminal_error_finalizes_active_entries_and_marks_idle() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().begin_provider_response();
        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );

        handle_provider_terminal_error(&mut state, "provider crashed".to_string())
            .expect("handle error");

        assert!(state.active_tool_is_empty());
        assert_eq!(state.app().status, AppStatus::Idle);
        assert!(state.app().active_task_had_error);
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::ExecCommand {
                    call_id,
                    status: agent_core::ExecCommandStatus::Failed,
                    ..
                } if call_id.as_deref() == Some("call-1")
            )
        }));
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Error(text)) if text == "provider crashed"
        ));
    }

    #[test]
    fn session_expired_error_clears_session_and_shows_friendly_message() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().begin_provider_response();
        // Set a session ID that will be cleared
        state
            .app_mut()
            .apply_session_handle(SessionHandle::ClaudeSession {
                session_id: "expired-session-123".to_string(),
            });

        handle_provider_terminal_error(
            &mut state,
            "No conversation found with session ID: expired-session-123".to_string(),
        )
        .expect("handle error");

        // Session should be cleared
        assert!(state.app().claude_session_id.is_none());
        assert!(state.app().codex_thread_id.is_none());
        // Should have exactly one error message (the friendly one)
        assert_eq!(state.app().transcript.len(), 1);
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Error(text)) if text.contains("session expired")
        ));
    }

    #[test]
    fn is_session_expired_error_detects_known_patterns() {
        assert!(is_session_expired_error(
            "No conversation found with session ID: abc123"
        ));
        assert!(is_session_expired_error("No conversation found"));
        assert!(is_session_expired_error("thread not found"));
        assert!(is_session_expired_error("session not found"));
        assert!(!is_session_expired_error("provider crashed"));
        assert!(!is_session_expired_error("network error"));
    }

    #[test]
    fn pending_stream_commits_keep_fast_poll_timeout_even_without_provider_channel() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.append_active_assistant_chunk("line 1\n");

        assert_eq!(event_poll_timeout(&state, false), Duration::from_millis(80));
        assert_eq!(event_poll_timeout(&state, true), Duration::from_millis(80));
    }

    #[test]
    fn multi_agent_mode_triggers_event_aggregator_flow() {
        use super::start_provider_request;
        use std::sync::mpsc;

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn an agent to activate multi-agent mode
        // This creates OVERVIEW at index 0 (focused) + worker at index 1
        let agent_id = state
            .spawn_agent(ProviderKind::Claude)
            .expect("spawn agent");

        // Focus the spawned worker agent so events go to it
        state.focus_agent(&agent_id);

        assert!(state.is_multi_agent_mode());

        // Start provider request - should use multi-agent flow
        let mut provider_rx: Option<mpsc::Receiver<agent_core::ProviderEvent>> = None;
        start_provider_request(&mut state, "hello".to_string(), &mut provider_rx);

        // In multi-agent mode, provider_rx should NOT be set (events go through EventAggregator)
        assert!(
            provider_rx.is_none(),
            "multi-agent mode should not use provider_rx"
        );

        // EventAggregator should have a registered channel
        assert_eq!(
            state.agent_channel_count(),
            1,
            "should have one registered channel"
        );

        // The channel should be for the spawned agent (either empty or has events)
        let registered = state.poll_agent_events();
        assert!(
            registered.empty_channels.contains(&agent_id),
            "spawned agent should have an empty channel registered"
        );
    }

    #[test]
    fn routed_direct_prompt_targets_named_agent_without_changing_focus() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let alpha_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        let bravo_id = state.spawn_agent(ProviderKind::Mock).expect("spawn bravo");
        let bravo_codename = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&bravo_id))
            .map(|slot| slot.codename().as_str().to_string())
            .expect("bravo codename");

        assert_eq!(state.focused_agent_codename(), "OVERVIEW");

        assert!(handle_multi_agent_submission(
            &mut state,
            format!("@{bravo_codename} investigate this"),
        ));

        assert_eq!(state.focused_agent_codename(), "OVERVIEW");
        assert_eq!(state.agent_channel_count(), 1);
        let poll_result = state.poll_agent_events();
        assert!(poll_result.empty_channels.contains(&bravo_id));
        let alpha_slot = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&alpha_id))
            .expect("alpha slot");
        assert!(alpha_slot.transcript().is_empty());
        let bravo_slot = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&bravo_id))
            .expect("bravo slot");
        assert!(bravo_slot.transcript().iter().any(|entry| {
            matches!(entry, TranscriptEntry::User(text) if text == "investigate this")
        }));
    }

    #[test]
    fn routed_broadcast_prompt_starts_all_named_agents() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let alpha_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        let bravo_id = state.spawn_agent(ProviderKind::Mock).expect("spawn bravo");
        let codenames = state
            .agent_pool
            .as_ref()
            .map(|pool| {
                [alpha_id.clone(), bravo_id.clone()]
                    .into_iter()
                    .map(|id| {
                        pool.get_slot_by_id(&id)
                            .expect("slot")
                            .codename()
                            .as_str()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
            })
            .expect("pool");

        assert!(handle_multi_agent_submission(
            &mut state,
            format!("@{},{} sync now", codenames[0], codenames[1]),
        ));

        assert_eq!(state.agent_channel_count(), 2);
        let poll_result = state.poll_agent_events();
        assert!(poll_result.empty_channels.contains(&alpha_id));
        assert!(poll_result.empty_channels.contains(&bravo_id));
    }

    #[test]
    fn agent_events_record_overview_logs_even_outside_overview_mode() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        state.focus_agent(&agent_id);
        state.view_state.switch_by_number(1); // Focused mode

        handle_agent_provider_event(
            &mut state,
            agent_id.clone(),
            agent_core::ProviderEvent::Status("working".to_string()),
        );

        assert_eq!(state.view_state.overview.log_buffer.len(), 1);
        assert_eq!(
            state.view_state.overview.log_buffer.back().unwrap().content,
            "working"
        );
    }

    #[test]
    fn agent_error_event_marks_slot_blocked() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        state.focus_agent(&agent_id);

        handle_agent_provider_event(
            &mut state,
            agent_id.clone(),
            agent_core::ProviderEvent::Error("need human input".to_string()),
        );

        let slot = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&agent_id))
            .expect("slot");
        assert!(slot.status().is_blocked());
        assert!(state.view_state.overview.log_buffer.back().is_some_and(
            |msg| msg.message_type == crate::overview_state::OverviewMessageType::Blocked
        ));
    }

    #[test]
    fn overview_assistant_output_is_recorded_in_scroll_log() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.ensure_overview_agent();
        let overview_id = state.focused_agent_id().expect("overview id");
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&overview_id)
        {
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting())
                .expect("set starting");
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now())
                .expect("set responding");
        }

        handle_agent_provider_event(
            &mut state,
            overview_id.clone(),
            agent_core::ProviderEvent::AssistantChunk(
                "Overview reply content".to_string(),
            ),
        );
        handle_agent_provider_event(
            &mut state,
            overview_id,
            agent_core::ProviderEvent::Finished,
        );

        assert!(
            state
                .view_state
                .overview
                .log_buffer
                .iter()
                .any(|msg| msg.content.contains("Overview reply content"))
        );
    }

    #[test]
    fn overview_logs_completed_assistant_message_once_instead_of_per_chunk() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.ensure_overview_agent();
        let overview_id = state.focused_agent_id().expect("overview id");
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&overview_id)
        {
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting())
                .expect("set starting");
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now())
                .expect("set responding");
        }

        handle_agent_provider_event(
            &mut state,
            overview_id.clone(),
            agent_core::ProviderEvent::AssistantChunk("I".to_string()),
        );
        handle_agent_provider_event(
            &mut state,
            overview_id.clone(),
            agent_core::ProviderEvent::AssistantChunk("’m".to_string()),
        );
        handle_agent_provider_event(
            &mut state,
            overview_id.clone(),
            agent_core::ProviderEvent::AssistantChunk(" loading".to_string()),
        );

        assert_eq!(
            state.view_state.overview.log_buffer.len(),
            0,
            "stream deltas should not appear as standalone overview log entries"
        );

        handle_agent_provider_event(
            &mut state,
            overview_id,
            agent_core::ProviderEvent::Finished,
        );

        let assistant_logs: Vec<_> = state
            .view_state
            .overview
            .log_buffer
            .iter()
            .filter(|msg| msg.content.contains("I’m loading"))
            .collect();
        assert_eq!(assistant_logs.len(), 1);
    }

    #[test]
    fn focused_work_agent_routes_exec_events_through_codex_style_active_and_history_views() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Codex, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let agent_id = state.spawn_agent(ProviderKind::Codex).expect("spawn alpha");
        state.focus_agent(&agent_id);
        state.view_state.switch_by_number(1); // Focused mode

        handle_agent_provider_event(
            &mut state,
            agent_id.clone(),
            agent_core::ProviderEvent::ExecCommandStarted {
                call_id: Some("call-1".to_string()),
                input_preview: Some("git status".to_string()),
                source: Some("agent".to_string()),
            },
        );

        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: agent_core::ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));

        handle_agent_provider_event(
            &mut state,
            agent_id.clone(),
            agent_core::ProviderEvent::ExecCommandOutputDelta {
                call_id: Some("call-1".to_string()),
                delta: "On branch main".to_string(),
            },
        );

        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::ExecCommand {
                output_preview,
                ..
            }) if output_preview.as_deref() == Some("On branch main")
        ));

        handle_agent_provider_event(
            &mut state,
            agent_id,
            agent_core::ProviderEvent::ExecCommandFinished {
                call_id: Some("call-1".to_string()),
                output_preview: None,
                status: agent_core::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(42),
                source: Some("agent".to_string()),
            },
        );

        assert!(state.active_entries_for_display().is_empty());
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::ExecCommand {
                    call_id,
                    output_preview,
                    status: agent_core::ExecCommandStatus::Completed,
                    exit_code: Some(0),
                    duration_ms: Some(42),
                    ..
                } if call_id.as_deref() == Some("call-1")
                    && output_preview.as_deref() == Some("On branch main")
            )
        }));
    }

    #[test]
    fn disconnected_active_agent_is_released_to_idle() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        state.focus_agent(&agent_id);
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
        {
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting())
                .expect("set starting");
            slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now())
                .expect("set responding");
        }
        let (_tx, rx) = std::sync::mpsc::channel();
        state.register_agent_channel(agent_id.clone(), rx);

        handle_agent_channel_disconnect(&mut state, agent_id.clone());

        let slot = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&agent_id))
            .expect("slot");
        assert!(slot.status().is_idle());
        assert_eq!(state.agent_channel_count(), 0);
    }

    #[test]
    fn shutdown_tui_state_saves_resume_artifacts() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.ensure_overview_agent();
        state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        state.view_state.mode = crate::view_mode::ViewMode::Overview;
        state.composer.insert_text("draft after restart");

        shutdown_tui_state(&mut state).expect("shutdown");

        let workplace =
            agent_core::workplace_store::WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        assert!(workplace.has_shutdown_snapshot());
        assert!(
            crate::tui_snapshot::load_resume_snapshot(&workplace)
                .expect("load tui snapshot")
                .is_some()
        );
    }

    #[test]
    fn namespaced_local_command_does_not_fall_through_to_chat() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let mut provider_rx = None;

        let handled =
            handle_command_submission(&mut state, "/local status".to_string(), &mut provider_rx)
                .expect("ok");
        assert!(handled);
        assert!(state.app().transcript.iter().all(|entry| {
            !matches!(entry, TranscriptEntry::User(text) if text == "/local status")
        }));
    }

    #[test]
    fn provider_passthrough_rejection_does_not_fall_through_to_chat() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.ensure_overview_agent();
        let mut provider_rx = None;

        let handled = handle_command_submission(
            &mut state,
            "/provider /status".to_string(),
            &mut provider_rx,
        )
        .expect("ok");
        assert!(handled);
        assert!(state.app().transcript.iter().all(|entry| {
            !matches!(entry, TranscriptEntry::User(text) if text == "/provider /status")
        }));
    }

    #[test]
    fn provider_passthrough_adds_provider_command_label_to_agent_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().selected_provider = ProviderKind::Codex;
        state.ensure_overview_agent();
        let overview_id = state.focused_agent_id().expect("overview id");
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&overview_id)
        {
            slot.set_session_handle(agent_core::SessionHandle::CodexThread {
                thread_id: "thr-provider".to_string(),
            });
        }

        let invocation = CommandInvocation {
            namespace: CommandNamespace::Provider,
            target: Some(CommandTargetSpec::AgentName("overview".to_string())),
            path: vec![],
            args: vec![],
            raw_tail: Some("/status".to_string()),
        };
        execute_provider_invocation(&mut state, invocation).expect("provider command");

        let slot = state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&overview_id))
            .expect("slot");
        assert!(slot.transcript().iter().any(|entry| {
            matches!(entry, TranscriptEntry::Status(text) if text == "provider command: /status")
        }));
    }

    #[test]
    fn quit_command_syncs_to_workplace_immediately() {
        use super::handle_local_command;
        use agent_core::commands::LocalCommand;

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Initially not quitting (should_quit is in workplace, not app)
        assert!(!state.workplace().loop_control.should_quit);

        // Execute quit command
        handle_local_command(&mut state, LocalCommand::Quit);

        // should_quit is now set in workplace
        assert!(state.workplace().loop_control.should_quit);
    }

    #[test]
    fn check_idle_agents_skips_when_no_ready_tasks() {
        use super::check_idle_agents_for_decision;

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn two agents (Claude spawns decision agents automatically)
        let alpha = state.spawn_agent(ProviderKind::Claude).expect("spawn alpha");
        let _bravo = state.spawn_agent(ProviderKind::Claude).expect("spawn bravo");

        // Backlog is empty, no ready tasks
        assert!(state.app().backlog.ready_tasks().is_empty());

        // Call check_idle_agents_for_decision - should return early without triggering
        check_idle_agents_for_decision(&mut state);

        // Verify no decision request was sent (decision agent still idle)
        let pool = state.agent_pool.as_ref().unwrap();
        let decision_agent = pool.decision_agent_for(&alpha).unwrap();
        assert!(decision_agent.is_idle());
        assert_eq!(decision_agent.decision_count(), 0);
    }
}
