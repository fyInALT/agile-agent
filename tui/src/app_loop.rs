use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::app::TranscriptEntry;
use agent_core::commands::LocalCommand;
use agent_core::commands::parse_local_command;
use agent_core::logging;
use agent_core::probe;
use agent_core::provider;
use agent_core::provider::ProviderEvent;
use agent_core::runtime_session::RuntimeSession;
use agent_core::task_engine;
use agent_core::task_engine::ExecutionGuardrails;
use agent_core::task_engine::TurnResolution;
use anyhow::Result;
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
use crate::provider_overlay::ProviderSelectionCommand;
use crate::render::render_app;
use crate::resume_overlay::{ResumeCommand, ResumeOverlay};
use crate::terminal::AppTerminal;
use crate::transcript::overlay::OverlayCommand;
use crate::ui_state::TuiState;

/// Interval for periodic persistence flush
const PERSISTENCE_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

pub fn run(terminal: &mut AppTerminal, resume_last: bool) -> Result<AppState> {
    let launch_cwd = env::current_dir()?;

    // Check for shutdown snapshot and show resume dialog if exists
    let effective_resume_last = if resume_last {
        check_resume_snapshot(terminal, &launch_cwd)?
    } else {
        false
    };

    let session = RuntimeSession::bootstrap(
        launch_cwd,
        provider::default_provider(),
        effective_resume_last,
    )?;
    let mut state = TuiState::from_session(session);
    let mut provider_rx: Option<mpsc::Receiver<ProviderEvent>> = None;
    let mut last_flush = Instant::now();

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
                    // Handle provider selection overlay
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
                            state.open_provider_overlay();
                        }
                        InputOutcome::StopFocusedAgent => {
                            let codename = state.focused_agent_codename().to_string();
                            state.open_stop_confirmation(&codename);
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
                            state
                                .app_mut()
                                .push_status_message("showing blocked agents only");
                        }
                        InputOutcome::OverviewFilterRunning => {
                            state.view_state.overview.filter =
                                crate::overview_state::OverviewFilter::RunningOnly;
                            state
                                .app_mut()
                                .push_status_message("showing running agents only");
                        }
                        InputOutcome::OverviewFilterAll => {
                            state.view_state.overview.filter =
                                crate::overview_state::OverviewFilter::All;
                            state.app_mut().push_status_message("showing all agents");
                        }
                        InputOutcome::OverviewPageUp => {
                            state.view_state.overview.page_up(1);
                        }
                        InputOutcome::OverviewPageDown => {
                            state.view_state.overview.page_down(1);
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
                            // Find agent by name and focus it
                            let statuses = state.agent_statuses();
                            if let Some(index) = statuses
                                .iter()
                                .position(|s| s.codename.as_str() == agent_name)
                            {
                                state.view_state.overview.focused_agent_index = index;
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
                            if let Some(command_result) = parse_local_command(&user_input) {
                                match command_result {
                                    Ok(command) => {
                                        logging::debug_event(
                                            "tui.command",
                                            "executed local TUI command",
                                            serde_json::json!({
                                                "command": format!("{:?}", command),
                                            }),
                                        );
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
                                    Err(error) => state.app_mut().push_error_message(error),
                                }
                                continue;
                            }

                            let augmented_prompt =
                                state.workplace().skills.build_injected_prompt(&user_input);
                            logging::debug_event(
                                "tui.submit",
                                "submitted prompt from TUI",
                                serde_json::json!({
                                    "provider": state.app().selected_provider.label(),
                                    "prompt": user_input,
                                    "active_task_id": state.app().active_task_id,
                                }),
                            );
                            state.app_mut().set_loop_phase(LoopPhase::Executing);
                            start_provider_request(&mut state, augmented_prompt, &mut provider_rx);
                        }
                    }
                }
                Event::Paste(text) => handle_paste_event(&mut state, &text),
                Event::Mouse(mouse_event) => {
                    // Handle mouse click in Overview mode for agent selection
                    if state.view_state.mode == crate::view_mode::ViewMode::Overview {
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
                                let statuses = state.agent_statuses();
                                let filter = state.view_state.overview.filter;

                                // Apply filter to get visible agents
                                let filtered: Vec<_> = statuses
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, s)| match filter {
                                        crate::overview_state::OverviewFilter::All => true,
                                        crate::overview_state::OverviewFilter::BlockedOnly => {
                                            s.status.is_blocked()
                                        }
                                        crate::overview_state::OverviewFilter::RunningOnly => {
                                            s.status.is_active()
                                        }
                                    })
                                    .collect();

                                // Select clicked agent
                                let clicked_index = click_row as usize;
                                if clicked_index < filtered.len() {
                                    let (original_index, snapshot) = filtered[clicked_index];
                                    state.view_state.overview.focused_agent_index = clicked_index;
                                    state.app_mut().push_status_message(format!(
                                        "focused {}",
                                        snapshot.codename.as_str()
                                    ));
                                }
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
                        // Generate scroll log message for Overview mode
                        if state.view_state.mode == crate::view_mode::ViewMode::Overview {
                            if let Some(msg) =
                                generate_overview_log_message(&event, &agent_id, &state)
                            {
                                state.view_state.overview.push_log_message(msg);
                            }
                        }

                        // For now, process events from any agent in focused slot
                        // In full implementation, would route to specific agent's transcript
                        match event {
                            ProviderEvent::Status(text) => {
                                state.app_mut().push_status_message(format!(
                                    "{}: {}",
                                    agent_id.as_str(),
                                    text
                                ));
                            }
                            ProviderEvent::AssistantChunk(chunk) => {
                                // If this is the focused agent, append to active display
                                if state.focused_agent_id().as_ref() == Some(&agent_id) {
                                    state.append_active_assistant_chunk(&chunk);
                                }
                                // Also append to agent's transcript in pool
                                if let Some(pool) = state.agent_pool.as_mut()
                                    && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
                                {
                                    slot.append_transcript(TranscriptEntry::Assistant(chunk));
                                }
                            }
                            ProviderEvent::ThinkingChunk(chunk) => {
                                if state.focused_agent_id().as_ref() == Some(&agent_id) {
                                    state.append_active_thinking_chunk(&chunk);
                                }
                            }
                            ProviderEvent::Finished => {
                                if let Some(pool) = state.agent_pool.as_mut()
                                    && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
                                {
                                    let _ = slot.transition_to(
                                        agent_core::agent_slot::AgentSlotStatus::idle(),
                                    );
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
                                    let _ = slot.transition_to(
                                        agent_core::agent_slot::AgentSlotStatus::error(
                                            error.clone(),
                                        ),
                                    );
                                }
                                state.unregister_agent_channel(&agent_id);
                                state.app_mut().push_error_message(format!(
                                    "{} error: {}",
                                    agent_id.as_str(),
                                    error
                                ));
                            }
                            ProviderEvent::SessionHandle(handle) => {
                                if let Some(pool) = state.agent_pool.as_mut()
                                    && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
                                {
                                    slot.set_session_handle(handle);
                                }
                            }
                            // Other events handled similarly
                            _ => {}
                        }
                    }
                    agent_core::event_aggregator::AgentEvent::ThreadFinished {
                        agent_id,
                        outcome,
                    } => {
                        if let Some(pool) = state.agent_pool.as_mut()
                            && let Some(slot) = pool.get_slot_mut_by_id(&agent_id)
                        {
                            slot.clear_provider_thread();
                            match outcome {
                                agent_core::agent_slot::ThreadOutcome::NormalExit => {
                                    let _ = slot.transition_to(
                                        agent_core::agent_slot::AgentSlotStatus::idle(),
                                    );
                                }
                                agent_core::agent_slot::ThreadOutcome::ErrorExit { error } => {
                                    let _ = slot.transition_to(
                                        agent_core::agent_slot::AgentSlotStatus::error(error),
                                    );
                                }
                                agent_core::agent_slot::ThreadOutcome::Cancelled => {
                                    let _ = slot.transition_to(
                                        agent_core::agent_slot::AgentSlotStatus::stopped(
                                            "cancelled",
                                        ),
                                    );
                                }
                            }
                        }
                        state.unregister_agent_channel(&agent_id);
                    }
                    _ => {}
                }
            }

            // Handle disconnected channels
            for disconnected_id in poll_result.disconnected_channels {
                state.unregister_agent_channel(&disconnected_id);
                state
                    .app_mut()
                    .push_status_message(format!("{} disconnected", disconnected_id.as_str()));
            }
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

    state.sync_app_input_from_composer();
    state.session.mark_stopped_and_persist()?;
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
    if let Err(err) = provider::start_provider(
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
    let provider_kind = state.app().selected_provider;
    let focused_id = state.focused_agent_id();

    logging::debug_event(
        "tui.multi_agent_request",
        "starting provider request for focused agent",
        serde_json::json!({
            "provider": provider_kind.label(),
            "focused_agent": focused_id.as_ref().map(|id| id.as_str()).unwrap_or("none"),
            "prompt": prompt,
        }),
    );

    // Start provider for focused agent and get event receiver
    let event_rx = state.start_provider_for_focused_agent(prompt.clone(), provider_kind);

    if let Some(rx) = event_rx {
        // Register the event channel with EventAggregator
        let agent_id = state
            .focused_agent_id()
            .expect("focused agent should exist after start");
        state.register_agent_channel(agent_id.clone(), rx);
        state.app_mut().begin_provider_response();
        state.app_mut().push_status_message(format!(
            "started {} ({})",
            agent_id.as_str(),
            provider_kind.label()
        ));
    } else {
        task_engine::handle_provider_start_failure(
            state.app_mut(),
            "failed to start provider for agent".to_string(),
        );
    }
}

