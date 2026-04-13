use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use diffy::Patch;
use textwrap::wrap;
use unicode_segmentation::UnicodeSegmentation;

const TOOL_PREVIEW_MAX_LINES: usize = 8;
const TOOL_PREVIEW_HEAD_LINES: usize = 4;
const TOOL_PREVIEW_TAIL_LINES: usize = 3;
const TOOL_OUTPUT_INITIAL_PREFIX: &str = "  └ ";
const TOOL_OUTPUT_CONTINUATION_PREFIX: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRenderMode {
    Preview,
    Full,
}

pub fn render_tool_call_lines(
    name: &str,
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    success: bool,
    started: bool,
    width: usize,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(tool_header_line(name, success, started));

    if let Some(input) = input_preview.filter(|value| !value.trim().is_empty()) {
        if name == "patch_apply" {
            lines.extend(render_patch_summary_block(input, width));
        } else {
            lines.extend(render_input_block(name, input, width));
        }
    }

    if let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) {
        lines.extend(render_output_block(name, input_preview, output, width, mode));
    } else if !started && name == "exec_command" {
        lines.push(Line::from(vec![
            Span::styled(
                TOOL_OUTPUT_INITIAL_PREFIX,
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ),
            Span::styled(
                "(no output)",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ),
        ]));
    }

    lines
}

fn tool_header_line(name: &str, success: bool, started: bool) -> Line<'static> {
    let (text, style) = if started {
        match name {
            "exec_command" => ("• running command".to_string(), Style::default().fg(Color::Blue)),
            "patch_apply" => ("• applying patch".to_string(), Style::default().fg(Color::Blue)),
            _ => (
                format!("• running tool {name}"),
                Style::default().fg(Color::Blue),
            ),
        }
    } else if success {
        match name {
            "exec_command" => (
                "• finished command".to_string(),
                Style::default().fg(Color::Green),
            ),
            "patch_apply" => (
                "• applied patch".to_string(),
                Style::default().fg(Color::Green),
            ),
            _ => (
                format!("• finished tool {name}"),
                Style::default().fg(Color::Green),
            ),
        }
    } else {
        match name {
            "exec_command" => (
                "• failed command".to_string(),
                Style::default().fg(Color::Red),
            ),
            "patch_apply" => (
                "• failed patch".to_string(),
                Style::default().fg(Color::Red),
            ),
            _ => (
                format!("• failed tool {name}"),
                Style::default().fg(Color::Red),
            ),
        }
    };

    Line::from(Span::styled(text, style))
}

fn render_input_block(name: &str, input: &str, width: usize) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let text = if name == "exec_command" {
        format!("$ {}", truncate_graphemes(input, body_width.saturating_mul(2)))
    } else {
        truncate_graphemes(input, body_width.saturating_mul(2))
    };

    wrap_prefixed(
        "  └ ",
        &text,
        if name == "exec_command" {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        },
        width,
    )
}

fn render_patch_summary_block(summary: &str, width: usize) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let mut lines = Vec::new();

    for (index, line) in summary.lines().enumerate() {
        let style = if line.starts_with("A ") {
            Style::default().fg(Color::Green)
        } else if line.starts_with("D ") {
            Style::default().fg(Color::Red)
        } else if line.starts_with("R ") {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        lines.extend(wrap_prefixed(
            if index == 0 {
                TOOL_OUTPUT_INITIAL_PREFIX
            } else {
                TOOL_OUTPUT_CONTINUATION_PREFIX
            },
            line,
            style,
            body_width + 4,
        ));
    }

    lines
}

fn render_output_block(
    name: &str,
    input_preview: Option<&str>,
    output: &str,
    width: usize,
    mode: ToolRenderMode,
) -> Vec<Line<'static>> {
    if looks_like_diff(input_preview, output) {
        return render_diff_block(output, width, mode);
    }

    if looks_like_git_status(input_preview, output) {
        return render_git_status_block(output, width, mode);
    }

    if looks_like_git_log(input_preview, output) {
        return render_git_log_block(output, width, mode);
    }

    let formatted = if let Some(compact_json) = format_json_compact(output) {
        compact_json
    } else {
        output.to_string()
    };

    render_text_block(name, &formatted, width, mode)
}

