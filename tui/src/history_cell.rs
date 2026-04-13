use agent_core::app::TranscriptEntry;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::wrap;

use crate::markdown;
use crate::tool_output;
use crate::tool_output::ToolRenderMode;

pub(crate) trait HistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }
}

pub(crate) fn history_cell_for_entry(entry: &TranscriptEntry) -> Box<dyn HistoryCell> {
    match entry {
        TranscriptEntry::User(text) => Box::new(PrefixedTextCell {
            prefix: "› ",
            text: text.clone(),
            style: Style::default(),
        }),
        TranscriptEntry::Assistant(text) => Box::new(MarkdownHistoryCell { text: text.clone() }),
        TranscriptEntry::Thinking(text) => Box::new(PrefixedTextCell {
            prefix: "• thinking ",
            text: text.clone(),
            style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
        }),
        TranscriptEntry::ToolCall {
            name,
            input_preview,
            output_preview,
            success,
            started,
            exit_code,
            duration_ms,
            ..
        } => Box::new(ToolCallHistoryCell {
            name: name.clone(),
            input_preview: input_preview.clone(),
            output_preview: output_preview.clone(),
            success: *success,
            started: *started,
            exit_code: *exit_code,
            duration_ms: *duration_ms,
        }),
        TranscriptEntry::Status(text) => Box::new(PrefixedTextCell {
            prefix: "• ",
            text: text.clone(),
            style: Style::default().fg(Color::DarkGray),
        }),
        TranscriptEntry::Error(text) => Box::new(PrefixedTextCell {
            prefix: "• error ",
            text: text.clone(),
            style: Style::default().fg(Color::Red),
        }),
    }
}

#[derive(Debug)]
struct PrefixedTextCell {
    prefix: &'static str,
    text: String,
    style: Style,
}

impl HistoryCell for PrefixedTextCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        wrap_prefixed(self.prefix, &self.text, self.style, width as usize)
    }
}

#[derive(Debug)]
struct MarkdownHistoryCell {
    text: String,
}

impl HistoryCell for MarkdownHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        markdown::render_markdown_lines(&self.text, width as usize)
    }
}

#[derive(Debug)]
struct ToolCallHistoryCell {
    name: String,
    input_preview: Option<String>,
    output_preview: Option<String>,
    success: bool,
    started: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

impl HistoryCell for ToolCallHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        tool_output::render_tool_call_lines(
            &self.name,
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            self.exit_code,
            self.duration_ms,
            width as usize,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        tool_output::render_tool_call_lines(
            &self.name,
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            self.exit_code,
            self.duration_ms,
            width as usize,
            ToolRenderMode::Full,
        )
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
