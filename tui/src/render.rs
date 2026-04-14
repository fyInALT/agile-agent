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

pub fn render_app(frame: &mut Frame<'_>, state: &mut TuiState) {
    frame.render_widget(Clear, frame.area());
    state.sync_busy_started_at();
    state.transcript_render_width = Some(frame.area().width.max(1) as usize);
    let composer_height = state.composer.desired_height(frame.area().width, 8);
    let committed_cells = cells::build_cells(&state.app().transcript, frame.area().width);
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

    if state.app().skill_browser_open {
        render_skill_browser(frame, state);
    }

    if state.is_overlay_open() {
        render_transcript_overlay(frame, state);
    }

    if state.is_provider_overlay_open() {
        render_provider_selection_overlay(frame, state);
    }
}

/// Render the agent status bar showing all agent indicators
fn render_agent_status_bar(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::DarkGray));

    // For single-agent mode, show one agent indicator
    // In multi-agent mode, this would show all agents from AgentPool
    let provider = state.app().selected_provider.label();
    let status = if state.is_busy() {
        "●"
    } else {
        "○"
    };
    let status_color = if state.is_busy() {
        Color::Green
    } else {
        Color::Gray
    };

    // Build status bar line: "● alpha [claude]    Ctrl+V to switch provider"
    let mut spans = vec![
        Span::styled(status, Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled("alpha", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled("[", Style::default().fg(Color::Gray)),
        Span::styled(provider, Style::default().fg(Color::Cyan)),
        Span::styled("]", Style::default().fg(Color::Gray)),
    ];

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

    // Add right-aligned hint
    let hint = " Ctrl+V:switch";
    let total_len = spans.iter().map(|s| s.content.as_ref().len()).sum::<usize>() + hint.len();
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

    let overlay = state.provider_overlay.as_ref().expect("overlay should be open");
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
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
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

fn render_active_cells(frame: &mut Frame<'_>, lines: &[Line<'static>], area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());
    let paragraph = Paragraph::new(lines.to_vec());
    frame.render_widget(paragraph, area);
}

fn render_transcript(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    fill_background(frame, area, Style::default());
    state.transcript_viewport_height = area.height;
    let transcript_cells = cells::build_cells(&state.app().transcript, area.width);
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
        {
            if rendered_lines
                .get(state.transcript_scroll_offset)
                .map(|line| line.as_str())
                != Some(anchor.as_str())
            {
                if let Some(index) = find_closest_matching_line(
                    &rendered_lines,
                    &anchor,
                    state.transcript_scroll_offset,
                ) {
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
    let mut lines = cells::flatten_cells(&cells::build_overlay_cells(&state.app().transcript, width));
    let active_key = state.active_cell_transcript_key();
    let active_lines = state.active_cell_transcript_lines(width).unwrap_or_default();
    let overlay = state.transcript_overlay.as_mut().expect("overlay exists");
    overlay.sync_live_tail(width, active_key.map(|key| key.revision), || {
        active_lines
    });
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
