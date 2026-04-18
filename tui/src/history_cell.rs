use agent_core::app::TranscriptEntry;
use agent_core::tool_calls::ExecCommandStatus;
use agent_core::tool_calls::McpInvocation;
use agent_core::tool_calls::McpToolCallStatus;
use agent_core::tool_calls::PatchApplyStatus;
use agent_core::tool_calls::PatchChange;
use agent_core::tool_calls::PatchChangeKind;
use agent_core::tool_calls::WebSearchAction;
use diffy::Patch;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::Options;
use textwrap::WordSplitter;
use textwrap::wrap;

use crate::exec_command::strip_shell_wrapper;
use crate::exec_semantics::ExploringOp;
use crate::markdown;
use crate::text_formatting::format_json_compact;
use crate::tool_output;
use crate::tool_output::ToolRenderMode;

const COMMAND_CONTINUATION_PREFIX: &str = "  │ ";
const DETAIL_INITIAL_PREFIX: &str = "  └ ";
const DETAIL_CONTINUATION_PREFIX: &str = "    ";
const TRANSCRIPT_HINT: &str = "ctrl + t to view transcript";
const EXEC_COMMAND_CONTINUATION_MAX_LINES: usize = 3;
const GENERIC_TOOL_PREVIEW_MAX_LINES: usize = 5;

pub(crate) trait HistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>>;

    fn desired_height(&self, width: u16) -> u16 {
        let width = width.max(1) as usize;
        self.display_lines(width as u16)
            .iter()
            .map(|line| line.width().div_ceil(width).max(1))
            .sum::<usize>()
            .try_into()
            .unwrap_or(u16::MAX)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    fn desired_transcript_height(&self, width: u16) -> u16 {
        let width = width.max(1) as usize;
        self.transcript_lines(width as u16)
            .iter()
            .map(|line| line.width().div_ceil(width).max(1))
            .sum::<usize>()
            .try_into()
            .unwrap_or(u16::MAX)
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
            source,
            allow_exploring_group: _,
            input_preview,
            output_preview,
            status,
            exit_code,
            duration_ms,
        } => Box::new(ExecHistoryCell {
            source: source.clone(),
            input_preview: input_preview.clone(),
            output_preview: output_preview.clone(),
            status: *status,
            exit_code: *exit_code,
            duration_ms: *duration_ms,
        }),
        TranscriptEntry::PatchApply {
            call_id: _,
            changes,
            status,
            output_preview,
        } => Box::new(PatchHistoryCell {
            changes: changes.clone(),
            status: *status,
            output_preview: output_preview.clone(),
        }),
        TranscriptEntry::WebSearch {
            call_id: _,
            query,
            action,
            started,
        } => Box::new(WebSearchHistoryCell {
            query: query.clone(),
            action: action.clone(),
            started: *started,
        }),
        TranscriptEntry::ViewImage { call_id: _, path } => {
            Box::new(ViewImageHistoryCell { path: path.clone() })
        }
        TranscriptEntry::ImageGeneration {
            call_id: _,
            revised_prompt,
            result,
            saved_path,
        } => Box::new(ImageGenerationHistoryCell {
            revised_prompt: revised_prompt.clone(),
            result: result.clone(),
            saved_path: saved_path.clone(),
        }),
        TranscriptEntry::McpToolCall {
            call_id: _,
            invocation,
            result_blocks,
            error,
            status,
            is_error,
        } => Box::new(McpToolCallHistoryCell {
            invocation: invocation.clone(),
            result_blocks: result_blocks.clone(),
            error: error.clone(),
            status: *status,
            is_error: *is_error,
        }),
        TranscriptEntry::GenericToolCall {
            name,
            input_preview,
            output_preview,
            success,
            started,
            ..
        } => Box::new(ToolCallHistoryCell {
            name: name.clone(),
            input_preview: input_preview.clone(),
            output_preview: output_preview.clone(),
            success: *success,
            started: *started,
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
        TranscriptEntry::Decision {
            agent_id,
            situation_type,
            action_type,
            reasoning,
            confidence,
            tier,
        } => Box::new(DecisionHistoryCell {
            agent_id: agent_id.clone(),
            situation_type: situation_type.clone(),
            action_type: action_type.clone(),
            reasoning: reasoning.clone(),
            confidence: *confidence,
            tier: tier.clone(),
        }),
    }
}

pub(crate) fn history_cell_for_exploring_exec_group(
    calls: Vec<ExploringExecCall>,
) -> Box<dyn HistoryCell> {
    Box::new(ExploringExecHistoryCell { calls })
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
}

#[derive(Debug)]
struct ExecHistoryCell {
    source: Option<String>,
    input_preview: Option<String>,
    output_preview: Option<String>,
    status: ExecCommandStatus,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExploringExecCall {
    pub(crate) source: Option<String>,
    pub(crate) input_preview: Option<String>,
    pub(crate) output_preview: Option<String>,
    pub(crate) status: ExecCommandStatus,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) ops: Vec<ExploringOp>,
}

#[derive(Debug)]
struct ExploringExecHistoryCell {
    calls: Vec<ExploringExecCall>,
}

impl HistoryCell for ExecHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_exec_history_lines(
            self.source.as_deref(),
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.status,
            self.exit_code,
            self.duration_ms,
            width,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_exec_transcript_lines(
            self.source.as_deref(),
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.status,
            self.exit_code,
            self.duration_ms,
            width,
        )
    }
}

impl HistoryCell for ExploringExecHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_exploring_exec_lines(&self.calls, width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (index, call) in self.calls.iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }
            lines.extend(render_exec_transcript_lines(
                call.source.as_deref(),
                call.input_preview.as_deref(),
                call.output_preview.as_deref(),
                call.status,
                call.exit_code,
                call.duration_ms,
                width,
            ));
        }
        lines
    }
}

fn render_exec_transcript_lines(
    _source: Option<&str>,
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    status: ExecCommandStatus,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let display_command = input_preview.map(strip_shell_wrapper).unwrap_or_default();
    if !display_command.trim().is_empty() {
        // Check if this is a git command for friendly label display
        let git_label = detect_git_command(&display_command);

        if let Some(ref git) = git_label {
            // Show git-friendly label in transcript
            let label_with_detail = if let Some(ref detail) = git.detail {
                format!("{} ({})", git.label, detail)
            } else {
                git.label.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Magenta)),
                Span::styled(label_with_detail, Style::default().fg(Color::Magenta)),
            ]));
        } else {
            // Show raw command for non-git commands
            let command_lines = display_command.lines().collect::<Vec<_>>();
            let first_segments = wrap_text_segments(
                command_lines.first().copied().unwrap_or_default(),
                width.saturating_sub(2).max(1) as usize,
            );
            let first_segment = first_segments.first().cloned().unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Magenta)),
                Span::styled(first_segment, Style::default().fg(Color::Magenta)),
            ]));

            let mut continuation_segments = first_segments.into_iter().skip(1).collect::<Vec<_>>();
            for raw_line in command_lines.into_iter().skip(1) {
                continuation_segments.extend(wrap_text_segments(
                    raw_line,
                    width.saturating_sub(4).max(1) as usize,
                ));
            }
            for segment in continuation_segments {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(segment, Style::default().fg(Color::Magenta)),
                ]));
            }
        }
    }

    if let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) {
        for raw_line in output.lines() {
            for segment in wrap_text_segments(raw_line, width.max(1) as usize) {
                lines.push(Line::from(segment));
            }
        }
    } else if !matches!(status, ExecCommandStatus::InProgress) {
        lines.push(Line::from(Span::styled(
            "(no output)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )));
    }

    if !matches!(status, ExecCommandStatus::InProgress) {
        lines.push(tool_output::render_exec_result_line_public(
            matches!(status, ExecCommandStatus::Completed),
            exit_code,
            duration_ms,
        ));
    }
    lines
}

#[derive(Debug)]
struct PatchHistoryCell {
    changes: Vec<PatchChange>,
    status: PatchApplyStatus,
    output_preview: Option<String>,
}

#[derive(Debug)]
struct WebSearchHistoryCell {
    query: String,
    action: Option<WebSearchAction>,
    started: bool,
}

#[derive(Debug)]
struct ViewImageHistoryCell {
    path: String,
}

#[derive(Debug)]
struct ImageGenerationHistoryCell {
    revised_prompt: Option<String>,
    result: Option<String>,
    saved_path: Option<String>,
}

#[derive(Debug)]
struct McpToolCallHistoryCell {
    invocation: McpInvocation,
    result_blocks: Vec<serde_json::Value>,
    error: Option<String>,
    status: McpToolCallStatus,
    is_error: bool,
}

impl HistoryCell for PatchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_patch_summary_lines(
            &self.changes,
            self.status,
            self.output_preview.as_deref(),
            width as usize,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_patch_summary_lines(
            &self.changes,
            self.status,
            self.output_preview.as_deref(),
            width as usize,
            ToolRenderMode::Full,
        )
    }
}

impl HistoryCell for ToolCallHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_generic_tool_call_lines(
            &self.name,
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            width as usize,
            ToolRenderMode::Preview,
        )
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        render_generic_tool_call_lines(
            &self.name,
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.success,
            self.started,
            width as usize,
            ToolRenderMode::Full,
        )
    }
}

