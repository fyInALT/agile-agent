use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::render::render_app;
use crate::test_support::ShellHarness;
use crate::transcript::cells;
use agent_core::app::TranscriptEntry;
use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::provider::ProviderKind;
use agent_core::workplace_store::WorkplaceStore;

#[test]
fn renders_shell_footer_and_prompt() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    let rendered = shell.render_to_string(80, 24);

    assert!(rendered.contains("Ask agile-agent to do anything"));
    assert!(rendered.contains("tab new agent"));
    assert!(rendered.contains("alpha"));
}

#[test]
fn tab_shows_no_agents_message_when_no_pool() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let rendered = shell.render_to_string(100, 24);

    // Without spawning agents first, Tab shows "no agents to switch"
    assert!(rendered.contains("no agents to switch"));
}

#[test]
fn ctrl_p_switches_provider() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    shell.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let rendered = shell.render_to_string(100, 24);

    assert!(rendered.contains("codex"));
}

#[test]
fn ctrl_t_opens_transcript_overlay() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    shell.paste("hello");
    shell.press(KeyCode::Enter, KeyModifiers::NONE);
    shell.press(KeyCode::Char('t'), KeyModifiers::CONTROL);

    let rendered = shell.render_to_string(100, 24);

    assert!(rendered.contains("Transcript"));
    assert!(rendered.contains("esc close"));
}

#[test]
fn streamed_output_remains_reachable_after_scrolling_up() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    for index in 0..6 {
        shell
            .state
            .app_mut()
            .push_status_message(format!("history line {index}"));
    }
    shell.state.app_mut().begin_provider_response();
    shell.state.app_mut().append_assistant_chunk(
        "第一段输出包含很多很多文字，用来占满屏幕并制造稳定的滚动上下文。\n\n- 这是一个会继续增长的列表项，先有这些内容。",
    );

    shell.render_to_string(20, 10);
    let bottom_offset = shell.state.transcript_scroll_offset;
    shell.state.scroll_transcript_up(2);
    shell.state.scroll_transcript_down(2);
    assert_eq!(shell.state.transcript_scroll_offset, bottom_offset);
    shell
        .state
        .app_mut()
        .append_assistant_chunk(" 然后继续流式追加更多内容，直到最后出现 FINAL-TOKEN。");
    let after_append = shell.render_to_string(20, 10);

    assert!(matches!(
        shell.state.app().transcript.last(),
        Some(TranscriptEntry::Assistant(text)) if text.contains("FINAL-TOKEN")
    ));
    assert!(after_append.contains("FINAL-TOKEN"));
}

#[test]
fn long_assistant_message_shows_complete_content_at_bottom() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    // Create a very long message that should wrap multiple times
    let long_content = "This is a very long line that needs to wrap. ".repeat(20);
    shell.state.app_mut().begin_provider_response();
    shell.state.app_mut().append_assistant_chunk(&long_content);
    shell
        .state
        .app_mut()
        .append_assistant_chunk(" FINAL-END-TOKEN");

    // Render with narrow width to force wrapping
    let rendered = shell.render_to_string(40, 10);

    // The final token should be visible at the bottom
    assert!(
        rendered.contains("FINAL-END-TOKEN"),
        "Final content should be visible. Got: {}",
        rendered
    );
}

#[test]
fn verifies_no_double_wrap_in_render() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    // A line exactly 80 chars that should NOT be double-wrapped
    let exact_80_chars = "X".repeat(80);
    shell.state.app_mut().push_status_message("test header");
    shell.state.app_mut().begin_provider_response();
    shell
        .state
        .app_mut()
        .append_assistant_chunk(&exact_80_chars);
    shell.state.app_mut().append_assistant_chunk("TAIL-MARKER");

    // Render at exactly 80 width - line should fit without wrapping
    let rendered_80 = shell.render_to_string(80, 5);

    // Both the 80-char line and TAIL-MARKER should be visible
    // If double-wrap occurs, the TAIL-MARKER might be pushed off screen
    assert!(
        rendered_80.contains("TAIL-MARKER"),
        "TAIL-MARKER should be visible at 80 width. Rendered:\n{}",
        rendered_80
    );
}

