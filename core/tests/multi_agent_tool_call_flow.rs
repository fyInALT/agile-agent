#![allow(dead_code, unused_imports, deprecated)]


//! Integration test: multi-agent tool call event flow
//!
//! Tests that tool execution events (ExecCommandStarted/Finished) are
//! correctly processed and logged by the runtime.

mod integration_common;

use integration_common::{MockProviderChannel, TestHarness};

/// ExecCommand events are processed for an agent.
#[test]
fn exec_command_events_processed() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let chan = harness.register_mock_provider(&mut session, &agent_id);

    // Pre-set slot to Responding
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
    }

    chan.send_status("running tests");
    chan.send_exec_command_started("cmd-1", "cargo test", "bash");
    chan.send_exec_command_finished("cmd-1", "test result: ok", agent_toolkit::ExecCommandStatus::Completed, 0);
    chan.send_finished();

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 4);

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    // Slot should transition to Finishing after Finished event
    let slot = session.agents().get_slot_by_id(&agent_id).unwrap();
    assert!(
        matches!(slot.status(), agent_core::agent_slot::AgentSlotStatus::Finishing),
        "agent should be finishing, got {:?}",
        slot.status()
    );
}

/// Multiple sequential tool calls are all processed.
#[test]
fn multiple_tool_calls_in_sequence() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let chan = harness.register_mock_provider(&mut session, &agent_id);

    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
    }

    chan.send_exec_command_started("cmd-1", "git status", "bash");
    chan.send_exec_command_finished("cmd-1", "On branch main", agent_toolkit::ExecCommandStatus::Completed, 0);
    chan.send_exec_command_started("cmd-2", "cargo check", "bash");
    chan.send_exec_command_finished("cmd-2", "Finished check", agent_toolkit::ExecCommandStatus::Completed, 0);
    chan.send_finished();

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 5);

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    let slot = session.agents().get_slot_by_id(&agent_id).unwrap();
    assert!(matches!(slot.status(), agent_core::agent_slot::AgentSlotStatus::Finishing));
}

/// Failed tool call is processed without crashing.
#[test]
fn failed_exec_command_event() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let chan = harness.register_mock_provider(&mut session, &agent_id);

    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
    }

    chan.send_exec_command_started("cmd-1", "false", "bash");
    chan.send_exec_command_finished("cmd-1", "", agent_toolkit::ExecCommandStatus::Failed, 1);
    chan.send_finished();

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 3);

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    let slot = session.agents().get_slot_by_id(&agent_id).unwrap();
    assert!(matches!(slot.status(), agent_core::agent_slot::AgentSlotStatus::Finishing));
}
