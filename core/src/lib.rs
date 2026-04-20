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
pub mod provider_profile;
pub mod runtime_mode;
pub mod runtime_session;
pub mod session_store;
pub mod shared_state;
pub mod shutdown_snapshot;
pub mod skills;
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

#[cfg(test)]
mod backward_compatibility_tests {
    //! Tests to verify backward compatibility of re-exports
    //!
    //! These tests ensure that external code using `agent_core::TypeName`
    //! continues to work after the crate split.

    use super::*;

    // Test that toolkit types are accessible from core
    #[test]
    fn toolkit_types_accessible_from_core() {
        let _kind: PatchChangeKind = PatchChangeKind::Add;
        let _status: PatchApplyStatus = PatchApplyStatus::Completed;
        let _exec: ExecCommandStatus = ExecCommandStatus::InProgress;
        let _mcp: McpToolCallStatus = McpToolCallStatus::Completed;
    }

    // Test that provider types are accessible from core
    #[test]
    fn provider_types_accessible_from_core() {
        let _kind: ProviderKind = ProviderKind::Claude;
        // ProviderEvent can be instantiated
        let _event = ProviderEvent::Finished;
    }

    // Test that worktree types are accessible from core
    #[test]
    fn worktree_types_accessible_from_core() {
        let _config: WorktreeConfig = WorktreeConfig::default();
        let _task_type: TaskType = TaskType::Feature;
        let _priority: TaskPriority = TaskPriority::High;
    }

    // Test that backlog types are accessible from core
    #[test]
    fn backlog_types_accessible_from_core() {
        let _state: BacklogState = BacklogState::default();
        let _task_status: TaskStatus = TaskStatus::Ready;
        let _todo_status: TodoStatus = TodoStatus::Ready;
    }

    // Test that storage function is accessible from core
    #[test]
    fn storage_function_accessible_from_core() {
        let result = app_data_root();
        assert!(result.is_ok());
    }

    // Test that types can be used in function signatures
    fn _accept_provider_kind(kind: ProviderKind) -> ProviderKind {
        kind
    }

    fn _accept_backlog_state(state: BacklogState) -> BacklogState {
        state
    }

    #[test]
    fn types_work_in_function_signatures() {
        let kind = ProviderKind::Mock;
        let result = _accept_provider_kind(kind);
        assert_eq!(result, ProviderKind::Mock);

        let state = BacklogState::default();
        let result = _accept_backlog_state(state);
        assert!(result.todos.is_empty());
    }
}
