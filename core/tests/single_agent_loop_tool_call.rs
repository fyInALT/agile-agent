#![allow(dead_code, unused_imports, deprecated)]

//! Integration test: single-agent loop — tool call flow
//!
//! Tests that ExecCommand tool events flowing through the loop update
//! the AppState transcript correctly.

mod integration_common;

use agent_core::backlog::{TodoItem, TodoStatus};
use agent_core::loop_runner::{LoopGuardrails, run_single_iteration_with_starter};
use agent_core::ProviderEvent;
use agent_toolkit::ExecCommandStatus;

use integration_common::{MockProviderStarter, TestHarness};

/// A task with a tool call produces transcript entries.
#[test]
fn single_iteration_with_tool_call_updates_transcript() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    state.backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Run command".to_string(),
        description: "Execute a shell command".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: vec!["output is non-empty".to_string()],
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    let starter = MockProviderStarter::new();
    starter.when_prompt_contains(
        "Run command",
        vec![
            ProviderEvent::ExecCommandStarted {
                call_id: Some("cmd-1".to_string()),
                input_preview: Some("echo hello".to_string()),
                source: Some("bash".to_string()),
            },
            ProviderEvent::ExecCommandFinished {
                call_id: Some("cmd-1".to_string()),
                output_preview: Some("hello".to_string()),
                status: ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(50),
                source: Some("bash".to_string()),
            },
            ProviderEvent::AssistantChunk("Command succeeded".to_string()),
            ProviderEvent::Finished,
        ],
    );

    let summary = run_single_iteration_with_starter(&mut state, &starter).expect("run iteration");

    assert!(summary.is_some());
    // Transcript should contain tool call entries
    let has_exec_started = state.transcript.iter().any(|e| {
        matches!(e, agent_core::app::TranscriptEntry::ExecCommand { input_preview: Some(text), .. } if text.contains("echo hello"))
    });
    assert!(has_exec_started, "transcript missing exec command started: {:?}", state.transcript);
}
