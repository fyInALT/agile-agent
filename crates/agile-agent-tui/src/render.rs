use agile_agent_core::app::AppState;
use agile_agent_core::app::AppStatus;
use agile_agent_core::app::TranscriptEntry;
use pulldown_cmark::{Event, Parser, Tag, TagEnd, CodeBlockKind};
use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;
use unicode_width::UnicodeWidthChar;

/// Render the full TUI application
pub fn render_app(frame: &mut Frame<'_>, state: &AppState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, state, areas[0]);
    render_transcript(frame, state, areas[1]);
    render_composer(frame, state, areas[2]);
}

fn render_header(frame: &mut Frame<'_>, state: &AppState, area: ratatui::layout::Rect) {
    let status_text = match state.status {
        AppStatus::Idle => "idle",
        AppStatus::Responding => "responding",
    };

    let session_info = match &state.current_session_handle() {
        Some(agile_agent_core::provider::SessionHandle::ClaudeSession { session_id }) => {
            if session_id.len() >= 8 {
                format!(" | session: {}...", &session_id[..8])
            } else {
                format!(" | session: {}", session_id)
            }
        }
        Some(agile_agent_core::provider::SessionHandle::CodexThread { thread_id }) => {
            if thread_id.len() >= 8 {
                format!(" | thread: {}...", &thread_id[..8])
            } else {
                format!(" | thread: {}", thread_id)
            }
        }
        None => String::new(),
    };

    let header = Paragraph::new(Line::from(format!(
        "agile-agent | provider: {} | status: {}{} | tab: switch | ctrl+l: clear | q/esc: quit",
        state.selected_provider.label(),
        status_text,
        session_info
    )))
    .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header, area);
}

fn render_transcript(frame: &mut Frame<'_>, state: &AppState, area: ratatui::layout::Rect) {
    let transcript_lines = if state.transcript.is_empty() {
        vec![Line::from("No messages yet.")]
    } else {
        state
            .transcript
            .iter()
            .flat_map(|entry| render_transcript_entry(entry, area.width as usize))
            .collect()
    };

    let transcript = Paragraph::new(transcript_lines)
        .block(Block::default().title("Transcript").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, area);
}

fn render_transcript_entry(entry: &TranscriptEntry, max_width: usize) -> Vec<Line<'static>> {
    match entry {
        TranscriptEntry::User(text) => {
            vec![Line::from(Span::styled(
                format!("You: {}", text),
                Style::default().fg(Color::Cyan),
            ))]
        }
        TranscriptEntry::Assistant(text) => render_markdown(text, max_width),
        TranscriptEntry::Thinking(text) => {
            let wrapped = wrap_thinking_text(text, max_width.saturating_sub(4));
            wrapped
                .iter()
                .map(|line| {
                    Line::from(Span::styled(
                        format!("💭 {}", line),
                        Style::default().fg(Color::Yellow),
                    ))
                })
                .collect()
        }
        TranscriptEntry::ToolCall {
            name,
            input_preview,
            output_preview,
            success,
            started,
            ..
        } => {
            let icon = if *started { "🔧" } else { if *success { "✓" } else { "✗" } };
            let color = if *started {
                Color::Blue
            } else if *success {
                Color::Green
            } else {
                Color::Red
            };

            let mut lines = Vec::new();

            if let Some(input) = input_preview {
                lines.push(Line::from(Span::styled(
                    format!("{} Tool: {} ({})", icon, name, truncate_preview(input, 50)),
                    Style::default().fg(color),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("{} Tool: {}", icon, name),
                    Style::default().fg(color),
                )));
            }

            if !started && output_preview.is_some() {
                let output = output_preview.as_ref().unwrap();
                lines.push(Line::from(Span::styled(
                    format!("   Output: {}", truncate_preview(output, 80)),
                    Style::default().fg(Color::Gray),
                )));
            }

            lines
        }
        TranscriptEntry::Status(text) => {
            vec![Line::from(Span::styled(
                format!("Status: {}", text),
                Style::default().fg(Color::Gray),
            ))]
        }
        TranscriptEntry::Error(text) => {
            vec![Line::from(Span::styled(
                format!("Error: {}", text),
                Style::default().fg(Color::Red),
            ))]
        }
    }
}

