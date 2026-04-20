//! Integration tests for decision crate

use agent_decision::task_preparation::{
    PreAction, PreparationStep, TaskPreparationPipeline, TaskPreparationRequest,
    TaskPreparationResult,
};
use std::path::PathBuf;

/// Integration test: TaskPreparationPipeline requires git operations
/// Test that pipeline correctly handles non-existent path (should return Failed)
#[test]
fn test_task_preparation_pipeline_nonexistent_path() {
    let pipeline = TaskPreparationPipeline::new();

    let request = TaskPreparationRequest {
        task_description: "Test task description".to_string(),
        task_id: Some("test-001".to_string()),
        worktree_path: PathBuf::from("/nonexistent/path"),
        agent_id: "agent-001".to_string(),
    };

    // Pipeline should handle nonexistent path gracefully
    let result = pipeline.prepare(&request);

    // Should return Failed since git state analysis will fail on nonexistent path
    match result {
        TaskPreparationResult::Failed { error, step } => {
            assert!(error.contains("Git state analysis failed"));
            assert_eq!(step, PreparationStep::GitStateAnalysis);
        }
        other => panic!("Expected Failed result for nonexistent path, got: {:?}", other),
    }
}

/// Test PreparationStep display formatting
#[test]
fn test_preparation_step_display_values() {
    assert_eq!(format!("{}", PreparationStep::MetaExtraction), "meta extraction");
    assert_eq!(format!("{}", PreparationStep::GitStateAnalysis), "git state analysis");
    assert_eq!(format!("{}", PreparationStep::UncommittedHandling), "uncommitted handling");
    assert_eq!(format!("{}", PreparationStep::BranchSetup), "branch setup");
    assert_eq!(format!("{}", PreparationStep::FinalVerification), "final verification");
}

/// Test PreAction summary generation
#[test]
fn test_pre_action_summary_content() {
    let commit_action = PreAction::HandleUncommitted {
        action: agent_decision::uncommitted_handler::UncommittedAction::Commit,
        commit_message: Some("wip: test message".to_string()),
        stash_description: None,
    };
    assert!(commit_action.summary().contains("Handle uncommitted"));
    assert!(commit_action.summary().contains("Commit"));

    let stash_action = PreAction::HandleUncommitted {
        action: agent_decision::uncommitted_handler::UncommittedAction::Stash,
        commit_message: None,
        stash_description: Some("WIP: test".to_string()),
    };
    assert!(stash_action.summary().contains("Handle uncommitted"));
    assert!(stash_action.summary().contains("Stash"));

    let create_branch = PreAction::CreateBranch {
        branch_name: "feature/test".to_string(),
        base_branch: "main".to_string(),
    };
    assert!(create_branch.summary().contains("Create branch"));
    assert!(create_branch.summary().contains("feature/test"));
    assert!(create_branch.summary().contains("main"));

    let rebase = PreAction::RebaseToMain {
        base_branch: "master".to_string(),
    };
    assert!(rebase.summary().contains("Rebase"));
    assert!(rebase.summary().contains("master"));
}

/// Test pipeline generates correct pre-actions for different result types
#[test]
fn test_pipeline_generate_pre_actions() {
    let pipeline = TaskPreparationPipeline::new();

    // Test Ready result - should generate no pre-actions
    let ready_result = TaskPreparationResult::Ready {
        task_meta: agent_decision::task_metadata::TaskMetadata::new("task-001", "Test task"),
        branch_ready: true,
        clean_state: true,
    };
    let actions = pipeline.generate_pre_actions(&ready_result);
    assert!(actions.is_empty());

    // Test NeedsHuman result - should generate no pre-actions
    let human_result = TaskPreparationResult::NeedsHuman {
        reason: "Test reason".to_string(),
    };
    let actions = pipeline.generate_pre_actions(&human_result);
    assert!(actions.is_empty());
}
