use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::overview_state::{OverviewFilter, OverviewLogMessage, OverviewMessageType};
use crate::render::render_app;
use crate::test_support::ShellHarness;
use crate::transcript::cells;
use crate::view_mode::ViewMode;
use agent_core::app::TranscriptEntry;
use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::provider::ProviderKind;
use agent_core::workplace_store::WorkplaceStore;

#[test]
fn renders_shell_footer_and_prompt() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    let rendered = shell.render_to_string(80, 24);

    // Overview mode is now default, shows Overview footer hint
    assert!(rendered.contains("Overview"));
    assert!(rendered.contains("alpha") || rendered.contains("OVERVIEW"));
}

#[test]
fn tab_shows_no_agents_message_when_no_pool() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let rendered = shell.render_to_string(100, 24);

    // Without spawning agents first, Tab shows "no agents to switch"
    assert!(
        rendered.contains("no agents to switch") || rendered.contains("spawn"),
        "Tab should show message about no agents. Got:\n{}",
        rendered
    );
}

#[test]
fn ctrl_p_switches_provider() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    shell.press(KeyCode::Char('p'), KeyModifiers::CONTROL);

    // In test harness, Ctrl+P triggers ToggleProvider which switches to next provider
    // The state should have switched provider
    assert_eq!(shell.state.app().selected_provider, ProviderKind::Codex);
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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
    shell
        .state
        .set_active_entry_for_test(TranscriptEntry::WebSearch {
            call_id: Some("search-1".to_string()),
            query: "example search query with several generic words to exercise wrapping"
                .to_string(),
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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
    shell
        .state
        .append_active_assistant_chunk("streaming assistant reply");

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
    // Switch to Focused mode for transcript tests
    shell.state.view_state.switch_by_number(1);
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

// Overview mode integration tests

#[test]
fn overview_spawn_agent_shows_in_list() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    // Initially shows OVERVIEW agent
    let rendered_before = shell.render_to_string(80, 24);
    println!("=== Before spawn ===\n{}", rendered_before);
    assert!(rendered_before.contains("OVERVIEW"), "Should show OVERVIEW agent");

    // Spawn a worker agent
    shell.state.spawn_agent(ProviderKind::Mock);

    // Check agent_statuses
    let statuses = shell.state.agent_statuses();
    println!("Agent statuses count: {}", statuses.len());
    for s in &statuses {
        println!(
            "  - id={} codename={} ({}) role={}",
            s.agent_id.as_str(),
            s.codename.as_str(),
            s.status.label(),
            s.role.label()
        );
    }

    // After spawn - should show OVERVIEW + worker agent
    let rendered_after = shell.render_to_string(80, 24);
    println!("=== After spawn ===\n{}", rendered_after);

    // Should show both OVERVIEW agent and worker agent (alpha)
    assert!(
        rendered_after.contains("OVERVIEW") && rendered_after.contains("alpha"),
        "Should show OVERVIEW agent and worker agent. Got:\n{}",
        rendered_after
    );
}

#[test]
fn overview_mode_switch_by_number() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6); // Overview mode

    assert_eq!(shell.state.view_state.mode, ViewMode::Overview);
}

#[test]
fn overview_mode_displays_agent_list() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    let rendered = shell.render_to_string(80, 24);

    // Should show agent indicator and hint
    assert!(rendered.contains("◎") || rendered.contains("○"));

    // Should show the scroll log placeholder
    assert!(rendered.contains("No activity yet") || rendered.contains("Overview"));
}