#[test]
fn response_completion_shows_final_content_correctly() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    // Fill transcript with history
    for i in 0..5 {
        shell
            .state
            .app_mut()
            .push_status_message(format!("line {}", i));
    }

    // Start response and add content while "responding"
    shell.state.app_mut().begin_provider_response();
    shell
        .state
        .app_mut()
        .append_assistant_chunk("Streaming content...");
    shell
        .state
        .app_mut()
        .append_assistant_chunk(" more content...");

    // Render while responding (working line takes 1 row)
    let while_responding = shell.render_to_string(60, 8);
    assert!(while_responding.contains("Streaming"));

    // Finish the response - this should make final content visible
    shell.state.app_mut().finish_provider_response();
    shell.state.app_mut().append_assistant_chunk(" FINAL-TOKEN");

    // Render after response completed (no working line, more space for transcript)
    let after_finished = shell.render_to_string(60, 8);

    // The final token must be visible after response completes
    assert!(
        after_finished.contains("FINAL-TOKEN"),
        "FINAL-TOKEN should be visible after response completes. While responding:\n{}\n\nAfter finished:\n{}",
        while_responding,
        after_finished
    );
}

#[test]
fn scroll_offset_accounts_for_ratatui_paragraph_wrap() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    // Create content that cells::build_cells wraps to known number of lines
    // At width 40, a 80-char line should wrap to ~2 lines by cells
    // But ratatui Paragraph.wrap might wrap again if not disabled
    let content = "A".repeat(80); // exactly 80 chars
    shell.state.app_mut().begin_provider_response();
    shell.state.app_mut().append_assistant_chunk(&content);
    shell.state.app_mut().append_assistant_chunk(" END-MARKER");
    shell.state.app_mut().finish_provider_response();

    // Render at width 40, height 5
    let rendered = shell.render_to_string(40, 5);

    // Get the internal line count from cells
    use crate::transcript::cells;
    let cells_lines = cells::flatten_cells(&cells::build_cells(&shell.state.app().transcript, 40));
    let cells_line_count = cells_lines.len();

    // If double-wrap occurs:
    // - cells_line_count would be ~2 (80 chars / 40 width)
    // - but ratatui might produce more actual rendered lines
    // - max_scroll based on cells_line_count would be wrong

    // The END-MARKER must be visible at bottom
    assert!(
        rendered.contains("END-MARKER"),
        "END-MARKER should be visible. cells produced {} lines, scroll_offset={}, max_scroll={}. Rendered:\n{}",
        cells_line_count,
        shell.state.transcript_scroll_offset,
        shell.state.transcript_max_scroll,
        rendered
    );
}

#[test]
fn provider_switch_logs_tui_action() {
    let _guard = logging::test_guard();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    workplace.ensure().expect("ensure");
    logging::init_for_workplace(&workplace, RunMode::Tui).expect("init logger");

    let mut shell = ShellHarness::new(ProviderKind::Claude);
    shell
        .state
        .switch_to_new_agent(ProviderKind::Codex)
        .expect("switch");

    let log_path = logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"tui.provider_switch\""));
}

#[test]
fn manual_scroll_keeps_same_top_line_while_streaming_reflows_content() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    for index in 0..6 {
        shell
            .state
            .app_mut()
            .push_status_message(format!("history line {index}"));
    }

    shell
        .state
        .app_mut()
        .append_assistant_chunk("## Focus\n\n- `agile-agent` is the prim");

    let lines_before = cells::flatten_cells(&cells::build_cells(&shell.state.app().transcript, 18));
    let original_offset = lines_before
        .iter()
        .position(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                == "the prim"
        })
        .expect("top anchor line");
    let original_top = lines_before[original_offset]
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();

    shell.state.transcript_scroll_offset = original_offset;
    shell.state.transcript_follow_tail = false;

    let backend = TestBackend::new(18, 6);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("first draw");
    shell.state.transcript_scroll_offset = original_offset;
    shell.state.transcript_follow_tail = false;
    let first_offset = shell.state.transcript_scroll_offset;
    let first_follow_tail = shell.state.transcript_follow_tail;
    let first_last_cell = shell.state.transcript_last_cell_range;

    shell
        .state
        .app_mut()
        .append_assistant_chunk("ary implementation target in this workspace.");
    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("second draw");

    let lines_after = cells::flatten_cells(&cells::build_cells(&shell.state.app().transcript, 18));
    let top_after = lines_after[shell.state.transcript_scroll_offset]
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert!(
        top_after.contains("primary"),
        "manual scroll anchor drifted out of the active paragraph: before=`{}` after=`{}` first_offset={} second_offset={} first_follow_tail={} second_follow_tail={} first_last_cell={:?} second_last_cell={:?}",
        original_top,
        top_after,
        first_offset,
        shell.state.transcript_scroll_offset,
        first_follow_tail,
        shell.state.transcript_follow_tail,
        first_last_cell,
        shell.state.transcript_last_cell_range
    );
}

