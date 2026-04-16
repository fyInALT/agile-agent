//! Worktree State Store for persistence
//!
//! Provides save/load operations for worktree states,
//! integrating with existing agent state file format.

use std::fs;
use std::path::PathBuf;

use serde_json::Value as JsonValue;

use crate::logging;
use crate::worktree_state::WorktreeState;

/// Store for persisting and loading worktree states
///
/// Worktree state is stored in the agent's state file:
/// `.state/agents/{agent_id}.json`
pub struct WorktreeStateStore {
    /// Base directory for state storage
    state_dir: PathBuf,
}

impl WorktreeStateStore {
    /// Create a new WorktreeStateStore
    pub fn new(state_dir: PathBuf) -> Self {
        Self { state_dir }
    }

    /// Get the agents directory path
    pub fn agents_dir(&self) -> PathBuf {
        self.state_dir.join("agents")
    }

    /// Get the agent state file path
    fn agent_state_path(&self, agent_id: &str) -> PathBuf {
        self.agents_dir().join(format!("{}.json", agent_id))
    }

    /// Ensure the agents directory exists
    fn ensure_dir_exists(&self) -> Result<(), WorktreeStateStoreError> {
        let agents_dir = self.agents_dir();
        if !agents_dir.exists() {
            fs::create_dir_all(&agents_dir)?;
            logging::debug_event(
                "worktree.store.dir.created",
                "Agents directory created",
                serde_json::json!({"path": agents_dir.display().to_string()}),
            );
        }
        Ok(())
    }

    /// Save worktree state for an agent
    ///
    /// The worktree state is embedded in the agent's state file.
    /// If the file doesn't exist, creates a minimal state file.
    pub fn save(&self, agent_id: &str, state: &WorktreeState) -> Result<(), WorktreeStateStoreError> {
        self.ensure_dir_exists()?;
        let path = self.agent_state_path(agent_id);

        logging::debug_event(
            "worktree.state.save",
            "Saving worktree state",
            serde_json::json!({
                "agent_id": agent_id,
                "path": path.display().to_string(),
                "worktree_id": state.worktree_id,
                "branch": state.branch
            }),
        );

        // Load existing agent state or create new
        let mut agent_state: JsonValue = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::json!({ "agent_id": agent_id })
        };

        // Update worktree field
        agent_state["worktree"] = serde_json::to_value(state)?;

        // Write back
        let content = serde_json::to_string_pretty(&agent_state)?;
        fs::write(&path, content)?;

        logging::debug_event(
            "worktree.state.saved",
            "Worktree state saved successfully",
            serde_json::json!({"agent_id": agent_id}),
        );

        Ok(())
    }

    /// Load worktree state for an agent
    ///
    /// Returns None if the agent has no worktree state.
    pub fn load(&self, agent_id: &str) -> Result<Option<WorktreeState>, WorktreeStateStoreError> {
        let path = self.agent_state_path(agent_id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let agent_state: JsonValue = serde_json::from_str(&content)?;

        if let Some(worktree_value) = agent_state.get("worktree") {
            let state: WorktreeState = serde_json::from_value(worktree_value.clone())?;
            logging::debug_event(
                "worktree.state.loaded",
                "Worktree state loaded",
                serde_json::json!({
                    "agent_id": agent_id,
                    "worktree_id": state.worktree_id,
                    "branch": state.branch
                }),
            );
            Ok(Some(state))
        } else {
            logging::debug_event(
                "worktree.state.not_found",
                "No worktree state for agent",
                serde_json::json!({"agent_id": agent_id}),
            );
            Ok(None)
        }
    }

    /// Delete worktree state from an agent's state file
    ///
    /// This removes the worktree field but keeps other agent data.
    pub fn delete(&self, agent_id: &str) -> Result<(), WorktreeStateStoreError> {
        let path = self.agent_state_path(agent_id);

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut agent_state: JsonValue = serde_json::from_str(&content)?;

            if let Some(obj) = agent_state.as_object_mut() {
                obj.remove("worktree");

                let new_content = serde_json::to_string_pretty(&agent_state)?;
                fs::write(&path, new_content)?;

                logging::debug_event(
                    "worktree.state.deleted",
                    "Worktree state deleted",
                    serde_json::json!({"agent_id": agent_id}),
                );
            }
        }

        Ok(())
    }

    /// Update worktree state for an agent
    ///
    /// Convenience method that loads, allows modification, and saves.
    pub fn update(
        &self,
        agent_id: &str,
        updater: impl FnOnce(&mut WorktreeState),
    ) -> Result<(), WorktreeStateStoreError> {
        let state = self.load(agent_id)?
            .ok_or(WorktreeStateStoreError::NoWorktreeState(agent_id.to_string()))?;

        let mut updated_state = state;
        updater(&mut updated_state);

        self.save(agent_id, &updated_state)?;
        Ok(())
    }

    /// List all agents with worktree states
    ///
    /// Returns a list of (agent_id, worktree_state) pairs.
    /// Useful for crash recovery to find orphaned worktrees.
    pub fn list_all(&self) -> Result<Vec<(String, WorktreeState)>, WorktreeStateStoreError> {
        let agents_dir = self.agents_dir();

        if !agents_dir.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();

        for entry in fs::read_dir(&agents_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = fs::read_to_string(&path)?;
                let agent_state: JsonValue = serde_json::from_str(&content)?;

                if let Some(agent_id) = agent_state.get("agent_id").and_then(|v| v.as_str()) {
                    if let Some(worktree_value) = agent_state.get("worktree") {
                        let state: WorktreeState = serde_json::from_value(worktree_value.clone())?;
                        result.push((agent_id.to_string(), state));
                    }
                }
            }
        }

        logging::debug_event(
            "worktree.state.list_all",
            "Listed all worktree states",
            serde_json::json!({"count": result.len()}),
        );

        Ok(result)
    }

    /// Find orphaned worktree states (worktrees that no longer exist)
    ///
    /// Returns list of (agent_id, worktree_state) where the worktree
    /// path doesn't exist on disk.
    pub fn find_orphaned(&self) -> Result<Vec<(String, WorktreeState)>, WorktreeStateStoreError> {
        let all = self.list_all()?;
        let orphaned: Vec<(String, WorktreeState)> = all
            .into_iter()
            .filter(|(_, state)| !state.exists())
            .collect();

        logging::debug_event(
            "worktree.state.find_orphaned",
            "Found orphaned worktree states",
            serde_json::json!({"count": orphaned.len()}),
        );

        Ok(orphaned)
    }

    /// Check if an agent has a worktree state
    pub fn has_worktree(&self, agent_id: &str) -> Result<bool, WorktreeStateStoreError> {
        Ok(self.load(agent_id)?.is_some())
    }

    /// Get the count of agents with worktree states
    pub fn count(&self) -> Result<usize, WorktreeStateStoreError> {
        Ok(self.list_all()?.len())
    }
}

