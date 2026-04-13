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
        TranscriptEntry::ExecCommand {
            call_id: _,
            input_preview,
            output_preview,
            success,
            started,
            exit_code,
            duration_ms,
        } => Box::new(ExecHistoryCell {
            input_preview: input_preview.clone(),
            output_preview: output_preview.clone(),
            success: *success,
            started: *started,
            exit_code: *exit_code,
            duration_ms: *duration_ms,
        }),
        TranscriptEntry::PatchApply {
            call_id: _,
            summary_preview,
            success,
            started,
        } => Box::new(PatchHistoryCell {
            summary_preview: summary_preview.clone(),
            success: *success,
            started: *started,
        }),
        TranscriptEntry::GenericToolCall {
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

#[derive(Debug)]
struct ExecHistoryCell {
    input_preview: Option<String>,
    output_preview: Option<String>,
    success: bool,
    started: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

impl HistoryCell for ExecHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_exec_history_lines(
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            self.exit_code,
            self.duration_ms,
            width,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_exec_history_lines(
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            self.exit_code,
            self.duration_ms,
            width,
            ToolRenderMode::Full,
        )
    }
}

#[derive(Debug)]
struct PatchHistoryCell {
    summary_preview: Option<String>,
    success: bool,
    started: bool,
}

impl HistoryCell for PatchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        tool_output::render_tool_call_lines(
            "patch_apply",
            self.summary_preview.as_deref(),
            None,
            self.success,
            self.started,
            None,
            None,
            width as usize,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        tool_output::render_tool_call_lines(
            "patch_apply",
            self.summary_preview.as_deref(),
            None,
            self.success,
            self.started,
            None,
            None,
            width as usize,
            ToolRenderMode::Full,
        )
    }
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

fn render_exec_history_lines(
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    success: bool,
    started: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    width: u16,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let title = if started { "Running" } else { "Ran" };
    let bullet_style = if started {
        Style::default().fg(Color::Blue)
    } else if success {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };

    let command = input_preview.unwrap_or("");
    let mut header = format!("• {title}");
    if !command.is_empty() {
        header.push(' ');
        header.push_str(command);
    }

    let wrap_width = width as usize;
    let wrapped = wrap(&header, wrap_width.max(1));
    if let Some((first, rest)) = wrapped.split_first() {
        lines.push(Line::from(vec![Span::styled(
            first.to_string(),
            bullet_style.add_modifier(Modifier::BOLD),
        )]));
        for line in rest {
            lines.push(Line::from(vec![
                Span::styled("  │ ", Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Magenta),
                ),
            ]));
        }
    }

    let output_lines = tool_output::render_tool_output_body(
        "exec_command",
        input_preview,
        output_preview,
        width as usize,
        mode,
    );
    lines.extend(output_lines);

    if !started {
        lines.push(tool_output::render_exec_result_line_public(
            success,
            exit_code,
            duration_ms,
        ));
    }

    lines
}
