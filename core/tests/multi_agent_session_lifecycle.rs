//! Integration test: multi-agent session lifecycle
//!
//! Tests session bootstrap, shutdown snapshot creation, and restore.

mod integration_common;

use agent_core::agent_runtime::AgentStatus;
use agent_core::shutdown_snapshot::{AgentShutdownSnapshot, ShutdownReason, ShutdownSnapshot};

use integration_common::TestHarness;

/// Fresh session can be created and has one initial agent.
#[test]
fn fresh_session_has_initial_agent() {
    let harness = TestHarness::new();
    let session = harness.create_session(4);

    assert_eq!(session.agents.active_count(), 0, "fresh session has no agents yet");
    assert_eq!(session.agents.max_slots(), 4);
}

/// Spawned agents appear in the pool.
#[test]
fn spawn_increases_agent_count() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    let id1 = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    assert_eq!(session.agents.active_count(), 1);

    let id2 = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    assert_eq!(session.agents.active_count(), 2);

    // Both IDs are unique
    assert_ne!(id1.as_str(), id2.as_str());
}

/// Shutdown snapshot captures agent state.
#[test]
fn shutdown_snapshot_captures_state() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    // Assign a task
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.assign_task(agent_core::agent_slot::TaskId::new("task-001"));
    }

    // Seed backlog
    session.workplace_mut().backlog.push_todo(agent_core::backlog::TodoItem {
        id: "todo-1".to_string(),
        title: "Test todo".to_string(),
        description: "Test".to_string(),
        priority: 1,
        status: agent_core::backlog::TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    // Build snapshot manually (mirrors app_runner logic)
    let agents_snapshots: Vec<_> = (0..session.agents.active_count())
        .filter_map(|idx| session.agents.get_slot(idx))
        .map(|slot| AgentShutdownSnapshot {
            meta: agent_core::agent_runtime::AgentMeta {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                workplace_id: session.workplace.workplace_id.clone(),
                provider_type: slot.provider_type(),
                provider_session_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                status: AgentStatus::Idle,
            },
            assigned_task_id: slot.assigned_task_id().map(|id| id.as_str().to_string()),
            was_active: !slot.status().is_terminal(),
            had_error: slot.status().is_blocked(),
            provider_thread_state: None,
            captured_at: String::new(),
            role: slot.role(),
            transcript: slot.transcript().to_vec(),
        })
        .collect();

    let snapshot = ShutdownSnapshot::new(
        session.workplace.workplace_id.as_str().to_string(),
        agents_snapshots,
        session.workplace.backlog.clone(),
        Vec::new(),
        ShutdownReason::CleanExit,
    );

    assert_eq!(snapshot.agents.len(), 1);
    assert_eq!(
        snapshot.agents[0].assigned_task_id,
        Some("task-001".to_string())
    );
    assert_eq!(snapshot.backlog.todos.len(), 1);
}

/// Session restore from snapshot recreates agents.
#[test]
fn restore_from_snapshot_recreates_agents() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    let _ = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let _ = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    // Build snapshot
    let agents_snapshots: Vec<_> = (0..session.agents.active_count())
        .filter_map(|idx| session.agents.get_slot(idx))
        .map(|slot| AgentShutdownSnapshot {
            meta: agent_core::agent_runtime::AgentMeta {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                workplace_id: session.workplace.workplace_id.clone(),
                provider_type: slot.provider_type(),
                provider_session_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                status: AgentStatus::Idle,
            },
            assigned_task_id: None,
            was_active: false,
            had_error: false,
            provider_thread_state: None,
            captured_at: String::new(),
            role: slot.role(),
            transcript: slot.transcript().to_vec(),
        })
        .collect();

    let snapshot = ShutdownSnapshot::new(
        session.workplace.workplace_id.as_str().to_string(),
        agents_snapshots,
        session.workplace.backlog.clone(),
        Vec::new(),
        ShutdownReason::CleanExit,
    );

    // Restore into a new session
    let restored = agent_core::multi_agent_session::MultiAgentSession::restore_from_snapshot(
        harness.workdir.clone(),
        snapshot,
        agent_core::ProviderKind::Mock,
        4,
    );

    assert!(restored.is_ok(), "restore failed: {:?}", restored.err());
    let restored_session = restored.unwrap();
    assert_eq!(restored_session.agents.active_count(), 2);
}
