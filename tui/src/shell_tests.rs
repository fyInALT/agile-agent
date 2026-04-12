use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::test_support::ShellHarness;
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