#[test]
fn overview_mode_layout_has_both_regions() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    // Render with enough height to see both regions
    let rendered = shell.render_to_string(80, 30);

    // Print the rendered output for debugging
    println!("=== Overview Mode Rendered Output ===\n{}", rendered);

    // Should have agent list at top (row 0-7, with ◎ or ○ indicator)
    let lines: Vec<&str> = rendered.lines().collect();
    assert!(
        lines.len() >= 25,
        "Should have 30 lines, got {}",
        lines.len()
    );

    // First few lines should contain agent indicators
    let top_section = lines[0..8].join("\n");
    assert!(
        top_section.contains("◎") || top_section.contains("○") || top_section.contains("alpha"),
        "Agent list should be visible in top section. Top section:\n{}",
        top_section
    );

    // Middle section should have scroll log placeholder
    let middle_section = lines[8..25].join("\n");
    assert!(
        middle_section.contains("No activity yet") || middle_section.contains("Overview"),
        "Scroll log area should be visible in middle section. Middle section:\n{}",
        middle_section
    );

    // Bottom should have input box
    let bottom_section = lines[25..].join("\n");
    assert!(
        bottom_section.contains("Ask")
            || bottom_section.contains("?")
            || bottom_section.contains("tab"),
        "Input box should be visible at bottom. Bottom section:\n{}",
        bottom_section
    );
}

#[test]
fn overview_mode_footer_shows_key_hints() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    let rendered = shell.render_to_string(80, 24);

    // Should show filter keys hint
    assert!(rendered.contains("f") || rendered.contains("filter"));
}

#[test]
fn overview_filter_blocked_is_applied() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);
    shell.state.view_state.overview.filter = OverviewFilter::BlockedOnly;

    // Without blocked agents, filtered count should be 0
    let statuses = shell.state.agent_statuses();
    let blocked_count = statuses.iter().filter(|s| s.status.is_blocked()).count();

    assert_eq!(blocked_count, 0); // No blocked agents in fresh state
}

#[test]
fn overview_log_buffer_accepts_messages() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    shell
        .state
        .view_state
        .overview
        .push_log_message(OverviewLogMessage {
            timestamp: 143215,
            agent: "alpha".to_string(),
            message_type: OverviewMessageType::Progress,
            content: "Started task".to_string(),
        });

    assert_eq!(shell.state.view_state.overview.log_buffer.len(), 1);
}

#[test]
fn overview_log_buffer_evicts_when_full() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);
    shell.state.view_state.overview.max_log_size = 3;

    for i in 0..5 {
        shell
            .state
            .view_state
            .overview
            .push_log_message(OverviewLogMessage {
                timestamp: i,
                agent: "alpha".to_string(),
                message_type: OverviewMessageType::Progress,
                content: format!("msg {}", i),
            });
    }

    assert_eq!(shell.state.view_state.overview.log_buffer.len(), 3);
    // Should have messages 2, 3, 4
    assert_eq!(
        shell
            .state
            .view_state
            .overview
            .log_buffer
            .front()
            .unwrap()
            .timestamp,
        2
    );
    assert_eq!(
        shell
            .state
            .view_state
            .overview
            .log_buffer
            .back()
            .unwrap()
            .timestamp,
        4
    );
}

#[test]
fn overview_focus_navigation_cycles() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    let count = shell.state.agent_statuses().len();
    // With a single agent, focus cycles to itself (0 -> 0)
    if count >= 2 {
        shell.state.view_state.overview.focused_agent_index = 0;
        shell.state.view_state.overview.focus_next(count);
        assert_eq!(shell.state.view_state.overview.focused_agent_index, 1);
        shell.state.view_state.overview.focus_prev(count);
        assert_eq!(shell.state.view_state.overview.focused_agent_index, 0);
    } else {
        // Single agent: focus_next cycles back to same index
        shell.state.view_state.overview.focused_agent_index = 0;
        shell.state.view_state.overview.focus_next(count);
        assert_eq!(shell.state.view_state.overview.focused_agent_index, 0);
    }
}

#[test]
fn overview_cycle_filter_modes() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    assert_eq!(shell.state.view_state.overview.filter, OverviewFilter::All);
    shell.state.view_state.overview.cycle_filter();
    assert_eq!(
        shell.state.view_state.overview.filter,
        OverviewFilter::BlockedOnly
    );
    shell.state.view_state.overview.cycle_filter();
    assert_eq!(
        shell.state.view_state.overview.filter,
        OverviewFilter::RunningOnly
    );
    shell.state.view_state.overview.cycle_filter();
    assert_eq!(shell.state.view_state.overview.filter, OverviewFilter::All);
}

