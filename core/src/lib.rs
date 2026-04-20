pub mod agent_mail;
pub mod agent_memory;
pub mod agent_messages;
pub mod agent_pool;
pub mod agent_role;
pub mod agent_runtime;
pub mod agent_slot;
pub mod agent_state;
pub mod agent_store;
pub mod agent_transcript;
pub mod app;
pub mod autonomy;
pub mod backlog;
pub mod backlog_store;
pub mod blocker_escalation;
pub mod command_bus;
pub mod commands;
pub mod data_migration;
pub mod decision_agent_slot;
pub mod decision_kanban;
pub mod decision_mail;
pub mod escalation;
pub mod event;
pub mod event_aggregator;
pub mod global_config;
pub mod logging;
pub mod loop_runner;
pub mod multi_agent_session;
pub mod persistence_coordinator;
pub mod pool;
pub mod provider_profile;
pub mod runtime_mode;
pub mod runtime_session;
pub mod session_store;
pub mod shared_state;
pub mod shutdown_snapshot;
pub mod skills;
pub mod slot;
pub mod sprint_planning;
pub mod standup_report;
pub mod storage;
pub mod task_artifacts;
pub mod task_engine;
pub mod verification;
pub mod workplace_store;

// Re-export tool call types from agent-toolkit for backward compatibility
pub use agent_toolkit::{
    PatchChangeKind, PatchApplyStatus, ExecCommandStatus,
    McpToolCallStatus, McpInvocation, WebSearchAction, PatchChange,
};

// Re-export provider types from agent-provider for backward compatibility
pub use agent_provider::{
    ProviderKind, ProviderEvent, SessionHandle, ProviderCapabilities,
    provider_capabilities, ProviderThreadHandle,
    start_provider, start_provider_with_handle, default_provider,
    mock_provider, probe, llm_caller,
    providers,
    launch_config,
};
pub use agent_provider::launch_config::{
    AgentLaunchBundle, LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin, ResolvedLaunchSpec,
    ProviderLaunchContext, ParseError, ValidationError,
    detect_source_mode, parse, parse_command_fragment, parse_env_only,
    format_launch_summary, is_sensitive_key, redact_env_map, redact_env_value,
    generate_host_default_input, resolve_bundle, resolve_decision_launch_spec,
    resolve_executable_path, resolve_host_env, resolve_launch_spec,
    RestoreError, check_restore_eligibility, validate_bundle_executable, validate_executable_exists,
    validate_launch_input_spec, validate_provider_consistency,
    validate_provider_supports_launch_config, validate_reserved_args,
};

// Re-export worktree types from agent-worktree for backward compatibility
pub use agent_worktree::{
    WorktreeManager, WorktreeError, WorktreeConfig, WorktreeInfo,
    WorktreeCreateOptions, WorktreeState, WorktreeStateStore, WorktreeStateStoreError,
    GitFlowExecutor, GitFlowError, GitFlowConfig, GitFlowConfigError,
    TaskType, TaskPriority, PreparationResult, WorkspaceHealthReport,
};

// Re-export backlog types from agent-backlog for backward compatibility
pub use agent_backlog::{
    BacklogState, ThreadSafeBacklog,
    TaskStatus, TodoStatus, TodoItem, TaskItem,
};

// Re-export storage utilities from agent-storage for backward compatibility
pub use agent_storage::app_data_root;

// Re-export slot types for backward compatibility
pub use slot::{AgentSlotStatus, TaskCompletionResult, ThreadOutcome};

// Re-export pool types for backward compatibility
pub use pool::{
    AgentBlockedEvent, AgentBlockedNotifier, NoOpAgentBlockedNotifier,
    BlockedHandler, BlockedHandlingConfig, BlockedHistoryEntry, BlockedTaskPolicy,
    AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot,
    DecisionExecutionResult, DecisionAgentCoordinator, DecisionAgentStats,
};

#[cfg(test)]
mod backward_compatibility_tests {
    //! Tests to verify backward compatibility of re-exports
    //!
    //! These tests ensure that external code using `agent_core::TypeName`
    //! continues to work after the crate split, AND that the types
    //! actually function correctly (not just compile).

    use super::*;

    // Test that toolkit types are accessible AND functional from core
    #[test]
    fn toolkit_types_accessible_and_functional() {
        // PatchChangeKind - verify serialization works
        let kind = PatchChangeKind::Add;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"add\"", "PatchChangeKind should serialize correctly");

        // PatchApplyStatus - verify all variants exist
        let statuses = [PatchApplyStatus::Completed, PatchApplyStatus::Failed];
        assert_eq!(statuses.len(), 2, "PatchApplyStatus variants should exist");

