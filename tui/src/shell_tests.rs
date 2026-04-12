use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::test_support::ShellHarness;
use agent_core::app::TranscriptEntry;
use agent_core::provider::ProviderKind;

#[test]
fn renders_shell_footer_and_prompt() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    let rendered = shell.render_to_string(80, 24);

    assert!(rendered.contains("Ask agile-agent to do anything"));
    assert!(rendered.contains("tab new agent"));
    assert!(rendered.contains("alpha"));
}

#[test]
fn tab_creates_new_agent_and_updates_provider_label() {
    let mut shell = ShellHarness::new(ProviderKind::Claude);

    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let rendered = shell.render_to_string(100, 24);

    assert!(rendered.contains("bravo"));
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
    shell.state.app_mut().append_assistant_chunk(
        " 然后继续流式追加更多内容，直到最后出现 FINAL-TOKEN。",
    );
    let after_append = shell.render_to_string(20, 10);

    assert!(matches!(
        shell.state.app().transcript.last(),
        Some(TranscriptEntry::Assistant(text)) if text.contains("FINAL-TOKEN")
    ));
    assert!(after_append.contains("FINAL-TOKEN"));
}
