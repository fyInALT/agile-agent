use agent_core::app::TranscriptEntry;
use ratatui::text::Line;

use crate::history_cell::history_cell_for_entry;
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
    entries
        .iter()
        .map(|entry| {
            let cell = history_cell_for_entry(entry);
            let lines = match mode {
                ToolRenderMode::Preview => cell.display_lines(width),
                ToolRenderMode::Full => cell.transcript_lines(width),
            };
            TranscriptCell { lines }
        })
        .filter(|cell| !cell.lines.is_empty())
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
        assert!(rendered.iter().any(|line| line.contains("README.md (+1 -1)")));
        assert!(rendered.iter().any(|line| line.contains("@@ -1 +1 @@")));
        assert!(rendered.iter().any(|line| line.contains("1 - old")));
        assert!(rendered.iter().any(|line| line.contains("1 + new")));
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

    #[test]
    fn preview_and_overlay_cells_diverge_for_long_tool_output() {
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

        let preview = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let overlay = lines_to_strings(&flatten_cells(&build_overlay_cells(&entries, 80)));

        assert!(preview.iter().any(|line| line.contains("… +")));
        assert!(!overlay.iter().any(|line| line.contains("… +")));
    }
}
