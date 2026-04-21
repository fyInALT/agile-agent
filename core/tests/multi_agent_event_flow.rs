//! Integration test: multi-agent provider event flow
//!
//! Tests that ProviderEvents injected via channels are correctly processed
//! by MultiAgentSession, updating agent slots, transcripts, and status.

mod integration_common;

use agent_core::agent_runtime::AgentId;
use agent_core::agent_slot::AgentSlotStatus;
use agent_core::app::TranscriptEntry;
use agent_core::event_aggregator::AgentEvent;

use integration_common::{EventSequence, TestHarness};

/// Two agents receive distinct event streams and maintain independent state.
#[test]
fn two_agents_receive_distinct_events() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    // Spawn two agents
    let id_a = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let id_b = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    // Register mock channels for both agents
    let chan_a = harness.register_mock_provider(&mut session, &id_a);
    let chan_b = harness.register_mock_provider(&mut session, &id_b);

    // Pre-set agent slots to Responding so Finished/Error transitions are valid
    for id in [&id_a, &id_b] {
        if let Some(slot) = session.agents_mut().get_slot_mut_by_id(id) {
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
        }
    }

    // Send different event sequences to each agent
    EventSequence::new()
        .status("agent A working")
        .assistant("Hello from A")
        .finished()
        .send_all(&chan_a);

    EventSequence::new()
        .status("agent B working")
        .assistant("Hello from B")
        .thinking("thinking...")
        .finished()
        .send_all(&chan_b);

    // Poll and process all events
    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 7, "expected 7 provider events (3+4)");

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    // Verify agent A state
    let slot_a = session.agents().get_slot_by_id(&id_a).unwrap();
    assert!(
        matches!(slot_a.status(), AgentSlotStatus::Finishing { .. }),
        "agent A should be finishing, got {:?}",
        slot_a.status()
    );

    // Verify agent B state
    let slot_b = session.agents().get_slot_by_id(&id_b).unwrap();
    assert!(
        matches!(slot_b.status(), AgentSlotStatus::Finishing { .. }),
        "agent B should be finishing, got {:?}",
        slot_b.status()
    );
}

/// An agent that receives an Error event transitions to Error status.
#[test]
fn agent_error_event_transitions_to_error_status() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let chan = harness.register_mock_provider(&mut session, &agent_id);

    // Pre-set slot to Responding so Error transition is valid
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
    }

    EventSequence::new()
        .status("starting")
        .assistant("working")
        .error("something went wrong")
        .send_all(&chan);

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 3);

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    let slot = session.agents().get_slot_by_id(&agent_id).unwrap();
    assert!(
        matches!(slot.status(), AgentSlotStatus::Error { .. }),
        "agent should be in error status, got {:?}",
        slot.status()
    );

    // Transcript should contain the error
    let transcript: Vec<_> = slot.transcript().iter().collect();
    assert!(
        transcript.iter().any(|e| matches!(e, TranscriptEntry::Error(text) if text.contains("something went wrong"))),
        "transcript missing error entry: {:?}",
        transcript
    );
}

/// SessionHandle event updates the agent's session metadata.
#[test]
fn session_handle_event_updates_slot() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let chan = harness.register_mock_provider(&mut session, &agent_id);

    // Pre-set slot to Responding so Finished transition is valid
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
        let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
    }

    chan.send_session_handle(agent_core::SessionHandle::ClaudeSession {
        session_id: "sess-test-123".to_string(),
    });
    chan.send_finished();

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 2);

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    let slot = session.agents().get_slot_by_id(&agent_id).unwrap();
    let handle = slot.session_handle().expect("session handle should be set");
    assert_eq!(
        format!("{:?}", handle),
        "ClaudeSession { session_id: \"sess-test-123\" }"
    );
}

/// Events are routed correctly even when polled in a single batch.
#[test]
fn mixed_events_routed_to_correct_agents() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    let id_a = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let id_b = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let id_c = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    let chan_a = harness.register_mock_provider(&mut session, &id_a);
    let chan_b = harness.register_mock_provider(&mut session, &id_b);
    // Agent C gets no events

    // Pre-set A and B slots to Responding; C stays idle
    for id in [&id_a, &id_b] {
        if let Some(slot) = session.agents_mut().get_slot_mut_by_id(id) {
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::starting());
            let _ = slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
        }
    }

    // Interleave events for A and B
    chan_a.send_status("A step 1");
    chan_b.send_status("B step 1");
    chan_a.send_assistant_chunk("A reply");
    chan_b.send_assistant_chunk("B reply");
    chan_a.send_finished();
    chan_b.send_finished();

    let poll_result = session.poll_events();
    assert_eq!(poll_result.events.len(), 6);

    // Verify routing: each event carries the correct agent_id
    let mut a_count = 0;
    let mut b_count = 0;
    for event in &poll_result.events {
        match event {
            AgentEvent::FromProvider { agent_id, .. } => {
                if agent_id == &id_a {
                    a_count += 1;
                } else if agent_id == &id_b {
                    b_count += 1;
                } else {
                    panic!("unexpected agent_id in event: {:?}", agent_id);
                }
            }
            _ => {}
        }
    }
    assert_eq!(a_count, 3, "agent A should have 3 events");
    assert_eq!(b_count, 3, "agent B should have 3 events");

    for event in poll_result.events {
        session.process_event(event).expect("process event");
    }

    // Agent C should still be idle
    let slot_c = session.agents().get_slot_by_id(&id_c).unwrap();
    assert!(
        slot_c.status().is_idle(),
        "agent C should remain idle, got {:?}",
        slot_c.status()
    );
}