        // ExecCommandStatus - verify roundtrip
        let status = ExecCommandStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        let parsed: ExecCommandStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status, "ExecCommandStatus roundtrip should work");

        // McpToolCallStatus - verify Display/Debug trait
        let mcp = McpToolCallStatus::Completed;
        let debug_str = format!("{:?}", mcp);
        assert!(debug_str.contains("Completed"), "McpToolCallStatus should implement Debug");
    }

    // Test that provider types are accessible AND functional from core
    #[test]
    fn provider_types_accessible_and_functional() {
        // ProviderKind - verify label() method works
        let kind = ProviderKind::Claude;
        assert_eq!(kind.label(), "claude", "ProviderKind::label() should return correct label");

        // ProviderKind - verify next() cycle works
        let mock = ProviderKind::Mock;
        let next = mock.next();
        assert_eq!(next, ProviderKind::Claude, "ProviderKind::next() should cycle correctly");

        // ProviderEvent - verify Debug trait works (used in logging)
        let event = ProviderEvent::Finished;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Finished"), "ProviderEvent should implement Debug");
    }

    // Test that worktree types are accessible AND functional from core
    #[test]
    fn worktree_types_accessible_and_functional() {
        // WorktreeConfig - verify default values are sensible
        let config = WorktreeConfig::default();
        assert!(config.max_worktrees > 0, "WorktreeConfig::max_worktrees should be positive");
        assert!(!config.prefix.is_empty(), "WorktreeConfig::prefix should not be empty");

        // TaskType - verify serialization roundtrip
        let task_type = TaskType::Feature;
        let json = serde_json::to_string(&task_type).unwrap();
        let parsed: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, task_type, "TaskType roundtrip should work");

        // TaskPriority - verify variants exist and are distinct
        let high = TaskPriority::High;
        let low = TaskPriority::Low;
        assert_ne!(high, low, "TaskPriority variants should be distinct");
    }

    // Test that backlog types are accessible AND functional from core
    #[test]
    fn backlog_types_accessible_and_functional() {
        // BacklogState - verify push_task actually adds task
        let mut state = BacklogState::default();
        state.push_task(TaskItem {
            id: "test-task".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "test objective".to_string(),
            scope: "test scope".to_string(),
            constraints: vec!["c1".to_string()],
            verification_plan: vec!["v1".to_string()],
            status: TaskStatus::Ready,
            result_summary: None,
        });
        assert_eq!(state.tasks.len(), 1, "BacklogState::push_task should add task");
        assert!(state.find_task("test-task").is_some(), "BacklogState::find_task should find added task");

        // TaskStatus - verify status transitions
        let status = TaskStatus::Ready;
        assert!(status != TaskStatus::Done, "TaskStatus variants should be distinct");

        // TodoStatus - verify serialization roundtrip
        let todo_status = TodoStatus::InProgress;
        let json = serde_json::to_string(&todo_status).unwrap();
        let parsed: TodoStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, todo_status, "TodoStatus roundtrip should work");
    }

    // Test that storage function is accessible AND functional from core
    #[test]
    fn storage_function_accessible_and_functional() {
        let result = app_data_root();
        assert!(result.is_ok(), "app_data_root() should return Ok");

        let path = result.unwrap();
        assert!(path.is_absolute(), "app_data_root() should return absolute path");
        assert!(path.ends_with("agile-agent"), "app_data_root() should end with 'agile-agent'");
    }

    // Test that types can be used in function signatures with actual behavior
    fn _accept_provider_kind(kind: ProviderKind) -> ProviderKind {
        kind.next()
    }

    fn _accept_backlog_state(state: BacklogState) -> usize {
        state.tasks.len()
    }

    #[test]
    fn types_work_in_function_signatures() {
        // ProviderKind - verify function returns transformed value
        let kind = ProviderKind::Mock;
        let result = _accept_provider_kind(kind);
        assert_eq!(result, ProviderKind::Claude, "Function should transform ProviderKind");

        // BacklogState - verify function can access internal state
        let mut state = BacklogState::default();
        state.push_task(TaskItem {
            id: "sig-test".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "test".to_string(),
            scope: "test".to_string(),
            constraints: vec!["c1".to_string()],
            verification_plan: vec!["v1".to_string()],
            status: TaskStatus::Ready,
            result_summary: None,
        });
        let count = _accept_backlog_state(state);
        assert_eq!(count, 1, "Function should return correct task count");
    }

    // Test launch_config types are accessible AND functional from core
    #[test]
    fn launch_config_types_accessible_and_functional() {
        // Test that launch_config module is accessible
        use launch_config::{LaunchInputSpec, LaunchSourceMode};

        // LaunchInputSpec - can be constructed using new()
        let spec = LaunchInputSpec::new(ProviderKind::Mock);
        assert_eq!(spec.provider, ProviderKind::Mock, "LaunchInputSpec::new should set provider");
        assert_eq!(spec.source_mode, LaunchSourceMode::HostDefault, "LaunchInputSpec::new should set HostDefault mode");

        // Test that validation functions are accessible
        let args = vec!["--test".to_string()];
        let result = launch_config::validate_reserved_args(&args, ProviderKind::Mock);
        assert!(result.is_ok(), "validate_reserved_args should be callable");

        // Test parse function exists and works with provider kind
        let input = "--prompt test";
        let result = launch_config::parse(ProviderKind::Mock, input);
        // parse returns Result, just check it's callable
        let _ = result;

        // Test LaunchSourceMode serialization
        let mode = LaunchSourceMode::EnvOnly;
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains("env_only") || json.contains("EnvOnly"), "LaunchSourceMode should serialize");
    }
}