fn handle_provider_terminal_error(state: &mut TuiState, error: String) -> Result<()> {
    state.finalize_active_entries_after_failure(Some(&error));
    state.app_mut().mark_active_task_error();
    state.app_mut().push_error_message(error);
    state.app_mut().finish_provider_response();
    if state.app().active_task_id.is_none() && state.app().loop_phase != LoopPhase::Escalating {
        state.app_mut().set_loop_phase(LoopPhase::Idle);
    }
    state.persist_if_changed()
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

    let Some(todo_id) = state.app().next_ready_todo_id() else {
        return None;
    };

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
                agent_core::tool_calls::ExecCommandStatus::Completed => "Success",
                agent_core::tool_calls::ExecCommandStatus::Failed => "Failed",
                agent_core::tool_calls::ExecCommandStatus::Declined => "Declined",
                agent_core::tool_calls::ExecCommandStatus::InProgress => "In Progress",
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

/// Get current time as HH:MM:SS packed into u32
fn current_time_as_u32() -> u32 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let secs_part = secs % 60;
    (hours * 10000 + mins * 100 + secs_part) as u32
}

#[cfg(test)]
mod tests {
    use super::event_poll_timeout;
    use super::handle_provider_terminal_error;
    use crate::ui_state::TuiState;
    use agent_core::app::AppStatus;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
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
                    status: agent_core::tool_calls::ExecCommandStatus::Failed,
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
        let agent_id = state
            .spawn_agent(ProviderKind::Claude)
            .expect("spawn agent");
        assert!(state.is_multi_agent_mode());

        // Start provider request - should use multi-agent flow
        let mut provider_rx: Option<mpsc::Receiver<agent_core::provider::ProviderEvent>> = None;
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
}
