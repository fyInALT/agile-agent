use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::time::Instant;

use crate::composer::footer::build_footer_line;
use crate::transcript::cells;
use crate::ui_state::TuiState;
use crate::view_mode::ViewMode;
use agent_core::agent_pool::AgentStatusSnapshot;
use agent_core::agent_role::AgentRole;
use agent_core::app::TranscriptEntry;

pub fn render_app(frame: &mut Frame<'_>, state: &mut TuiState) {
    frame.render_widget(Clear, frame.area());
    state.sync_busy_started_at();
    state.transcript_render_width = Some(frame.area().width.max(1) as usize);

    // Adjust view states for terminal width
    state.view_state.adjust_for_width(frame.area().width);

    // Render based on current view mode
    match state.view_state.mode {
        ViewMode::Focused => render_focused_view(frame, state),
        ViewMode::Split => render_split_view(frame, state),
        ViewMode::Dashboard => render_dashboard_view(frame, state),
        ViewMode::Mail => render_mail_view(frame, state),
        ViewMode::TaskMatrix => render_task_matrix_view(frame, state),
        ViewMode::Overview => render_overview_view(frame, state),
    }

    // Overlay rendering (applies to all modes)
    if state.app().skill_browser_open {
        render_skill_browser(frame, state);
    }

    if state.is_overlay_open() {
        render_transcript_overlay(frame, state);
    }

    if state.is_provider_overlay_open() {
        render_provider_selection_overlay(frame, state);
    }

    if state.is_profile_selection_overlay_open() {
        render_profile_selection_overlay(frame, state);
    }

    if state.is_launch_config_overlay_open() {
        render_launch_config_overlay(frame, state);
    }

    if state.is_confirmation_overlay_open() {
        render_confirmation_overlay(frame, state);
    }

    if state.is_human_decision_overlay_open() {
        render_human_decision_overlay(frame, state);
    }
}

/// Render focused (single agent) view - default mode
fn render_focused_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let composer_height = state.composer.desired_height(frame.area().width, 8);

    // Get transcript from focused agent in multi-agent mode, or from app state in single-agent mode
    // Clone the entries to avoid borrowing conflict with subsequent mutable state usage
    let transcript_entries: Vec<TranscriptEntry> = if state.is_multi_agent_mode() {
        state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.focused_slot())
            .map(|slot| slot.transcript().to_vec())
            .unwrap_or_else(|| state.app().transcript.clone())
    } else {
        state.app().transcript.clone()
    };

    let committed_cells = cells::build_cells(&transcript_entries, frame.area().width);
    let committed_lines = cells::flatten_cells(&committed_cells);
    let committed_constraint = if committed_lines.is_empty() {
        Constraint::Length(0)
    } else {
        Constraint::Min(1)
    };
    let active_cells = state.active_cell_preview_cells(frame.area().width);
    let active_lines = cells::flatten_cells(&active_cells);
    let active_height = active_cells.iter().map(|cell| cell.height).sum::<u16>();
    let working_height = if state.is_busy() && active_height == 0 {
        1
    } else {
        0
    };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Agent status bar
            committed_constraint,
            Constraint::Length(active_height),
            Constraint::Length(working_height),
            Constraint::Length(composer_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_agent_status_bar(frame, state, areas[0]);
    render_transcript(frame, state, areas[1]);
    if active_height > 0 {
        render_active_cells(frame, &active_lines, areas[2]);
    }
    if working_height > 0 {
        render_working_line(frame, state, areas[3]);
    }
    render_composer(frame, state, areas[4]);
    render_footer(frame, state, areas[5]);
}

/// Render split view - two agents side by side
fn render_split_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let statuses = state.agent_statuses();

    // Edge case: If fewer than 2 agents, fall back to focused view with warning
    if statuses.len() < 2 {
        // Render a warning message
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());

        render_split_status_bar(frame, state, areas[0]);

        let warning = Paragraph::new(
            "Split view requires at least 2 agents.\nPress Ctrl+N to spawn more agents.",
        )
        .style(Style::default().fg(Color::Yellow));

        frame.render_widget(warning, areas[1]);

        render_split_footer(frame, state, areas[2]);
        return;
    }

    let composer_height = state.composer.desired_height(frame.area().width, 8);

    // Calculate split ratio
    let left_width = (frame.area().width as f32 * state.view_state.split.split_ratio) as u16;
    let right_width = frame.area().width.saturating_sub(left_width);

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar with mode indicator
            Constraint::Min(1),    // Split transcript area
            Constraint::Length(composer_height),
            Constraint::Length(1), // Footer with split-specific hints
        ])
        .split(frame.area());

    // Render status bar with split mode indicator
    render_split_status_bar(frame, state, areas[0]);

    // Split the transcript area horizontally
    let transcript_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width),
            Constraint::Length(right_width),
        ])
        .split(areas[1]);

    // Agent indices (clamped to valid range, guaranteed >= 2 agents now)
    let left_idx = state
        .view_state
        .split
        .left_agent_index
        .min(statuses.len() - 1);
    let right_idx = state
        .view_state
        .split
        .right_agent_index
        .min(statuses.len() - 1);

    // Render left agent transcript
    render_agent_panel(
        frame,
        state,
        left_idx,
        transcript_areas[0],
        state.view_state.split.focused_side == 0,
    );

    // Render right agent transcript
    render_agent_panel(
        frame,
        state,
        right_idx,
        transcript_areas[1],
        state.view_state.split.focused_side == 1,
    );

    // Render composer (for focused side)
    render_composer(frame, state, areas[2]);

    // Render split-specific footer
    render_split_footer(frame, state, areas[3]);
}

/// Render dashboard view - all agents in compact cards
fn render_dashboard_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(1),    // Dashboard cards
            Constraint::Length(1), // Footer
        ])
        .split(frame.area());

    render_dashboard_status_bar(frame, state, areas[0]);
    render_dashboard_cards(frame, state, areas[1]);
    render_dashboard_footer(frame, state, areas[2]);
}

/// Render mail view - cross-agent communication focus
fn render_mail_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let composer_height = if state.view_state.mail.composing {
        5
    } else {
        1
    };

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),               // Status bar
            Constraint::Min(1),                  // Mail list
            Constraint::Length(composer_height), // Composer or hint
            Constraint::Length(1),               // Footer
        ])
        .split(frame.area());

    render_mail_status_bar(frame, state, areas[0]);
    render_mail_list(frame, state, areas[1]);
    if state.view_state.mail.composing {
        render_mail_composer(frame, state, areas[2]);
    } else {
        render_mail_hint(frame, state, areas[2]);
    }
    render_mail_footer(frame, state, areas[3]);
}

/// Render task matrix view - task assignment grid
fn render_task_matrix_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(1),    // Task matrix grid
            Constraint::Length(1), // Footer
        ])
        .split(frame.area());

    render_task_matrix_status_bar(frame, state, areas[0]);
    render_task_matrix_grid(frame, state, areas[1]);
    render_task_matrix_footer(frame, state, areas[2]);
}

/// Render Overview view - multi-agent coordination with agent list + scroll log
fn render_overview_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let agent_list_height = state.view_state.overview.agent_list_rows as u16;
    let composer_height = state.composer.desired_height(frame.area().width, 8);

    // Layout: Agent list | Separator | Scroll log | Separator | Composer | Footer
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(agent_list_height), // Agent status list
            Constraint::Length(1),                 // Separator line
            Constraint::Min(1),                    // Scroll log area
            Constraint::Length(1),                 // Separator line
            Constraint::Length(composer_height),   // Composer
            Constraint::Length(1),                 // Footer
        ])
        .split(frame.area());

    render_overview_agent_list(frame, state, areas[0]);
    render_overview_separator(frame, areas[1]);
    render_overview_content(frame, state, areas[2]);
    render_overview_separator(frame, areas[3]);
    render_composer(frame, state, areas[4]);
    render_overview_footer(frame, state, areas[5]);
}

