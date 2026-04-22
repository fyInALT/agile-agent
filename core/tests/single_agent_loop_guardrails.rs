#![allow(dead_code, unused_imports, deprecated)]

//! Integration test: single-agent loop — guardrails
//!
//! Tests that loop guardrails (max iterations, max verification failures)
//! correctly stop the autonomous loop.

mod integration_common;

use agent_core::backlog::{TodoItem, TodoStatus};
use agent_core::loop_runner::{LoopGuardrails, run_loop_with_starter};
use agent_core::ProviderEvent;

use integration_common::{MockProviderStarter, TestHarness};

/// Loop stops at max_iterations across multiple tasks.
#[test]
fn loop_stops_at_max_iterations() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    // Push 3 ready todos
    for i in 1..=3 {
        state.backlog.push_todo(TodoItem {
            id: format!("todo-{}", i),
            title: format!("Task {}", i),
            description: format!("Task {} desc", i),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["output is non-empty".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });
    }

    let starter = MockProviderStarter::new();
    // All tasks get the same quick reply
    starter.when_prompt_contains_reply("Task", "done");

    let summary = run_loop_with_starter(
        &mut state,
        LoopGuardrails {
            max_iterations: 2,
            max_continuations_per_task: 1,
            max_verification_failures: 1,
        },
        &starter,
    )
    .expect("run loop");

    assert_eq!(summary.iterations, 2);
    assert_eq!(summary.stopped_reason, "max iterations reached");
}

/// Loop stops at max continuations when assistant keeps requesting continuation.
#[test]
fn loop_stops_at_max_continuations() {
    let harness = TestHarness::new();
    let mut state = harness.create_app_state();

    state.backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Multi-step task".to_string(),
        description: "Needs several steps".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    let starter = MockProviderStarter::new();
    // "next steps" triggers continuation_prompt → TurnResolution::Continue
    starter.when_prompt_contains(
        "Multi-step task",
        vec![
            ProviderEvent::AssistantChunk("Here are the next steps: ...".to_string()),
            ProviderEvent::Finished,
        ],
    );

    let summary = run_loop_with_starter(
        &mut state,
        LoopGuardrails {
            max_iterations: 10,
            max_continuations_per_task: 2,
            max_verification_failures: 1,
        },
        &starter,
    )
    .expect("run loop");

    // Should stop because max continuations (2) + initial = 3 iterations total,
    // but let's just verify it didn't hit max_iterations
    assert!(
        summary.iterations <= 3,
        "expected at most 3 iterations, got {}",
        summary.iterations
    );
    assert_ne!(summary.stopped_reason, "max iterations reached");
}