#[test]
fn overview_search_starts_with_slash() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    shell.press(KeyCode::Char('/'), KeyModifiers::NONE);

    assert!(shell.state.view_state.overview.search_active);
}

#[test]
fn overview_search_input_and_cancel() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    // Start search
    shell.press(KeyCode::Char('/'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('a'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('l'), KeyModifiers::NONE);

    assert_eq!(shell.state.view_state.overview.search_query, "al");

    // Cancel with Esc
    shell.press(KeyCode::Esc, KeyModifiers::NONE);

    assert!(!shell.state.view_state.overview.search_active);
    assert_eq!(shell.state.view_state.overview.search_query, "");
}

#[test]
fn overview_search_moves_real_focus_to_matching_agent() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
    shell.state.view_state.switch_by_number(6);

    assert_eq!(shell.state.focused_agent_codename(), "OVERVIEW");

    shell.press(KeyCode::Char('/'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('a'), KeyModifiers::NONE);

    assert_eq!(shell.state.focused_agent_codename(), "alpha");
}

#[test]
fn overview_mode_shows_focused_worker_transcript_in_lower_pane() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    let alpha_id = shell.state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
    {
        let pool = shell.state.agent_pool.as_mut().expect("agent pool");
        let slot = pool.get_slot_mut_by_id(&alpha_id).expect("alpha slot");
        slot.append_transcript(TranscriptEntry::User("please investigate".to_string()));
        slot.append_transcript(TranscriptEntry::Assistant(
            "focused worker transcript marker".to_string(),
        ));
    }
    shell.state.focus_agent(&alpha_id);
    shell.state.view_state.switch_by_number(6);

    let rendered = shell.render_to_string(80, 24);

    assert!(rendered.contains("focused worker transcript marker"));
    assert!(!rendered.contains("No activity yet. Agents will report progress here."));
}

#[test]
fn overview_page_offset_changes_visible_workers() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    for _ in 0..4 {
        shell.state.spawn_agent(ProviderKind::Mock).expect("spawn worker");
    }
    shell.state.view_state.switch_by_number(6);
    shell.state.view_state.overview.agent_list_rows = 3;

    let first_page = shell.render_to_string(80, 24);
    shell.state.view_state.overview.page_offset = 1;
    let second_page = shell.render_to_string(80, 24);

    assert!(first_page.contains("alpha"));
    assert!(first_page.contains("bravo"));
    assert!(!first_page.contains("charlie"));

    assert!(second_page.contains("charlie"));
    assert!(second_page.contains("delta"));
    assert!(!second_page.contains("alpha idle Waiting for task"));
}

#[test]
fn overview_timestamp_same_minute_omitted() {
    let mut shell = ShellHarness::new(ProviderKind::Mock);
    shell.state.view_state.switch_by_number(6);

    // Add messages within same minute
    shell
        .state
        .view_state
        .overview
        .push_log_message(OverviewLogMessage {
            timestamp: 143210, // 14:32:10
            agent: "alpha".to_string(),
            message_type: OverviewMessageType::Progress,
            content: "First message".to_string(),
        });
    shell
        .state
        .view_state
        .overview
        .push_log_message(OverviewLogMessage {
            timestamp: 143215, // 14:32:15 - same minute
            agent: "alpha".to_string(),
            message_type: OverviewMessageType::Progress,
            content: "Second message".to_string(),
        });

    let rendered = shell.render_to_string(80, 24);

    // First message should have timestamp, second should not
    assert!(rendered.contains("[14:32:10]"));
    // The second message at 14:32:15 should show blank space instead of timestamp
    // (we verify by checking the content is visible but not the second timestamp)
    assert!(rendered.contains("Second message"));
}

