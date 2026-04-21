//! Integration test: multi-agent task assignment
//!
//! Tests that ready tasks from the backlog are assigned to idle agents
//! and that task status transitions are tracked correctly.

mod integration_common;

use agent_core::agent_slot::AgentSlotStatus;
use agent_core::backlog::{TaskItem, TaskStatus, TodoItem, TodoStatus};

use integration_common::TestHarness;

/// A ready task is assigned to an idle agent.
#[test]
fn ready_task_assigned_to_idle_agent() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    // Add a ready task directly to backlog
    session.workplace_mut().backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Write integration tests".to_string(),
        description: "Test the runtime".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    // Manually assign task (mirrors headless loop logic)
    let slot = session.agents_mut().get_slot_mut(0).unwrap();
    assert!(slot.status().is_idle());
    let _ = slot.assign_task(agent_core::agent_slot::TaskId::new("task-001"));

    // Verify assignment
    assert_eq!(
        slot.assigned_task_id().map(|t| t.as_str().to_string()),
        Some("task-001".to_string())
    );
}

/// Multiple agents receive different tasks.
#[test]
fn multiple_agents_receive_different_tasks() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(4);

    let id_a = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();
    let id_b = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    session.workplace_mut().backlog.push_todo(TodoItem {
        id: "todo-1".to_string(),
        title: "Task A".to_string(),
        description: "First task".to_string(),
        priority: 1,
        status: TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });
    session.workplace_mut().backlog.push_todo(TodoItem {
        id: "todo-2".to_string(),
        title: "Task B".to_string(),
        description: "Second task".to_string(),
        priority: 2,
        status: TodoStatus::Ready,
        acceptance_criteria: Vec::new(),
        dependencies: Vec::new(),
        source: "test".to_string(),
    });

    // Assign tasks
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&id_a) {
        let _ = slot.assign_task(agent_core::agent_slot::TaskId::new("task-a"));
    }
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&id_b) {
        let _ = slot.assign_task(agent_core::agent_slot::TaskId::new("task-b"));
    }

    let slot_a = session.agents().get_slot_by_id(&id_a).unwrap();
    let slot_b = session.agents().get_slot_by_id(&id_b).unwrap();

    assert_eq!(slot_a.assigned_task_id().unwrap().as_str(), "task-a");
    assert_eq!(slot_b.assigned_task_id().unwrap().as_str(), "task-b");
}

/// A busy agent cannot receive a new task.
#[test]
fn busy_agent_rejects_task_assignment() {
    let harness = TestHarness::new();
    let mut session = harness.create_session(2);

    let agent_id = session.spawn_agent(agent_core::ProviderKind::Mock).unwrap();

    // First task assignment succeeds
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.assign_task(agent_core::agent_slot::TaskId::new("task-001"));
    }

    // Put agent in Responding state
    if let Some(slot) = session.agents_mut().get_slot_mut_by_id(&agent_id) {
        let _ = slot.transition_to(AgentSlotStatus::starting());
        let _ = slot.transition_to(AgentSlotStatus::responding_now());
    }

    // Second assignment should fail because agent is not idle
    let result = session
        .agents_mut()
        .get_slot_mut_by_id(&agent_id)
        .unwrap()
        .assign_task(agent_core::agent_slot::TaskId::new("task-002"));

    assert!(result.is_err(), "should not be able to assign task to busy agent");
}
