use agent_core::app::TranscriptEntry;
use ratatui::text::Line;

use crate::exec_semantics::parse_exploring_ops;
use crate::history_cell::ExploringExecCall;
use crate::history_cell::history_cell_for_entry;
use crate::history_cell::history_cell_for_exploring_exec_group;
use crate::tool_output::ToolRenderMode;

#[derive(Debug, Clone)]
pub struct TranscriptCell {
    pub lines: Vec<Line<'static>>,
    pub height: u16,
}

pub fn build_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(entries, width, ToolRenderMode::Preview, CellSelection::All)
}

#[allow(dead_code)]
pub fn build_active_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(
        entries,
        width,
        ToolRenderMode::Preview,
        CellSelection::Active,
    )
}

pub fn build_live_tail_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(entries, width, ToolRenderMode::Preview, CellSelection::All)
}

pub fn build_overlay_cells(entries: &[TranscriptEntry], width: u16) -> Vec<TranscriptCell> {
    build_cells_with_mode(entries, width, ToolRenderMode::Full, CellSelection::All)
}

fn build_cells_with_mode(
    entries: &[TranscriptEntry],
    width: u16,
    mode: ToolRenderMode,
    selection: CellSelection,
) -> Vec<TranscriptCell> {
    let mut cells = Vec::new();
    let mut index = 0usize;

    while index < entries.len() {
        if let ToolRenderMode::Preview = mode
            && let Some((next_index, cell)) =
                exploring_exec_group_cell(entries, index, width, selection)
        {
            if let Some(cell) = cell {
                cells.push(cell);
            }
            index = next_index;
            continue;
        }

        if !selection_matches_entry(selection, &entries[index]) {
            index += 1;
            continue;
        }

        let cell = history_cell_for_entry(&entries[index]);
        let lines = match mode {
            ToolRenderMode::Preview => cell.display_lines(width),
            ToolRenderMode::Full => cell.transcript_lines(width),
        };
        let height = match mode {
            ToolRenderMode::Preview => cell.desired_height(width),
            ToolRenderMode::Full => cell.desired_transcript_height(width),
        };
        if !lines.is_empty() {
            cells.push(TranscriptCell { lines, height });
        }
        index += 1;
    }

    cells
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum CellSelection {
    Committed,
    Active,
    All,
}

fn selection_matches_entry(selection: CellSelection, entry: &TranscriptEntry) -> bool {
    match selection {
        CellSelection::All => true,
        CellSelection::Committed => !is_active_entry(entry),
        CellSelection::Active => is_active_entry(entry),
    }
}

fn is_active_entry(entry: &TranscriptEntry) -> bool {
    match entry {
        TranscriptEntry::ExecCommand { status, .. } => {
            matches!(
                status,
                agent_core::tool_calls::ExecCommandStatus::InProgress
            )
        }
        TranscriptEntry::PatchApply { status, .. } => {
            matches!(status, agent_core::tool_calls::PatchApplyStatus::InProgress)
        }
        TranscriptEntry::WebSearch { started, .. } => *started,
        TranscriptEntry::McpToolCall { status, .. } => {
            matches!(
                status,
                agent_core::tool_calls::McpToolCallStatus::InProgress
            )
        }
        TranscriptEntry::GenericToolCall { started, .. } => *started,
        _ => false,
    }
}

fn exploring_exec_group_cell(
    entries: &[TranscriptEntry],
    start: usize,
    width: u16,
    selection: CellSelection,
) -> Option<(usize, Option<TranscriptCell>)> {
    let mut index = start;
    let mut calls = Vec::new();
    let mut contains_active = false;

    while index < entries.len() {
        let TranscriptEntry::ExecCommand {
            source,
            allow_exploring_group,
            input_preview,
            output_preview,
            status,
            exit_code,
            duration_ms,
            ..
        } = &entries[index]
        else {
            break;
        };

        if !allow_exploring_group {
            break;
        }

        let Some(command) = input_preview.as_deref() else {
            break;
        };
        let Some(ops) = parse_exploring_ops(command, source.as_deref()) else {
            break;
        };

        contains_active |= matches!(
            status,
            agent_core::tool_calls::ExecCommandStatus::InProgress
        );
        calls.push(ExploringExecCall {
            source: source.clone(),
            input_preview: input_preview.clone(),
            output_preview: output_preview.clone(),
            status: *status,
            exit_code: *exit_code,
            duration_ms: *duration_ms,
            ops,
        });
        index += 1;
    }

    if calls.is_empty() {
        return None;
    }

    match selection {
        CellSelection::Committed if contains_active => return Some((index, None)),
        CellSelection::Active if !contains_active => return None,
        _ => {}
    }

    let cell = history_cell_for_exploring_exec_group(calls);
    let lines = cell.display_lines(width);
    let height = cell.desired_height(width);
    if lines.is_empty() {
        Some((index, None))
    } else {
        Some((index, Some(TranscriptCell { lines, height })))
    }
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
    use super::build_active_cells;
    use super::build_cells;
    use super::build_live_tail_cells;
    use super::build_overlay_cells;
    use super::flatten_cells;
    use agent_core::app::TranscriptEntry;
    use ratatui::text::Line;

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
    fn tool_calls_render_command_and_structured_output_preview() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("git diff README.md".to_string()),
            output_preview: Some(
                "diff --git a/README.md b/README.md\n@@ -1 +1 @@\n-old\n+new".to_string(),
            ),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(1234),
        }];

        let lines = flatten_cells(&build_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        assert!(rendered.iter().any(|line| line.contains("Ran")));
        // git commands show friendly label instead of raw command
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("Git Diff"))
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("README.md (+1 -1)"))
        );
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
            allow_exploring_group: true,
            input_preview: Some("git log --oneline".to_string()),
            output_preview: Some(output),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
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
            allow_exploring_group: true,
            input_preview: Some("git log --oneline".to_string()),
            output_preview: Some(output),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
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
            allow_exploring_group: true,
            input_preview: Some("git status".to_string()),
            output_preview: Some("On branch main".to_string()),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(50),
        }];

        let lines = flatten_cells(&build_cells(&entries, 80));
        let rendered = lines_to_strings(&lines);

        // git commands show friendly label
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("You ran Git Status"))
        );
    }

    #[test]
    fn multiline_exec_command_uses_codex_style_branch_continuations() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("set -o pipefail\ncargo test -p codex-tui --quiet".to_string()),
            output_preview: Some(String::new()),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
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
        assert!(
            rendered
                .iter()
                .any(|line| line == "  └ Found styling guidance in styles.md")
        );
        assert!(
            !rendered
                .iter()
                .any(|line| line.contains("finished tool search.find_docs"))
        );
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
        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("  └ metrics.get_nearby_metric("))
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("Line one of the response,"))
        );
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
            status: agent_core::tool_calls::PatchApplyStatus::Completed,
            output_preview: None,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered
                .iter()
                .any(|line| line == "• Edited 2 files (+2 -1)")
        );
        assert!(rendered.iter().any(|line| line == "  └ README.md (+1 -1)"));
        assert!(rendered.iter().any(|line| line == "  └ src/lib.rs (+1 -0)"));
        assert!(
            rendered.iter().any(|line| line == "    1  line one"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "    2 -line two"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "    2 +line two changed"),
            "{rendered:?}"
        );
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
            status: agent_core::tool_calls::PatchApplyStatus::Completed,
            output_preview: None,
        }];

        let preview = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let overlay = lines_to_strings(&flatten_cells(&build_overlay_cells(&entries, 80)));

        assert!(
            preview.iter().any(|line| line.contains("… +")),
            "{preview:?}"
        );
        assert!(
            !overlay.iter().any(|line| line.contains("… +")),
            "{overlay:?}"
        );
        assert!(
            overlay
                .iter()
                .any(|line| line.contains("10 +line 10 changed")),
            "{overlay:?}"
        );
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
            status: agent_core::tool_calls::PatchApplyStatus::Completed,
            output_preview: None,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered
                .iter()
                .any(|line| line == "• Edited old_name.rs → new_name.rs (+1 -1)"),
            "{rendered:?}"
        );
    }

    #[test]
    fn patch_apply_failed_and_declined_render_distinct_headers() {
        let failed = vec![TranscriptEntry::PatchApply {
            call_id: Some("patch-failed".to_string()),
            changes: Vec::new(),
            status: agent_core::tool_calls::PatchApplyStatus::Failed,
            output_preview: Some("patch rejected by user".to_string()),
        }];
        let declined = vec![TranscriptEntry::PatchApply {
            call_id: Some("patch-declined".to_string()),
            changes: Vec::new(),
            status: agent_core::tool_calls::PatchApplyStatus::Declined,
            output_preview: Some("patch canceled".to_string()),
        }];

        let failed_rendered = lines_to_strings(&flatten_cells(&build_cells(&failed, 80)));
        let declined_rendered = lines_to_strings(&flatten_cells(&build_cells(&declined, 80)));

        assert!(
            failed_rendered.iter().any(|line| line == "• Failed patch"),
            "{failed_rendered:?}"
        );
        assert!(
            failed_rendered
                .iter()
                .any(|line| line == "  └ patch rejected by user"),
            "{failed_rendered:?}"
        );
        assert!(
            declined_rendered
                .iter()
                .any(|line| line == "• Declined patch"),
            "{declined_rendered:?}"
        );
        assert!(
            declined_rendered
                .iter()
                .any(|line| line == "  └ patch canceled"),
            "{declined_rendered:?}"
        );
    }

    #[test]
    fn web_search_renders_dedicated_history_cell() {
        let started = vec![TranscriptEntry::WebSearch {
            call_id: Some("search-1".to_string()),
            query: "example search query with several generic words to exercise wrapping"
                .to_string(),
            action: None,
            started: true,
        }];
        let completed = vec![TranscriptEntry::WebSearch {
            call_id: Some("search-1".to_string()),
            query: "example search query with several generic words to exercise wrapping"
                .to_string(),
            action: Some(agent_core::tool_calls::WebSearchAction::Other),
            started: false,
        }];

        let started_rendered = lines_to_strings(&flatten_cells(&build_active_cells(&started, 80)));
        let completed_rendered = lines_to_strings(&flatten_cells(&build_cells(&completed, 80)));

        assert!(
            started_rendered
                .iter()
                .any(|line| line.contains("Searching the web")),
            "{started_rendered:?}"
        );
        assert!(
            completed_rendered
                .iter()
                .any(|line| line.contains("Searched example search query")),
            "{completed_rendered:?}"
        );
    }

    #[test]
    fn image_view_and_generation_render_dedicated_history_cells() {
        let entries = vec![
            TranscriptEntry::ViewImage {
                call_id: Some("image-view-1".to_string()),
                path: "example.png".to_string(),
            },
            TranscriptEntry::ImageGeneration {
                call_id: Some("image-gen-1".to_string()),
                revised_prompt: Some("A tiny blue square".to_string()),
                result: Some("image.png".to_string()),
                saved_path: Some("/tmp/ig-1.png".to_string()),
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Viewed Image"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ example.png"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "• Generated Image:"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ A tiny blue square"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "  └ Saved to: /tmp/ig-1.png"),
            "{rendered:?}"
        );
    }

    #[test]
    fn mcp_tool_call_renders_dedicated_history_cell() {
        let invocation = agent_core::tool_calls::McpInvocation {
            server: "search".to_string(),
            tool: "find_docs".to_string(),
            arguments: Some(serde_json::json!({
                "query": "ratatui styling",
                "limit": 3
            })),
        };
        let started = vec![TranscriptEntry::McpToolCall {
            call_id: Some("mcp-1".to_string()),
            invocation: invocation.clone(),
            result_blocks: Vec::new(),
            error: None,
            status: agent_core::tool_calls::McpToolCallStatus::InProgress,
            is_error: false,
        }];
        let completed = vec![TranscriptEntry::McpToolCall {
            call_id: Some("mcp-1".to_string()),
            invocation: invocation.clone(),
            result_blocks: vec![serde_json::json!({
                "type": "text",
                "text": "Found styling guidance in styles.md"
            })],
            error: None,
            status: agent_core::tool_calls::McpToolCallStatus::Completed,
            is_error: false,
        }];
        let failed = vec![TranscriptEntry::McpToolCall {
            call_id: Some("mcp-2".to_string()),
            invocation,
            result_blocks: Vec::new(),
            error: Some("network timeout".to_string()),
            status: agent_core::tool_calls::McpToolCallStatus::Failed,
            is_error: true,
        }];

        let started_rendered = lines_to_strings(&flatten_cells(&build_active_cells(&started, 80)));
        let completed_rendered = lines_to_strings(&flatten_cells(&build_cells(&completed, 80)));
        let failed_rendered = lines_to_strings(&flatten_cells(&build_cells(&failed, 80)));

        assert!(
            started_rendered
                .iter()
                .any(|line| line.starts_with("• Calling search.find_docs(")
                    && line.contains("\"query\":\"ratatui styling\"")
                    && line.contains("\"limit\":3")),
            "{started_rendered:?}"
        );
        assert!(
            completed_rendered
                .iter()
                .any(|line| line.starts_with("• Called search.find_docs(")
                    && line.contains("\"query\":\"ratatui styling\"")
                    && line.contains("\"limit\":3")),
            "{completed_rendered:?}"
        );
        assert!(
            completed_rendered
                .iter()
                .any(|line| line == "  └ Found styling guidance in styles.md"),
            "{completed_rendered:?}"
        );
        assert!(
            failed_rendered
                .iter()
                .any(|line| line == "  └ Error: network timeout"),
            "{failed_rendered:?}"
        );
    }

    #[test]
    fn preview_coalesces_adjacent_exploring_exec_calls() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-1".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("ls -la".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-2".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("cat foo.txt".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-3".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("cat bar.txt".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Explored"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ List ls -la"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "    Read foo.txt, bar.txt"),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line.contains("Ran ls -la")),
            "{rendered:?}"
        );
    }

    #[test]
    fn preview_marks_started_exploring_cluster_as_exploring() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-1".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("rg shimmer_spans src".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-2".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("cat src/shimmer.rs".to_string()),
                output_preview: None,
                status: agent_core::tool_calls::ExecCommandStatus::InProgress,
                exit_code: None,
                duration_ms: None,
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_active_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Exploring"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "  └ Search shimmer_spans in src"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "    Read src/shimmer.rs"),
            "{rendered:?}"
        );
    }

    #[test]
    fn overlay_keeps_individual_exec_cells_for_exploring_cluster() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-1".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("ls -la".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-2".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("cat foo.txt".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_overlay_cells(&entries, 80)));

        assert!(rendered.iter().any(|line| line.contains("$ ls -la")), "{rendered:?}");
        assert!(
            rendered.iter().any(|line| line.contains("$ cat foo.txt")),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line == "• Explored"),
            "{rendered:?}"
        );
    }

    #[test]
    fn user_shell_exec_does_not_coalesce_into_exploring_cell() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("userShell".to_string()),
            allow_exploring_group: true,
            input_preview: Some("cat foo.txt".to_string()),
            output_preview: Some(String::new()),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(10),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered
                .iter()
                .any(|line| line.contains("You ran cat foo.txt")),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line == "• Explored"),
            "{rendered:?}"
        );
    }

    #[test]
    fn exploring_search_query_keeps_long_url_like_token_intact() {
        let url_like = "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/with/a/very/long/path";
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-6".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some(format!("rg {url_like}")),
            output_preview: Some(String::new()),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(10),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 36)));

        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.contains(url_like))
                .count(),
            1,
            "{rendered:?}"
        );
    }

    #[test]
    fn user_shell_exec_strips_bash_lc_wrapper_in_header() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-7".to_string()),
            source: Some("userShell".to_string()),
            allow_exploring_group: true,
            input_preview: Some(
                "bash -lc 'python3 -c '\\''print(\"Hello, world!\")'\\'''".to_string(),
            ),
            output_preview: Some("Hello, world!".to_string()),
            status: agent_core::tool_calls::ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(10),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered
                .iter()
                .any(|line| line == "• You ran python3 -c 'print(\"Hello, world!\")'"),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line.contains("bash -lc")),
            "{rendered:?}"
        );
    }

    #[test]
    fn started_exec_renders_live_output_delta_preview() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-10".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello\nworld".to_string()),
            status: agent_core::tool_calls::ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_active_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Running printf hello"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ hello"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "    world"),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line.contains("✓")),
            "{rendered:?}"
        );
    }

    #[test]
    fn committed_cells_omit_in_progress_exec_entries() {
        let entries = vec![
            TranscriptEntry::Status("done before".to_string()),
            TranscriptEntry::ExecCommand {
                call_id: Some("call-live".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("printf hello".to_string()),
                output_preview: Some("hello".to_string()),
                status: agent_core::tool_calls::ExecCommandStatus::InProgress,
                exit_code: None,
                duration_ms: None,
            },
        ];

        let committed = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let active = lines_to_strings(&flatten_cells(&build_active_cells(&entries, 80)));

        assert!(
            committed.iter().any(|line| line == "• done before"),
            "{committed:?}"
        );
        assert!(
            committed
                .iter()
                .any(|line| line.contains("Running printf hello")),
            "{committed:?}"
        );
        assert!(active.iter().any(|line| line == "• Running printf hello"), "{active:?}");
    }

    #[test]
    fn live_tail_cells_render_streaming_assistant_entries() {
        let entries = vec![TranscriptEntry::Assistant(
            "streaming assistant response".to_string(),
        )];

        let rendered = lines_to_strings(&flatten_cells(&build_live_tail_cells(&entries, 80)));

        assert!(rendered.iter().any(|line| line.contains("streaming assistant response")));
    }

    #[test]
    fn committed_cells_include_in_progress_patch_web_search_and_mcp_entries() {
        let entries = vec![
            TranscriptEntry::Status("done before".to_string()),
            TranscriptEntry::PatchApply {
                call_id: Some("patch-live".to_string()),
                changes: Vec::new(),
                status: agent_core::tool_calls::PatchApplyStatus::InProgress,
                output_preview: Some("patch running".to_string()),
            },
            TranscriptEntry::WebSearch {
                call_id: Some("search-live".to_string()),
                query: "ratatui styling".to_string(),
                action: None,
                started: true,
            },
            TranscriptEntry::McpToolCall {
                call_id: Some("mcp-live".to_string()),
                invocation: agent_core::tool_calls::McpInvocation {
                    server: "search".to_string(),
                    tool: "find_docs".to_string(),
                    arguments: Some(serde_json::json!({
                        "query": "ratatui styling",
                        "limit": 3
                    })),
                },
                result_blocks: Vec::new(),
                error: None,
                status: agent_core::tool_calls::McpToolCallStatus::InProgress,
                is_error: false,
            },
        ];

        let committed = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let active = lines_to_strings(&flatten_cells(&build_active_cells(&entries, 80)));

        assert!(
            committed.iter().any(|line| line == "• done before"),
            "{committed:?}"
        );
        assert!(
            committed.iter().any(|line| line.contains("Applying patch")),
            "{committed:?}"
        );
        assert!(
            committed
                .iter()
                .any(|line| line.contains("Searching the web")),
            "{committed:?}"
        );
        assert!(
            committed
                .iter()
                .any(|line| line.contains("Calling search.find_docs")),
            "{committed:?}"
        );
        assert!(
            active.iter().any(|line| line == "• Applying patch"),
            "{active:?}"
        );
        assert!(
            active.iter().any(|line| line.contains("Searching the web")),
            "{active:?}"
        );
        assert!(
            active
                .iter()
                .any(|line| line.contains("• Calling search.find_docs(")),
            "{active:?}"
        );
    }

    #[test]
    fn failed_and_declined_exec_render_distinct_headers() {
        let failed = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-failed".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: false,
            input_preview: Some("false".to_string()),
            output_preview: Some(String::new()),
            status: agent_core::tool_calls::ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: Some(5),
        }];
        let declined = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-declined".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: false,
            input_preview: Some("rm -rf /tmp/demo".to_string()),
            output_preview: Some(String::new()),
            status: agent_core::tool_calls::ExecCommandStatus::Declined,
            exit_code: None,
            duration_ms: None,
        }];

        let failed_rendered = lines_to_strings(&flatten_cells(&build_cells(&failed, 80)));
        let declined_rendered = lines_to_strings(&flatten_cells(&build_cells(&declined, 80)));

        assert!(
            failed_rendered.iter().any(|line| line == "• Ran false"),
            "{failed_rendered:?}"
        );
        assert!(
            declined_rendered
                .iter()
                .any(|line| line == "• Declined command rm -rf /tmp/demo"),
            "{declined_rendered:?}"
        );
    }

    #[test]
    fn exploring_cluster_uses_unwrapped_shell_commands() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-8".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("bash -lc 'ls -la'".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-9".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("/bin/bash -lc 'cat foo.txt'".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Explored"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ List ls -la"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "    Read foo.txt"),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line.contains("bash -lc")),
            "{rendered:?}"
        );
    }

    #[test]
    fn orphan_finished_exec_does_not_merge_into_active_exploring_group() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-exploring".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("cat /dev/null".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::InProgress,
                exit_code: None,
                duration_ms: None,
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-orphan".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: false,
                input_preview: Some("echo repro-marker".to_string()),
                output_preview: Some("repro-marker\n".to_string()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(5),
            },
        ];

        let committed = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));
        let active = lines_to_strings(&flatten_cells(&build_active_cells(&entries, 80)));

        assert!(
            active.iter().any(|line| line == "• Exploring"),
            "{active:?}"
        );
        assert!(
            active.iter().any(|line| line == "  └ Read /dev/null"),
            "{active:?}"
        );
        assert!(
            committed
                .iter()
                .any(|line| line.contains("Ran echo repro-marker")),
            "{committed:?}"
        );
        assert!(
            !active
                .iter()
                .any(|line| line == "    Read /dev/null, echo repro-marker"),
            "{active:?}"
        );
    }

    #[test]
    fn orphan_finished_exploring_exec_does_not_merge_into_completed_group() {
        let entries = vec![
            TranscriptEntry::ExecCommand {
                call_id: Some("call-ls".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: true,
                input_preview: Some("ls -la".to_string()),
                output_preview: Some(String::new()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(10),
            },
            TranscriptEntry::ExecCommand {
                call_id: Some("call-after".to_string()),
                source: Some("agent".to_string()),
                allow_exploring_group: false,
                input_preview: Some("cat foo.txt".to_string()),
                output_preview: Some("hello\n".to_string()),
                status: agent_core::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(5),
            },
        ];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(
            rendered.iter().any(|line| line == "• Explored"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ List ls -la"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line.contains("Ran cat foo.txt")),
            "{rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line == "    Read foo.txt"),
            "{rendered:?}"
        );
    }

    #[test]
    fn long_exec_command_shows_command_ellipsis_before_output() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-4".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some(
                "echo\nthis_is_a_very_long_single_token_that_will_wrap_over_multiple_lines"
                    .to_string(),
            ),
            output_preview: Some(
                "error: first line on stderr\nerror: second line on stderr".to_string(),
            ),
            status: agent_core::tool_calls::ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: Some(50),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 24)));

        assert!(
            rendered.iter().any(|line| line == "• Ran echo"),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("  │ this_is_a_very_")),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  │ … +2 lines"),
            "{rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  └ error: first line"),
            "{rendered:?}"
        );
    }

    #[test]
    fn exec_output_preview_keeps_only_head_and_tail_lines_like_codex() {
        let entries = vec![TranscriptEntry::ExecCommand {
            call_id: Some("call-5".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("seq 1 10 1>&2 && false".to_string()),
            output_preview: Some(
                (1..=10)
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            status: agent_core::tool_calls::ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: Some(250),
        }];

        let rendered = lines_to_strings(&flatten_cells(&build_cells(&entries, 80)));

        assert!(rendered.iter().any(|line| line == "  └ 1"));
        assert!(rendered.iter().any(|line| line == "    2"));
        assert!(
            rendered
                .iter()
                .any(|line| line == "    … +6 lines (ctrl + t to view transcript)")
        );
        assert!(rendered.iter().any(|line| line == "    9"));
        assert!(rendered.iter().any(|line| line == "    10"));
        assert!(!rendered.iter().any(|line| line == "    3"));
    }
}
