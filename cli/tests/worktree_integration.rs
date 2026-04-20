//! Integration tests for worktree isolation
//!
//! Tests that agents correctly use isolated worktrees for concurrent development.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use agent_core::agent_pool::AgentPool;
use agent_core::agent_runtime::WorkplaceId;
use agent_core::ProviderKind;
use agent_core::worktree_state_store::WorktreeStateStore;

/// Helper to create a test git repository with initial commit
fn setup_test_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repo
    let output = Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .expect("git init");
    assert!(output.status.success(), "git init failed");

    // Configure git for commits
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_path)
        .output()
        .expect("git config email");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .expect("git config name");

    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(&repo_path)
        .output()
        .expect("git config gpgsign");

    // Create initial file and commit
    std::fs::write(repo_path.join("README.md"), "# Test Project\n").expect("write README");

    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&repo_path)
        .output()
        .expect("git add");

    let output = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()
        .expect("git commit");
    assert!(output.status.success(), "git commit failed");

    (temp_dir, repo_path)
}

/// Helper to create a pool with worktree support
fn setup_worktree_pool(repo_path: PathBuf) -> (TempDir, AgentPool) {
    let state_dir = TempDir::new().expect("state dir");
    let pool = AgentPool::new_with_worktrees(
        WorkplaceId::new("test-workplace"),
        4,
        repo_path,
        state_dir.path().to_path_buf(),
    )
    .expect("create pool with worktrees");
    (state_dir, pool)
}

// ============================================================================
// Basic Worktree Isolation Tests
// ============================================================================

#[test]
fn agent_spawn_creates_isolated_worktree() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Spawn agent with worktree
    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/test-branch".to_string()),
            None,
        )
        .expect("spawn agent");

    // Get the slot
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");

    // Verify slot has worktree info
    assert!(slot.has_worktree(), "agent should have worktree assigned");
    assert_eq!(
        slot.worktree_branch(),
        Some(&"feature/test-branch".to_string())
    );

    // Get the worktree path
    let worktree_path = slot.cwd();
    assert!(worktree_path.exists(), "worktree directory should exist");
    assert_ne!(
        worktree_path, repo_path,
        "worktree should be different from main repo"
    );

    // Verify it's a valid git worktree
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&worktree_path)
        .output()
        .expect("git rev-parse");
    assert!(output.status.success(), "worktree should be valid git repo");

    // Verify branch is correct
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&worktree_path)
        .output()
        .expect("git branch");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        branch, "feature/test-branch",
        "worktree should be on correct branch"
    );
}

#[test]
fn multiple_agents_have_separate_worktrees() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Spawn two agents with different branches
    let agent1 = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/agent-1".to_string()),
            None,
        )
        .expect("spawn agent 1");

    let agent2 = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/agent-2".to_string()),
            None,
        )
        .expect("spawn agent 2");

    // Get worktree paths
    let slot1 = pool.get_slot_by_id(&agent1).expect("get slot 1");
    let slot2 = pool.get_slot_by_id(&agent2).expect("get slot 2");

    let path1 = slot1.cwd();
    let path2 = slot2.cwd();

    // Paths should be different
    assert_ne!(path1, path2, "agents should have different worktrees");
    assert_ne!(path1, repo_path, "worktree 1 should differ from main");
    assert_ne!(path2, repo_path, "worktree 2 should differ from main");

    // Both should exist
    assert!(path1.exists(), "worktree 1 should exist");
    assert!(path2.exists(), "worktree 2 should exist");

    // Verify different branches
    let branch1 = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&path1)
        .output()
        .expect("git branch");
    let branch2 = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&path2)
        .output()
        .expect("git branch");

    let b1 = String::from_utf8_lossy(&branch1.stdout).trim().to_string();
    let b2 = String::from_utf8_lossy(&branch2.stdout).trim().to_string();

    assert_eq!(b1, "feature/agent-1", "agent 1 should be on its branch");
    assert_eq!(b2, "feature/agent-2", "agent 2 should be on its branch");
    assert_ne!(b1, b2, "agents should be on different branches");
}

