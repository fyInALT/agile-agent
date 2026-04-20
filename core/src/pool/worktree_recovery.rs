//! Worktree recovery operations for AgentPool
//!
//! Provides methods for recovering orphaned worktrees and cleaning up
//! idle worktrees that have no active agents.

use std::path::PathBuf;

use crate::agent_slot::AgentSlot;
use crate::logging;
use crate::pool::WorktreeCoordinator;
use crate::WorktreeError;

use agent_worktree::{WorktreeCreateOptions, WorktreeState, WorktreeStateStoreError};
use chrono::Utc;

/// Report of worktree recovery operations
#[derive(Debug, Clone)]
pub struct WorktreeRecoveryReport {
    /// Successfully recovered worktrees (agent_id, worktree_id)
    pub recovered: Vec<(String, String)>,
    /// Cleaned up stale worktree states (agent_id, reason)
    pub cleaned_up: Vec<(String, String)>,
}

/// Errors for AgentPool worktree operations
#[derive(Debug, thiserror::Error)]
pub enum AgentPoolWorktreeError {
    #[error("worktree support not enabled for this pool")]
    WorktreeNotEnabled,

    #[error("agent pool is full")]
    PoolFull,

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("agent has no worktree: {0}")]
    NoWorktree(String),

    #[error("worktree directory not found on disk: {0}")]
    WorktreeNotFound(PathBuf),

    #[error("worktree state not found: {0}")]
    StateNotFound(String),

    #[error("agent is not paused: {0}")]
    AgentNotPaused(String),

    #[error("worktree error: {0}")]
    WorktreeError(#[from] WorktreeError),

    #[error("state store error: {0}")]
    StateStoreError(String),

    #[error("slot transition error: {0}")]
    SlotTransitionError(String),
}

impl From<WorktreeStateStoreError> for AgentPoolWorktreeError {
    fn from(err: WorktreeStateStoreError) -> Self {
        AgentPoolWorktreeError::StateStoreError(err.to_string())
    }
}

/// Worktree recovery operations coordinator
///
/// This zero-sized type provides recovery methods for worktree management.
pub struct WorktreeRecovery;

impl WorktreeRecovery {
    /// Recover orphaned worktrees
    ///
    /// Checks all persisted worktree states and:
    /// - For worktrees that still exist: preserves them for manual recovery
    /// - For missing worktrees with `recreate_missing=true`: recreates them
    /// - For missing worktrees with `recreate_missing=false`: cleans up state
    ///
    /// Agents that are already in the pool are skipped.
    pub fn recover_orphaned(
        slots: &[AgentSlot],
        worktree_coordinator: &WorktreeCoordinator,
        recreate_missing: bool,
    ) -> Result<WorktreeRecoveryReport, AgentPoolWorktreeError> {
        if !worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let worktree_state_store = worktree_coordinator.state_store().unwrap();
        let worktree_manager = worktree_coordinator.manager().unwrap();

        let all_states = worktree_state_store.list_all()?;

        let mut recovered = Vec::new();
        let mut cleaned_up = Vec::new();

        for (agent_id, state) in all_states {
            // Check if this agent is already in the pool
            if slots.iter().any(|s| s.agent_id().as_str() == agent_id) {
                continue;
            }

            if state.exists() {
                logging::debug_event(
                    "worktree.orphan_found",
                    "found orphaned worktree state",
                    serde_json::json!({
                        "agent_id": agent_id,
                        "worktree_id": state.worktree_id,
                        "path": state.path.to_string_lossy(),
                    }),
                );
            } else if recreate_missing {
                let options = WorktreeCreateOptions {
                    path: worktree_manager.worktrees_dir().join(&state.worktree_id),
                    branch: state.branch.clone(),
                    create_branch: state.branch.is_some(),
                    base: Some(state.base_commit.clone()),
                    lock_reason: None,
                };

                match worktree_manager.create(&state.worktree_id, options) {
                    Ok(_) => {
                        recovered.push((agent_id.clone(), state.worktree_id.clone()));
                        logging::debug_event(
                            "worktree.recovered",
                            "recovered missing worktree",
                            serde_json::json!({
                                "agent_id": agent_id,
                                "worktree_id": state.worktree_id,
                            }),
                        );
                    }
                    Err(e) => {
                        worktree_state_store.delete(&agent_id)?;
                        cleaned_up.push((agent_id.clone(), e.to_string()));
                        logging::debug_event(
                            "worktree.cleanup_failed_recreate",
                            "cleaned up worktree state after failed recreation",
                            serde_json::json!({
                                "agent_id": agent_id,
                                "error": e.to_string(),
                            }),
                        );
                    }
                }
            } else {
                worktree_state_store.delete(&agent_id)?;
                cleaned_up.push((
                    agent_id.clone(),
                    "worktree missing, state deleted".to_string(),
                ));
            }
        }

        Ok(WorktreeRecoveryReport {
            recovered,
            cleaned_up,
        })
    }

    /// Auto cleanup idle worktrees
    ///
    /// Checks for worktrees that have been idle for longer than the specified
    /// duration and have no commits/uncommitted changes. Cleans up both the
    /// worktree directory and the persisted state.
    ///
    /// Returns a list of cleaned up worktree IDs.
    pub fn auto_cleanup_idle(
        slots: &mut [AgentSlot],
        worktree_coordinator: &WorktreeCoordinator,
        idle_duration: chrono::Duration,
    ) -> Result<Vec<String>, AgentPoolWorktreeError> {
        if !worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let all_states = worktree_coordinator.state_store().unwrap().list_all()?;

        // First pass: collect worktrees to clean up
        let mut to_cleanup: Vec<(String, WorktreeState, bool)> = Vec::new();

        for (agent_id, state) in &all_states {
            let slot = slots.iter().find(|s| s.agent_id().as_str() == agent_id);
            let is_pool_idle = slot.map_or(false, |s| s.status().is_idle() || s.status().is_paused());

            // Skip if agent is active
            if slot.is_some() && !is_pool_idle {
                continue;
            }

            if state.is_idle_longer_than(idle_duration) && state.is_empty() {
                to_cleanup.push((agent_id.clone(), state.clone(), slot.is_some()));
            }
        }

        let mut cleaned_up = Vec::new();

        // Second pass: do the cleanup
        for (agent_id, state, in_pool) in to_cleanup {
            if state.exists() {
                if let Some(wm) = worktree_coordinator.manager() {
                    wm.remove(&state.worktree_id, true)?;
                }
            }

            if let Some(store) = worktree_coordinator.state_store() {
                store.delete(&agent_id)?;
            }

            if in_pool {
                if let Some(slot) = slots.iter_mut().find(|s| s.agent_id().as_str() == agent_id) {
                    slot.clear_worktree();
                }
            }

            cleaned_up.push(state.worktree_id.clone());

            logging::debug_event(
                "worktree.auto_cleanup",
                "cleaned up idle worktree",
                serde_json::json!({
                    "agent_id": &agent_id,
                    "worktree_id": &state.worktree_id,
                    "idle_seconds": (Utc::now() - state.last_active_at).num_seconds(),
                }),
            );
        }

        Ok(cleaned_up)
    }
}