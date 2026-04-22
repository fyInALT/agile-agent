#![allow(dead_code, unused_imports, deprecated)]

//! Integration test: single-agent loop — simple task completion
//!
//! Tests the full autonomous loop with an injectable ProviderStarter.

mod integration_common;

use agent_core::backlog::{TodoItem, TodoStatus};
use agent_core::loop_runner::{LoopGuardrails, run_loop_with_starter};

use integration_common::{MockProviderStarter, TestHarness};

/// A ready todo with objective and verification plan completes in one iteration.
#[test]
fn loop_completes_single_task() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    // Seed backlog with a well-formed task
    state.backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Say hello".to_string(),
        description: "Simple greeting task".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: vec!["output is non-empty".to_string()],
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    let starter = MockProviderStarter::new();
    starter.when_prompt_contains_reply("Say hello", "Hello world!");

    let summary = run_loop_with_starter(
        &mut state,
        LoopGuardrails {
            max_iterations: 5,
            max_continuations_per_task: 3,
            max_verification_failures: 1,
        },
        &starter,
    )
    .expect("run loop");

    assert_eq!(summary.iterations, 1);
    assert_eq!(summary.verification_failures, 0);
    assert_eq!(
        summary.stopped_reason, "no ready todo available",
        "loop should stop because todo is done"
    );

    // Backlog todo should be Done
    assert_eq!(state.backlog.todos[0].status, TodoStatus::Done);
    // Task should be completed
    assert_eq!(state.backlog.tasks.len(), 1);
    assert_eq!(state.backlog.tasks[0].status, agent_core::backlog::TaskStatus::Done);
}

/// Loop with no ready todos stops immediately.
#[test]
fn loop_stops_when_no_ready_todos() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    let starter = MockProviderStarter::new();

    let summary = run_loop_with_starter(
        &mut state,
        LoopGuardrails {
            max_iterations: 5,
            max_continuations_per_task: 3,
            max_verification_failures: 1,
        },
        &starter,
    )
    .expect("run loop");

    assert_eq!(summary.iterations, 0);
    assert_eq!(summary.stopped_reason, "no ready todo available");
}

/// A task whose provider returns empty text fails and gets retried until max_iterations.
#[test]
fn loop_fails_when_provider_returns_empty() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    state.backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Do nothing".to_string(),
        description: "Empty reply".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    let starter = MockProviderStarter::new();
    // Send Finished without any AssistantChunk → empty summary
    starter.when_prompt_contains(
        "Do nothing",
        vec![agent_core::ProviderEvent::Finished],
    );

    let summary = run_loop_with_starter(
        &mut state,
        LoopGuardrails {
            max_iterations: 3,
            max_continuations_per_task: 1,
            max_verification_failures: 1,
        },
        &starter,
    )
    .expect("run loop");

    // fail_active_task resets todo to Ready, so loop retries until max_iterations
    assert_eq!(summary.iterations, 3);
    assert_eq!(summary.stopped_reason, "max iterations reached");
}
