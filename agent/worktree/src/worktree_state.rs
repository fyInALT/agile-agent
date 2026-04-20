//! Worktree State for persistence and resume support
//!
//! Stores worktree information for each agent to enable
//! seamless resume after restart or crash.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persistent worktree state for agent resume
///
/// Stored in agent's state file alongside other agent metadata.
/// This enables resuming an agent in the same worktree after restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeState {
    /// Unique identifier for this worktree
    pub worktree_id: String,

    /// Absolute path to the worktree directory
    pub path: PathBuf,

    /// Branch name (may not exist if worktree was deleted)
    pub branch: Option<String>,

    /// Base commit SHA when worktree was created
    pub base_commit: String,

    /// Task ID this worktree is associated with
    pub task_id: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_active_at: DateTime<Utc>,

    /// Whether worktree should be preserved after task completion
    pub preserve_on_completion: bool,

    /// Commit SHAs made by this agent in this worktree
    pub commits: Vec<String>,

    /// Current HEAD commit SHA
    pub head_commit: Option<String>,

    /// Whether there are uncommitted changes
    pub has_uncommitted_changes: bool,

    /// Agent ID that owns this worktree
    pub agent_id: String,
}

impl WorktreeState {
    /// Create a new worktree state
    pub fn new(
        worktree_id: String,
        path: PathBuf,
        branch: Option<String>,
        base_commit: String,
        task_id: Option<String>,
        agent_id: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            worktree_id,
            path,
            branch,
            base_commit,
            task_id,
            created_at: now,
            last_active_at: now,
            preserve_on_completion: false,
            commits: Vec::new(),
            head_commit: None,
            has_uncommitted_changes: false,
            agent_id,
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_active_at = Utc::now();
    }

    /// Record a new commit made in this worktree
    pub fn record_commit(&mut self, commit_sha: String) {
        self.commits.push(commit_sha.clone());
        self.head_commit = Some(commit_sha);
        self.touch();
    }

    /// Update HEAD commit without adding to commits list
    pub fn update_head(&mut self, head_sha: String) {
        self.head_commit = Some(head_sha);
        self.touch();
    }

    /// Set uncommitted changes status
    pub fn set_uncommitted(&mut self, has_changes: bool) {
        self.has_uncommitted_changes = has_changes;
        self.touch();
    }

    /// Check if worktree directory still exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get relative path from repo root
    pub fn relative_path(&self, repo_root: &Path) -> Option<PathBuf> {
        pathdiff::diff_paths(&self.path, repo_root)
    }

    /// Get the worktree name (directory name)
    pub fn name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }

    /// Check if this worktree is older than given seconds
    pub fn is_idle_for(&self, seconds: u64) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_active_at);
        duration.num_seconds() as u64 > seconds
    }

    /// Get elapsed time since last activity
    pub fn elapsed_seconds(&self) -> i64 {
        let now = Utc::now();
        now.signed_duration_since(self.last_active_at).num_seconds()
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        let branch_str = self.branch.as_deref().unwrap_or("detached");
        let head_short = self
            .head_commit
            .as_ref()
            .map(|h| if h.len() >= 8 { &h[..8] } else { h.as_str() })
            .unwrap_or("unknown");
        let status = if self.has_uncommitted_changes {
            " (dirty)"
        } else {
            ""
        };
        format!(
            "{} [{}] {}{} - {} commits",
            self.path.display(),
            branch_str,
            head_short,
            status,
            self.commits.len()
        )
    }

    /// Check if the worktree needs recreation (path doesn't exist but has valid branch)
    pub fn needs_recreation(&self) -> bool {
        !self.exists() && self.branch.is_some()
    }

    /// Check if worktree is idle for longer than given duration
    pub fn is_idle_longer_than(&self, duration: chrono::Duration) -> bool {
        let now = Utc::now();
        now.signed_duration_since(self.last_active_at) > duration
    }

    /// Check if worktree has no meaningful content (no commits, no uncommitted changes)
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty() && !self.has_uncommitted_changes
    }
}

/// Result of a worktree resume operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeResult {
    /// Worktree existed and was verified
    ExistingWorktree,
    /// Worktree was recreated from stored state
    RecreatedWorktree,
    /// Worktree could not be recovered
    FailedRecovery { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn worktree_state_new() {
        let state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            Some("task-123".to_string()),
            "agent_001".to_string(),
        );

        assert_eq!(state.worktree_id, "wt-001");
        assert_eq!(state.branch, Some("feature/test".to_string()));
        assert!(state.commits.is_empty());
        assert!(!state.preserve_on_completion);
    }

    #[test]
    fn worktree_state_record_commit() {
        let mut state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );

        state.record_commit("def456".to_string());
        assert_eq!(state.commits.len(), 1);
        assert_eq!(state.head_commit, Some("def456".to_string()));
    }

    #[test]
    fn worktree_state_touch_updates_timestamp() {
        let mut state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );

        let initial_time = state.last_active_at;
        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.touch();

        assert!(state.last_active_at > initial_time);
    }

    #[test]
    fn worktree_state_is_idle_for() {
        let mut state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );

        // Fresh state should not be idle
        assert!(!state.is_idle_for(1));

        // Manually set old timestamp
        state.last_active_at = Utc::now() - chrono::Duration::seconds(100);
        assert!(state.is_idle_for(60));
    }

    #[test]
    fn worktree_state_needs_recreation() {
        let temp_dir = TempDir::new().unwrap();
        let existing_path = temp_dir.path().to_path_buf();

        // State with existing path
        let state_existing = WorktreeState::new(
            "wt-001".to_string(),
            existing_path.clone(),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );
        assert!(!state_existing.needs_recreation());

        // State with non-existing path
        let state_missing = WorktreeState::new(
            "wt-002".to_string(),
            PathBuf::from("/non/existing/path"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );
        assert!(state_missing.needs_recreation());

        // State without branch info
        let state_no_branch = WorktreeState::new(
            "wt-003".to_string(),
            PathBuf::from("/non/existing/path"),
            None,
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );
        assert!(!state_no_branch.needs_recreation());
    }

    #[test]
    fn worktree_state_serialization() {
        let state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            Some("task-123".to_string()),
            "agent_001".to_string(),
        );

        let json = serde_json::to_string(&state).unwrap();
        let decoded: WorktreeState = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.worktree_id, state.worktree_id);
        assert_eq!(decoded.branch, state.branch);
        assert_eq!(decoded.base_commit, state.base_commit);
    }

    #[test]
    fn worktree_state_summary() {
        let state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );

        let summary = state.summary();
        assert!(summary.contains("feature/test"));
        assert!(summary.contains("0 commits"));
    }

    #[test]
    fn worktree_state_summary_with_dirty() {
        let mut state = WorktreeState::new(
            "wt-001".to_string(),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            None,
            "agent_001".to_string(),
        );
        state.has_uncommitted_changes = true;

        let summary = state.summary();
        assert!(summary.contains("(dirty)"));
    }
}
