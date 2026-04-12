use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::app::TranscriptEntry;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
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
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use textwrap::wrap;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

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
    if state.skill_browser_open {
        render_skill_browser(frame, state);
    }
}

fn render_header(frame: &mut Frame<'_>, state: &AppState, area: ratatui::layout::Rect) {
    let status_text = match state.status {
        AppStatus::Idle => "idle",
        AppStatus::Responding => "responding",
    };
    let loop_text = match state.loop_phase {
        LoopPhase::Idle => "idle",
        LoopPhase::Planning => "planning",
        LoopPhase::Executing => "executing",
        LoopPhase::Verifying => "verifying",
        LoopPhase::Escalating => "escalating",
    };

    let session_info = match &state.current_session_handle() {
        Some(agent_core::provider::SessionHandle::ClaudeSession { session_id }) => {
            if session_id.len() >= 8 {
                format!(" | session: {}...", &session_id[..8])
            } else {
                format!(" | session: {}", session_id)
            }
        }
        Some(agent_core::provider::SessionHandle::CodexThread { thread_id }) => {
            if thread_id.len() >= 8 {
                format!(" | thread: {}...", &thread_id[..8])
            } else {
                format!(" | thread: {}", thread_id)
            }
        }
        None => String::new(),
    };

    let header = Paragraph::new(Line::from(format!(
        "agile-agent | provider: {} | status: {} | loop: {}{} | skills: {} | tab: switch | ctrl+l: clear | q/esc: quit",
        state.selected_provider.label(),
        status_text,
        loop_text,
        session_info,
        state.skills.enabled_count()
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
            let icon = if *started {
                "🔧"
            } else {
                if *success { "✓" } else { "✗" }
            };
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
    let mut blockquote_depth = 0usize;
    let mut list_stack: Vec<Option<u64>> = Vec::new();

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
                    let paragraph_text = spans_to_string(&current_line_spans);
                    let wrapped =
                        wrap_text(&paragraph_text, content_width(max_width, blockquote_depth));
                    for wrapped_line in wrapped {
                        lines.push(line_with_blockquote_prefix(wrapped_line, blockquote_depth));
                    }
                    current_line_spans.clear();
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                blockquote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                blockquote_depth = blockquote_depth.saturating_sub(1);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                lines.push(styled_line_with_blockquote_prefix(
                    format!("```{}", code_block_lang),
                    Style::default().fg(Color::Yellow),
                    blockquote_depth,
                ));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                for code_line in code_block_content.lines() {
                    lines.push(styled_line_with_blockquote_prefix(
                        format!("  {}", code_line),
                        Style::default().fg(Color::Green),
                        blockquote_depth,
                    ));
                }
                lines.push(styled_line_with_blockquote_prefix(
                    "```".to_string(),
                    Style::default().fg(Color::Yellow),
                    blockquote_depth,
                ));
                code_block_content.clear();
                code_block_lang.clear();
            }
            Event::Start(Tag::List(start)) => {
                list_stack.push(start);
            }
            Event::End(TagEnd::List { .. }) => {}
            Event::Start(Tag::Item) => {
                let marker = match list_stack.last_mut() {
                    Some(Some(next_index)) => {
                        let marker = format!("{}. ", *next_index);
                        *next_index += 1;
                        marker
                    }
                    _ => "- ".to_string(),
                };
                current_line_spans.push(Span::styled(marker, Style::default().fg(Color::Blue)));
            }
            Event::End(TagEnd::Item) => {
                if !current_line_spans.is_empty() {
                    let item_text = spans_to_string(&current_line_spans);
                    let wrapped = wrap_text(&item_text, content_width(max_width, blockquote_depth));
                    for wrapped_line in wrapped {
                        lines.push(line_with_blockquote_prefix(wrapped_line, blockquote_depth));
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
                    let wrapped =
                        wrap_text(&paragraph_text, content_width(max_width, blockquote_depth));
                    for wrapped_line in wrapped {
                        lines.push(line_with_blockquote_prefix(wrapped_line, blockquote_depth));
                    }
                    current_line_spans.clear();
                }
            }
            Event::Start(Tag::Link { .. }) => {
                current_style = Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::UNDERLINED);
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
        let wrapped = wrap_text(&paragraph_text, content_width(max_width, blockquote_depth));
        for wrapped_line in wrapped {
            lines.push(line_with_blockquote_prefix(wrapped_line, blockquote_depth));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(text.to_string()));
    }

    // Prefix first line with "Assistant: "
    if let Some(first_line) = lines.first_mut() {
        let original_spans = first_line.spans.clone();
        *first_line = Line::from(
            std::iter::once(Span::styled(
                "Assistant: ",
                Style::default().fg(Color::Green),
            ))
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

fn content_width(max_width: usize, blockquote_depth: usize) -> usize {
    max_width.saturating_sub(blockquote_depth * 2)
}

fn blockquote_prefix(depth: usize) -> String {
    "> ".repeat(depth)
}

fn line_with_blockquote_prefix(text: String, depth: usize) -> Line<'static> {
    if depth == 0 {
        Line::from(text)
    } else {
        Line::from(format!("{}{}", blockquote_prefix(depth), text))
    }
}

fn styled_line_with_blockquote_prefix(text: String, style: Style, depth: usize) -> Line<'static> {
    if depth == 0 {
        Line::from(Span::styled(text, style))
    } else {
        Line::from(vec![
            Span::raw(blockquote_prefix(depth)),
            Span::styled(text, style),
        ])
    }
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
        ">  ($: skills)".to_string()
    } else {
        format!("> {}", state.input)
    };
    let composer = Paragraph::new(composer_text)
        .block(Block::default().title("Composer").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(composer, area);
}

fn render_skill_browser(frame: &mut Frame<'_>, state: &AppState) {
    use ratatui::layout::Alignment;
    use ratatui::widgets::Clear;

    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = format!("Skills ({})", state.skills.enabled_names.len());

    let mut lines = Vec::new();
    for (index, skill) in state.skills.discovered.iter().enumerate() {
        let enabled = state.skills.is_enabled(&skill.name);
        let selected = index == state.skill_browser_selected;
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
        lines.push(Line::from(Span::styled(
            format!("    {}", skill.path.display()),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from("No skills found."));
    }

    let browser = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL),
        )
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

#[cfg(test)]
mod tests {
    use super::render_markdown;

    fn lines_to_string(lines: Vec<ratatui::text::Line<'static>>) -> String {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_heading_and_paragraph_readably() {
        let markdown = "# Title\n\nThis is a paragraph.";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("Assistant: # Title"));
        assert!(rendered.contains("This is a paragraph."));
    }

    #[test]
    fn renders_list_items_readably() {
        let markdown = "- one\n- two";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("- one"));
        assert!(rendered.contains("- two"));
    }

    #[test]
    fn renders_ordered_list_items_readably() {
        let markdown = "1. one\n2. two";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("1. one"));
        assert!(rendered.contains("2. two"));
    }

    #[test]
    fn renders_code_blocks_readably() {
        let markdown = "```rust\nfn main() {}\n```";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("```rust"));
        assert!(rendered.contains("fn main() {}"));
        assert!(rendered.contains("```"));
    }

    #[test]
    fn renders_blockquotes_with_prefix() {
        let markdown = "> quoted text";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("> quoted text"));
    }

    #[test]
    fn renders_link_text_instead_of_destination_url() {
        let markdown = "[OpenAI](https://openai.com)";
        let rendered = lines_to_string(render_markdown(markdown, 80));

        assert!(rendered.contains("OpenAI"));
        assert!(!rendered.contains("https://openai.com"));
    }
}