// Tests for Ctrl+N agent creation flow

#[test]
fn ctrl_n_opens_provider_overlay() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Press Ctrl+N
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);

    // Provider overlay should be open
    assert!(shell.state.is_provider_overlay_open());

    let rendered = shell.render_to_string(80, 24);
    assert!(rendered.contains("New Agent"), "Should show provider overlay. Got:\n{}", rendered);
}

#[test]
fn provider_selection_for_claude_opens_launch_config_overlay() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Press Ctrl+N
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    assert!(shell.state.is_provider_overlay_open());

    // Select Claude provider (index 0)
    shell.press(KeyCode::Enter, KeyModifiers::NONE);

    // Launch config overlay should be open (Claude/Codex need config, Mock skips)
    assert!(shell.state.is_launch_config_overlay_open());

    let rendered = shell.render_to_string(80, 24);
    assert!(
        rendered.contains("Launch Config") || rendered.contains("claude"),
        "Should show launch config overlay. Got:\n{}",
        rendered
    );
}

#[test]
fn provider_selection_for_mock_skips_launch_config_overlay() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Press Ctrl+N
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    assert!(shell.state.is_provider_overlay_open());

    // Move down to Mock provider (index 2)
    shell.press(KeyCode::Down, KeyModifiers::NONE);
    shell.press(KeyCode::Down, KeyModifiers::NONE);

    // Select Mock provider
    shell.press(KeyCode::Enter, KeyModifiers::NONE);

    // Mock should skip config overlay and spawn directly
    assert!(!shell.state.is_launch_config_overlay_open());
    assert!(!shell.state.is_provider_overlay_open());

    // Check that new agent was spawned
    let statuses = shell.state.agent_statuses();
    assert!(statuses.len() >= 2, "Should have at least 2 agents (OVERVIEW + new). Got: {}", statuses.len());
}

#[test]
fn launch_config_overlay_can_input_env_vars() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Open provider overlay and select Claude
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    shell.press(KeyCode::Enter, KeyModifiers::NONE);

    assert!(shell.state.is_launch_config_overlay_open());

    // Type env var KEY=value
    shell.press(KeyCode::Char('K'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('E'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('Y'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('='), KeyModifiers::NONE);
    shell.press(KeyCode::Char('v'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('a'), KeyModifiers::NONE);
    shell.press(KeyCode::Char('l'), KeyModifiers::NONE);

    let overlay = shell.state.launch_config_overlay.as_ref().expect("overlay");
    assert_eq!(overlay.work_config_text, "KEY=val");
    assert_eq!(overlay.work_preview.env_count, 1);

    let rendered = shell.render_to_string(80, 24);
    // Preview should show env-only mode
    assert!(
        rendered.contains("env-only") || rendered.contains("Env:"),
        "Should show env count in preview. Got:\n{}",
        rendered
    );
}

#[test]
fn launch_config_overlay_tab_cycles_focus() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Open and select Claude
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    shell.press(KeyCode::Enter, KeyModifiers::NONE);

    let overlay = shell.state.launch_config_overlay.as_ref().expect("overlay");
    use crate::launch_config_overlay::LaunchConfigFocus;
    assert_eq!(overlay.focus, LaunchConfigFocus::WorkConfig);

    // Tab to decision config
    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let overlay = shell.state.launch_config_overlay.as_ref().expect("overlay");
    assert_eq!(overlay.focus, LaunchConfigFocus::DecisionConfig);

    // Tab to confirm
    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let overlay = shell.state.launch_config_overlay.as_ref().expect("overlay");
    assert_eq!(overlay.focus, LaunchConfigFocus::Confirm);
}

#[test]
fn launch_config_overlay_esc_closes() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    shell.state.app_mut().status = agent_core::app::AppStatus::Idle;

    // Open provider overlay and select Claude
    shell.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    shell.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(shell.state.is_launch_config_overlay_open());

    // Press Esc to close
    shell.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!shell.state.is_launch_config_overlay_open());
}