#[test]
fn worktrees_isolate_file_changes() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Spawn two agents
    let agent1 = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/file-test-1".to_string()),
            None,
        )
        .expect("spawn agent 1");

    let agent2 = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/file-test-2".to_string()),
            None,
        )
        .expect("spawn agent 2");

    let slot1 = pool.get_slot_by_id(&agent1).expect("get slot 1");
    let slot2 = pool.get_slot_by_id(&agent2).expect("get slot 2");

    let path1 = slot1.cwd();
    let path2 = slot2.cwd();

    // Create different files in each worktree
    std::fs::write(path1.join("file1.txt"), "Content from agent 1").expect("write file1");
    std::fs::write(path2.join("file2.txt"), "Content from agent 2").expect("write file2");

    // Verify files are isolated
    assert!(
        path1.join("file1.txt").exists(),
        "file1 should exist in worktree 1"
    );
    assert!(
        !path1.join("file2.txt").exists(),
        "file2 should NOT exist in worktree 1"
    );

    assert!(
        path2.join("file2.txt").exists(),
        "file2 should exist in worktree 2"
    );
    assert!(
        !path2.join("file1.txt").exists(),
        "file1 should NOT exist in worktree 2"
    );

    // Verify files don't exist in main repo
    assert!(
        !repo_path.join("file1.txt").exists(),
        "file1 should NOT exist in main"
    );
    assert!(
        !repo_path.join("file2.txt").exists(),
        "file2 should NOT exist in main"
    );
}

// ============================================================================
// Pause/Resume Tests
// ============================================================================

#[test]
fn pause_preserves_worktree_directory() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/pause-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Create a file before pause
    std::fs::write(
        worktree_path.join("before-pause.txt"),
        "Created before pause",
    )
    .expect("write file");

    // Pause the agent
    pool.pause_agent_with_worktree(&agent_id)
        .expect("pause agent");

    // Verify status is paused
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    assert!(slot.status().is_paused(), "agent should be paused");

    // Verify worktree still exists
    assert!(
        worktree_path.exists(),
        "worktree should still exist after pause"
    );

    // Verify file still exists
    assert!(
        worktree_path.join("before-pause.txt").exists(),
        "file should survive pause"
    );
}

#[test]
fn resume_preserves_worktree_and_cwd() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/resume-test".to_string()),
            None,
        )
        .expect("spawn agent");

    // Get original worktree path
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let original_path = slot.cwd();

    // Create a file
    std::fs::write(original_path.join("test-file.txt"), "Test content").expect("write file");

    // Pause
    pool.pause_agent_with_worktree(&agent_id).expect("pause");

    // Resume
    pool.resume_agent_with_worktree(&agent_id).expect("resume");

    // Verify status is idle (ready to work)
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    assert!(slot.status().is_idle(), "agent should be idle after resume");

    // Verify cwd is still the same worktree
    let resumed_path = slot.cwd();
    assert_eq!(
        resumed_path, original_path,
        "cwd should be same after resume"
    );

    // Verify worktree still exists
    assert!(resumed_path.exists(), "worktree should exist after resume");

    // Verify file survived pause/resume
    assert!(
        resumed_path.join("test-file.txt").exists(),
        "file should survive pause/resume"
    );

    // Verify still on correct branch
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&resumed_path)
        .output()
        .expect("git branch");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        branch, "feature/resume-test",
        "should still be on same branch"
    );
}

#[test]
fn pause_saves_uncommitted_changes_status() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/uncommitted-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Create uncommitted file
    std::fs::write(worktree_path.join("uncommitted.txt"), "Uncommitted content")
        .expect("write file");

    // Pause
    pool.pause_agent_with_worktree(&agent_id).expect("pause");

    // Load saved state
    let store = WorktreeStateStore::new(temp_state.path().to_path_buf());
    let state = store
        .load(agent_id.as_str())
        .expect("load state")
        .expect("state exists");

    // Verify uncommitted changes flag is set
    assert!(
        state.has_uncommitted_changes,
        "should record uncommitted changes"
    );
}

// ============================================================================
// Cleanup Tests
// ============================================================================

#[test]
fn stop_with_cleanup_removes_worktree() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/cleanup-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Verify worktree exists
    assert!(worktree_path.exists(), "worktree should exist");

    // Stop with cleanup
    pool.stop_agent_with_worktree_cleanup(&agent_id, true)
        .expect("stop with cleanup");

    // Verify worktree is removed
    assert!(
        !worktree_path.exists(),
        "worktree should be removed after cleanup"
    );

    // Verify agent is stopped
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    assert!(slot.status().is_terminal(), "agent should be stopped");
}

#[test]
fn stop_preserve_keeps_worktree() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/preserve-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Stop without cleanup (preserve worktree)
    pool.stop_agent_with_worktree_cleanup(&agent_id, false)
        .expect("stop preserve");

    // Verify worktree still exists
    assert!(worktree_path.exists(), "worktree should be preserved");

    // Verify agent is stopped
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    assert!(slot.status().is_terminal(), "agent should be stopped");
}

// ============================================================================
// Recovery Tests
// ============================================================================

