//! Worktree coordinator for managing worktree state
//!
//! Provides WorktreeCoordinator struct that manages worktree manager,
//! state store, and git flow executor. Used as a delegate within AgentPool.

use std::path::PathBuf;

use crate::{
    WorktreeConfig, WorktreeCreateOptions, WorktreeError, WorktreeManager,
    WorktreeState, WorktreeStateStore, WorktreeStateStoreError,
    GitFlowExecutor, GitFlowConfig,
};

/// Coordinator for worktree management
///
/// Manages worktree manager, state store, and git flow executor.
/// Used as a delegate within AgentPool for worktree state operations.
pub struct WorkerWorktreeManager {
    /// Worktree manager for git worktree operations
    manager: Option<WorktreeManager>,
    /// State store for persistence
    state_store: Option<WorktreeStateStore>,
    /// Git flow executor for task preparation
    git_flow_executor: Option<GitFlowExecutor>,
}

impl std::fmt::Debug for WorkerWorktreeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorktreeCoordinator")
            .field("has_manager", &self.manager.is_some())
            .field("has_state_store", &self.state_store.is_some())
            .field("has_git_flow_executor", &self.git_flow_executor.is_some())
            .finish()
    }
}

impl WorkerWorktreeManager {
    /// Create a new coordinator without worktree support
    pub fn new() -> Self {
        Self {
            manager: None,
            state_store: None,
            git_flow_executor: None,
        }
    }

    /// Create a coordinator with worktree support
    pub fn with_worktrees(
        repo_root: PathBuf,
        state_dir: PathBuf,
    ) -> Result<Self, WorktreeError> {
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_root.clone(), config)?;
        let state_store = WorktreeStateStore::new(state_dir);

        let git_flow_config = GitFlowConfig::default();
        let git_flow_executor = GitFlowExecutor::new(manager.clone(), git_flow_config);