/// Render Markdown text with basic formatting
fn render_markdown(text: &str, max_width: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return vec![Line::from("Assistant: (waiting...)")];
    }

    let mut lines = Vec::new();
    let parser = Parser::new(text);

    let mut current_line_spans: Vec<Span<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    let mut current_style = Style::default();
    let mut _heading_level = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                _heading_level = level as usize;
                current_style = Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD);
                current_line_spans.push(Span::styled(
                    "#".repeat(level as usize) + " ",
                    current_style,
                ));
            }
            Event::End(TagEnd::Heading(_level)) => {
                lines.push(Line::from(current_line_spans.clone()));
                current_line_spans.clear();
                current_style = Style::default();
                _heading_level = 0;
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                if !current_line_spans.is_empty() {
                    // Wrap paragraph text
                    let paragraph_text = spans_to_string(&current_line_spans);
                    let wrapped = wrap_text(&paragraph_text, max_width);
                    for wrapped_line in wrapped {
                        lines.push(Line::from(wrapped_line));
                    }
                    current_line_spans.clear();
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                lines.push(Line::from(Span::styled(
                    format!("```{}", code_block_lang),
                    Style::default().fg(Color::Yellow),
                )));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                // Render code block content
                for code_line in code_block_content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", code_line),
                        Style::default().fg(Color::Green),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "```",
                    Style::default().fg(Color::Yellow),
                )));
                code_block_content.clear();
                code_block_lang.clear();
            }
            Event::Start(Tag::List { .. }) => {}
            Event::End(TagEnd::List { .. }) => {}
            Event::Start(Tag::Item) => {
                current_line_spans.push(Span::styled("- ", Style::default().fg(Color::Blue)));
            }
            Event::End(TagEnd::Item) => {
                if !current_line_spans.is_empty() {
                    let item_text = spans_to_string(&current_line_spans);
                    let wrapped = wrap_text(&item_text, max_width.saturating_sub(2));
                    for wrapped_line in wrapped {
                        lines.push(Line::from(format!("  {}", wrapped_line)));
                    }
                    current_line_spans.clear();
                }
            }
            Event::Start(Tag::Emphasis) => {
                current_style = current_style.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                current_style = current_style.remove_modifier(Modifier::ITALIC);
            }
            Event::Start(Tag::Strong) => {
                current_style = current_style.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                current_style = current_style.remove_modifier(Modifier::BOLD);
            }
            Event::Code(code) => {
                current_line_spans.push(Span::styled(
                    format!("`{}`", code),
                    Style::default().fg(Color::Green),
                ));
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else {
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if !in_code_block && !current_line_spans.is_empty() {
                    let paragraph_text = spans_to_string(&current_line_spans);
                    let wrapped = wrap_text(&paragraph_text, max_width);
                    for wrapped_line in wrapped {
                        lines.push(Line::from(wrapped_line));
                    }
                    current_line_spans.clear();
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                current_style = Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED);
                current_line_spans.push(Span::styled(dest_url.to_string(), current_style));
            }
            Event::End(TagEnd::Link) => {
                current_style = Style::default();
            }
            _ => {}
        }
    }

    // Handle remaining content
    if !current_line_spans.is_empty() {
        let paragraph_text = spans_to_string(&current_line_spans);
        let wrapped = wrap_text(&paragraph_text, max_width);
        for wrapped_line in wrapped {
            lines.push(Line::from(wrapped_line));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(text.to_string()));
    }

    // Prefix first line with "Assistant: "
    if let Some(first_line) = lines.first_mut() {
        let original_spans = first_line.spans.clone();
        *first_line = Line::from(
            std::iter::once(Span::styled("Assistant: ", Style::default().fg(Color::Green)))
                .chain(original_spans.into_iter())
                .collect::<Vec<_>>(),
        );
    } else {
        lines.push(Line::from(Span::styled(
            format!("Assistant: {}", text),
            Style::default().fg(Color::Green),
        )));
    }

    lines
}

fn spans_to_string(spans: &[Span<'static>]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect::<String>()
}

/// Wrap text to fit within max_width, respecting unicode width
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let options = textwrap::Options::new(max_width)
        .word_separator(textwrap::WordSeparator::UnicodeBreakProperties);

    wrap(text, options)
        .into_iter()
        .map(|cow| cow.into_owned())
        .collect()
}

fn wrap_thinking_text(text: &str, max_width: usize) -> Vec<String> {
    wrap_text(text, max_width)
}

fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.width() <= max_len {
        text.to_string()
    } else {
        let mut result = String::new();
        let mut width = 0;
        for ch in text.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if width + ch_width > max_len - 3 {
                result.push_str("...");
                break;
            }
            result.push(ch);
            width += ch_width;
        }
        result
    }
}

fn render_composer(frame: &mut Frame<'_>, state: &AppState, area: ratatui::layout::Rect) {
    let composer_text = if state.input.is_empty() {
        "> ".to_string()
    } else {
        format!("> {}", state.input)
    };
    let composer = Paragraph::new(composer_text)
        .block(Block::default().title("Composer").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(composer, area);
}