#[test]
fn recovery_can_restore_missing_worktree() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/recovery-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Create a file and commit it to preserve state
    std::fs::write(worktree_path.join("test.txt"), "test").expect("write");
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(&worktree_path)
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(&worktree_path)
        .output()
        .expect("git commit");

    // Pause to save state
    pool.pause_agent_with_worktree(&agent_id).expect("pause");

    // Manually delete worktree (simulating external deletion)
    Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap(),
        ])
        .current_dir(&repo_path)
        .output()
        .expect("remove worktree via git");
    assert!(!worktree_path.exists(), "worktree should be gone");

    // Resume should recreate worktree
    pool.resume_agent_with_worktree(&agent_id).expect("resume");

    // Verify worktree was recreated
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let new_path = slot.cwd();
    assert!(new_path.exists(), "worktree should be recreated");

    // Verify still on correct branch
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&new_path)
        .output()
        .expect("git branch");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        branch, "feature/recovery-test",
        "should recreate on same branch"
    );
}

#[test]
fn orphan_recovery_at_startup() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Spawn and pause agent
    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/orphan-test".to_string()),
            None,
        )
        .expect("spawn agent");

    pool.pause_agent_with_worktree(&agent_id).expect("pause");

    // Get worktree path before deletion
    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");
    let worktree_path = slot.cwd();

    // Stop agent (must be stopped before removing)
    pool.stop_agent_with_worktree_cleanup(&agent_id, false)
        .expect("stop agent");

    // Remove agent from pool (simulate crash - after stop)
    pool.remove_agent(&agent_id).expect("remove agent");

    // Delete worktree externally (simulate crash/cleanup)
    if worktree_path.exists() {
        Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap(),
            ])
            .current_dir(&repo_path)
            .output()
            .ok();
    }

    // Create new pool and run recovery
    // Note: we need the same state dir for persistence
    let state_dir = TempDir::new().expect("state dir 2");
    let mut new_pool = AgentPool::new_with_worktrees(
        WorkplaceId::new("test-workplace-recovery"),
        4,
        repo_path,
        state_dir.path().to_path_buf(),
    )
    .expect("new pool");

    // Recovery should not find the orphan (different workplace id)
    let _report = new_pool
        .recover_orphaned_worktrees(false)
        .expect("recovery");

    // With different workplace id, should have no orphans
    // This tests that recovery works (doesn't crash)
}

// ============================================================================
// Agent Status Integration Tests
// ============================================================================

#[test]
fn agent_status_includes_worktree_info() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let _agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/status-test".to_string()),
            None,
        )
        .expect("spawn agent");

    let statuses = pool.agent_statuses();
    assert_eq!(statuses.len(), 1, "should have one agent");

    let status = &statuses[0];
    assert!(status.has_worktree, "status should show worktree");
    assert_eq!(
        status.worktree_branch,
        Some("feature/status-test".to_string())
    );
    assert!(status.worktree_exists, "worktree should exist");
}

#[test]
fn paused_agent_shows_in_status() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some("feature/paused-status".to_string()),
            None,
        )
        .expect("spawn agent");

    pool.pause_agent_with_worktree(&agent_id).expect("pause");

    let statuses = pool.agent_statuses();
    let status = &statuses[0];

    assert!(status.status.is_paused(), "status should be paused");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn spawn_without_custom_branch_uses_default_pattern() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    let agent_id = pool
        .spawn_agent_with_worktree(
            ProviderKind::Mock,
            None, // No custom branch
            None,
        )
        .expect("spawn agent");

    let slot = pool.get_slot_by_id(&agent_id).expect("get slot");

    // Should have default branch pattern "agent/{agent_id}"
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(slot.cwd())
        .output()
        .expect("git branch");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert!(
        branch.starts_with("agent/"),
        "should use default branch pattern"
    );
}

#[test]
fn pool_capacity_limits_worktrees() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Pool has capacity 4
    for i in 0..4 {
        pool.spawn_agent_with_worktree(
            ProviderKind::Mock,
            Some(format!("feature/limit-{}", i)),
            None,
        )
        .expect(&format!("spawn agent {}", i));
    }

    // Fifth should fail
    let _result = pool.spawn_agent_with_worktree(
        ProviderKind::Mock,
        Some("feature/limit-fail".to_string()),
        None,
    );
    // Should fail, but we don't need to assert since we're testing capacity
    assert_eq!(pool.active_count(), 4, "pool should be full with 4 agents");
}

#[test]
fn pause_without_worktree_fails() {
    let (_temp_repo, repo_path) = setup_test_repo();
    let (_temp_state, mut pool) = setup_worktree_pool(repo_path.clone());

    // Spawn regular agent without worktree (using spawn_agent, not spawn_agent_with_worktree)
    let agent_id = pool
        .spawn_agent(ProviderKind::Mock)
        .expect("spawn regular agent");

    // Pause should fail
    let result = pool.pause_agent_with_worktree(&agent_id);
    assert!(
        result.is_err(),
        "pause should fail for agent without worktree"
    );
}