#[test]
fn manual_up_scroll_does_not_reenable_follow_tail_during_streaming() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    let backend = TestBackend::new(18, 6);
    let mut terminal = Terminal::new(backend).expect("terminal");

    for index in 0..12 {
        shell
            .state
            .app_mut()
            .push_status_message(format!("history line {index}"));
    }
    shell.state.app_mut().append_assistant_chunk(
        "## Focus\n\n- `agile-agent` is the primary implementation target in this workspace.",
    );

    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("initial draw");

    for _ in 0..5 {
        shell.state.scroll_transcript_up(1);
        shell
            .state
            .app_mut()
            .append_assistant_chunk(" more streaming text");
        terminal
            .draw(|frame| render_app(frame, &mut shell.state))
            .expect("streaming draw");
        assert!(
            !shell.state.transcript_follow_tail,
            "manual scroll unexpectedly re-enabled follow-tail: offset={} max_scroll={}",
            shell.state.transcript_scroll_offset, shell.state.transcript_max_scroll
        );
    }
}

#[test]
fn render_does_not_reenable_follow_tail_just_because_offset_hits_bottom() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    let backend = TestBackend::new(18, 6);
    let mut terminal = Terminal::new(backend).expect("terminal");

    for index in 0..10 {
        shell
            .state
            .app_mut()
            .push_status_message(format!("history line {index}"));
    }

    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("initial draw");

    shell.state.transcript_follow_tail = false;
    shell.state.transcript_scroll_offset = shell.state.transcript_max_scroll;

    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("manual bottom draw");

    assert!(
        !shell.state.transcript_follow_tail,
        "render should not silently re-enable follow-tail when user is in manual mode"
    );
}

#[test]
fn active_cell_height_accounts_for_wrapped_rows() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    shell.state.set_active_entry_for_test(TranscriptEntry::WebSearch {
        call_id: Some("search-1".to_string()),
        query: "example search query with several generic words to exercise wrapping".to_string(),
        action: None,
        started: true,
    });

    let rendered = shell.render_to_string(30, 12);

    assert!(
        rendered.contains("wrapping"),
        "active cell wrapped rows were clipped:\n{}",
        rendered
    );
    assert!(
        rendered.contains("ctrl+j newline"),
        "footer should still render after active cell wrapping:\n{}",
        rendered
    );
}

#[test]
fn streaming_assistant_renders_in_live_tail_area() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    shell.state.append_active_assistant_chunk("streaming assistant reply");

    let rendered = shell.render_to_string(40, 10);

    assert!(
        rendered.contains("streaming assistant reply"),
        "streaming assistant content should render in live tail:\n{}",
        rendered
    );
}

#[test]
fn transcript_redraw_clears_stale_suffix_when_scrolling_to_shorter_lines() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);
    let backend = TestBackend::new(24, 3);
    let mut terminal = Terminal::new(backend).expect("terminal");

    shell
        .state
        .app_mut()
        .push_status_message("short".to_string());
    shell
        .state
        .app_mut()
        .push_status_message("this line is much longer TRAIL-END".to_string());

    shell.state.transcript_follow_tail = false;
    shell.state.transcript_scroll_offset = 3;
    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("first draw");

    shell.state.transcript_scroll_offset = 0;
    terminal
        .draw(|frame| render_app(frame, &mut shell.state))
        .expect("second draw");

    let buf = terminal.backend().buffer();
    let mut rendered = String::new();
    for y in 0..3 {
        for x in 0..24 {
            rendered.push_str(buf[(x, y)].symbol());
        }
        rendered.push('\n');
    }

    assert!(
        !rendered.contains("END"),
        "stale suffix remained after redraw:\n{}",
        rendered
    );
}
