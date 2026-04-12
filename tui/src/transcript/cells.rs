use agent_core::app::TranscriptEntry;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::wrap;

use crate::markdown;

#[derive(Debug, Clone)]
pub struct TranscriptCell {
    pub lines: Vec<Line<'static>>,
}

pub fn build_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    let content_width = width.max(4) as usize;
    entries
        .iter()
        .filter_map(|entry| build_cell(entry, content_width))
        .collect()
}

pub fn flatten_cells(cells: &[TranscriptCell]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0
            && !lines
                .last()
                .is_some_and(|line: &Line<'static>| line.spans.is_empty())
        {
            lines.push(Line::from(""));
        }
        lines.extend(cell.lines.clone());
    }
    lines
}

fn build_cell(entry: &TranscriptEntry, width: usize) -> Option<TranscriptCell> {
    let lines = match entry {
        TranscriptEntry::User(text) => wrap_prefixed("› ", text, Style::default(), width),
        TranscriptEntry::Assistant(text) => markdown::render_markdown_lines(text, width),
        TranscriptEntry::Thinking(text) => wrap_prefixed(
            "• thinking ",
            text,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
            width,
        ),
        TranscriptEntry::ToolCall {
            name,
            input_preview,
            output_preview,
            success,
            started,
            ..
        } => {
            let mut lines = Vec::new();
            let summary = if *started {
                format!("• running tool {name}")
            } else if *success {
                format!("• finished tool {name}")
            } else {
                format!("• failed tool {name}")
            };
            let style = if *started {
                Style::default().fg(Color::Blue)
            } else if *success {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            lines.push(Line::from(Span::styled(summary, style)));
            if let Some(input) = input_preview {
                lines.extend(wrap_prefixed(
                    "  input: ",
                    &markdown::render_tool_preview(input, width.saturating_sub(10)),
                    Style::default().fg(Color::DarkGray),
                    width,
                ));
            }
            if let Some(output) = output_preview {
                lines.extend(wrap_prefixed(
                    "  output: ",
                    &markdown::render_tool_preview(output, width.saturating_sub(11)),
                    Style::default().fg(Color::DarkGray),
                    width,
                ));
            }
            lines
        }
        TranscriptEntry::Status(text) => {
            wrap_prefixed("• ", text, Style::default().fg(Color::DarkGray), width)
        }
        TranscriptEntry::Error(text) => {
            wrap_prefixed("• error ", text, Style::default().fg(Color::Red), width)
        }
    };

    if lines.is_empty() {
        None
    } else {
        Some(TranscriptCell { lines })
    }
}

fn wrap_prefixed(prefix: &str, text: &str, style: Style, width: usize) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(prefix.len()).max(1);
    let wrapped = wrap(text, content_width)
        .into_iter()
        .map(|line| line.into_owned())
        .collect::<Vec<_>>();

    if wrapped.is_empty() {
        return vec![Line::from(Span::styled(prefix.to_string(), style))];
    }

    wrapped
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let leader = if index == 0 {
                prefix.to_string()
            } else {
                " ".repeat(prefix.len())
            };
            Line::from(vec![Span::styled(leader, style), Span::styled(line, style)])
        })
        .collect()
}
