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
    let composer_height = state.composer.desired_height(frame.area().width, 8);
    let committed_cells = cells::build_cells(&state.app().transcript, frame.area().width);
    let committed_lines = cells::flatten_cells(&committed_cells);
    let committed_constraint = if committed_lines.is_empty() {
        Constraint::Length(0)
    } else {
        Constraint::Min(1)
    };
    let active_entries = state.active_entries_for_display();
    let active_cells = cells::build_active_cells(&active_entries, frame.area().width);
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
            committed_constraint,
            Constraint::Length(active_height),
            Constraint::Length(working_height),
            Constraint::Length(composer_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_transcript(frame, state, areas[0]);
    if active_height > 0 {
        render_active_cells(frame, &active_lines, areas[1]);
    }
    if working_height > 0 {
        render_working_line(frame, state, areas[2]);
    }
    render_composer(frame, state, areas[3]);
    render_footer(frame, state, areas[4]);

    if state.app().skill_browser_open {
        render_skill_browser(frame, state);
    }

    if state.is_overlay_open() {
        render_transcript_overlay(frame, state);
    }
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

    let mut lines = cells::flatten_cells(&cells::build_overlay_cells(
        &state.app().transcript,
        chunks[1].width,
    ));
    let active_revision = state.active_entries_revision_key();
    let active_entries = state.active_entries_for_display();
    let overlay = state.transcript_overlay.as_mut().expect("overlay exists");
    overlay.sync_live_tail(chunks[1].width, active_revision, || {
        cells::flatten_cells(&cells::build_overlay_cells(
            &active_entries,
            chunks[1].width,
        ))
    });
    if !lines.is_empty() && !overlay.live_tail_lines().is_empty() {
        lines.push(Line::from(""));
    }
    lines.extend_from_slice(overlay.live_tail_lines());
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

fn render_skill_browser(frame: &mut Frame<'_>, state: &TuiState) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = format!("Skills ({})", state.app().skills.enabled_names.len());

    let mut lines = Vec::new();
    for (index, skill) in state.app().skills.discovered.iter().enumerate() {
        let enabled = state.app().skills.is_enabled(&skill.name);
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
    use super::build_working_line;
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
        state.active_entries.push(TranscriptEntry::ExecCommand {
            call_id: Some("1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: None,
            output_preview: None,
            status: agent_core::tool_calls::ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

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
        state.active_entries.push(TranscriptEntry::ExecCommand {
            call_id: Some("1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: agent_core::tool_calls::ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

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
    fn footer_surfaces_agent_codename() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        let rendered = line_to_string(&crate::composer::footer::build_footer_line(&state, 120));

        assert!(rendered.contains("alpha"));
        assert!(rendered.contains("mock"));
    }
}