impl HistoryCell for WebSearchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let prefix = if self.started {
            "• Searching the web "
        } else {
            "• Searched "
        };
        wrap_prefixed(
            prefix,
            &web_search_detail(self.action.as_ref(), &self.query),
            Style::default(),
            width as usize,
        )
    }
}

impl HistoryCell for ViewImageHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                "Viewed Image".to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.extend(wrap_prefixed(
            DETAIL_INITIAL_PREFIX,
            &self.path,
            Style::default().add_modifier(Modifier::DIM),
            width as usize,
        ));
        lines
    }
}

impl HistoryCell for ImageGenerationHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                "Generated Image:".to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        let detail = self
            .revised_prompt
            .clone()
            .or_else(|| self.result.clone())
            .unwrap_or_else(|| "image generation".to_string());
        lines.extend(wrap_prefixed(
            DETAIL_INITIAL_PREFIX,
            &detail,
            Style::default().add_modifier(Modifier::DIM),
            width as usize,
        ));
        if let Some(saved_path) = self.saved_path.as_deref() {
            lines.extend(wrap_prefixed(
                DETAIL_INITIAL_PREFIX,
                &format!("Saved to: {saved_path}"),
                Style::default().add_modifier(Modifier::DIM),
                width as usize,
            ));
        }
        lines
    }
}

