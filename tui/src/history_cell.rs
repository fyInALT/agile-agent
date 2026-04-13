use agent_core::app::TranscriptEntry;
use agent_core::tool_calls::ExecCommandStatus;
use agent_core::tool_calls::McpInvocation;
use agent_core::tool_calls::McpToolCallStatus;
use agent_core::tool_calls::PatchChange;
use agent_core::tool_calls::PatchChangeKind;
use agent_core::tool_calls::PatchApplyStatus;
use agent_core::tool_calls::WebSearchAction;
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
        TranscriptEntry::ViewImage { call_id: _, path } => Box::new(ViewImageHistoryCell {
            path: path.clone(),
        }),
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
        render_exec_history_lines(
            self.source.as_deref(),
            self.input_preview.as_deref(),
            self.output_preview.as_deref(),
            self.status,
            self.exit_code,
            self.duration_ms,
            width,
            ToolRenderMode::Full,
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
            lines.extend(render_exec_history_lines(
                call.source.as_deref(),
                call.input_preview.as_deref(),
                call.output_preview.as_deref(),
                call.status,
                call.exit_code,
                call.duration_ms,
                width,
                ToolRenderMode::Full,
            ));
        }
        lines
    }
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
            Span::styled("Viewed Image".to_string(), Style::default().add_modifier(Modifier::BOLD)),
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
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ),
            Span::styled(
                "(no output)",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
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
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
    };
    let title = if any_started { "Exploring" } else { "Explored" };
    lines.push(Line::from(vec![
        Span::styled("• ", bullet_style),
        Span::styled(title.to_string(), Style::default().add_modifier(Modifier::BOLD)),
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
                    if let ExploringOp::Read(name) = op && !names.contains(name) {
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
        Some(WebSearchAction::Search { query: action_query, queries }) => action_query
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
            block.get("uri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
        ),
        Some("resource") => format!(
            "embedded resource: {}",
            block.get("uri")
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

    let command_lines = command.lines().collect::<Vec<_>>();
    let first_segments = wrap_text_segments(
        command_lines.first().copied().unwrap_or_default(),
        width.saturating_sub(header_prefix.len() + 1).max(1),
    );
    let first_segment = first_segments.first().cloned().unwrap_or_default();
    let mut header = header_prefix;
    if !first_segment.is_empty() {
        header.push(' ');
        header.push_str(&first_segment);
    }
    lines.push(Line::from(vec![Span::styled(
        header,
        bullet_style.add_modifier(Modifier::BOLD),
    )]));

    let mut continuation_segments = first_segments.into_iter().skip(1).collect::<Vec<_>>();
    for raw_line in command_lines.into_iter().skip(1) {
        continuation_segments.extend(wrap_text_segments(
            raw_line,
            width
                .saturating_sub(COMMAND_CONTINUATION_PREFIX.len())
                .max(1),
        ));
    }

    for segment in truncate_segments_from_start(
        continuation_segments,
        EXEC_COMMAND_CONTINUATION_MAX_LINES,
    ) {
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
    let mut lines = Vec::new();
    let bullet_style = if started {
        Style::default().fg(Color::Blue)
    } else if success {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    let header_text = if started { "Calling" } else { "Called" };
    let invocation = format_tool_invocation(name, input_preview);
    let inline_invocation = !invocation.is_empty()
        && invocation.len() + header_text.len() + 3 <= width.max(1);

    if inline_invocation {
        lines.push(Line::from(vec![
            Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_text.to_string(),
                bullet_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(invocation.clone()),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("• ", bullet_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                header_text.to_string(),
                bullet_style.add_modifier(Modifier::BOLD),
            ),
        ]));

        if !invocation.is_empty() {
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
            PatchApplyStatus::InProgress => {
            "• Applying patch"
            }
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
        lines.extend(render_patch_diff_lines(change, DETAIL_CONTINUATION_PREFIX, mode));
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
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            number,
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
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
    Some((parse_hunk_range_start(old_part), parse_hunk_range_start(new_part)))
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

fn format_tool_invocation(name: &str, input_preview: Option<&str>) -> String {
    match input_preview.filter(|value| !value.trim().is_empty()) {
        Some(input) => format!("{name}({input})"),
        None => name.to_string(),
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
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
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
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("… +{omitted} lines ({TRANSCRIPT_HINT})"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ])
}
