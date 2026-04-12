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

use crate::composer::footer::build_footer_line;
use crate::transcript::cells;
use crate::ui_state::TuiState;

pub fn render_app(frame: &mut Frame<'_>, state: &mut TuiState) {
    let composer_height = state.composer.desired_height(frame.area().width, 8);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(composer_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_transcript(frame, state, areas[0]);
    render_composer(frame, state, areas[1]);
    render_footer(frame, state, areas[2]);

    if state.app.skill_browser_open {
        render_skill_browser(frame, state);
    }

    if state.is_overlay_open() {
        render_transcript_overlay(frame, state);
    }
}

fn render_transcript(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    state.transcript_viewport_height = area.height;
    let lines = cells::flatten_cells(&cells::build_cells(&state.app.transcript, area.width));
    let max_scroll = lines.len().saturating_sub(area.height as usize);
    if state.transcript_follow_tail {
        state.transcript_scroll_offset = max_scroll;
    } else if state.transcript_scroll_offset > max_scroll {
        state.transcript_scroll_offset = max_scroll;
    }
    if state.transcript_scroll_offset >= max_scroll {
        state.transcript_follow_tail = true;
    }
    let transcript = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((
        state.transcript_scroll_offset.min(u16::MAX as usize) as u16,
        0,
    ));
    frame.render_widget(transcript, area);
}

fn render_composer(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    state.composer_width = area.width;
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

fn render_transcript_overlay(frame: &mut Frame<'_>, state: &mut TuiState) {
    let Some(overlay) = state.transcript_overlay.as_mut() else {
        return;
    };

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

    let lines = cells::flatten_cells(&cells::build_cells(&state.app.transcript, chunks[1].width));
    let content_height = lines.len();
    let max_scroll = content_height.saturating_sub(chunks[1].height as usize);
    if overlay.scroll_offset > max_scroll {
        overlay.scroll_offset = max_scroll;
    }
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((overlay.scroll_offset.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, chunks[1]);

    let percent = if max_scroll == 0 {
        100
    } else {
        (((overlay.scroll_offset as f32 / max_scroll as f32) * 100.0).round() as u8).min(100)
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

    let title = format!("Skills ({})", state.app.skills.enabled_names.len());

    let mut lines = Vec::new();
    for (index, skill) in state.app.skills.discovered.iter().enumerate() {
        let enabled = state.app.skills.is_enabled(&skill.name);
        let selected = index == state.app.skill_browser_selected;
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
