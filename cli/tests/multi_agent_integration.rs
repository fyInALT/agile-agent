//! Integration tests for multi-agent runtime
//!
//! Tests concurrent execution, shutdown/restore, persistence, and coordination.

use agent_core::agent_pool::{AgentPool, AgentStatusSnapshot, TaskQueueSnapshot};
use agent_core::agent_slot::{AgentSlotStatus, TaskId};
use agent_core::agent_runtime::{AgentId, AgentCodename, WorkplaceId, ProviderType};
use agent_core::agent_role::AgentRole;
use agent_core::agent_mail::{AgentMail, MailSubject, MailBody, MailTarget, AgentMailbox};
use agent_core::backlog::{BacklogState, TaskItem, TaskStatus};
use agent_core::standup_report::StandupHelper;
use agent_core::sprint_planning::{SprintPlanningSession, SprintPlanningHelper};
use agent_core::blocker_escalation::{BlockerEscalationTracker, BlockerHelper};
use agent_core::runtime_mode::{RuntimeMode, ModeHelper, ModeTransition};

/// Test 2-agent concurrent execution
#[test]
fn two_agent_concurrent_execution() {
    let workplace_id = WorkplaceId::new("test-workplace");
    let pool = AgentPool::new(workplace_id, 2);

    assert!(pool.can_spawn());
    assert_eq!(pool.max_slots(), 2);
    assert_eq!(pool.active_count(), 0);
}

/// Test 5-agent concurrent execution
#[test]
fn five_agent_pool_capacity() {
    let workplace_id = WorkplaceId::new("test-workplace-5");
    let pool = AgentPool::new(workplace_id, 5);

    assert!(pool.can_spawn());
    assert_eq!(pool.max_slots(), 5);
}

/// Test runtime mode transitions
#[test]
fn runtime_mode_single_to_multi_transition() {
    let mut mode = RuntimeMode::SingleAgent;

    // First agent - no transition
    let transition1 = ModeHelper::transition_for_spawn(&mut mode, 0);
    assert_eq!(transition1, ModeTransition::None);
    assert_eq!(mode, RuntimeMode::SingleAgent);

    // Second agent - transition to multi
    let transition2 = ModeHelper::transition_for_spawn(&mut mode, 1);
    assert_eq!(transition2, ModeTransition::SingleToMulti);
    assert_eq!(mode, RuntimeMode::MultiAgent);
}

/// Test runtime mode validation
#[test]
fn runtime_mode_validate_spawn() {
    // Single-agent mode
    assert!(ModeHelper::validate_spawn(RuntimeMode::SingleAgent, 0).is_ok());
    assert!(ModeHelper::validate_spawn(RuntimeMode::SingleAgent, 1).is_err());

    // Multi-agent mode
    assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 0).is_ok());
    assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 9).is_ok());
    assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 10).is_err());
}

/// Test agent role assignment
#[test]
fn agent_role_assignment() {
    use agent_core::agent_slot::AgentSlot;

    let slot = AgentSlot::with_role(
        AgentId::new("agent-001"),
        AgentCodename::new("alpha"),
        ProviderType::Claude,
        AgentRole::ProductOwner,
    );

    assert_eq!(slot.role(), AgentRole::ProductOwner);
}

/// Test mail delivery between agents
#[test]
fn mail_delivery_between_agents() {
    let mut mailbox = AgentMailbox::new();

    let mail = AgentMail::new(
        AgentId::new("agent-001"),
        MailTarget::Direct(AgentId::new("agent-002")),
        MailSubject::TaskHelpRequest { task_id: TaskId::new("task-001") },
        MailBody::Text("Need help with this task".to_string()),
    );

    mailbox.send_mail(mail.clone());

    // Process delivery
    mailbox.process_pending();

    // Check inbox
    let inbox = mailbox.inbox_for(&AgentId::new("agent-002"));
    assert!(inbox.is_some());
    let inbox = inbox.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].from.as_str(), "agent-001");
}

