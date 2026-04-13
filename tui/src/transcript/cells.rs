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
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some("git diff README.md".to_string()),
            output_preview: Some(
                "diff --git a/README.md b/README.md\n@@ -1 +1 @@\n-old\n+new".to_string(),
            ),
            success: true,
            started: false,
            exit_code: Some(0),
            duration_ms: Some(1234),
        }];

        let lines = flatten_cells(&build_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("Ran")));
        assert!(rendered.iter().any(|line| line.contains("git diff README.md")));
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
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some("git log --oneline".to_string()),
            output_preview: Some(output),
            success: true,
            started: false,
            exit_code: Some(0),
            duration_ms: Some(1000),
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
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some("git log --oneline".to_string()),
            output_preview: Some(output),
            success: true,
            started: false,
            exit_code: Some(0),
            duration_ms: Some(1000),
        }];

        let preview = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let overlay = lines_to_strings(&flatten_cells(&build_overlay_cells(&entries, 80)));

        assert!(preview.iter().any(|line| line.contains("… +")));
        assert!(!overlay.iter().any(|line| line.contains("… +")));
    }

    #[test]
    fn user_shell_exec_renders_you_ran_title() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("userShell".to_string()),
            input_preview: Some("git status".to_string()),
            output_preview: Some("On branch main".to_string()),
            success: true,
            started: false,
            exit_code: Some(0),
            duration_ms: Some(50),
        }];

        let lines = flatten_cells(&build_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("You ran git status")));
    }

    #[test]
    fn multiline_exec_command_uses_codex_style_branch_continuations() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some("set -o pipefail\ncargo test -p codex-tui --quiet".to_string()),
            output_preview: Some(String::new()),
            success: true,
            started: false,
            exit_code: Some(0),
            duration_ms: Some(12),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 28)));

        assert!(rendered.iter().any(|line| line == "• Ran set -o pipefail"));
        assert!(rendered.iter().any(|line| line == "  │ cargo test -p"));
        assert!(rendered.iter().any(|line| line == "  │ codex-tui --quiet"));
        assert!(rendered.iter().any(|line| line == "  └ (no output)"));
    }

    #[test]
    fn generic_tool_call_renders_called_header_like_codex() {
        let entries = vec![TranscriptEntry::GenericToolCall {
            name: "search.find_docs".to_string(),
            call_id: Some("call-2".to_string()),
            input_preview: Some("{\"query\":\"ratatui styling\",\"limit\":3}".to_string()),
            output_preview: Some("Found styling guidance in styles.md".to_string()),
            success: true,
            started: false,
            exit_code: None,
            duration_ms: None,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(rendered.iter().any(|line| {
            line == "• Called search.find_docs({\"query\":\"ratatui styling\",\"limit\":3})"
        }));
        assert!(rendered.iter().any(|line| line == "  └ Found styling guidance in styles.md"));
        assert!(!rendered
            .iter()
            .any(|line| line.contains("finished tool search.find_docs")));
    }

    #[test]
    fn generic_tool_call_wraps_long_invocation_below_header() {
        let entries = vec![TranscriptEntry::GenericToolCall {
            name: "metrics.get_nearby_metric".to_string(),
            call_id: Some("call-3".to_string()),
            input_preview: Some(
                "{\"query\":\"very_long_query_that_needs_wrapping_to_display_properly_in_the_history\",\"limit\":1}"
                    .to_string(),
            ),
            output_preview: Some(
                "Line one of the response, which is quite long and needs wrapping.".to_string(),
            ),
            success: true,
            started: false,
            exit_code: None,
            duration_ms: None,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 36)));

        assert!(rendered.iter().any(|line| line == "• Called"));
        assert!(rendered
            .iter()
            .any(|line| line.starts_with("  └ metrics.get_nearby_metric(")));
        assert!(rendered
            .iter()
            .any(|line| line.contains("Line one of the response,")));
    }

    #[test]
    fn patch_apply_renders_codex_style_change_summary() {
        let entries = vec![TranscriptEntry::PatchApply {
            call_id: Some("patch-1".to_string()),
            changes: vec![
                agent_core::tool_calls::PatchChange {
                    path: "README.md".to_string(),
                    move_path: None,
                    kind: agent_core::tool_calls::PatchChangeKind::Update,
                    diff: "@@ -1,3 +1,3 @@\n line one\n-line two\n+line two changed\n line three"
                        .to_string(),
                    added: 1,
                    removed: 1,
                },
                agent_core::tool_calls::PatchChange {
                    path: "src/lib.rs".to_string(),
                    move_path: None,
                    kind: agent_core::tool_calls::PatchChangeKind::Add,
                    diff: "+fn main() {}".to_string(),
                    added: 1,
                    removed: 0,
                },
            ],
            success: true,
            started: false,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(rendered
            .iter()
            .any(|line| line == "• Edited 2 files (+2 -1)"));
        assert!(rendered
            .iter()
            .any(|line| line == "  └ README.md (+1 -1)"));
        assert!(rendered
            .iter()
            .any(|line| line == "  └ src/lib.rs (+1 -0)"));
        assert!(rendered.iter().any(|line| line == "    1  line one"), "{rendered:?}");
        assert!(rendered.iter().any(|line| line == "    2 -line two"), "{rendered:?}");
        assert!(rendered.iter().any(|line| line == "    2 +line two changed"), "{rendered:?}");
        assert!(!rendered.iter().any(|line| line.contains("applied patch")));
    }

    #[test]
    fn patch_preview_and_overlay_diverge_for_large_diff() {
        let diff = "@@ -1,10 +1,10 @@\n line 1\n-line 2\n+line 2 changed\n line 3\n-line 4\n+line 4 changed\n line 5\n-line 6\n+line 6 changed\n line 7\n-line 8\n+line 8 changed\n line 9\n-line 10\n+line 10 changed";
        let entries = vec![TranscriptEntry::PatchApply {
            call_id: Some("patch-2".to_string()),
            changes: vec![agent_core::tool_calls::PatchChange {
                path: "README.md".to_string(),
                move_path: None,
                kind: agent_core::tool_calls::PatchChangeKind::Update,
                diff: diff.to_string(),
                added: 5,
                removed: 5,
            }],
            success: true,
            started: false,
        }];

        let preview = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let overlay = lines_to_strings(&flatten_cells(&build_overlay_cells(&entries, 80)));

        assert!(preview.iter().any(|line| line.contains("… +")), "{preview:?}");
        assert!(!overlay.iter().any(|line| line.contains("… +")), "{overlay:?}");
        assert!(overlay
            .iter()
            .any(|line| line.contains("10 +line 10 changed")), "{overlay:?}");
    }

    #[test]
    fn patch_apply_render_uses_rename_arrow() {
        let entries = vec![TranscriptEntry::PatchApply {
            call_id: Some("patch-3".to_string()),
            changes: vec![agent_core::tool_calls::PatchChange {
                path: "old_name.rs".to_string(),
                move_path: Some("new_name.rs".to_string()),
                kind: agent_core::tool_calls::PatchChangeKind::Update,
                diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
                added: 1,
                removed: 1,
            }],
            success: true,
            started: false,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(rendered
            .iter()
            .any(|line| line == "• Edited old_name.rs → new_name.rs (+1 -1)"), "{rendered:?}");
    }

    #[test]
    fn long_exec_command_shows_command_ellipsis_before_output() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-4".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some(
                "echo\nthis_is_a_very_long_single_token_that_will_wrap_over_multiple_lines"
                    .to_string(),
            ),
            output_preview: Some(
                "error: first line on stderr\nerror: second line on stderr".to_string(),
            ),
            success: false,
            started: false,
            exit_code: Some(1),
            duration_ms: Some(50),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 24)));

        assert!(rendered.iter().any(|line| line == "• Ran echo"), "{rendered:?}");
        assert!(rendered
            .iter()
            .any(|line| line.starts_with("  │ this_is_a_very_")), "{rendered:?}");
        assert!(rendered.iter().any(|line| line == "  │ … +2 lines"), "{rendered:?}");
        assert!(rendered
            .iter()
            .any(|line| line == "  └ error: first line"), "{rendered:?}");
    }

    #[test]
    fn exec_output_preview_keeps_only_head_and_tail_lines_like_codex() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-5".to_string()),
            source: Some("agent".to_string()),
            input_preview: Some("seq 1 10 1>&2 && false".to_string()),
            output_preview: Some(
                (1..=10)
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            success: false,
            started: false,
            exit_code: Some(1),
            duration_ms: Some(250),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(rendered.iter().any(|line| line == "  └ 1"));
        assert!(rendered.iter().any(|line| line == "    2"));
        assert!(rendered
            .iter()
            .any(|line| line == "    … +6 lines (ctrl + t to view transcript)"));
        assert!(rendered.iter().any(|line| line == "    9"));
        assert!(rendered.iter().any(|line| line == "    10"));
        assert!(!rendered.iter().any(|line| line == "    3"));
    }
}
