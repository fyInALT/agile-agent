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

#[derive(Debug, Clone)]
pub struct TranscriptCell {
    pub lines: Vec<Line<'static>>,
}

pub fn build_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(entries, width, ToolRenderMode::Preview)
}

pub fn build_overlay_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(entries, width, ToolRenderMode::Full)
}

fn build_cells_with_mode(
    entries: &[TranscriptEntry],
    width: u16,
    mode: ToolRenderMode,
) -> Vec<TranscriptCell> {
    let content_width = width.max(4) as usize;
    entries
        .iter()
        .filter_map(|entry| build_cell(entry, content_width, mode))
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

fn build_cell(entry: &TranscriptEntry, width: usize, mode: ToolRenderMode) -> Option<TranscriptCell> {
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
        } => tool_output::render_tool_call_lines(
            name,
            input_preview.as_deref(),
            output_preview.as_deref(),
            *success,
            *started,
            width,
            mode,
        ),
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

#[cfg(test)]
mod tests {
    use super::build_cells;
    use super::build_overlay_cells;
    use super::flatten_cells;
    use agent_core::app::TranscriptEntry;
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
    fn tool_calls_render_command_and_structured_output_preview() {
        let entries = vec![TranscriptEntry::ToolCall {
            name: "exec_command".to_string(),
            call_id: Some("call-1".to_string()),
            input_preview: Some("git diff README.md".to_string()),
            output_preview: Some(
                "diff --git a/README.md b/README.md\n@@ -1 +1 @@\n-old\n+new".to_string(),
            ),
            success: true,
            started: false,
        }];

        let lines = flatten_cells(&build_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("finished command")));
        assert!(rendered.iter().any(|line| line.contains("$ git diff README.md")));
        assert!(rendered.iter().any(|line| line.contains("diff --git a/README.md b/README.md")));
        assert!(rendered.iter().any(|line| line.contains("+new")));
        assert!(!rendered.iter().any(|line| line.contains("output:")));
    }

    #[test]
    fn overlay_cells_keep_full_tool_output() {
        let output = (1..=20)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let entries = vec![TranscriptEntry::ToolCall {
            name: "exec_command".to_string(),
            call_id: Some("call-1".to_string()),
            input_preview: Some("git log --oneline".to_string()),
            output_preview: Some(output),
            success: true,
            started: false,
        }];

        let lines = flatten_cells(&build_overlay_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("line 1")));
        assert!(rendered.iter().any(|line| line.contains("line 20")));
        assert!(!rendered.iter().any(|line| line.contains("… +")));
    }
}