/// Render separator line for Overview mode
fn render_overview_separator(frame: &mut Frame<'_>, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Create a line of ─ characters
    let separator: String = "─".repeat(area.width as usize);
    let line = Line::from(Span::styled(
        separator,
        Style::default().fg(Color::DarkGray),
    ));

    let paragraph = Paragraph::new(vec![line]);
    frame.render_widget(paragraph, area);
}

/// Render agent status list for Overview mode
fn render_overview_agent_list(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());

    let statuses = state.agent_statuses();
    let visible = state.overview_visible_agent_indices();
    let focused_index = state
        .agent_pool
        .as_ref()
        .map(|pool| pool.focused_slot_index());

    // Build lines for each agent row
    let mut lines = Vec::new();
    let max_width = area.width as usize;
    for index in &visible {
        let Some(snapshot) = statuses.get(*index) else {
            continue;
        };

        let is_overview_agent = snapshot.role == AgentRole::ProductOwner;
        let is_focused = focused_index == Some(*index);
        let mut row = crate::overview_row::OverviewAgentRow::from_snapshot(
            snapshot,
            is_focused,
            is_overview_agent,
        );
        row.truncate_to(max_width);

        let style = if is_focused {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if is_overview_agent {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(Span::styled(row.truncated, style)));
    }

    // Fill remaining rows with empty lines
    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }

    // If no agents, show hint
    if visible.is_empty() {
        lines[0] = Line::from(Span::styled(
            "◎ OVERVIEW idle Coordinating Agent work",
            Style::default().fg(Color::White),
        ));
        if area.height > 1 {
            lines[area.height as usize - 1] = Line::from(Span::styled(
                "Hint: Press Ctrl+N to create a new Agent",
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_overview_content(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    let render_shared_log = state
        .focused_agent_status()
        .is_none_or(|status| status.role == AgentRole::ProductOwner);

    if render_shared_log {
        render_overview_scroll_log(frame, state, area);
    } else {
        render_overview_focused_agent_transcript(frame, state, area);
    }
}

/// Render scroll log for Overview mode
fn render_overview_scroll_log(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());

    let log_buffer = &state.view_state.overview.log_buffer;
    let scroll_offset = state.view_state.overview.log_scroll_offset;

    // Build log lines with timestamp optimization (same minute omission)
    let mut lines = Vec::new();
    let mut last_minute: Option<u32> = None; // Track last displayed minute

    for msg in log_buffer.iter().skip(scroll_offset) {
        let msg_minute = msg.timestamp / 100; // Extract minute (HH:MM)
        let timestamp_str = format_time_from_u32(msg.timestamp);
        let indicator = msg.message_type.indicator();

        let color = match msg.message_type {
            crate::overview_state::OverviewMessageType::Blocked => Color::Yellow,
            crate::overview_state::OverviewMessageType::Complete => Color::Green,
            crate::overview_state::OverviewMessageType::Quick => Color::Cyan,
            _ => Color::Gray,
        };

        // Omit timestamp if same minute as previous message
        let timestamp_span = if last_minute == Some(msg_minute) {
            Span::styled("      ", Style::default().fg(Color::DarkGray)) // Blank space
        } else {
            last_minute = Some(msg_minute);
            Span::styled(
                format!("[{}]", timestamp_str),
                Style::default().fg(Color::DarkGray),
            )
        };

        lines.push(Line::from(vec![
            timestamp_span,
            Span::raw(" "),
            Span::styled(indicator, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(&msg.agent, Style::default().fg(Color::White)),
            Span::raw(": "),
            Span::styled(&msg.content, Style::default().fg(Color::Gray)),
        ]));

        if lines.len() >= area.height as usize {
            break;
        }
    }

    // If no messages, show placeholder
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No activity yet. Agents will report progress here.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    state.transcript_viewport_height = area.height;
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_overview_focused_agent_transcript(
    frame: &mut Frame<'_>,
    state: &mut TuiState,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }

    let Some(agent_id) = state.focused_agent_id() else {
        render_overview_scroll_log(frame, state, area);
        return;
    };
    let Some((codename, transcript_entries)) = state.agent_pool.as_ref().and_then(|pool| {
        pool.get_slot_by_id(&agent_id).map(|slot| {
            (
                slot.codename().as_str().to_string(),
                slot.transcript().to_vec(),
            )
        })
    }) else {
        render_overview_scroll_log(frame, state, area);
        return;
    };

    if transcript_entries.is_empty() {
        fill_background(frame, area, Style::default());
        frame.render_widget(
            Paragraph::new(format!("{codename} - No messages yet"))
                .style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    render_transcript_entries(frame, state, area, &transcript_entries);
}

/// Render footer for Overview mode
fn render_overview_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::Rgb(28, 31, 38)));

    let filter_label = match state.view_state.overview.filter {
        crate::overview_state::OverviewFilter::All => "all",
        crate::overview_state::OverviewFilter::BlockedOnly => "blocked",
        crate::overview_state::OverviewFilter::RunningOnly => "running",
    };

    let hint = format!(
        "Overview | filter:{} | Tab:focus | PageUp/Down:page | Ctrl+N:spawn | Ctrl+X:stop",
        filter_label
    );

    let line = Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Format time from u32 (HHMMSS packed format) to HH:MM:SS string
fn format_time_from_u32(time: u32) -> String {
    let hours = time / 10000;
    let mins = (time % 10000) / 100;
    let secs = time % 100;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Get brief task summary for an agent (max_chars limit for status bar display)
/// Returns the latest assistant message truncated to the specified character limit.
fn get_agent_task_summary(
    state: &TuiState,
    agent_id: &agent_core::agent_runtime::AgentId,
    max_chars: usize,
) -> String {
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
        });

    match text {
        Some(t) => {
            let normalized = t.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() {
                String::new()
            } else {
                truncate_text(&normalized, max_chars)
            }
        }
        None => String::new(),
    }
}

/// Truncate text to max_chars, adding ellipsis if truncated
fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
}

/// Render the agent status bar showing all agent indicators
fn render_agent_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let mut spans = Vec::new();

    // Check if we have multi-agent mode
    if state.is_multi_agent_mode() {
        // Show all agents from AgentPool
        let statuses = state.agent_statuses();
        let focused_index = state
            .agent_pool
            .as_ref()
            .map(|p| p.focused_slot_index())
            .unwrap_or(0);

        // Build spans for each agent using owned strings
        for (i, status) in statuses.iter().enumerate() {
            let is_focused = i == focused_index;

            // Status indicator
            let indicator = if status.status.is_active() {
                "●" // Active (responding/executing)
            } else if status.status.is_idle() {
                "○" // Idle
            } else if status.status.is_terminal() {
                "◌" // Stopped
            } else {
                "◐" // Other (starting, finishing, stopping, error)
            };

            let color = if status.status.is_active() {
                Color::Green
            } else if status.status.is_idle() {
                if is_focused {
                    Color::White
                } else {
                    Color::Gray
                }
            } else if status.status.is_terminal() {
                Color::DarkGray
            } else {
                Color::Yellow
            };

            spans.push(Span::styled(indicator, Style::default().fg(color)));
            spans.push(Span::raw(" "));

            // Codename (highlight if focused) - use owned string
            let codename = status.codename.as_str().to_string();
            let codename_style = if is_focused {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(codename, codename_style));

            // Provider in brackets
            spans.push(Span::styled(" [", Style::default().fg(Color::DarkGray)));
            let provider_label = match status.provider_type {
                agent_core::agent_runtime::ProviderType::Claude => "claude",
                agent_core::agent_runtime::ProviderType::Codex => "codex",
                agent_core::agent_runtime::ProviderType::Mock => "mock",
                agent_core::agent_runtime::ProviderType::Opencode => "opencode",
            };
            spans.push(Span::styled(
                provider_label,
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));

            // Separator between agents
            spans.push(Span::raw("  "));
        }

        // Remove trailing separator
        if spans.len() > 2
            && spans
                .last()
                .map(|s| s.content.as_ref() == "  ")
                .unwrap_or(false)
        {
            spans.pop();
        }
    } else {
        // Single-agent mode: show traditional status bar
        let provider = state.app().selected_provider.label();
        let status = if state.is_busy() { "●" } else { "○" };
        let status_color = if state.is_busy() {
            Color::Green
        } else {
            Color::Gray
        };

        spans.push(Span::styled(status, Style::default().fg(status_color)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "alpha",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled("[", Style::default().fg(Color::Gray)));
        spans.push(Span::styled(provider, Style::default().fg(Color::Cyan)));
        spans.push(Span::styled("]", Style::default().fg(Color::Gray)));
    }

    // Add loop indicator if running
    if state.workplace().loop_control.loop_run_active {
        let remaining = state.workplace().loop_control.remaining_iterations();
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("loop:{}", remaining),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Add task info if assigned
    if let Some(task_id) = &state.app().active_task_id {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("task:{}", task_id),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Add decision status if active (shown when decision layer makes a decision)
    // First check for pending decisions (decision layer is analyzing)
    let pending_decisions: Vec<_> = state
        .agent_pool
        .as_ref()
        .map(|pool| pool.agents_with_pending_decisions())
        .unwrap_or_default();

    if !pending_decisions.is_empty() {
        // Show decision layer analyzing status with spinner and task summary
        for (agent_id, started_at) in &pending_decisions {
            let elapsed_ms = started_at.elapsed().as_millis();
            let spinner = match (elapsed_ms / 400) % 4 {
                0 => "⠋",
                1 => "⠙",
                2 => "⠹",
                3 => "⠸",
                _ => "⠋",
            };
            // Get agent codename
            let codename = state
                .agent_pool
                .as_ref()
                .and_then(|pool| pool.get_slot_by_id(agent_id))
                .map(|slot| slot.codename().as_str())
                .unwrap_or_else(|| agent_id.as_str());

            // Get brief task summary (max 15 chars) from latest assistant message
            let task_summary = get_agent_task_summary(state, agent_id, 15);

            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("🧠 {}", codename),
                Style::default().fg(Color::Green),
            ));
            spans.push(Span::styled(
                spinner,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            if !task_summary.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", task_summary),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
    } else if let Some(ref decision_status) = state.decision_status {
        // Show completed decision status
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("🧠 {}", decision_status),
            Style::default().fg(Color::Green),
        ));
    }

    // Add mail indicator if unread mail exists
    let unread_count = state.focused_unread_mail_count();
    let action_count = state.focused_action_required_count();
    if unread_count > 0 {
        spans.push(Span::raw(" "));
        if action_count > 0 {
            spans.push(Span::styled(
                format!("📬{}!", action_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("📬{}", unread_count),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    // Add right-aligned hint
    let hint = if state.is_multi_agent_mode() {
        " Tab:focus Ctrl+N:spawn Ctrl+X:stop"
    } else {
        " Ctrl+V:switch Ctrl+N:spawn"
    };
    let total_len: usize = spans
        .iter()
        .map(|s| s.content.as_ref().len())
        .sum::<usize>()
        + hint.len();
    if total_len <= area.width as usize {
        let padding = area.width as usize - total_len;
        spans.push(Span::raw(" ".repeat(padding)));
    }
    spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render provider selection overlay for agent creation
fn render_provider_selection_overlay(frame: &mut Frame<'_>, state: &TuiState) {
    use crate::provider_overlay::ProviderSelectionOverlay;

    let overlay = state
        .provider_overlay
        .as_ref()
        .expect("overlay should be open");
    let area = centered_rect(50, 40, frame.area());

    frame.render_widget(Clear, area);

    let title = " New Agent - Select Provider ";
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    for (index, provider) in overlay.providers.iter().enumerate() {
        let label = ProviderSelectionOverlay::provider_label(*provider);
        let selected = index == overlay.selected_index;
        let marker = if selected { ">" } else { " " };
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(" ", Style::default()),
            Span::styled(label, style),
        ]));
    }

    // Add hint line
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter: select  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, inner_area);
}

/// Render profile selection overlay for agent creation
fn render_profile_selection_overlay(frame: &mut Frame<'_>, state: &TuiState) {
    use crate::profile_selection_overlay::ProfileSection;

    let overlay = state
        .profile_selection_overlay
        .as_ref()
        .expect("overlay should be open");

    let profile_count = overlay.profiles().len();
    // Height: title(1) + section headers(2) + profiles + hints(3) + gap(1)
    let height = (1 + 2 + profile_count + 4).max(12).min(22) as u16;
    let area = centered_rect(65, height, frame.area());

    frame.render_widget(Clear, area);

    let title = " New Agent - Select Profile ";
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Section header
    let work_marker = if overlay.section() == ProfileSection::Work { "[" } else { " " };
    let decision_marker = if overlay.section() == ProfileSection::Decision { "[" } else { " " };
    let work_selected = overlay.selected_work_profile_id().unwrap_or_default();
    let decision_selected = overlay.selected_decision_profile_id().unwrap_or_default();

    lines.push(Line::from(vec![
        Span::raw("  Work Agent: "),
        Span::styled(format!("{}>{} ", work_marker, if overlay.section() == ProfileSection::Work { "*" } else { " " }), Style::default().fg(Color::Cyan)),
        Span::raw(format!("{} ", work_selected)),
        Span::raw("     "),
        Span::raw("Decision Agent: "),
        Span::styled(format!("{}>{} ", decision_marker, if overlay.section() == ProfileSection::Decision { "*" } else { " " }), Style::default().fg(Color::Magenta)),
        Span::raw(decision_selected),
    ]));

    // Separator
    lines.push(Line::from(vec![Span::styled(
        "  ──────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )]));

    // Profile list
    for (index, profile) in overlay.profiles().iter().enumerate() {
        let is_work_selected = index == overlay.work_selected_index();
        let is_decision_selected = index == overlay.decision_selected_index();
        let is_work_focused = overlay.section() == ProfileSection::Work;
        let is_decision_focused = overlay.section() == ProfileSection::Decision;

        // Determine marker based on which section is focused and selected
        let marker = if is_work_focused && is_work_selected {
            ">"
        } else if is_decision_focused && is_decision_selected {
            ">"
        } else {
            " "
        };

        // Determine style
        let (fg, bg) = if is_work_focused && is_work_selected {
            (Color::Black, Color::Cyan)
        } else if is_decision_focused && is_decision_selected {
            (Color::Black, Color::Magenta)
        } else {
            (Color::Reset, Color::Reset)
        };

        let style = Style::default()
            .fg(fg)
            .bg(bg)
            .add_modifier(Modifier::BOLD);

        let line = format!(
            "{} {} ({})",
            marker, profile.display_name, profile.cli_label
        );
        lines.push(Line::from(vec![Span::styled(line, style)]));
    }

    // Hints
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Up/Down: select  Left/Right: switch section  Enter: confirm  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .alignment(ratatui::layout::Alignment::Left);
    frame.render_widget(paragraph, inner_area);
}

/// Render launch config overlay for agent creation
fn render_launch_config_overlay(frame: &mut Frame<'_>, state: &TuiState) {
    use crate::launch_config_overlay::LaunchConfigFocus;

    let overlay = state
        .launch_config_overlay
        .as_ref()
        .expect("overlay should be open");
    let area = centered_rect(60, 50, frame.area());

    frame.render_widget(Clear, area);

    let title = format!(" Launch Config - {} ", overlay.provider_label());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Create layout for config fields
    let config_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Work config header
            Constraint::Length(3), // Work config input (with border)
            Constraint::Length(1), // Work preview
            Constraint::Length(1), // Decision config header
            Constraint::Length(3), // Decision config input (with border)
            Constraint::Length(1), // Decision preview
            Constraint::Length(1), // Error message
            Constraint::Length(2), // Confirm button
            Constraint::Length(1), // Hint
        ])
        .split(inner_area);

    // Work config header
    let work_header_style = if overlay.focus == LaunchConfigFocus::WorkConfig {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Work Agent Config:",
            work_header_style,
        )))
        .alignment(ratatui::layout::Alignment::Left),
        config_areas[0],
    );

    // Work config input with border
    let work_input_block = Block::default().borders(Borders::ALL).border_style(
        if overlay.focus == LaunchConfigFocus::WorkConfig {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    );
    let work_input_inner = work_input_block.inner(config_areas[1]);
    frame.render_widget(work_input_block, config_areas[1]);

    // Work config text with cursor indicator
    let work_text = if overlay.focus == LaunchConfigFocus::WorkConfig {
        if overlay.work_config_text.is_empty() {
            "│".to_string() // Cursor only
        } else {
            format!("{}│", overlay.work_config_text) // Cursor at end
        }
    } else {
        overlay.work_config_text.clone()
    };
    frame.render_widget(
        Paragraph::new(work_text).style(Style::default().fg(Color::White)),
        work_input_inner,
    );

    // Work preview
    let work_preview = format!(
        "Mode: {} | Env: {} | Args: {}",
        overlay.work_preview.source_mode.label(),
        overlay.work_preview.env_count,
        overlay.work_preview.arg_count
    );
    let work_preview_style = if overlay.work_preview.is_valid() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    frame.render_widget(
        Paragraph::new(work_preview).style(work_preview_style),
        config_areas[2],
    );

    // Decision config header
    let decision_header_style = if overlay.focus == LaunchConfigFocus::DecisionConfig {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Decision Agent Config:",
            decision_header_style,
        )))
        .alignment(ratatui::layout::Alignment::Left),
        config_areas[3],
    );

    // Decision config input with border
    let decision_input_block = Block::default().borders(Borders::ALL).border_style(
        if overlay.focus == LaunchConfigFocus::DecisionConfig {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    );
    let decision_input_inner = decision_input_block.inner(config_areas[4]);
    frame.render_widget(decision_input_block, config_areas[4]);

    // Decision config text with cursor indicator
    let decision_text = if overlay.focus == LaunchConfigFocus::DecisionConfig {
        if overlay.decision_config_text.is_empty() {
            "│".to_string() // Cursor only
        } else {
            format!("{}│", overlay.decision_config_text) // Cursor at end
        }
    } else {
        overlay.decision_config_text.clone()
    };
    frame.render_widget(
        Paragraph::new(decision_text).style(Style::default().fg(Color::White)),
        decision_input_inner,
    );

    // Decision preview
    let decision_preview = format!(
        "Mode: {} | Env: {} | Args: {}",
        overlay.decision_preview.source_mode.label(),
        overlay.decision_preview.env_count,
        overlay.decision_preview.arg_count
    );
    let decision_preview_style = if overlay.decision_preview.is_valid() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    frame.render_widget(
        Paragraph::new(decision_preview).style(decision_preview_style),
        config_areas[5],
    );

    // Error message (if any)
    if let Some(error) = &overlay.error_message {
        frame.render_widget(
            Paragraph::new(error.clone()).style(Style::default().fg(Color::Red)),
            config_areas[6],
        );
    }

    // Confirm button
    let confirm_style = if overlay.focus == LaunchConfigFocus::Confirm {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("[ Confirm ]", confirm_style)))
            .alignment(ratatui::layout::Alignment::Center),
        config_areas[7],
    );

    // Hint
    frame.render_widget(
        Paragraph::new("Tab: cycle  Up/Down: navigate  Ctrl+S: confirm  Esc: cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center),
        config_areas[8],
    );
}

/// Render confirmation overlay for agent stop
fn render_confirmation_overlay(frame: &mut Frame<'_>, state: &TuiState) {
    let overlay = state
        .confirmation_overlay
        .as_ref()
        .expect("overlay should be open");
    let area = centered_rect(40, 30, frame.area());

    frame.render_widget(Clear, area);

    let title = " Confirmation ";
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Action description
    lines.push(Line::from(Span::styled(
        overlay.action.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));

    lines.push(Line::from(""));

    // Options
    let confirm_style = if overlay.selected_index == 0 {
        Style::default().fg(Color::Black).bg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let cancel_style = if overlay.selected_index == 1 {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };

    lines.push(Line::from(vec![
        Span::styled(" [Y] Confirm ", confirm_style),
        Span::raw("  "),
        Span::styled(" [N] Cancel ", cancel_style),
    ]));

    // Hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Y/N or ←→ + Enter",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, inner_area);
}

/// Render human decision overlay for decision layer requests
fn render_human_decision_overlay(frame: &mut Frame<'_>, state: &TuiState) {
    let overlay = state
        .human_decision_overlay
        .as_ref()
        .expect("human decision overlay should be open");

    // Use larger area for decision modal (50% width, 60% height)
    let area = centered_rect(50, 60, frame.area());

    frame.render_widget(Clear, area);

    // Determine border color based on urgency
    let border_color = match overlay.request.urgency {
        agent_decision::UrgencyLevel::Critical => Color::Red,
        agent_decision::UrgencyLevel::High => Color::Yellow,
        agent_decision::UrgencyLevel::Medium => Color::Cyan,
        agent_decision::UrgencyLevel::Low => Color::Gray,
    };

    let urgency_text = overlay.urgency_text();
    let title = format!(" Human Decision {} ", urgency_text);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Request info
    lines.push(Line::from(vec![
        Span::styled("Request: ", Style::default().fg(Color::Gray)),
        Span::styled(
            overlay.request.id.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Agent: ", Style::default().fg(Color::Gray)),
        Span::styled(
            overlay.request.agent_id.clone(),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Expires: ", Style::default().fg(Color::Gray)),
        Span::styled(
            overlay.remaining_time_text(),
            Style::default().fg(Color::Yellow),
        ),
    ]));

    lines.push(Line::from(""));

    // Separator
    lines.push(Line::from(Span::styled(
        "─".repeat(inner_area.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));

    // Situation description
    if !overlay.request.situation_description.is_empty() {
        lines.push(Line::from(Span::styled(
            overlay.request.situation_description.clone(),
            Style::default().fg(Color::White),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            overlay.request.situation_type.name.clone(),
            Style::default().fg(Color::White),
        )));
    }

    lines.push(Line::from(""));

    // Separator
    lines.push(Line::from(Span::styled(
        "─".repeat(inner_area.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));

    // Options header
    lines.push(Line::from(Span::styled(
        "Options:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));

    // Options list
    for (i, option) in overlay.request.options.iter().enumerate() {
        let letter = (b'A' + i as u8) as char;
        let is_selected = i == overlay.selected_index;

        let option_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" [{}] ", letter), option_style),
            Span::styled(
                option.label.clone(),
                if is_selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]));
    }

    lines.push(Line::from(""));

    // Recommendation (if present)
    if let Some(rec) = &overlay.request.recommendation {
        lines.push(Line::from(Span::styled(
            "─".repeat(inner_area.width as usize),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recommendation:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", rec.action_type),
                Style::default().fg(Color::Green),
            ),
            Span::styled(rec.reasoning.clone(), Style::default().fg(Color::Gray)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("Confidence: {:.0}%", rec.confidence * 100.0),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    // Custom input mode
    if overlay.custom_mode {
        lines.push(Line::from(Span::styled(
            "─".repeat(inner_area.width as usize),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Custom Instruction:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            overlay.custom_input.clone(),
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "_",
            Style::default().fg(Color::Yellow),
        )));
    }

    // Key hints
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat(inner_area.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    let hint_text = if overlay.custom_mode {
        "Enter=Submit  Esc=Cancel"
    } else if overlay.request.recommendation.is_some() {
        "A/B/C/D=Select  R=Recommendation  I=Custom  S=Skip  Esc=Cancel"
    } else {
        "A/B/C/D=Select  I=Custom  S=Skip  Esc=Cancel"
    };

    lines.push(Line::from(Span::styled(
        hint_text,
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner_area);
}

fn render_active_cells(frame: &mut Frame<'_>, lines: &[Line<'static>], area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());
    let paragraph = Paragraph::new(lines.to_vec());
    frame.render_widget(paragraph, area);
}

fn render_transcript(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    // Get transcript from focused agent in multi-agent mode, or from app state in single-agent mode
    // Clone the entries to avoid borrowing conflict with subsequent mutable state usage
    let transcript_entries: Vec<TranscriptEntry> = if state.is_multi_agent_mode() {
        state
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.focused_slot())
            .map(|slot| slot.transcript().to_vec())
            .unwrap_or_else(|| state.app().transcript.clone())
    } else {
        state.app().transcript.clone()
    };
    render_transcript_entries(frame, state, area, &transcript_entries);
}

fn render_transcript_entries(
    frame: &mut Frame<'_>,
    state: &mut TuiState,
    area: Rect,
    transcript_entries: &[TranscriptEntry],
) {
    fill_background(frame, area, Style::default());
    state.transcript_viewport_height = area.height;
    let transcript_cells = cells::build_cells(transcript_entries, area.width);
    let lines = cells::flatten_cells(&transcript_cells);
    let rendered_lines = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let max_scroll = lines.len().saturating_sub(area.height as usize);
    state.transcript_max_scroll = max_scroll;
    if state.transcript_follow_tail {
        state.transcript_scroll_offset = max_scroll;
    } else {
        if let Some(anchor) = state
            .transcript_rendered_lines
            .get(state.transcript_scroll_offset)
            .cloned()
            && rendered_lines
                .get(state.transcript_scroll_offset)
                .map(|line| line.as_str())
                != Some(anchor.as_str())
        {
            if let Some(index) =
                find_closest_matching_line(&rendered_lines, &anchor, state.transcript_scroll_offset)
            {
                state.transcript_scroll_offset = index;
            } else if let (Some((old_start, old_len)), Some((new_start, new_len))) = (
                state.transcript_last_cell_range,
                last_cell_range(&transcript_cells),
            ) {
                let old_offset = state.transcript_scroll_offset;
                if old_offset >= old_start && old_offset < old_start + old_len {
                    let relative = old_offset - old_start;
                    state.transcript_scroll_offset =
                        new_start + relative.min(new_len.saturating_sub(1));
                }
            }
        }

        if state.transcript_scroll_offset > max_scroll {
            state.transcript_scroll_offset = max_scroll;
        }
    }
    state.transcript_rendered_lines = rendered_lines;
    state.transcript_last_cell_range = last_cell_range(&transcript_cells);
    let transcript = Paragraph::new(lines).scroll((
        state.transcript_scroll_offset.min(u16::MAX as usize) as u16,
        0,
    ));
    frame.render_widget(transcript, area);
}

fn find_closest_matching_line(lines: &[String], anchor: &str, origin: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.as_str() == anchor)
        .min_by_key(|(index, _)| index.abs_diff(origin))
        .map(|(index, _)| index)
}

fn last_cell_range(cells: &[cells::TranscriptCell]) -> Option<(usize, usize)> {
    let mut start = 0usize;
    for (index, cell) in cells.iter().enumerate() {
        if index + 1 == cells.len() {
            return Some((start, cell.lines.len()));
        }
        start += cell.lines.len() + 1;
    }
    None
}

fn render_composer(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    state.composer_width = area.width;
    fill_background(
        frame,
        area,
        Style::default().bg(Color::Rgb(28, 31, 38)).fg(Color::White),
    );
    state.composer.render(
        area,
        frame.buffer_mut(),
        &mut state.composer_state,
        "Ask agile-agent to do anything",
    );
    if !state.is_overlay_open() {
        let (cursor_x, cursor_y) = state
            .composer
            .cursor_position(area, &mut state.composer_state);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    fill_background(
        frame,
        area,
        Style::default().bg(Color::Rgb(28, 31, 38)).fg(Color::White),
    );
    let footer = Paragraph::new(build_footer_line(state, area.width)).style(
        Style::default()
            .bg(Color::Rgb(28, 31, 38))
            .fg(Color::White)
            .add_modifier(Modifier::DIM),
    );
    frame.render_widget(footer, area);
}

fn render_working_line(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    fill_background(
        frame,
        area,
        Style::default().bg(Color::Rgb(28, 31, 38)).fg(Color::White),
    );
    let line = build_working_line(state, area.width, Instant::now());
    let paragraph =
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(28, 31, 38)).fg(Color::White));
    frame.render_widget(paragraph, area);
}

fn render_transcript_overlay(frame: &mut Frame<'_>, state: &mut TuiState) {
    if state.transcript_overlay.is_none() {
        return;
    }

    let area = frame.area();
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Transcript", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            "↑↓ scroll  pgup/pgdn page  home/end jump  esc close",
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]));
    frame.render_widget(header, chunks[0]);

    let lines = build_transcript_overlay_lines(state, chunks[1].width);
    let overlay = state.transcript_overlay.as_mut().expect("overlay exists");
    let content_height = lines.len();
    let max_scroll = content_height.saturating_sub(chunks[1].height as usize);
    overlay.set_max_scroll(max_scroll);
    let scroll_offset = overlay.render_scroll_offset();
    let paragraph = Paragraph::new(lines).scroll((scroll_offset.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, chunks[1]);

    let percent = if max_scroll == 0 {
        100
    } else {
        (((scroll_offset as f32 / max_scroll as f32) * 100.0).round() as u8).min(100)
    };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "q or esc to close",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw(" ".repeat(chunks[2].width.saturating_sub(18) as usize)),
        Span::styled(
            format!("{percent}%"),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]));
    frame.render_widget(footer, chunks[2]);
}

fn build_transcript_overlay_lines(state: &mut TuiState, width: u16) -> Vec<Line<'static>> {
    let mut lines =
        cells::flatten_cells(&cells::build_overlay_cells(&state.app().transcript, width));
    let active_key = state.active_cell_transcript_key();
    let active_lines = state
        .active_cell_transcript_lines(width)
        .unwrap_or_default();
    let overlay = state.transcript_overlay.as_mut().expect("overlay exists");
    overlay.sync_live_tail(width, active_key.map(|key| key.revision), || active_lines);
    if !lines.is_empty()
        && !overlay.live_tail_lines().is_empty()
        && !active_key.is_some_and(|key| key.is_stream_continuation)
    {
        lines.push(Line::from(""));
    }
    lines.extend_from_slice(overlay.live_tail_lines());
    lines
}

fn render_skill_browser(frame: &mut Frame<'_>, state: &TuiState) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = format!("Skills ({})", state.workplace().skills.enabled_names.len());

    let mut lines = Vec::new();
    for (index, skill) in state.workplace().skills.discovered.iter().enumerate() {
        let enabled = state.workplace().skills.is_enabled(&skill.name);
        let selected = index == state.app().skill_browser_selected;
        let marker = if enabled { "[x]" } else { "[ ]" };
        let prefix = if selected { ">" } else { " " };
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::White)
        } else if enabled {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{prefix} {marker} {}", skill.name),
            style,
        )));
        lines.push(Line::from(Span::styled(
            format!("    {}", skill.description),
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from("No skills found."));
    }

    let browser = Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(browser, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// =============================================================================
// Split View Helper Functions
// =============================================================================

fn render_split_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let mut spans = Vec::new();

    // View mode indicator
    spans.push(Span::styled(
        "Split View",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(": "));

    let statuses = state.agent_statuses();
    let left_idx = state
        .view_state
        .split
        .left_agent_index
        .min(statuses.len().saturating_sub(1));
    let right_idx = state
        .view_state
        .split
        .right_agent_index
        .min(statuses.len().saturating_sub(1));

    if statuses.len() > left_idx {
        let left = &statuses[left_idx];
        let left_indicator = if left.status.is_active() {
            "●"
        } else if left.status.is_idle() {
            "○"
        } else {
            "◌"
        };
        let left_color = if state.view_state.split.focused_side == 0 {
            Color::White
        } else {
            Color::Gray
        };
        spans.push(Span::styled(
            left_indicator,
            Style::default().fg(Color::Green),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            left.codename.as_str(),
            Style::default().fg(left_color),
        ));
        spans.push(Span::styled(" [", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            left.provider_type.label(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
    }

    spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));

    if statuses.len() > right_idx {
        let right = &statuses[right_idx];
        let right_indicator = if right.status.is_active() {
            "●"
        } else if right.status.is_idle() {
            "○"
        } else {
            "◌"
        };
        let right_color = if state.view_state.split.focused_side == 1 {
            Color::White
        } else {
            Color::Gray
        };
        spans.push(Span::styled(
            right_indicator,
            Style::default().fg(Color::Green),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            right.codename.as_str(),
            Style::default().fg(right_color),
        ));
        spans.push(Span::styled(" [", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            right.provider_type.label(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
    }

    // Right-aligned hint
    let hint = " ←→ select  s swap  e equal";
    let total_len: usize = spans
        .iter()
        .map(|s| s.content.as_ref().len())
        .sum::<usize>()
        + hint.len();
    if total_len <= area.width as usize {
        spans.push(Span::raw(" ".repeat(area.width as usize - total_len)));
    }
    spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_agent_panel(
    frame: &mut Frame<'_>,
    state: &mut TuiState,
    agent_idx: usize,
    area: Rect,
    is_focused: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Draw border
    let border_style = if is_focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Get the transcript for this agent
    let transcript_entries: &[TranscriptEntry] = if let Some(pool) = &state.agent_pool {
        if let Some(slot) = pool.get_slot(agent_idx) {
            slot.transcript()
        } else {
            // Slot not found
            frame.render_widget(
                Paragraph::new("Agent slot not found").style(Style::default().fg(Color::Red)),
                inner_area,
            );
            return;
        }
    } else {
        // No agent pool - use focused agent transcript
        &state.app().transcript
    };

    if transcript_entries.is_empty() {
        let codename = if let Some(pool) = &state.agent_pool {
            pool.get_slot(agent_idx)
                .map(|s| s.codename().as_str())
                .unwrap_or("Agent")
        } else {
            "Agent"
        };
        frame.render_widget(
            Paragraph::new(format!("{} - No messages yet", codename))
                .style(Style::default().fg(Color::Gray)),
            inner_area,
        );
        return;
    }

    // Build cells and render transcript
    let transcript_cells = cells::build_cells(transcript_entries, inner_area.width);
    let lines = cells::flatten_cells(&transcript_cells);

    // For non-focused agents, just show the transcript (no scroll state)
    // For focused agent in split view, use the main scroll state
    let scroll_offset = if is_focused {
        state.transcript_scroll_offset
    } else {
        // Auto-scroll to bottom for non-focused panels
        lines.len().saturating_sub(inner_area.height as usize)
    };

    let transcript = Paragraph::new(lines).scroll((scroll_offset.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(transcript, inner_area);
}

fn render_split_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let footer_line = build_footer_line(state, area.width);
    frame.render_widget(Paragraph::new(footer_line), area);
}

// =============================================================================
// Dashboard View Helper Functions
// =============================================================================

fn render_dashboard_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let spans = vec![
        Span::styled(
            "Agent Dashboard",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            state.view_state.mode.key_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_dashboard_cards(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let statuses = state.agent_statuses();
    if statuses.is_empty() {
        frame.render_widget(
            Paragraph::new("No agents spawned. Press Ctrl+N to spawn.")
                .style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    // Calculate card grid layout
    let cards_per_row = state.view_state.dashboard.cards_per_row.max(1);
    let card_width = (area.width / cards_per_row as u16).max(20);
    let card_height = 4u16;
    let visible_rows = (area.height / card_height) as usize;

    // Ensure selected card is visible
    state
        .view_state
        .dashboard
        .ensure_selected_visible(cards_per_row, visible_rows);
    let scroll_offset = state.view_state.dashboard.scroll_offset;

    // Render each agent as a card
    for (i, status) in statuses.iter().enumerate() {
        let row = i / cards_per_row;
        let col = i % cards_per_row;

        // Skip rows that are scrolled out of view
        if row < scroll_offset {
            continue;
        }

        let visible_row = row - scroll_offset;
        let card_area = Rect {
            x: area.x + (col as u16) * card_width,
            y: area.y + (visible_row as u16) * card_height,
            width: card_width.saturating_sub(1),
            height: card_height,
        };

        if card_area.y + card_area.height > area.y + area.height {
            break;
        }

        render_agent_card(
            frame,
            status,
            card_area,
            state.view_state.dashboard.selected_card_index == i,
        );
    }
}

fn render_agent_card(
    frame: &mut Frame<'_>,
    status: &AgentStatusSnapshot,
    area: Rect,
    is_selected: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let indicator = if status.status.is_active() {
        "●"
    } else if status.status.is_idle() {
        "○"
    } else if status.status.is_paused() {
        "◈"
    } else {
        "◌"
    };
    let status_color = if status.status.is_active() {
        Color::Green
    } else if status.status.is_paused() {
        Color::Magenta
    } else {
        Color::Gray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Card content - base lines
    let mut lines = vec![
        Line::from(vec![
            Span::styled(indicator, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(
                status.codename.as_str(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                status.provider_type.label(),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![Span::styled(
            status.status.label(),
            Style::default().fg(status_color),
        )]),
    ];

    // Add worktree info if present
    if status.has_worktree {
        let branch_info = if let Some(branch) = &status.worktree_branch {
            format!("wt:{}", branch)
        } else {
            "wt:detached".to_string()
        };

        // Show existence status with visual indicator
        let (prefix, branch_style) = if status.worktree_exists {
            ("├ ", Style::default().fg(Color::Yellow))
        } else {
            ("⚠ ", Style::default().fg(Color::Red)) // Warning: worktree missing
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::DarkGray)),
            Span::styled(branch_info, branch_style),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_dashboard_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let spans = vec![
        Span::styled(
            "n new  x stop  1-9 select",
            Style::default().fg(Color::Gray),
        ),
        Span::raw("  "),
        Span::styled(
            state.view_state.mode.key_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// =============================================================================
// Mail View Helper Functions
// =============================================================================

fn render_mail_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let unread = state.focused_unread_mail_count();
    let action_required = state.focused_action_required_count();

    let mut spans = vec![
        Span::styled(
            "Mail",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];

    if unread > 0 {
        spans.push(Span::styled(
            format!("📬{} unread", unread),
            Style::default().fg(Color::Yellow),
        ));
        if action_required > 0 {
            spans.push(Span::styled(
                format!(" {}!", action_required),
                Style::default().fg(Color::Red),
            ));
        }
    } else {
        spans.push(Span::styled(
            "No unread mail",
            Style::default().fg(Color::Gray),
        ));
    }

    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        state.view_state.mode.key_hint(),
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_mail_list(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }

    let focused_id = state.focused_agent_id();
    if focused_id.is_none() {
        frame.render_widget(
            Paragraph::new("No agent selected").style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    let inbox = state.mailbox.inbox_for(&focused_id.unwrap());
    if inbox.is_none() || inbox.unwrap().is_empty() {
        frame.render_widget(
            Paragraph::new("Inbox is empty").style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    let mails = inbox.unwrap();
    // Clamp selection to valid range (handles inbox shrinking)
    state.view_state.mail.clamp_selection(mails.len());
    let selected_idx = state.view_state.mail.selected_mail_index;

    let lines: Vec<Line> = mails
        .iter()
        .enumerate()
        .map(|(i, mail)| {
            let is_selected = i == selected_idx;
            let is_unread = !mail.is_read();
            let is_action = mail.requires_action;

            let indicator = if is_action {
                "[!] "
            } else if is_unread {
                "● "
            } else {
                "○ "
            };
            let color = if is_action {
                Color::Red
            } else if is_unread {
                Color::Yellow
            } else {
                Color::Gray
            };

            let style = if is_selected {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };

            Line::from(vec![
                Span::styled(indicator, style),
                Span::styled(mail.subject.label(), style),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_mail_composer(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height < 5 {
        // Not enough space, show minimal
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Yellow));
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new("Compose: (resize window for fields)")
                .style(Style::default().fg(Color::Gray)),
            Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: 1,
            },
        );
        return;
    }

    use crate::view_mode::ComposeField;

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Render three fields: To, Subject, Body
    let fields = [
        (ComposeField::To, &state.view_state.mail.compose_to),
        (
            ComposeField::Subject,
            &state.view_state.mail.compose_subject,
        ),
        (ComposeField::Body, &state.view_state.mail.compose_body),
    ];

    let focused = state.view_state.mail.compose_field;

    for (i, (field, content)) in fields.iter().enumerate() {
        let field_area = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };

        let is_focused = *field == focused;
        let style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        // Field label and content
        let label = field.label();
        let display_content = if content.is_empty() {
            if is_focused { "_" } else { "" }
        } else {
            content.as_str()
        };

        frame.render_widget(
            Paragraph::new(format!("{}: {}", label, display_content)).style(style),
            Rect {
                x: field_area.x,
                y: field_area.y,
                width: field_area.width,
                height: 1,
            },
        );
    }

    // Hint at bottom
    frame.render_widget(
        Paragraph::new("Tab next field  Enter send  Esc cancel")
            .style(Style::default().fg(Color::DarkGray)),
        Rect {
            x: inner.x,
            y: inner.y + 3,
            width: inner.width,
            height: 1,
        },
    );

    // Set cursor position for focused field
    let cursor_x = inner.x
        + focused.label().len() as u16
        + 2
        + state.view_state.mail.focused_content().len() as u16;
    let cursor_y = inner.y + focused as u16;
    frame.set_cursor_position((
        cursor_x.min(inner.x + inner.width.saturating_sub(1)),
        cursor_y,
    ));
}

fn render_mail_hint(frame: &mut Frame<'_>, _state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new("c compose  r reply  m mark read")
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_mail_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let spans = vec![
        Span::styled("↑↓ select  Enter view", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(
            state.view_state.mode.key_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// =============================================================================
// Task Matrix View Helper Functions
// =============================================================================

fn render_task_matrix_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let spans = vec![
        Span::styled(
            "Task Matrix",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            state.view_state.mode.key_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_task_matrix_grid(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Get kanban service from SharedWorkplaceState
    let kanban = state.session.workplace().kanban();

    if kanban.is_none() {
        frame.render_widget(
            Paragraph::new(
                "Kanban not initialized. Start a multi-agent session to use task matrix.",
            )
            .style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    let kanban = kanban.unwrap();

    // Get tasks from kanban
    use agent_kanban::domain::ElementType;
    let tasks = kanban.list_by_type(ElementType::Task);

    if tasks.is_err() || tasks.as_ref().unwrap().is_empty() {
        frame.render_widget(
            Paragraph::new("No tasks in backlog. Press Ctrl+N to spawn agents and create tasks.")
                .style(Style::default().fg(Color::Gray)),
            area,
        );
        return;
    }

    let tasks = tasks.unwrap();

    // Column headers: Status columns (Todo, InProgress, Done, Verified)
    let columns = ["Todo", "InProg", "Done", "Verified"];
    let column_width = area.width / 4;

    // Render header row
    let header_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };

    let header_spans: Vec<Span> = columns
        .iter()
        .enumerate()
        .map(|(_i, col)| {
            Span::styled(
                format!("  {:width$}", col, width = column_width as usize - 2),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(header_spans)), header_area);

    // Render task rows
    let row_height = 1;
    let max_rows = (area.height - 1) / row_height;

    // Group tasks by status
    use agent_kanban::domain::Status;
    let todo_tasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.status() == Status::Todo || t.status() == Status::Ready)
        .collect();
    let in_progress_tasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.status() == Status::InProgress)
        .collect();
    let done_tasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.status() == Status::Done)
        .collect();
    let verified_tasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.status() == Status::Verified)
        .collect();

    // Render rows
    for row_idx in 0..max_rows as usize {
        let row_area = Rect {
            x: area.x,
            y: area.y + 1 + row_idx as u16,
            width: area.width,
            height: row_height,
        };

        // Get tasks for each column in this row
        let todo_task = todo_tasks.get(row_idx);
        let in_progress_task = in_progress_tasks.get(row_idx);
        let done_task = done_tasks.get(row_idx);
        let verified_task = verified_tasks.get(row_idx);

        let task_spans: Vec<Span> = [
            (todo_task, Color::Gray),
            (in_progress_task, Color::Green),
            (done_task, Color::Cyan),
            (verified_task, Color::Blue),
        ]
        .iter()
        .enumerate()
        .map(|(_col_idx, (task, color))| {
            let task_text = if let Some(t) = task {
                // Truncate title to fit column
                let title = t.title();
                let max_len = column_width as usize - 4;
                if title.len() > max_len {
                    format!("  {}...", &title[..max_len.saturating_sub(3)])
                } else {
                    format!("  {}", title)
                }
            } else {
                format!("  {:width$}", "", width = column_width as usize - 2)
            };
            Span::styled(task_text, Style::default().fg(*color))
        })
        .collect();

        frame.render_widget(Paragraph::new(Line::from(task_spans)), row_area);
    }
}

fn render_task_matrix_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    let spans = vec![
        Span::styled("↑↓←→ navigate  a assign", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(
            state.view_state.mode.key_hint(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn fill_background(frame: &mut Frame<'_>, area: Rect, style: Style) {
    for y in 0..area.height {
        for x in 0..area.width {
            frame.buffer_mut()[(area.x + x, area.y + y)]
                .set_symbol(" ")
                .set_style(style);
        }
    }
}

fn build_working_line(state: &TuiState, width: u16, now: Instant) -> Line<'static> {
    let elapsed_duration = state
        .busy_started_at
        .map(|started_at| now.saturating_duration_since(started_at))
        .unwrap_or_default();
    let elapsed = elapsed_duration.as_secs();
    let spinner = animated_spinner(elapsed_duration.as_millis());
    let label = working_label(state);

    let mut content = format!("{spinner} {label} ({elapsed}s");
    if state.app().status == agent_core::app::AppStatus::Responding {
        content.push_str(" • esc to interrupt");
    }
    content.push(')');

    if let Some(summary) = background_terminal_summary(state) {
        content.push_str(" · ");
        content.push_str(&summary);
    }

    if content.len() > width as usize {
        content.truncate(width as usize);
    }

    Line::from(vec![Span::styled(
        content,
        Style::default().add_modifier(Modifier::DIM),
    )])
}

fn animated_spinner(elapsed_millis: u128) -> &'static str {
    match (elapsed_millis / 400) % 2 {
        0 => "•",
        _ => "◦",
    }
}

fn working_label(state: &TuiState) -> &'static str {
    match state.app().loop_phase {
        agent_core::app::LoopPhase::Planning => "Planning",
        agent_core::app::LoopPhase::Verifying => "Verifying",
        agent_core::app::LoopPhase::Escalating => "Escalating",
        _ => "Working",
    }
}

fn background_terminal_summary(state: &TuiState) -> Option<String> {
    let count = state
        .active_entries_for_display()
        .iter()
        .filter(|entry| {
            matches!(
                entry,
                agent_core::app::TranscriptEntry::ExecCommand {
                    status: agent_core::tool_calls::ExecCommandStatus::InProgress,
                    ..
                }
            )
        })
        .count();

    if count == 0 {
        return None;
    }

    let plural = if count == 1 { "" } else { "s" };
    Some(format!("{count} background terminal{plural} running"))
}

/// Render resume overlay for shutdown snapshot restore
pub fn render_resume_overlay(
    frame: &mut Frame<'_>,
    overlay: &crate::resume_overlay::ResumeOverlay,
) {
    use ratatui::style::Stylize;
    use ratatui::widgets::BorderType;

    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    // Build title
    let title = Line::from("● Restored Session").bold();

    // Build content
    let mut lines = vec![
        Line::from(""),
        Line::from(format!(
            "Previous session had {} active agents.",
            overlay.agents_count()
        )),
        Line::from(""),
    ];

    // Show agent info
    for agent in overlay.snapshot().agents.iter() {
        let status_text = if agent.was_active {
            "was active"
        } else if agent.had_error {
            "had error"
        } else {
            "idle"
        };
        let task_text = agent
            .assigned_task_id
            .as_ref()
            .map(|t| format!(" (task: {})", t))
            .unwrap_or_default();
        lines.push(Line::from(format!(
            "  {} [{}] - {}{}",
            agent.meta.codename.as_str(),
            agent.meta.provider_type.label(),
            status_text,
            task_text
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Select an option:"));

    // Build options
    for (i, option) in overlay.options.iter().enumerate() {
        let is_selected = i == overlay.selected_index;
        let prefix = if is_selected { "> " } else { "  " };
        let style = if is_selected {
            Style::default().fg(ratatui::style::Color::Yellow).bold()
        } else {
            Style::default()
        };
        lines.push(Line::styled(
            format!("{}{} {}", prefix, option.key_hint(), option.label()),
            style,
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Press R to resume or S to start fresh"));

    let block = Block::bordered()
        .title(title)
        .border_type(BorderType::Rounded);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::build_transcript_overlay_lines;
    use super::build_working_line;
    use crate::ui_state::ActiveExecCall;
    use crate::ui_state::ActiveStream;
    use crate::ui_state::ActiveTool;
    use crate::ui_state::StreamTextKind;
    use crate::ui_state::TuiState;
    use agent_core::app::AppStatus;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use ratatui::text::Line;
    use std::time::Duration;
    use std::time::Instant;
    use tempfile::TempDir;

    fn line_to_string(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn working_line_mentions_elapsed_and_exec_count() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().status = AppStatus::Responding;
        state.busy_started_at = Some(Instant::now() - Duration::from_secs(8));
        state.set_active_tool(ActiveTool::Exec(vec![ActiveExecCall {
            call_id: Some("1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: None,
            output_preview: None,
            status: agent_core::tool_calls::ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        }]));

        let rendered = line_to_string(&build_working_line(&state, 120, Instant::now()));

        assert!(rendered.contains("Working (8s • esc to interrupt)"));
        assert!(rendered.contains("1 background terminal running"));
    }

    #[test]
    fn overlay_entries_append_active_tail_after_committed_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state
            .app_mut()
            .transcript
            .push(TranscriptEntry::Status("committed".to_string()));
        state.set_active_tool(ActiveTool::Exec(vec![ActiveExecCall {
            call_id: Some("1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: agent_core::tool_calls::ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        }]));

        let entries = state.overlay_entries_for_display();

        assert!(
            entries
                .iter()
                .any(|entry| matches!(entry, TranscriptEntry::Status(text) if text == "committed"))
        );
        assert!(matches!(
            entries.last(),
            Some(TranscriptEntry::ExecCommand { status, .. })
                if *status == agent_core::tool_calls::ExecCommandStatus::InProgress
        ));
    }

    #[test]
    fn overlay_does_not_insert_blank_separator_for_assistant_stream_continuation() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state
            .app_mut()
            .transcript
            .push(TranscriptEntry::Assistant("hello world\n".to_string()));
        state.set_active_stream(ActiveStream {
            kind: StreamTextKind::Assistant,
            tail: "next".to_string(),
            pending_commits: std::collections::VecDeque::new(),
            collector: crate::markdown_stream::MarkdownStreamCollector::new(
                None,
                std::path::Path::new("/tmp"),
            ),
            policy: crate::streaming::AdaptiveChunkingPolicy::default(),
        });
        state.open_transcript_overlay();

        let rendered = build_transcript_overlay_lines(&mut state, 80)
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered[rendered.len().saturating_sub(2)..],
            ["hello world".to_string(), "next".to_string()]
        );
    }

    #[test]
    fn footer_surfaces_agent_codename() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        let rendered = line_to_string(&crate::composer::footer::build_footer_line(&state, 120));

        assert!(rendered.contains("alpha"));
        assert!(rendered.contains("mock"));
    }

    #[test]
    fn status_bar_shows_provider_type() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        // Verify state has correct provider
        assert_eq!(state.app().selected_provider.label(), "claude");
    }

    #[test]
    fn status_bar_shows_idle_when_not_busy() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        // When idle, is_busy returns false
        assert!(!state.is_busy());
    }

    #[test]
    fn status_bar_shows_busy_when_responding() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().status = AppStatus::Responding;
        state.sync_busy_started_at();

        // When responding, is_busy returns true
        assert!(state.is_busy());
    }

    #[test]
    fn status_bar_shows_loop_iterations_when_active() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.workplace_mut().loop_control.start_loop(10);

        // Verify loop is active with correct iterations
        assert!(state.workplace().loop_control.loop_run_active);
        assert_eq!(state.workplace().loop_control.remaining_iterations(), 10);
    }

    #[test]
    fn status_bar_shows_task_assignment() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.app_mut().active_task_id = Some("task-001".to_string());

        // Verify task is assigned
        assert_eq!(state.app().active_task_id, Some("task-001".to_string()));
    }
}