/// Errors for worktree state store operations
#[derive(Debug, thiserror::Error)]
pub enum WorktreeStateStoreError {
    #[error("no worktree state found for agent: {0}")]
    NoWorktreeState(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktree_state::WorktreeState;
    use tempfile::TempDir;

    fn create_test_store() -> (TempDir, WorktreeStateStore) {
        let temp_dir = TempDir::new().unwrap();
        let store = WorktreeStateStore::new(temp_dir.path().to_path_buf());
        (temp_dir, store)
    }

    fn create_test_state(agent_id: &str) -> WorktreeState {
        WorktreeState::new(
            format!("wt-{}", agent_id),
            PathBuf::from("/path/to/worktree"),
            Some("feature/test".to_string()),
            "abc123".to_string(),
            Some("task-123".to_string()),
            agent_id.to_string(),
        )
    }

    #[test]
    fn store_save_and_load() {
        let (_temp, store) = create_test_store();
        let state = create_test_state("agent_001");

        store.save("agent_001", &state).unwrap();
        let loaded = store.load("agent_001").unwrap();

        assert!(loaded.is_some());
        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.worktree_id, state.worktree_id);
        assert_eq!(loaded_state.branch, state.branch);
    }

    #[test]
    fn store_load_nonexistent_returns_none() {
        let (_temp, store) = create_test_store();
        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn store_delete() {
        let (_temp, store) = create_test_store();
        let state = create_test_state("agent_001");

        store.save("agent_001", &state).unwrap();
        assert!(store.load("agent_001").unwrap().is_some());

        store.delete("agent_001").unwrap();
        assert!(store.load("agent_001").unwrap().is_none());
    }

    #[test]
    fn store_update() {
        let (_temp, store) = create_test_store();
        let state = create_test_state("agent_001");

        store.save("agent_001", &state).unwrap();

        store.update("agent_001", |s| {
            s.record_commit("def456".to_string());
        }).unwrap();

        let loaded = store.load("agent_001").unwrap().unwrap();
        assert_eq!(loaded.commits.len(), 1);
        assert_eq!(loaded.head_commit, Some("def456".to_string()));
    }

    #[test]
    fn store_update_nonexistent_fails() {
        let (_temp, store) = create_test_store();
        let result = store.update("nonexistent", |_s| {});
        assert!(matches!(result, Err(WorktreeStateStoreError::NoWorktreeState(_))));
    }

    #[test]
    fn store_list_all() {
        let (_temp, store) = create_test_store();

        store.save("agent_001", &create_test_state("agent_001")).unwrap();
        store.save("agent_002", &create_test_state("agent_002")).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn store_count() {
        let (_temp, store) = create_test_store();

        assert_eq!(store.count().unwrap(), 0);

        store.save("agent_001", &create_test_state("agent_001")).unwrap();
        assert_eq!(store.count().unwrap(), 1);

        store.save("agent_002", &create_test_state("agent_002")).unwrap();
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn store_has_worktree() {
        let (_temp, store) = create_test_store();

        assert!(!store.has_worktree("agent_001").unwrap());

        store.save("agent_001", &create_test_state("agent_001")).unwrap();
        assert!(store.has_worktree("agent_001").unwrap());
    }

    #[test]
    fn store_preserves_other_agent_data() {
        let (_temp, store) = create_test_store();
        let state = create_test_state("agent_001");

        // Create agent state file with other data
        let agent_path = store.agent_state_path("agent_001");
        let initial_state = serde_json::json!({
            "agent_id": "agent_001",
            "codename": "alpha",
            "provider_type": "claude",
            "status": "running"
        });
        fs::write(&agent_path, serde_json::to_string_pretty(&initial_state).unwrap()).unwrap();

        // Save worktree state
        store.save("agent_001", &state).unwrap();

        // Load and check other data preserved
        let content = fs::read_to_string(&agent_path).unwrap();
        let loaded: JsonValue = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded["codename"], "alpha");
        assert_eq!(loaded["provider_type"], "claude");
        assert!(loaded.get("worktree").is_some());
    }
}
