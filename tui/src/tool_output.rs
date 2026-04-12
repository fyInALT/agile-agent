use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::wrap;
use unicode_segmentation::UnicodeSegmentation;

const TOOL_PREVIEW_MAX_LINES: usize = 8;
const TOOL_PREVIEW_HEAD_LINES: usize = 5;
const TOOL_PREVIEW_TAIL_LINES: usize = 2;

pub fn render_tool_call_lines(
    name: &str,
    input_preview: Option<&str>,
    output_preview: Option<&str>,
    success: bool,
    started: bool,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(tool_header_line(name, success, started));

    if let Some(input) = input_preview.filter(|value| !value.trim().is_empty()) {
        lines.extend(render_input_block(name, input, width));
    }

    if let Some(output) = output_preview.filter(|value| !value.trim().is_empty()) {
        lines.extend(render_output_block(name, input_preview, output, width));
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

fn render_output_block(
    name: &str,
    input_preview: Option<&str>,
    output: &str,
    width: usize,
) -> Vec<Line<'static>> {
    if looks_like_diff(input_preview, output) {
        return render_diff_block(output, width);
    }

    let formatted = if let Some(compact_json) = format_json_compact(output) {
        compact_json
    } else {
        output.to_string()
    };

    render_text_block(name, &formatted, width)
}

fn looks_like_diff(input_preview: Option<&str>, output: &str) -> bool {
    input_preview
        .is_some_and(|input| input.contains("git diff") || input.contains("git show"))
        || output.starts_with("diff --git ")
        || (output.contains("\n--- ") && output.contains("\n+++ "))
}

fn render_diff_block(output: &str, width: usize) -> Vec<Line<'static>> {
    let rendered = summarize_lines(output, TOOL_PREVIEW_HEAD_LINES, TOOL_PREVIEW_TAIL_LINES);
    let body_width = width.saturating_sub(4).max(1);
    let mut lines = Vec::new();

    for line in rendered {
        match line {
            PreviewLine::Text(text) => {
                let style = diff_style_for_line(&text);
                lines.extend(wrap_prefixed("    ", &text, style, body_width + 4));
            }
            PreviewLine::Ellipsis(omitted) => lines.push(Line::from(Span::styled(
                format!("    … +{omitted} lines"),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ))),
        }
    }

    lines
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

fn render_text_block(name: &str, output: &str, width: usize) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(4).max(1);
    let preview_lines = summarize_lines(output, TOOL_PREVIEW_HEAD_LINES, TOOL_PREVIEW_TAIL_LINES);
    let mut lines = Vec::new();

    for line in preview_lines {
        match line {
            PreviewLine::Text(text) => {
                let style = if name == "exec_command" {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.extend(wrap_prefixed("    ", &text, style, body_width + 4));
            }
            PreviewLine::Ellipsis(omitted) => lines.push(Line::from(Span::styled(
                format!("    … +{omitted} lines"),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ))),
        }
    }

    lines
}

enum PreviewLine {
    Text(String),
    Ellipsis(usize),
}

fn summarize_lines(text: &str, head: usize, tail: usize) -> Vec<PreviewLine> {
    let lines = text.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if lines.len() <= TOOL_PREVIEW_MAX_LINES {
        return lines.into_iter().map(PreviewLine::Text).collect();
    }

    let mut preview = Vec::new();
    preview.extend(
        lines[..head.min(lines.len())]
            .iter()
            .cloned()
            .map(PreviewLine::Text),
    );
    let omitted = lines.len().saturating_sub(head + tail);
    if omitted > 0 {
        preview.push(PreviewLine::Ellipsis(omitted));
    }
    let tail_start = lines.len().saturating_sub(tail);
    preview.extend(lines[tail_start..].iter().cloned().map(PreviewLine::Text));
    preview
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
        );
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line.contains("finished command")));
        assert!(rendered.iter().any(|line| line.contains("$ git diff README.md")));
        assert!(rendered.iter().any(|line| line.contains("diff --git a/README.md b/README.md")));
        assert!(rendered.iter().any(|line| line.contains("+new")));
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
        );
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("\"ok\": true")));
        assert!(rendered.iter().any(|line| line.contains("\"items\": [1, 2, 3]")));
    }
}