impl HistoryCell for McpToolCallHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let (bullet_style, header) = match self.status {
            McpToolCallStatus::InProgress => (Style::default().fg(Color::Blue), "Calling"),
            McpToolCallStatus::Completed if !self.is_error => {
                (Style::default().fg(Color::Green), "Called")
            }
            _ => (Style::default().fg(Color::Red), "Called"),
        };
        let invocation = format_mcp_invocation(&self.invocation);
        let inline = invocation.len() + header.len() + 3 <= width as usize;

        if inline {
            lines.push(Line::from(vec![
                Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    header.to_string(),
                    bullet_style.add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::raw(invocation),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    header.to_string(),
                    bullet_style.add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.extend(wrap_prefixed(
                DETAIL_INITIAL_PREFIX,
                &invocation,
                Style::default(),
                width as usize,
            ));
        }

        if let Some(error) = self.error.as_deref() {
            lines.extend(wrap_prefixed(
                DETAIL_INITIAL_PREFIX,
                &format!("Error: {error}"),
                Style::default().add_modifier(Modifier::DIM),
                width as usize,
            ));
            return lines;
        }

        for block in &self.result_blocks {
            let detail = render_mcp_result_block(block);
            for line in detail.lines() {
                lines.extend(wrap_prefixed(
                    DETAIL_INITIAL_PREFIX,
                    line,
                    Style::default().add_modifier(Modifier::DIM),
                    width as usize,
                ));
            }
        }

        lines
    }
}

/// Decision layer output cell
///
/// Displays decision agent outputs with special formatting to distinguish
/// from work agent outputs. Uses a dark green theme with horizontal separator
/// for clear visual separation from work agent output.
#[derive(Debug)]
struct DecisionHistoryCell {
    agent_id: String,
    situation_type: String,
    action_type: String,
    reasoning: String,
    confidence: u8,
    tier: String,
}

impl HistoryCell for DecisionHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let width_usize = width as usize;

        // Decision header with special styling (green for decision layer)
        let decision_color = Color::Green;

        // Horizontal separator line before decision output
        let separator: String = "─".repeat(width_usize.min(40));
        lines.push(Line::from(vec![Span::styled(
            separator,
            Style::default()
                .fg(decision_color)
                .add_modifier(Modifier::DIM),
        )]));

        // Decision header line
        lines.push(Line::from(vec![
            Span::styled(
                "◆ ",
                Style::default()
                    .fg(decision_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "DECISION",
                Style::default()
                    .fg(decision_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" [", Style::default().fg(decision_color)),
            Span::styled(
                self.tier.clone(),
                Style::default()
                    .fg(decision_color)
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled("]", Style::default().fg(decision_color)),
        ]));

        // Agent and situation info
        lines.push(Line::from(vec![
            Span::styled("  Agent: ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(self.agent_id.clone(), Style::default().fg(Color::Cyan)),
            Span::styled(
                " │ Situation: ",
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(
                self.situation_type.clone(),
                Style::default().fg(Color::Yellow),
            ),
        ]));

        // Action taken
        lines.push(Line::from(vec![
            Span::styled("  Action: ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                self.action_type.clone(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " │ Confidence: ",
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!("{}%", self.confidence),
                Style::default().fg(Color::Green),
            ),
        ]));

        // Reasoning (wrapped if needed)
        if !self.reasoning.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "  Reasoning: ",
                Style::default().add_modifier(Modifier::DIM),
            )]));
            lines.extend(wrap_prefixed(
                "    ",
                &self.reasoning,
                Style::default().fg(Color::DarkGray),
                width_usize,
            ));
        }

        lines
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

fn wrap_prefixed_no_break_words(
    prefix: &str,
    text: &str,
    style: Style,
    width: usize,
) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(prefix.len()).max(1);
    let wrapped = wrap_words_without_breaking(text, content_width);

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

fn wrap_words_without_breaking(text: &str, content_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };

        if current.is_empty() || candidate.chars().count() <= content_width {
            current = candidate;
            continue;
        }

        lines.push(std::mem::take(&mut current));
        current = word.to_string();
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn render_exec_history_lines(
    source: Option<&str>,
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    status: ExecCommandStatus,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    width: u16,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let title = if matches!(status, ExecCommandStatus::InProgress) {
        "Running"
    } else if matches!(status, ExecCommandStatus::Declined) {
        "Declined command"
    } else if matches!(source, Some("userShell")) {
        "You ran"
    } else {
        "Ran"
    };
    let bullet_style = match status {
        ExecCommandStatus::InProgress => Style::default().fg(Color::Blue),
        ExecCommandStatus::Completed => Style::default().fg(Color::Green),
        ExecCommandStatus::Failed => Style::default().fg(Color::Red),
        ExecCommandStatus::Declined => Style::default().fg(Color::Yellow),
    };
    let display_command = input_preview.map(strip_shell_wrapper);
    lines.extend(render_exec_header_lines(
        title,
        display_command.as_deref().unwrap_or(""),
        bullet_style,
        width as usize,
    ));

    let output_lines = tool_output::render_tool_output_body(
        "exec_command",
        display_command.as_deref(),
        output_preview,
        width as usize,
        mode,
    );
    if output_lines.is_empty() && !matches!(status, ExecCommandStatus::InProgress) {
        lines.push(Line::from(vec![
            Span::styled(
                DETAIL_INITIAL_PREFIX,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(
                "(no output)",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    } else {
        lines.extend(output_lines);
    }

    if !matches!(status, ExecCommandStatus::InProgress) {
        lines.push(tool_output::render_exec_result_line_public(
            matches!(status, ExecCommandStatus::Completed),
            exit_code,
            duration_ms,
        ));
    }

    lines
}

fn render_exploring_exec_lines(calls: &[ExploringExecCall], width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let any_started = calls
        .iter()
        .any(|call| matches!(call.status, ExecCommandStatus::InProgress));
    let bullet_style = if any_started {
        Style::default().fg(Color::Blue)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };
    let title = if any_started { "Exploring" } else { "Explored" };
    lines.push(Line::from(vec![
        Span::styled("• ", bullet_style),
        Span::styled(
            title.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    let mut grouped = Vec::new();
    let mut index = 0usize;
    while index < calls.len() {
        let call = &calls[index];
        if call.ops.iter().all(|op| matches!(op, ExploringOp::Read(_))) {
            let mut names = Vec::new();
            while index < calls.len()
                && calls[index]
                    .ops
                    .iter()
                    .all(|op| matches!(op, ExploringOp::Read(_)))
            {
                for op in &calls[index].ops {
                    if let ExploringOp::Read(name) = op
                        && !names.contains(name)
                    {
                        names.push(name.clone());
                    }
                }
                index += 1;
            }
            grouped.push(("Read".to_string(), names.join(", ")));
            continue;
        }

        for op in &call.ops {
            match op {
                ExploringOp::Read(name) => grouped.push(("Read".to_string(), name.clone())),
                ExploringOp::List(target) => grouped.push(("List".to_string(), target.clone())),
                ExploringOp::Search { query, path } => grouped.push((
                    "Search".to_string(),
                    match path {
                        Some(path) => format!("{query} in {path}"),
                        None => query.clone(),
                    },
                )),
            }
        }
        index += 1;
    }

    for (index, (label, text)) in grouped.into_iter().enumerate() {
        let prefix = if index == 0 {
            DETAIL_INITIAL_PREFIX
        } else {
            DETAIL_CONTINUATION_PREFIX
        };
        let content = format!("{label} {text}");
        lines.extend(wrap_prefixed_no_break_words(
            prefix,
            &content,
            Style::default().fg(Color::Cyan),
            width as usize,
        ));
    }

    lines
}

fn web_search_detail(action: Option<&WebSearchAction>, query: &str) -> String {
    match action {
        Some(WebSearchAction::Search {
            query: action_query,
            queries,
        }) => action_query
            .clone()
            .or_else(|| queries.as_ref().and_then(|items| items.first().cloned()))
            .unwrap_or_else(|| query.to_string()),
        Some(WebSearchAction::OpenPage { url }) => url.clone().unwrap_or_else(|| query.to_string()),
        Some(WebSearchAction::FindInPage { url, pattern }) => match (pattern, url) {
            (Some(pattern), Some(url)) => format!("'{pattern}' in {url}"),
            (Some(pattern), None) => pattern.clone(),
            (None, Some(url)) => url.clone(),
            (None, None) => query.to_string(),
        },
        Some(WebSearchAction::Other) | None => query.to_string(),
    }
}

fn format_mcp_invocation(invocation: &McpInvocation) -> String {
    let args = invocation
        .arguments
        .as_ref()
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| value.to_string()))
        .unwrap_or_default();
    format!("{}.{}({args})", invocation.server, invocation.tool)
}

fn render_mcp_result_block(block: &serde_json::Value) -> String {
    match block.get("type").and_then(|value| value.as_str()) {
        Some("text") => block
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string(),
        Some("image") => "<image content>".to_string(),
        Some("audio") => "<audio content>".to_string(),
        Some("resource_link") => format!(
            "link: {}",
            block
                .get("uri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
        ),
        Some("resource") => format!(
            "embedded resource: {}",
            block
                .get("uri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
        ),
        _ => format_json_compact(&block.to_string()).unwrap_or_else(|| block.to_string()),
    }
}

fn render_exec_header_lines(
    title: &str,
    command: &str,
    bullet_style: Style,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let header_prefix = format!("• {title}");

    if command.trim().is_empty() {
        lines.push(Line::from(vec![Span::styled(
            header_prefix,
            bullet_style.add_modifier(Modifier::BOLD),
        )]));
        return lines;
    }

    // Check if this is a git command for friendly label display
    let git_label = detect_git_command(command);

    let command_lines = command.lines().collect::<Vec<_>>();
    let first_segments = wrap_text_segments(
        command_lines.first().copied().unwrap_or_default(),
        width.saturating_sub(header_prefix.len() + 1).max(1),
    );
    let first_segment = first_segments.first().cloned().unwrap_or_default();

    // Build header with git-specific label if detected
    let header_text = if let Some(ref git) = git_label {
        if let Some(ref detail) = git.detail {
            format!("{} ({})", git.label, detail)
        } else {
            git.label.to_string()
        }
    } else {
        first_segment.clone()
    };

    let mut header = header_prefix;
    if !header_text.is_empty() {
        header.push(' ');
        header.push_str(&header_text);
    }
    lines.push(Line::from(vec![Span::styled(
        header,
        bullet_style.add_modifier(Modifier::BOLD),
    )]));

    // For git commands, don't show the raw command as continuation since we show the friendly label
    if git_label.is_some() {
        return lines;
    }

    // Show raw command continuation for non-git commands
    let mut continuation_segments = first_segments.into_iter().skip(1).collect::<Vec<_>>();
    for raw_line in command_lines.into_iter().skip(1) {
        continuation_segments.extend(wrap_text_segments(
            raw_line,
            width
                .saturating_sub(COMMAND_CONTINUATION_PREFIX.len())
                .max(1),
        ));
    }

    for segment in
        truncate_segments_from_start(continuation_segments, EXEC_COMMAND_CONTINUATION_MAX_LINES)
    {
        lines.push(Line::from(vec![
            Span::styled(
                COMMAND_CONTINUATION_PREFIX,
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(segment, Style::default().fg(Color::Magenta)),
        ]));
    }

    lines
}

fn render_generic_tool_call_lines(
    name: &str,
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    success: bool,
    started: bool,
    width: usize,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    // Special handling for Edit tool with diff-style rendering
    if name == "Edit" {
        return render_edit_tool_lines(
            input_preview,
            output_preview,
            success,
            started,
            width,
            mode,
        );
    }

    // All git commands use the same color to distinguish from other commands
    const GIT_COLOR: Color = Color::Magenta;

    let mut lines = Vec::new();
    let bullet_style = if started {
        Style::default().fg(Color::Blue)
    } else if success {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };

    // Extract command from input_preview (handles JSON-wrapped commands like {"command":"git diff",...})
    let display_command = extract_command_from_input(input_preview);

    // Check if this is a git command using the extracted command
    let git_label = display_command
        .as_ref()
        .and_then(|cmd| detect_git_command(cmd));

    let (header_text, header_style) = if git_label.is_some() {
        let prefix = if started { "Calling" } else { "Called" };
        (prefix, Style::default().fg(GIT_COLOR))
    } else {
        let prefix = if started { "Calling" } else { "Called" };
        (prefix, bullet_style)
    };

    let invocation = format_tool_invocation(name, input_preview, display_command.as_deref());
    let inline_invocation =
        !invocation.is_empty() && invocation.len() + header_text.len() + 3 <= width.max(1);

    // Build display line with git-specific label if detected
    if inline_invocation {
        lines.push(Line::from(vec![
            Span::styled("• ", header_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_text.to_string(),
                header_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            if let Some(ref git) = git_label {
                // Show git-specific label with detail if available
                if let Some(ref detail) = git.detail {
                    Span::styled(format!("{} ({})", git.label, detail), header_style)
                } else {
                    Span::styled(git.label, header_style)
                }
            } else {
                Span::raw(invocation.clone())
            },
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("• ", header_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_text.to_string(),
                header_style.add_modifier(Modifier::BOLD),
            ),
        ]));

        // Show git-specific label or invocation on next line
        if let Some(ref git) = git_label {
            if let Some(ref detail) = git.detail {
                lines.extend(render_prefixed_text_block(
                    &format!("{} ({})", git.label, detail),
                    width,
                    DETAIL_INITIAL_PREFIX,
                    DETAIL_CONTINUATION_PREFIX,
                    header_style,
                    ToolRenderMode::Full,
                ));
            } else {
                lines.extend(render_prefixed_text_block(
                    git.label,
                    width,
                    DETAIL_INITIAL_PREFIX,
                    DETAIL_CONTINUATION_PREFIX,
                    header_style,
                    ToolRenderMode::Full,
                ));
            }
        } else if !invocation.is_empty() {
            lines.extend(render_prefixed_text_block(
                &invocation,
                width,
                DETAIL_INITIAL_PREFIX,
                DETAIL_CONTINUATION_PREFIX,
                Style::default(),
                ToolRenderMode::Full,
            ));
        }
    }

    if let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) {
        lines.extend(render_prefixed_tool_output(
            output,
            width,
            if inline_invocation {
                DETAIL_INITIAL_PREFIX
            } else {
                DETAIL_CONTINUATION_PREFIX
            },
            DETAIL_CONTINUATION_PREFIX,
            mode,
        ));
    }

    lines
}

fn render_patch_summary_lines(
    changes: &[PatchChange],
    status: PatchApplyStatus,
    output_preview: Option<&str>,
    width: usize,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    if changes.is_empty() {
        let mut lines = Vec::new();
        let style = match status {
            PatchApplyStatus::InProgress => Style::default().fg(Color::Blue),
            PatchApplyStatus::Completed => Style::default().fg(Color::Green),
            PatchApplyStatus::Failed => Style::default().fg(Color::Red),
            PatchApplyStatus::Declined => Style::default().fg(Color::Yellow),
        };
        let title = match status {
            PatchApplyStatus::InProgress => "• Applying patch",
            PatchApplyStatus::Completed => "• Applied patch",
            PatchApplyStatus::Failed => "• Failed patch",
            PatchApplyStatus::Declined => "• Declined patch",
        };
        lines.push(Line::from(Span::styled(
            title.to_string(),
            style.add_modifier(Modifier::BOLD),
        )));
        return append_patch_output(lines, output_preview, width, mode);
    }

    let bullet_style = match status {
        PatchApplyStatus::InProgress => Style::default().fg(Color::Blue),
        PatchApplyStatus::Completed => Style::default().fg(Color::Green),
        PatchApplyStatus::Failed => Style::default().fg(Color::Red),
        PatchApplyStatus::Declined => Style::default().fg(Color::Yellow),
    };

    let total_added = changes.iter().map(|change| change.added).sum::<usize>();
    let total_removed = changes.iter().map(|change| change.removed).sum::<usize>();
    let mut lines = Vec::new();

    if let [change] = changes {
        lines.push(Line::from(vec![
            Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                patch_change_title(change.kind).to_string(),
                bullet_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(patch_change_path(change)),
            Span::raw(" "),
            Span::raw(format!("(+{} -{})", change.added, change.removed)),
        ]));
        lines.extend(render_patch_diff_lines(
            change,
            DETAIL_CONTINUATION_PREFIX,
            mode,
        ));
        return append_patch_output(lines, output_preview, width, mode);
    }

    lines.push(Line::from(vec![
        Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
        Span::styled(
            "Edited".to_string(),
            bullet_style.add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " {} files (+{} -{})",
            changes.len(),
            total_added,
            total_removed
        )),
    ]));

    for change in changes {
        lines.extend(wrap_prefixed(
            DETAIL_INITIAL_PREFIX,
            &format!(
                "{} (+{} -{})",
                patch_change_path(change),
                change.added,
                change.removed
            ),
            Style::default().add_modifier(Modifier::DIM),
            width,
        ));
        lines.extend(render_patch_diff_lines(
            change,
            DETAIL_CONTINUATION_PREFIX,
            mode,
        ));
    }

    append_patch_output(lines, output_preview, width, mode)
}

fn append_patch_output(
    mut lines: Vec<Line<'static>>,
    output_preview: Option<&str>,
    width: usize,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) else {
        return lines;
    };

    let detail = render_prefixed_text_block(
        output,
        width,
        DETAIL_INITIAL_PREFIX,
        DETAIL_CONTINUATION_PREFIX,
        Style::default().add_modifier(Modifier::DIM),
        mode,
    );
    lines.extend(detail);
    lines
}

fn patch_change_title(kind: PatchChangeKind) -> &'static str {
    match kind {
        PatchChangeKind::Add => "Added",
        PatchChangeKind::Delete => "Deleted",
        _ => "Edited",
    }
}

fn patch_change_path(change: &PatchChange) -> String {
    match change.move_path.as_deref() {
        Some(move_path) => format!("{} → {move_path}", change.path),
        None => change.path.clone(),
    }
}

fn render_patch_diff_lines(
    change: &PatchChange,
    prefix: &str,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let entries = collect_patch_diff_entries(change);
    if entries.is_empty() {
        return Vec::new();
    }

    let line_no_width = entries
        .iter()
        .filter_map(|entry| entry.line_no)
        .max()
        .unwrap_or(1)
        .to_string()
        .len()
        .max(1);

    let lines = entries
        .into_iter()
        .map(|entry| render_patch_diff_line(prefix, line_no_width, entry))
        .collect::<Vec<_>>();

    match mode {
        ToolRenderMode::Preview => truncate_lines_middle(lines, 5),
        ToolRenderMode::Full => lines,
    }
}

fn render_patch_diff_line(
    prefix: &str,
    line_no_width: usize,
    entry: PatchDiffEntry,
) -> Line<'static> {
    let number = entry
        .line_no
        .map(|value| format!("{value:>width$}", width = line_no_width))
        .unwrap_or_else(|| " ".repeat(line_no_width));
    let marker = if entry.sign == ' ' {
        " ".to_string()
    } else {
        entry.sign.to_string()
    };

    Line::from(vec![
        Span::styled(
            prefix.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            number,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::raw(" "),
        Span::styled(marker, entry.style),
        Span::styled(entry.content, entry.style),
    ])
}

fn collect_patch_diff_entries(change: &PatchChange) -> Vec<PatchDiffEntry> {
    match change.kind {
        PatchChangeKind::Add => change
            .diff
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .enumerate()
            .map(|(index, line)| PatchDiffEntry {
                line_no: Some(index + 1),
                sign: '+',
                content: line.trim_start_matches('+').to_string(),
                style: Style::default().fg(Color::Green),
            })
            .collect(),
        PatchChangeKind::Delete => change
            .diff
            .lines()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .enumerate()
            .map(|(index, line)| PatchDiffEntry {
                line_no: Some(index + 1),
                sign: '-',
                content: line.trim_start_matches('-').to_string(),
                style: Style::default().fg(Color::Red),
            })
            .collect(),
        PatchChangeKind::Update => collect_update_diff_entries(&change.diff),
    }
}

fn collect_update_diff_entries(diff: &str) -> Vec<PatchDiffEntry> {
    let mut entries = Vec::new();
    let mut old_ln = 1usize;
    let mut new_ln = 1usize;

    for line in diff.lines() {
        if let Some((parsed_old, parsed_new)) = parse_hunk_header(line) {
            old_ln = parsed_old;
            new_ln = parsed_new;
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            entries.push(PatchDiffEntry {
                line_no: Some(new_ln),
                sign: '+',
                content: line.trim_start_matches('+').to_string(),
                style: Style::default().fg(Color::Green),
            });
            new_ln += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            entries.push(PatchDiffEntry {
                line_no: Some(old_ln),
                sign: '-',
                content: line.trim_start_matches('-').to_string(),
                style: Style::default().fg(Color::Red),
            });
            old_ln += 1;
        } else if let Some(content) = line.strip_prefix(' ') {
            entries.push(PatchDiffEntry {
                line_no: Some(new_ln),
                sign: ' ',
                content: content.to_string(),
                style: Style::default().add_modifier(Modifier::DIM),
            });
            old_ln += 1;
            new_ln += 1;
        }
    }

    entries
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let line = line.strip_prefix("@@ -")?;
    let (old_part, rest) = line.split_once(" +")?;
    let (new_part, _) = rest.split_once(" @@")?;
    Some((
        parse_hunk_range_start(old_part),
        parse_hunk_range_start(new_part),
    ))
}

fn parse_hunk_range_start(range: &str) -> usize {
    range
        .split_once(',')
        .map(|(start, _)| start)
        .unwrap_or(range)
        .parse()
        .unwrap_or(1)
}

#[derive(Debug, Clone)]
struct PatchDiffEntry {
    line_no: Option<usize>,
    sign: char,
    content: String,
    style: Style,
}

/// Represents a detected git command with a display-friendly name and style
struct GitCommandLabel {
    label: &'static str,
    detail: Option<String>,
}

/// Git command pattern for detection
struct GitPattern {
    /// The git subcommand to match
    subcommand: &'static str,
    /// Display label (e.g., "Git Diff")
    label: &'static str,
    /// Whether to show arguments as detail
    show_args_as_detail: bool,
}

/// Table of git commands for detection
const GIT_PATTERNS: &[GitPattern] = &[
    GitPattern {
        subcommand: "diff",
        label: "Git Diff",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "status",
        label: "Git Status",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "log",
        label: "Git Log",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "commit",
        label: "Git Commit",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "add",
        label: "Git Add",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "branch",
        label: "Git Branch",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "checkout",
        label: "Git Checkout",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "push",
        label: "Git Push",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "pull",
        label: "Git Pull",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "fetch",
        label: "Git Fetch",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "stash",
        label: "Git Stash",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "merge",
        label: "Git Merge",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "rebase",
        label: "Git Rebase",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "clone",
        label: "Git Clone",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "show",
        label: "Git Show",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "remote",
        label: "Git Remote",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "config",
        label: "Git Config",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "reset",
        label: "Git Reset",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "init",
        label: "Git Init",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "rm",
        label: "Git RM",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "mv",
        label: "Git MV",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "restore",
        label: "Git Restore",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "switch",
        label: "Git Switch",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "clean",
        label: "Git Clean",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "bisect",
        label: "Git Bisect",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "cherry-pick",
        label: "Git Cherry-Pick",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "revert",
        label: "Git Revert",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "tag",
        label: "Git Tag",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "describe",
        label: "Git Describe",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "reflog",
        label: "Git Reflog",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "worktree",
        label: "Git Worktree",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "grep",
        label: "Git Grep",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "archive",
        label: "Git Archive",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "bundle",
        label: "Git Bundle",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "fsck",
        label: "Git FSCK",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "gc",
        label: "Git GC",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "prune",
        label: "Git Prune",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "submodule",
        label: "Git Submodule",
        show_args_as_detail: true,
    },
    GitPattern {
        subcommand: "notes",
        label: "Git Notes",
        show_args_as_detail: false,
    },
    GitPattern {
        subcommand: "patch-id",
        label: "Git Patch-ID",
        show_args_as_detail: false,
    },
];

/// Detect git commands and return a formatted label for display
///
/// Returns a GitCommandLabel with:
/// - label: A short display name like "Git Diff" or "Git Commit"
/// - detail: Optional additional context (e.g., file paths, branch names)
fn detect_git_command(input: &str) -> Option<GitCommandLabel> {
    let trimmed = input.trim();

    // Handle special cases
    if trimmed == "git diff --staged" || trimmed == "git diff --cached" {
        return Some(GitCommandLabel {
            label: "Git Diff",
            detail: Some("staged".to_string()),
        });
    }

    // Check for "git <subcommand>" pattern
    let prefix = "git ";
    if !trimmed.starts_with(prefix) {
        return None;
    }

    let after_git = &trimmed[prefix.len()..];

    for pattern in GIT_PATTERNS {
        let subcommand_with_args = format!("{} ", pattern.subcommand);

        if after_git == pattern.subcommand {
            // Exact match (e.g., "git add" without args)
            let detail = if pattern.subcommand == "add" {
                Some("all".to_string())
            } else {
                None
            };
            return Some(GitCommandLabel {
                label: pattern.label,
                detail,
            });
        } else if after_git.starts_with(&subcommand_with_args) {
            // Prefix match with arguments (e.g., "git add file.txt")
            let detail = if pattern.show_args_as_detail {
                let args = after_git[subcommand_with_args.len()..].trim().to_string();
                if args.is_empty() { None } else { Some(args) }
            } else {
                None
            };
            return Some(GitCommandLabel {
                label: pattern.label,
                detail,
            });
        }
    }

    None
}

/// Extract command string from input_preview, handling both plain commands and JSON-wrapped commands
/// like {"command":"git diff","description":"..."}
fn extract_command_from_input(input_preview: Option<&str>) -> Option<String> {
    let input = input_preview?.trim();
    if input.is_empty() {
        return None;
    }

    // Try to parse as JSON to extract "command" field
    if input.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(input) {
            if let Some(command) = parsed.get("command").and_then(|v| v.as_str()) {
                return Some(command.to_string());
            }
        }
    }

    // Return as-is if not JSON
    Some(input.to_string())
}

fn format_tool_invocation(
    name: &str,
    input_preview: Option<&str>,
    display_command: Option<&str>,
) -> String {
    // Use the extracted display_command if available, otherwise use input_preview
    let display = display_command.unwrap_or_else(|| input_preview.unwrap_or(""));
    match display.trim().is_empty() {
        false => format!("{name}({display})"),
        true => name.to_string(),
    }
}

fn render_prefixed_tool_output(
    output: &str,
    width: usize,
    initial_prefix: &str,
    continuation_prefix: &str,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let formatted = format_json_compact(output).unwrap_or_else(|| output.to_string());
    render_prefixed_text_block(
        &formatted,
        width,
        initial_prefix,
        continuation_prefix,
        Style::default().add_modifier(Modifier::DIM),
        mode,
    )
}

fn render_prefixed_text_block(
    text: &str,
    width: usize,
    initial_prefix: &str,
    continuation_prefix: &str,
    style: Style,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let mut rendered = Vec::new();
    let content_width = width
        .saturating_sub(initial_prefix.len().max(continuation_prefix.len()))
        .max(1);

    for (index, raw_line) in text.lines().enumerate() {
        let prefix = if index == 0 {
            initial_prefix
        } else {
            continuation_prefix
        };

        for (segment_index, segment) in wrap_text_segments(raw_line, content_width)
            .into_iter()
            .enumerate()
        {
            let leader = if segment_index == 0 {
                prefix.to_string()
            } else {
                continuation_prefix.to_string()
            };
            rendered.push(Line::from(vec![
                Span::styled(
                    leader,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(segment, style),
            ]));
        }
    }

    match mode {
        ToolRenderMode::Preview => truncate_lines_middle(rendered, GENERIC_TOOL_PREVIEW_MAX_LINES),
        ToolRenderMode::Full => rendered,
    }
}

fn wrap_text_segments(text: &str, width: usize) -> Vec<String> {
    let options = Options::new(width.max(1)).word_splitter(WordSplitter::NoHyphenation);
    let wrapped = wrap(text, options)
        .into_iter()
        .map(|line| line.into_owned())
        .flat_map(|line| chunk_long_segment(&line, width.max(1)))
        .collect::<Vec<_>>();

    if wrapped.is_empty() {
        vec![String::new()]
    } else {
        wrapped
    }
}

fn chunk_long_segment(text: &str, width: usize) -> Vec<String> {
    if text.chars().count() <= width.max(1) {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if current.chars().count() >= width.max(1) {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn truncate_segments_from_start(lines: Vec<String>, keep: usize) -> Vec<String> {
    if lines.len() <= keep {
        return lines;
    }
    if keep == 0 {
        return vec![format!("… +{} lines", lines.len())];
    }

    let omitted = lines.len().saturating_sub(keep);
    let mut out = lines.into_iter().take(keep).collect::<Vec<_>>();
    out.pop();
    out.push(format!("… +{} lines", omitted + 1));
    out
}

fn truncate_lines_middle(lines: Vec<Line<'static>>, max_lines: usize) -> Vec<Line<'static>> {
    if lines.len() <= max_lines {
        return lines;
    }
    if max_lines == 0 {
        return Vec::new();
    }
    if max_lines == 1 {
        return vec![ellipsis_line(lines.len())];
    }

    let head = (max_lines - 1) / 2;
    let tail = max_lines.saturating_sub(head + 1);
    let omitted = lines.len().saturating_sub(head + tail);

    let mut out = Vec::new();
    out.extend(lines[..head].iter().cloned());
    out.push(ellipsis_line(omitted));
    out.extend(lines[lines.len().saturating_sub(tail)..].iter().cloned());
    out
}

fn ellipsis_line(omitted: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            DETAIL_CONTINUATION_PREFIX,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("… +{omitted} lines ({TRANSCRIPT_HINT})"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
    ])
}

/// Renders Edit tool calls with codex-style diff display.
fn render_edit_tool_lines(
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    success: bool,
    started: bool,
    _width: usize,
    _mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let bullet_style = if started {
        Style::default().fg(Color::Blue)
    } else if success {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };

    // Parse Edit tool input to extract file path and compute diff
    let edit_input = parse_edit_input(input_preview);
    let (added, removed) = compute_edit_stats(&edit_input);

    // Header line: "• Edited path (+X -Y)" or "• Editing path" when in progress
    let header_prefix = if started { "Editing" } else { "Edited" };
    if let Some(path) = &edit_input.file_path {
        let stats = if started {
            String::new()
        } else {
            format!(" (+{} -{})", added, removed)
        };
        lines.push(Line::from(vec![
            Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_prefix.to_string(),
                bullet_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(path.clone(), Style::default().fg(Color::Cyan)),
            Span::styled(stats, Style::default().add_modifier(Modifier::DIM)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_prefix.to_string(),
                bullet_style.add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Render full diff lines (no truncation) with background colors
    if !started && let Some(diff_text) = build_edit_diff_text(&edit_input) {
        let diff_lines = render_edit_diff_with_background(&diff_text);
        lines.extend(diff_lines);
    }

    // Show output preview if present and not a success message
    if let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) {
        // Skip common success messages like "String replaced" or "File updated successfully"
        let is_success_message = output.trim() == "String replaced."
            || output.trim() == "The file was edited successfully."
            || output.contains("successfully edited");
        if !is_success_message {
            lines.push(Line::from(""));
            for line in output.lines() {
                lines.push(Line::from(vec![
                    Span::styled(
                        DETAIL_CONTINUATION_PREFIX,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                    Span::styled(
                        line.to_string(),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ]));
            }
        }
    }

    lines
}

/// Renders diff with background colors for added/removed lines.
fn render_edit_diff_with_background(diff_text: &str) -> Vec<Line<'static>> {
    let Ok(patch) = Patch::from_str(diff_text) else {
        return Vec::new();
    };

    // Compute line number width
    let max_line_no = patch
        .hunks()
        .iter()
        .flat_map(|hunk| [hunk.old_range().start(), hunk.new_range().start()])
        .max()
        .unwrap_or(1);
    let line_no_width = max_line_no.to_string().len().max(1);

    let mut lines = Vec::new();
    let mut first_hunk = true;

    for hunk in patch.hunks() {
        // Add spacing between hunks
        if !first_hunk {
            lines.push(Line::from(""));
        }
        first_hunk = false;

        // Render hunk header @@ -X,Y +X,Y @@
        lines.push(Line::from(vec![
            Span::styled(
                DETAIL_CONTINUATION_PREFIX,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!(
                    "@@ -{} +{} @@",
                    format_hunk_range(hunk.old_range().start(), hunk.old_range().len()),
                    format_hunk_range(hunk.new_range().start(), hunk.new_range().len())
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        let mut old_ln = hunk.old_range().start();
        let mut new_ln = hunk.new_range().start();

        for line in hunk.lines() {
            match line {
                diffy::Line::Insert(text) => {
                    // Added line: light green background
                    let content = text.trim_end_matches('\n');
                    lines.push(render_edit_diff_line(
                        line_no_width,
                        None,
                        Some(new_ln),
                        '+',
                        content,
                        Color::Green,
                        true, // has background
                    ));
                    new_ln += 1;
                }
                diffy::Line::Delete(text) => {
                    // Removed line: light red background
                    let content = text.trim_end_matches('\n');
                    lines.push(render_edit_diff_line(
                        line_no_width,
                        Some(old_ln),
                        None,
                        '-',
                        content,
                        Color::Red,
                        true, // has background
                    ));
                    old_ln += 1;
                }
                diffy::Line::Context(text) => {
                    // Context line: no special styling
                    let content = text.trim_end_matches('\n');
                    lines.push(render_edit_diff_line(
                        line_no_width,
                        Some(old_ln),
                        Some(new_ln),
                        ' ',
                        content,
                        Color::Reset,
                        false,
                    ));
                    old_ln += 1;
                    new_ln += 1;
                }
            }
        }
    }

    lines
}

fn render_edit_diff_line(
    line_no_width: usize,
    old_line_no: Option<usize>,
    new_line_no: Option<usize>,
    sign: char,
    content: &str,
    fg_color: Color,
    has_bg: bool,
) -> Line<'static> {
    // Line number from the appropriate side
    let line_no = new_line_no.or(old_line_no);
    let number = line_no
        .map(|value| format!("{value:>width$}", width = line_no_width))
        .unwrap_or_else(|| " ".repeat(line_no_width));

    // Style with light background colors (darker variants)
    let (bg_color, fg_for_sign) = if has_bg {
        if fg_color == Color::Green {
            // Added line: darker light green background
            (Color::Indexed(28), Color::Green) // Dark green (#008700)
        } else {
            // Removed line: darker light red background
            (Color::Indexed(52), Color::Red) // Dark red (#5f0000)
        }
    } else {
        (Color::Reset, Color::DarkGray)
    };

    let content_style = if has_bg {
        Style::default().fg(Color::White).bg(bg_color)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let sign_style = if has_bg {
        Style::default().fg(fg_for_sign).bg(bg_color)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };

    Line::from(vec![
        Span::styled(
            DETAIL_CONTINUATION_PREFIX,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            number,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::raw(" "),
        Span::styled(sign.to_string(), sign_style),
        Span::styled(content.to_string(), content_style),
    ])
}

fn format_hunk_range(start: usize, len: usize) -> String {
    if len == 1 {
        start.to_string()
    } else {
        format!("{start},{len}")
    }
}

/// Builds a unified diff text from Edit tool input for rendering.
fn build_edit_diff_text(edit_input: &EditInput) -> Option<String> {
    let old = edit_input.old_string.as_ref()?;
    let new = edit_input.new_string.as_ref()?;

    // Create a unified diff patch using diffy
    let patch = diffy::create_patch(old, new);

    // Format as unified diff text
    Some(patch.to_string())
}

/// Computes added/removed line counts from Edit tool input.
fn compute_edit_stats(edit_input: &EditInput) -> (usize, usize) {
    let Some(old) = &edit_input.old_string else {
        return (0, 0);
    };
    let Some(new) = &edit_input.new_string else {
        return (0, 0);
    };

    let patch = diffy::create_patch(old, new);

    let added = patch
        .hunks()
        .iter()
        .flat_map(|hunk| hunk.lines())
        .filter(|line| matches!(line, diffy::Line::Insert(_)))
        .count();

    let removed = patch
        .hunks()
        .iter()
        .flat_map(|hunk| hunk.lines())
        .filter(|line| matches!(line, diffy::Line::Delete(_)))
        .count();

    (added, removed)
}

/// Parsed Edit tool input.
#[derive(Debug, Clone, Default)]
struct EditInput {
    file_path: Option<String>,
    old_string: Option<String>,
    new_string: Option<String>,
}

fn parse_edit_input(input_preview: Option<&str>) -> EditInput {
    let Some(input) = input_preview else {
        return EditInput::default();
    };

    // Try to parse as JSON
    let parsed: serde_json::Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) => return EditInput::default(),
    };
    EditInput {
        file_path: parsed
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(String::from),
        old_string: parsed
            .get("old_string")
            .and_then(|v| v.as_str())
            .map(String::from),
        new_string: parsed
            .get("new_string")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}

#[cfg(test)]
mod edit_tool_tests {
    use super::*;

    fn lines_to_strings(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn renders_edit_tool_with_file_path_and_diff_stats() {
        let input = serde_json::json!({
            "file_path": "src/main.rs",
            "old_string": "fn old() {}\n",
            "new_string": "fn new() {}\n"
        });
        let input_preview = Some(serde_json::to_string(&input).unwrap());
        let lines = render_edit_tool_lines(
            input_preview.as_deref(),
            None,
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("Edited")));
        assert!(rendered.iter().any(|line| line.contains("src/main.rs")));
        assert!(rendered.iter().any(|line| line.contains("(+1 -1)")));
    }

    #[test]
    fn renders_edit_tool_in_progress_without_stats() {
        let input = serde_json::json!({
            "file_path": "src/main.rs",
            "old_string": "old",
            "new_string": "new"
        });
        let input_preview = Some(serde_json::to_string(&input).unwrap());
        let lines = render_edit_tool_lines(
            input_preview.as_deref(),
            None,
            true,
            true,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("Editing")));
        assert!(rendered.iter().any(|line| line.contains("src/main.rs")));
        assert!(
            !rendered
                .iter()
                .any(|line| line.contains("(+") || line.contains("(-"))
        );
    }

    #[test]
    fn renders_edit_diff_with_line_numbers() {
        let input = serde_json::json!({
            "file_path": "src/main.rs",
            "old_string": "line 1\nline 2\nline 3\n",
            "new_string": "line 1\nmodified line\nline 3\n"
        });
        let input_preview = Some(serde_json::to_string(&input).unwrap());
        let lines = render_edit_tool_lines(
            input_preview.as_deref(),
            None,
            true,
            false,
            80,
            ToolRenderMode::Full,
        );
        let rendered = lines_to_strings(&lines);

        // Should show line numbers
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("1") && line.contains("line 1"))
        );
        // Should show removed line with '-' marker
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("-") && line.contains("line 2"))
        );
        // Should show added line with '+' marker
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("+") && line.contains("modified line"))
        );
    }

    #[test]
    fn skips_common_success_messages() {
        let input = serde_json::json!({
            "file_path": "src/main.rs",
            "old_string": "old",
            "new_string": "new"
        });
        let input_preview = Some(serde_json::to_string(&input).unwrap());
        let lines = render_edit_tool_lines(
            input_preview.as_deref(),
            Some("String replaced."),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        // Should not show "String replaced." message
        assert!(!rendered.iter().any(|line| line.contains("String replaced")));
    }

    #[test]
    fn handles_missing_input_gracefully() {
        let lines = render_edit_tool_lines(None, None, true, false, 80, ToolRenderMode::Preview);
        let rendered = lines_to_strings(&lines);

        // Should still show header
        assert!(rendered.iter().any(|line| line.contains("Edited")));
    }
}

#[cfg(test)]
mod git_detection_tests {
    use super::*;

    fn lines_to_strings(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn detect_git_command_recognizes_git_diff() {
        assert!(detect_git_command("git diff").is_some());
        assert!(detect_git_command("git diff --staged").is_some());
        assert!(detect_git_command("git diff --cached").is_some());
        assert!(detect_git_command("git diff --stat").is_some());
        assert!(detect_git_command("git diff --stat HEAD~1").is_some());
        assert!(detect_git_command("git diff HEAD").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_status() {
        assert!(detect_git_command("git status").is_some());
        assert!(detect_git_command("git status --porcelain").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_commit() {
        assert!(detect_git_command("git commit -m \"fix: update\"").is_some());
        assert!(detect_git_command("git commit").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_add() {
        assert!(detect_git_command("git add src/main.rs").is_some());
        assert!(detect_git_command("git add .").is_some());
        assert!(detect_git_command("git add -A").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_branch() {
        assert!(detect_git_command("git branch").is_some());
        assert!(detect_git_command("git branch -a").is_some());
        assert!(detect_git_command("git branch feature/test").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_checkout() {
        assert!(detect_git_command("git checkout main").is_some());
        assert!(detect_git_command("git checkout -b feature/test").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_push() {
        assert!(detect_git_command("git push").is_some());
        assert!(detect_git_command("git push origin main").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_pull() {
        assert!(detect_git_command("git pull").is_some());
        assert!(detect_git_command("git pull origin main").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_fetch() {
        assert!(detect_git_command("git fetch").is_some());
        assert!(detect_git_command("git fetch origin").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_stash() {
        assert!(detect_git_command("git stash").is_some());
        assert!(detect_git_command("git stash pop").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_merge() {
        assert!(detect_git_command("git merge feature/test").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_rebase() {
        assert!(detect_git_command("git rebase main").is_some());
        assert!(detect_git_command("git rebase -i HEAD~3").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_clone() {
        assert!(detect_git_command("git clone https://github.com/user/repo").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_show() {
        assert!(detect_git_command("git show HEAD").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_log() {
        assert!(detect_git_command("git log").is_some());
        assert!(detect_git_command("git log --oneline -5").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_remote() {
        assert!(detect_git_command("git remote -v").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_config() {
        assert!(detect_git_command("git config --global user.name").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_reset() {
        assert!(detect_git_command("git reset HEAD~1").is_some());
    }

    #[test]
    fn detect_git_command_rejects_non_git_commands() {
        assert!(detect_git_command("ls -la").is_none());
        assert!(detect_git_command("npm install").is_none());
        assert!(detect_git_command("cargo build").is_none());
        assert!(detect_git_command("echo hello").is_none());
    }

    #[test]
    fn detect_git_command_handles_whitespace() {
        let result = detect_git_command("  git diff  ");
        assert!(result.is_some());
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_diff() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git diff --stat"),
            Some("file.rs | 10 +++----"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        // Should show "Git Diff" label instead of full command
        assert!(
            rendered.iter().any(|line| line.contains("Git Diff")),
            "Expected 'Git Diff' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_commit() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git commit -m \"fix: update\""),
            Some("[main abc123] fix: update"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Commit")),
            "Expected 'Git Commit' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_status() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git status"),
            Some("On branch main"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Status")),
            "Expected 'Git Status' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_add() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git add src/main.rs"),
            Some(""),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Add")),
            "Expected 'Git Add' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_branch() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git branch"),
            Some("  develop\n* main"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Branch")),
            "Expected 'Git Branch' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_git_label_for_git_push() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git push origin main"),
            Some("To https://github.com/user/repo\n   abc123..def456  main -> main"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Push")),
            "Expected 'Git Push' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_shows_non_git_tool_invocation() {
        let lines = render_generic_tool_call_lines(
            "some_tool",
            Some("file.txt"),
            Some("output"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        // Should show tool name in parentheses
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("some_tool(file.txt)")),
            "Expected 'some_tool(file.txt)' in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_uses_magenta_color_for_git_commands() {
        let lines = render_generic_tool_call_lines(
            "exec_command",
            Some("git diff"),
            Some(""),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );

        // Find the line with the Git Diff label
        for line in &lines {
            let line_str = line
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>();
            if line_str.contains("Git Diff") {
                // All spans should use magenta color (or style)
                for span in &line.spans {
                    if let Some(fg) = span.style.fg {
                        assert_eq!(fg, Color::Magenta, "Expected magenta color for git command");
                    }
                }
                return;
            }
        }
        panic!("Expected to find Git Diff label in output: {:?}", lines);
    }

    // Tests for additional git commands added
    #[test]
    fn detect_git_command_recognizes_git_init() {
        assert!(detect_git_command("git init").is_some());
        assert!(detect_git_command("git init my-repo").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_restore() {
        assert!(detect_git_command("git restore src/main.rs").is_some());
        assert!(detect_git_command("git restore --staged .").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_switch() {
        assert!(detect_git_command("git switch main").is_some());
        assert!(detect_git_command("git switch -c feature/test").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_clean() {
        assert!(detect_git_command("git clean -fd").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_rm() {
        assert!(detect_git_command("git rm src/main.rs").is_some());
        assert!(detect_git_command("git rm -f src/main.rs").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_mv() {
        assert!(detect_git_command("git mv old.txt new.txt").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_cherry_pick() {
        assert!(detect_git_command("git cherry-pick abc123").is_some());
        assert!(detect_git_command("git cherry-pick --no-commit abc123").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_revert() {
        assert!(detect_git_command("git revert HEAD~1").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_tag() {
        assert!(detect_git_command("git tag v1.0.0").is_some());
        assert!(detect_git_command("git tag -a v1.0.0 -m \"release\"").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_worktree() {
        assert!(detect_git_command("git worktree list").is_some());
        assert!(detect_git_command("git worktree add feature origin/main").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_grep() {
        assert!(detect_git_command("git grep \"pattern\"").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_bisect() {
        assert!(detect_git_command("git bisect start").is_some());
        assert!(detect_git_command("git bisect bad").is_some());
        assert!(detect_git_command("git bisect good v1.0.0").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_describe() {
        assert!(detect_git_command("git describe").is_some());
        assert!(detect_git_command("git describe --tags").is_some());
    }

    #[test]
    fn detect_git_command_recognizes_git_reflog() {
        assert!(detect_git_command("git reflog").is_some());
        assert!(detect_git_command("git reflog show --all").is_some());
    }

    #[test]
    fn render_exec_header_shows_git_label_for_git_diff() {
        let lines = render_exec_header_lines(
            "Ran",
            "git diff --stat",
            Style::default().fg(Color::Green),
            80,
        );
        let rendered = lines_to_strings(&lines);

        // Should show friendly label instead of raw command
        assert!(
            rendered.iter().any(|line| line.contains("Git Diff")),
            "Expected 'Git Diff' in: {:?}",
            rendered
        );
        // Should not show the raw command
        assert!(
            !rendered.iter().any(|line| line.contains("git diff --stat")),
            "Should not contain raw command: {:?}",
            rendered
        );
    }

    #[test]
    fn render_exec_header_shows_git_label_for_git_status() {
        let lines =
            render_exec_header_lines("Ran", "git status", Style::default().fg(Color::Green), 80);
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Status")),
            "Expected 'Git Status' in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_exec_header_shows_non_git_command() {
        let lines =
            render_exec_header_lines("Ran", "cargo build", Style::default().fg(Color::Green), 80);
        let rendered = lines_to_strings(&lines);

        // Non-git commands should show the raw command
        assert!(
            rendered.iter().any(|line| line.contains("cargo build")),
            "Expected 'cargo build' in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_exec_transcript_shows_git_label_for_git_diff() {
        let lines = render_exec_transcript_lines(
            None,
            Some("git diff --stat HEAD~1"),
            Some("file.rs | 10 +++----"),
            ExecCommandStatus::Completed,
            Some(0),
            Some(100),
            80,
        );
        let rendered = lines_to_strings(&lines);

        // Transcript should show git-friendly label
        assert!(
            rendered.iter().any(|line| line.contains("Git Diff")),
            "Expected 'Git Diff' in transcript: {:?}",
            rendered
        );
        // Should not show raw command
        assert!(
            !rendered.iter().any(|line| line.contains("git diff")),
            "Should not contain raw command in transcript: {:?}",
            rendered
        );
    }

    #[test]
    fn render_exec_transcript_shows_non_git_command() {
        let lines = render_exec_transcript_lines(
            None,
            Some("cargo build"),
            Some("Compiling..."),
            ExecCommandStatus::Completed,
            Some(0),
            Some(1000),
            80,
        );
        let rendered = lines_to_strings(&lines);

        // Non-git commands should show raw command
        assert!(
            rendered.iter().any(|line| line.contains("cargo build")),
            "Expected 'cargo build' in: {:?}",
            rendered
        );
    }

    #[test]
    fn extract_command_from_input_parses_json_command() {
        let json_input = r#"{"command":"git diff --stat","description":"Show diff stats"}"#;
        let extracted = extract_command_from_input(Some(json_input));
        assert_eq!(extracted, Some("git diff --stat".to_string()));
    }

    #[test]
    fn extract_command_from_input_handles_plain_command() {
        let plain_input = "git status";
        let extracted = extract_command_from_input(Some(plain_input));
        assert_eq!(extracted, Some("git status".to_string()));
    }

    #[test]
    fn extract_command_from_input_handles_none() {
        assert_eq!(extract_command_from_input(None), None);
    }

    #[test]
    fn extract_command_from_input_handles_empty_string() {
        assert_eq!(extract_command_from_input(Some("")), None);
        assert_eq!(extract_command_from_input(Some("  ")), None);
    }

    #[test]
    fn render_generic_tool_call_extracts_git_from_json_command() {
        let json_input = r#"{"command":"git diff --stat","description":"Show diff stats"}"#;
        let lines = render_generic_tool_call_lines(
            "Bash",
            Some(json_input),
            Some("file.rs | 10 +++----"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        // Should show git-friendly label
        assert!(
            rendered.iter().any(|line| line.contains("Git Diff")),
            "Expected 'Git Diff' label in: {:?}",
            rendered
        );
        // Should NOT show the raw JSON
        assert!(
            !rendered.iter().any(|line| line.contains("command")),
            "Should not contain 'command' in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_extracts_git_commit_from_json() {
        let json_input =
            r#"{"command":"git commit -m \"fix: update\"","description":"Commit changes"}"#;
        let lines = render_generic_tool_call_lines(
            "Bash",
            Some(json_input),
            Some("[main abc123] fix: update"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(
            rendered.iter().any(|line| line.contains("Git Commit")),
            "Expected 'Git Commit' label in: {:?}",
            rendered
        );
    }

    #[test]
    fn render_generic_tool_call_non_git_json_shows_command() {
        let json_input = r#"{"command":"npm install","description":"Install deps"}"#;
        let lines = render_generic_tool_call_lines(
            "Bash",
            Some(json_input),
            Some("added 100 packages"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        // Should show the command, not the JSON
        assert!(
            rendered.iter().any(|line| line.contains("npm install")),
            "Expected 'npm install' in: {:?}",
            rendered
        );
        assert!(
            !rendered.iter().any(|line| line.contains("command")),
            "Should not contain 'command' in: {:?}",
            rendered
        );
    }
}