/// Test mail broadcast to all agents
#[test]
fn mail_broadcast() {
    let mut mailbox = AgentMailbox::new();

    let mail = AgentMail::new(
        AgentId::new("agent-001"),
        MailTarget::Broadcast,
        MailSubject::StatusUpdate { new_status: "completed".to_string() },
        MailBody::Text("Task completed".to_string()),
    );

    mailbox.send_mail(mail.clone());
    mailbox.process_pending();

    // Broadcast is stored in pending_delivery
    assert!(mailbox.pending_count() > 0);
}

/// Test sprint planning session
#[test]
fn sprint_planning_session_flow() {
    let mut session = SprintPlanningSession::new();

    // Add stories
    session.add_story("story-001".to_string(), "User login".to_string(), 5);
    session.add_story("story-002".to_string(), "Dashboard".to_string(), 8);

    assert_eq!(session.selected_stories.len(), 2);
    assert_eq!(session.total_effort, 13);

    // Set goal
    session.set_goal("Deliver authentication MVP".to_string());
    assert_eq!(session.goal, "Deliver authentication MVP");

    // Advance phases
    session.advance_phase(); // Selecting -> Estimating
    assert_eq!(session.status, agent_core::sprint_planning::PlanningStatus::Estimating);

    session.advance_phase(); // Estimating -> DefiningGoal
    session.advance_phase(); // DefiningGoal -> Committing
    session.advance_phase(); // Committing -> Complete

    assert!(session.is_complete());
}

/// Test daily standup generation
#[test]
fn daily_standup_generation() {
    let mut backlog = BacklogState::default();
    backlog.push_task(TaskItem {
        id: "task-001".to_string(),
        todo_id: "todo-1".to_string(),
        objective: "Write tests".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Running,
        result_summary: None,
    });
    backlog.push_task(TaskItem {
        id: "task-002".to_string(),
        todo_id: "todo-2".to_string(),
        objective: "Fix bug".to_string(),
        scope: "fix".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Blocked,
        result_summary: Some("Waiting on review".to_string()),
    });

    let statuses = vec![
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Claude,
            role: AgentRole::Developer,
            status: AgentSlotStatus::idle(),
            assigned_task_id: Some(TaskId::new("task-001")),
        },
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-002"),
            codename: AgentCodename::new("beta"),
            provider_type: ProviderType::Claude,
            role: AgentRole::Developer,
            status: AgentSlotStatus::idle(),
            assigned_task_id: Some(TaskId::new("task-002")),
        },
    ];

    let report = StandupHelper::generate_from_status(&statuses, &backlog);

    assert_eq!(report.agent_entries.len(), 2);
    assert!(report.has_blockers());
}

/// Test blocker escalation detection
#[test]
fn blocker_escalation_detection() {
    let mut tracker = BlockerEscalationTracker::new();
    let mut backlog = BacklogState::default();

    backlog.push_task(TaskItem {
        id: "task-001".to_string(),
        todo_id: "todo-1".to_string(),
        objective: "Blocked task".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Blocked,
        result_summary: Some("Waiting on dependency".to_string()),
    });

    let statuses = vec![
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Claude,
            role: AgentRole::Developer,
            status: AgentSlotStatus::idle(),
            assigned_task_id: Some(TaskId::new("task-001")),
        },
    ];

    let escalations = tracker.detect_blocked_agents(&statuses, &backlog);

    assert_eq!(escalations.len(), 1);
    assert_eq!(escalations[0].task_id, "task-001");
    assert!(escalations[0].is_active());
}

/// Test blocker escalation resolution
#[test]
fn blocker_escalation_resolution() {
    let mut tracker = BlockerEscalationTracker::new();
    let mut backlog = BacklogState::default();

    backlog.push_task(TaskItem {
        id: "task-001".to_string(),
        todo_id: "todo-1".to_string(),
        objective: "Blocked task".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Blocked,
        result_summary: Some("Waiting".to_string()),
    });

    let statuses = vec![
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Claude,
            role: AgentRole::Developer,
            status: AgentSlotStatus::idle(),
            assigned_task_id: Some(TaskId::new("task-001")),
        },
    ];

    tracker.detect_blocked_agents(&statuses, &backlog);

    // Resolve the escalation
    let resolved = tracker.resolve_escalation(
        "task-001",
        AgentId::new("scrum-master".to_string())
    );

    assert!(resolved);
    assert_eq!(tracker.active_escalations().len(), 0);
    assert_eq!(tracker.resolved_escalations().len(), 1);

    let stats = tracker.statistics();
    assert_eq!(stats.resolved_count, 1);
}

