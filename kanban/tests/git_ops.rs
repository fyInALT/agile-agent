//! Integration tests for GitOperations

use agent_kanban::git_ops::{GitError, GitOperations};

#[test]
fn test_git_operations_integration() {
    let ops = GitOperations::new("/tmp/test-repo");
    assert_eq!(ops.repo_path(), &std::path::PathBuf::from("/tmp/test-repo"));
}

#[test]
fn test_commit_changes_integration() {
    let ops = GitOperations::new("/tmp/test-repo");
    let result = ops.commit_changes("agent-1", "Add kanban elements");
    assert!(result.is_ok());
}

#[test]
fn test_fetch_and_rebase_integration() {
    let ops = GitOperations::new("/tmp/test-repo");
    let result = ops.fetch_and_rebase("main");
    assert!(result.is_ok());
}

#[test]
fn test_has_conflicts_integration() {
    let ops = GitOperations::new("/tmp/test-repo");
    assert!(!ops.has_conflicts());
}

#[test]
fn test_git_error_integration() {
    let err = GitError::new("connection refused");
    assert_eq!(format!("{}", err), "git error: connection refused");
}