        Ok(Self {
            manager: Some(manager),
            state_store: Some(state_store),
            git_flow_executor: Some(git_flow_executor),
        })
    }

    /// Check if worktree support is enabled
    pub fn is_enabled(&self) -> bool {
        self.manager.is_some() && self.state_store.is_some()
    }

    /// Get worktree manager reference
    pub fn manager(&self) -> Option<&WorktreeManager> {
        self.manager.as_ref()
    }

    /// Get worktree manager mutable reference
    pub fn manager_mut(&mut self) -> Option<&mut WorktreeManager> {
        self.manager.as_mut()
    }

    /// Get state store reference
    pub fn state_store(&self) -> Option<&WorktreeStateStore> {
        self.state_store.as_ref()
    }

    /// Get state store mutable reference
    pub fn state_store_mut(&mut self) -> Option<&mut WorktreeStateStore> {
        self.state_store.as_mut()
    }

    /// Get git flow executor reference
    pub fn git_flow_executor(&self) -> Option<&GitFlowExecutor> {
        self.git_flow_executor.as_ref()
    }

    /// Get git flow executor mutable reference
    pub fn git_flow_executor_mut(&mut self) -> Option<&mut GitFlowExecutor> {
        self.git_flow_executor.as_mut()
    }

    /// Get worktrees directory path
    pub fn worktrees_dir(&self) -> Option<PathBuf> {
        self.manager.as_ref().map(|m| m.worktrees_dir().to_path_buf())
    }

    /// List all stored worktree states
    pub fn list_all_states(&self) -> Vec<(String, WorktreeState)> {
        self.state_store
            .as_ref()
            .map(|s| s.list_all().unwrap_or_default())
            .unwrap_or_default()
    }

    /// List all agent branches
    pub fn list_agent_branches(&self) -> Vec<String> {
        self.manager
            .as_ref()
            .map(|m| m.list_agent_branches().unwrap_or_default())
            .unwrap_or_default()
    }

    /// Save worktree state
    pub fn save_state(&self, agent_id: &str, state: &WorktreeState) -> Result<(), WorktreeStateStoreError> {
        if let Some(store) = &self.state_store {
            store.save(agent_id, state)?;
        }
        Ok(())
    }

    /// Load worktree state
    pub fn load_state(&self, agent_id: &str) -> Option<WorktreeState> {
        self.state_store.as_ref().and_then(|s| s.load(agent_id).ok().flatten())
    }

    /// Delete worktree state
    pub fn delete_state(&self, agent_id: &str) -> Result<(), WorktreeStateStoreError> {
        if let Some(store) = &self.state_store {
            store.delete(agent_id)?;
        }
        Ok(())
    }

    /// Create a worktree
    pub fn create_worktree(
        &self,
        worktree_id: &str,
        options: WorktreeCreateOptions,
    ) -> Option<Result<crate::WorktreeInfo, WorktreeError>> {
        self.manager.as_ref().map(|m| m.create(worktree_id, options))
    }

    /// Remove a worktree
    pub fn remove_worktree(&self, worktree_id: &str, force: bool) -> Option<Result<bool, WorktreeError>> {
        self.manager.as_ref().map(|m| m.remove(worktree_id, force).map(|_| true))
    }

    /// Check if branch exists
    pub fn branch_exists(&self, branch: &str) -> bool {
        self.manager
            .as_ref()
            .map(|m| m.branch_exists(branch).unwrap_or(false))
            .unwrap_or(false)
    }

    /// Get current head commit
    pub fn get_current_head(&self) -> Option<String> {
        self.manager
            .as_ref()
            .and_then(|m| m.get_current_head().ok())
    }

    /// Check if worktree has changes
    #[allow(clippy::ptr_arg)]
    pub fn has_changes(&self, worktree_path: &PathBuf) -> bool {
        self.manager
            .as_ref()
            .map(|m| m.has_uncommitted_changes(worktree_path).unwrap_or(false))
            .unwrap_or(false)
    }

    /// Get head commit for worktree
    #[allow(clippy::ptr_arg)]
    pub fn get_head_commit(&self, worktree_path: &PathBuf) -> Option<String> {
        self.manager
            .as_ref()
            .and_then(|m| m.get_head_commit(worktree_path))
    }

    /// Find max agent index from existing states and branches
    pub fn find_max_agent_index(&self) -> Option<usize> {
        // Check existing states
        let max_state_index = self.list_all_states()
            .iter()
            .filter_map(|(agent_id, _)| parse_agent_index(agent_id))
            .max();

        // Check existing branches
        let max_branch_index = self.list_agent_branches()
            .iter()
            .filter_map(|branch| branch.strip_prefix("agent/").and_then(parse_agent_index))
            .max();

        max_state_index.max(max_branch_index)
    }
}

impl Default for WorkerWorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse agent index from agent_id string (e.g., "agent_001" -> 1)
fn parse_agent_index(agent_id: &str) -> Option<usize> {
    agent_id.strip_prefix("agent_")?.parse::<usize>().ok()
}

pub type WorktreeCoordinator = WorkerWorktreeManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinator_new_has_no_worktree_support() {
        let coord = WorktreeCoordinator::new();
        assert!(!coord.is_enabled());
        assert!(coord.manager().is_none());
        assert!(coord.state_store().is_none());
    }

    #[test]
    fn parse_agent_index_parses_correctly() {
        assert_eq!(parse_agent_index("agent_001"), Some(1));
        assert_eq!(parse_agent_index("agent_007"), Some(7));
        assert_eq!(parse_agent_index("agent_100"), Some(100));
        assert_eq!(parse_agent_index("agent_"), None);
        assert_eq!(parse_agent_index("invalid"), None);
    }

    #[test]
    fn list_all_states_empty_when_no_store() {
        let coord = WorktreeCoordinator::new();
        let states = coord.list_all_states();
        assert!(states.is_empty());
    }

    #[test]
    fn list_agent_branches_empty_when_no_manager() {
        let coord = WorktreeCoordinator::new();
        let branches = coord.list_agent_branches();
        assert!(branches.is_empty());
    }
}