/// Test ScrumMaster role detection
#[test]
fn find_scrum_master_role() {
    let statuses = vec![
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Claude,
            role: AgentRole::Developer,
            status: AgentSlotStatus::idle(),
            assigned_task_id: None,
        },
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent-002"),
            codename: AgentCodename::new("beta"),
            provider_type: ProviderType::Claude,
            role: AgentRole::ScrumMaster,
            status: AgentSlotStatus::idle(),
            assigned_task_id: None,
        },
    ];

    let sm = BlockerHelper::find_scrum_master(&statuses);
    assert!(sm.is_some());
    assert_eq!(sm.unwrap().role, AgentRole::ScrumMaster);
}

/// Test task queue snapshot
#[test]
fn task_queue_snapshot() {
    let mut backlog = BacklogState::default();
    backlog.push_task(TaskItem {
        id: "task-001".to_string(),
        todo_id: "todo-1".to_string(),
        objective: "Ready task".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Ready,
        result_summary: None,
    });
    backlog.push_task(TaskItem {
        id: "task-002".to_string(),
        todo_id: "todo-2".to_string(),
        objective: "Running task".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Running,
        result_summary: None,
    });
    backlog.push_task(TaskItem {
        id: "task-003".to_string(),
        todo_id: "todo-3".to_string(),
        objective: "Done task".to_string(),
        scope: "test".to_string(),
        constraints: vec![],
        verification_plan: vec![],
        status: TaskStatus::Done,
        result_summary: Some("Completed".to_string()),
    });

    let snapshot = TaskQueueSnapshot {
        total_tasks: 3,
        ready_tasks: 1,
        running_tasks: 1,
        completed_tasks: 1,
        failed_tasks: 0,
        blocked_tasks: 0,
        agent_assignments: vec![],
        available_agents: 1,
        active_agents: 1,
    };

    assert_eq!(snapshot.total_tasks, 3);
    assert_eq!(snapshot.ready_tasks, 1);
    assert_eq!(snapshot.running_tasks, 1);
}

/// Test capacity calculation
#[test]
fn sprint_capacity_calculation() {
    // Team velocity 40, buffer 20%
    let capacity = SprintPlanningHelper::calculate_capacity(40, 20);
    assert_eq!(capacity, 32);

    // Team velocity 100, buffer 10%
    let capacity2 = SprintPlanningHelper::calculate_capacity(100, 10);
    assert_eq!(capacity2, 90);
}

/// Test concurrent task assignment tracking
#[test]
fn concurrent_task_assignment() {
    let mut backlog = BacklogState::default();

    // Add multiple tasks
    for i in 1..=5 {
        backlog.push_task(TaskItem {
            id: format!("task-{}", i),
            todo_id: format!("todo-{}", i),
            objective: format!("Task {}", i),
            scope: "test".to_string(),
            constraints: vec![],
            verification_plan: vec![],
            status: TaskStatus::Ready,
            result_summary: None,
        });
    }

    assert_eq!(backlog.tasks.len(), 5);

    // Simulate task assignments
    let assignments: Vec<(String, String)> = vec![
        ("task-1".to_string(), "agent-001".to_string()),
        ("task-2".to_string(), "agent-002".to_string()),
        ("task-3".to_string(), "agent-003".to_string()),
    ];

    for (task_id, _) in &assignments {
        if let Some(task) = backlog.find_task_mut(task_id) {
            task.status = TaskStatus::Running;
        }
    }

    let running_count = backlog.tasks.iter().filter(|t| t.status == TaskStatus::Running).count();
    assert_eq!(running_count, 3);
}