fn looks_like_diff(input_preview: Option<&str>, output: &str) -> bool {
    input_preview
        .is_some_and(|input| input.contains("git diff") || input.contains("git show"))
        || output.starts_with("diff --git ")
        || (output.contains("\n--- ") && output.contains("\n+++ "))
}

fn looks_like_git_status(input_preview: Option<&str>, output: &str) -> bool {
    input_preview.is_some_and(|input| input.contains("git status"))
        || output.starts_with("On branch ")
}

fn looks_like_git_log(input_preview: Option<&str>, output: &str) -> bool {
    if input_preview.is_some_and(|input| input.contains("git log")) {
        return true;
    }

    let non_empty = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .collect::<Vec<_>>();
    !non_empty.is_empty() && non_empty.iter().all(|line| looks_like_git_log_line(line))
}

fn render_diff_block(output: &str, _width: usize, mode: ToolRenderMode) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.extend(render_diff_summary(output));
    let rendered = render_unified_diff_lines(output);
    lines.extend(match mode {
        ToolRenderMode::Preview => truncate_rendered_lines_middle(rendered, TOOL_PREVIEW_MAX_LINES),
        ToolRenderMode::Full => rendered,
    });

    lines
}

fn render_diff_summary(output: &str) -> Vec<Line<'static>> {
    let summaries = summarize_unified_diff(output);
    if summaries.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for summary in summaries {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(summary.path, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(
                format!("(+{} -{})", summary.added, summary.removed),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines
}

fn render_git_status_block(output: &str, width: usize, mode: ToolRenderMode) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let rendered = render_wrapped_preview_lines(
        output.lines().map(ToOwned::to_owned).collect(),
        body_width + 4,
        git_status_style_for_line,
    );
    match mode {
        ToolRenderMode::Preview => truncate_rendered_lines_middle(rendered, TOOL_PREVIEW_MAX_LINES),
        ToolRenderMode::Full => rendered,
    }
}

fn git_status_style_for_line(line: &str) -> Style {
    let trimmed = line.trim_start();
    if line.starts_with("On branch ") || line.starts_with("Your branch ") {
        Style::default().fg(Color::Cyan)
    } else if line.ends_with(':') {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if trimmed.starts_with("modified:")
        || trimmed.starts_with("deleted:")
        || trimmed.starts_with("renamed:")
    {
        Style::default().fg(Color::Red)
    } else if trimmed.starts_with("new file:") || trimmed.starts_with("added:") {
        Style::default().fg(Color::Green)
    } else if trimmed.starts_with("Changes not staged")
        || trimmed.starts_with("Changes to be committed")
        || trimmed.starts_with("Untracked files")
    {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

fn render_git_log_block(output: &str, width: usize, mode: ToolRenderMode) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let rendered = render_wrapped_preview_lines(
        output
            .lines()
            .map(|line| {
                if let Some((hash, rest)) = split_git_log_line(line) {
                    format!("{hash} {rest}")
                } else {
                    line.to_string()
                }
            })
            .collect(),
        body_width + 4,
        |_| Style::default().add_modifier(Modifier::DIM),
    );
    match mode {
        ToolRenderMode::Preview => truncate_rendered_lines_middle(rendered, TOOL_PREVIEW_MAX_LINES),
        ToolRenderMode::Full => rendered,
    }
}

fn diff_style_for_line(line: &str) -> Style {
    if line.starts_with('+') && !line.starts_with("+++") {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') && !line.starts_with("---") {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
    {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

fn render_unified_diff_lines(output: &str) -> Vec<Line<'static>> {
    let Ok(patch) = Patch::from_str(output) else {
        return render_wrapped_preview_lines(
            output.lines().map(ToOwned::to_owned).collect(),
            120,
            diff_style_for_line,
        );
    };

    let max_line_number = patch
        .hunks()
        .iter()
        .flat_map(|hunk| [hunk.old_range().start(), hunk.new_range().start()])
        .max()
        .unwrap_or(1);
    let line_no_width = max_line_number.to_string().len().max(1);

    let mut out = Vec::new();
    let mut first_hunk = true;
    for hunk in patch.hunks() {
        if !first_hunk {
            out.push(Line::from(""));
        }
        first_hunk = false;
        out.push(Line::from(vec![
            Span::styled(
                TOOL_OUTPUT_INITIAL_PREFIX,
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
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
                    let content = text.trim_end_matches('\n');
                    out.push(diff_line(
                        TOOL_OUTPUT_CONTINUATION_PREFIX,
                        line_no_width,
                        Some(new_ln),
                        '+',
                        content,
                        Style::default().fg(Color::Green),
                    ));
                    new_ln += 1;
                }
                diffy::Line::Delete(text) => {
                    let content = text.trim_end_matches('\n');
                    out.push(diff_line(
                        TOOL_OUTPUT_CONTINUATION_PREFIX,
                        line_no_width,
                        Some(old_ln),
                        '-',
                        content,
                        Style::default().fg(Color::Red),
                    ));
                    old_ln += 1;
                }
                diffy::Line::Context(text) => {
                    let content = text.trim_end_matches('\n');
                    out.push(diff_line(
                        TOOL_OUTPUT_CONTINUATION_PREFIX,
                        line_no_width,
                        Some(new_ln),
                        ' ',
                        content,
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                    old_ln += 1;
                    new_ln += 1;
                }
            }
        }
    }

    out
}

fn format_hunk_range(start: usize, len: usize) -> String {
    if len == 1 {
        start.to_string()
    } else {
        format!("{start},{len}")
    }
}

fn diff_line(
    prefix: &str,
    line_no_width: usize,
    line_no: Option<usize>,
    sign: char,
    content: &str,
    style: Style,
) -> Line<'static> {
    let number = line_no
        .map(|value| format!("{value:>width$}", width = line_no_width))
        .unwrap_or_else(|| " ".repeat(line_no_width));
    Line::from(vec![
        Span::styled(
            prefix.to_string(),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(number, Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
        Span::raw(" "),
        Span::styled(format!("{sign}"), style),
        Span::raw(" "),
        Span::styled(content.to_string(), style),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffFileSummary {
    path: String,
    added: usize,
    removed: usize,
}

fn summarize_unified_diff(diff: &str) -> Vec<DiffFileSummary> {
    let mut summaries = Vec::new();
    let mut current: Option<DiffFileSummary> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some(summary) = current.take() {
                summaries.push(summary);
            }
            let path = rest
                .split_whitespace()
                .next()
                .unwrap_or(rest)
                .trim_end_matches(" b/")
                .to_string();
            current = Some(DiffFileSummary {
                path,
                added: 0,
                removed: 0,
            });
            continue;
        }

        if let Some(summary) = current.as_mut() {
            if line.starts_with('+') && !line.starts_with("+++") {
                summary.added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                summary.removed += 1;
            }
        }
    }

    if let Some(summary) = current.take() {
        summaries.push(summary);
    }

    summaries
}

fn render_text_block(name: &str, output: &str, width: usize, mode: ToolRenderMode) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let style = if name == "exec_command" {
        Style::default().add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let rendered = render_wrapped_preview_lines(
        output.lines().map(ToOwned::to_owned).collect(),
        body_width + 4,
        |_| style,
    );
    match mode {
        ToolRenderMode::Preview => truncate_rendered_lines_middle(rendered, TOOL_PREVIEW_MAX_LINES),
        ToolRenderMode::Full => rendered,
    }
}

fn looks_like_git_log_line(line: &str) -> bool {
    split_git_log_line(line).is_some()
}

fn split_git_log_line(line: &str) -> Option<(&str, &str)> {
    let (hash, rest) = line.split_once(' ')?;
    if hash.len() < 7 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some((hash, rest))
}

fn render_wrapped_preview_lines<F>(raw_lines: Vec<String>, width: usize, style_for: F) -> Vec<Line<'static>>
where
    F: Fn(&str) -> Style,
{
    let mut rendered = Vec::new();

    for (index, raw_line) in raw_lines.iter().enumerate() {
        rendered.extend(wrap_prefixed(
            if index == 0 {
                TOOL_OUTPUT_INITIAL_PREFIX
            } else {
                TOOL_OUTPUT_CONTINUATION_PREFIX
            },
            raw_line,
            style_for(raw_line),
            width,
        ));
    }

    rendered
}

fn truncate_rendered_lines_middle(lines: Vec<Line<'static>>, max_rows: usize) -> Vec<Line<'static>> {
    if lines.len() <= max_rows {
        return lines;
    }
    if max_rows == 0 {
        return Vec::new();
    }
    if max_rows == 1 {
        return vec![ellipsis_line(lines.len())];
    }

    let head = TOOL_PREVIEW_HEAD_LINES.min(max_rows.saturating_sub(1));
    let tail = TOOL_PREVIEW_TAIL_LINES.min(max_rows.saturating_sub(head + 1));
    let omitted = lines.len().saturating_sub(head + tail);

    let mut out = Vec::new();
    out.extend(lines[..head].iter().cloned());
    if omitted > 0 {
        out.push(ellipsis_line(omitted));
    }
    out.extend(lines[lines.len().saturating_sub(tail)..].iter().cloned());
    out
}

fn ellipsis_line(omitted: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            TOOL_OUTPUT_CONTINUATION_PREFIX,
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("… +{omitted} lines"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ])
}

fn wrap_prefixed(prefix: &str, text: &str, style: Style, width: usize) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(prefix.len()).max(1);
    wrap(text, content_width)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let leader = if index == 0 {
                prefix.to_string()
            } else {
                " ".repeat(prefix.len())
            };
            Line::from(vec![
                Span::styled(leader, style),
                Span::styled(line.into_owned(), style),
            ])
        })
        .collect()
}

fn truncate_graphemes(text: &str, max_graphemes: usize) -> String {
    let graphemes = text.graphemes(true).collect::<Vec<_>>();
    if graphemes.len() <= max_graphemes {
        return text.to_string();
    }
    if max_graphemes <= 3 {
        return graphemes.into_iter().take(max_graphemes).collect();
    }
    let mut out = graphemes
        .into_iter()
        .take(max_graphemes - 3)
        .collect::<String>();
    out.push_str("...");
    out
}

fn format_json_compact(text: &str) -> Option<String> {
    let json = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let pretty = serde_json::to_string_pretty(&json).ok()?;
    let mut result = String::new();
    let mut chars = pretty.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if !escape_next => {
                in_string = !in_string;
                result.push(ch);
            }
            '\\' if in_string => {
                escape_next = !escape_next;
                result.push(ch);
            }
            '\n' | '\r' if !in_string => {}
            ' ' | '\t' if !in_string => {
                if let Some(&next_ch) = chars.peek()
                    && let Some(last_ch) = result.chars().last()
                    && (last_ch == ':' || last_ch == ',')
                    && !matches!(next_ch, '}' | ']')
                {
                    result.push(' ');
                }
            }
            _ => {
                if escape_next && in_string {
                    escape_next = false;
                }
                result.push(ch);
            }
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::ToolRenderMode;
    use super::render_tool_call_lines;
    use ratatui::text::Line;

    fn lines_to_strings(lines: &[Line<'static>]) -> Vec<String> {
        lines.iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn renders_exec_command_with_diff_preview() {
        let lines = render_tool_call_lines(
            "exec_command",
            Some("git diff README.md"),
            Some("diff --git a/README.md b/README.md\nindex 123..456 100644\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line.contains("finished command")));
        assert!(rendered.iter().any(|line| line.contains("$ git diff README.md")));
        assert!(rendered.iter().any(|line| line.contains("README.md (+1 -1)")));
        assert!(rendered.iter().any(|line| line.contains("@@ -1 +1 @@")));
        assert!(rendered.iter().any(|line| line.contains("1 - old")));
        assert!(rendered.iter().any(|line| line.contains("1 + new")));
        assert!(!rendered.iter().any(|line| line.contains("output:")));
    }

    #[test]
    fn long_plain_output_is_collapsed_with_ellipsis() {
        let output = (1..=12)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let lines = render_tool_call_lines(
            "exec_command",
            Some("git log --oneline"),
            Some(&output),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("… +")));
        assert!(rendered.iter().any(|line| line.contains("line 1")));
        assert!(rendered.iter().any(|line| line.contains("line 12")));
    }

    #[test]
    fn json_output_is_compacted_before_render() {
        let lines = render_tool_call_lines(
            "tool_result",
            None,
            Some("{\"ok\":true,\"items\":[1,2,3]}"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("\"ok\": true")));
        assert!(rendered.iter().any(|line| line.contains("\"items\": [1, 2, 3]")));
    }

    #[test]
    fn git_status_output_is_rendered_with_status_specific_lines() {
        let lines = render_tool_call_lines(
            "exec_command",
            Some("git status"),
            Some("On branch main\nChanges not staged for commit:\n  modified:   README.md\n  deleted:    old.txt"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("On branch main")));
        assert!(rendered.iter().any(|line| line.contains("Changes not staged for commit:")));
        assert!(rendered.iter().any(|line| line.contains("modified:   README.md")));
        assert!(rendered.iter().any(|line| line.contains("deleted:    old.txt")));
    }

    #[test]
    fn git_log_output_is_rendered_as_commit_list() {
        let lines = render_tool_call_lines(
            "exec_command",
            Some("git log --oneline -4"),
            Some("927a1e4 feat: add end-to-end debug observability\n0d7485f feat: log codex jsonrpc transport"),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("927a1e4 feat: add end-to-end debug observability")));
        assert!(rendered.iter().any(|line| line.contains("0d7485f feat: log codex jsonrpc transport")));
    }

    #[test]
    fn finished_command_with_no_output_shows_explicit_empty_marker() {
        let lines = render_tool_call_lines(
            "exec_command",
            Some("git rev-parse HEAD"),
            Some(""),
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("$ git rev-parse HEAD")));
        assert!(rendered.iter().any(|line| line.contains("(no output)")));
    }

    #[test]
    fn patch_apply_summary_renders_file_list() {
        let lines = render_tool_call_lines(
            "patch_apply",
            Some("M /repo/README.md (+1 -1)\nA /repo/src/lib.rs (+1 -0)"),
            None,
            true,
            false,
            80,
            ToolRenderMode::Preview,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("applied patch")));
        assert!(rendered.iter().any(|line| line.contains("M /repo/README.md (+1 -1)")));
        assert!(rendered.iter().any(|line| line.contains("A /repo/src/lib.rs (+1 -0)")));
    }

    #[test]
    fn full_mode_keeps_all_wrapped_output_lines() {
        let output = (1..=20)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = render_tool_call_lines(
            "exec_command",
            Some("git log --oneline"),
            Some(&output),
            true,
            false,
            80,
            ToolRenderMode::Full,
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("line 1")));
        assert!(rendered.iter().any(|line| line.contains("line 20")));
        assert!(!rendered.iter().any(|line| line.contains("… +")));
    }
}
