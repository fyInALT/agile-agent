//! Agent lifecycle manager for spawn, pause, resume, stop operations
//!
//! Provides AgentLifecycleManager that coordinates agent lifecycle operations.
//! This module extracts lifecycle methods from AgentPool to improve execution flow clarity.

use std::path::PathBuf;

use crate::agent_runtime::{AgentCodename, AgentId, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus};
use crate::decision_agent_slot::DecisionAgentSlot;
use crate::decision_mail::DecisionMail;
use crate::logging;
use crate::provider_profile::{ProfileId, get_effective_profile, AgentType as ProfileAgentType};
use crate::ProviderKind;
use crate::pool::{WorkerDecisionRouter, WorktreeCoordinator};
use crate::{WorktreeCreateOptions, WorktreeError, WorktreeState};

/// Error type for agent pool worktree operations
#[derive(Debug)]
pub enum LifecycleError {
    /// Pool is at capacity
    PoolFull,
    /// Agent not found in pool
    AgentNotFound(String),
    /// Worktree support not enabled
    WorktreeNotEnabled,
    /// No worktree for agent
    NoWorktree(String),
    /// Worktree not found on disk
    WorktreeNotFound(PathBuf),
    /// Worktree state not found in store
    StateNotFound(String),
    /// Agent not in paused state
    AgentNotPaused(String),
    /// Slot transition failed
    SlotTransitionError(String),
    /// Worktree operation failed
    WorktreeError(String),
    /// State store operation failed
    StateStoreError(String),
    /// Profile store not loaded
    NoProfileStore,
    /// Profile error
    ProfileError(String),
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleError::PoolFull => write!(f, "Agent pool is full"),
            LifecycleError::AgentNotFound(id) => write!(f, "Agent not found: {}", id),
            LifecycleError::WorktreeNotEnabled => write!(f, "Worktree support not enabled"),
            LifecycleError::NoWorktree(id) => write!(f, "Agent {} has no worktree", id),
            LifecycleError::WorktreeNotFound(path) => write!(f, "Worktree not found: {}", path.display()),
            LifecycleError::StateNotFound(id) => write!(f, "Worktree state not found for agent {}", id),
            LifecycleError::AgentNotPaused(id) => write!(f, "Agent {} is not paused", id),
            LifecycleError::SlotTransitionError(msg) => write!(f, "Slot transition failed: {}", msg),
            LifecycleError::WorktreeError(msg) => write!(f, "Worktree error: {}", msg),
            LifecycleError::StateStoreError(msg) => write!(f, "State store error: {}", msg),
            LifecycleError::NoProfileStore => write!(f, "Profile store not loaded"),
            LifecycleError::ProfileError(msg) => write!(f, "Profile error: {}", msg),
        }
    }
}

impl std::error::Error for LifecycleError {}

impl From<WorktreeError> for LifecycleError {
    fn from(e: WorktreeError) -> Self {
        LifecycleError::WorktreeError(e.to_string())
    }
}

/// Agent lifecycle manager - coordinates lifecycle operations
///
/// This struct provides lifecycle methods that operate on pool state.
/// It uses the coordinator pattern to delegate to BlockedHandler,
/// WorkerDecisionRouter, and WorktreeCoordinator.
pub struct AgentLifecycleManager;

impl AgentLifecycleManager {
    /// Spawn a new agent without worktree
    ///
    /// Creates a simple agent slot without isolated workspace.
    pub fn spawn_simple(
        slots: &mut Vec<AgentSlot>,
        max_slots: usize,
        next_agent_index: &mut usize,
        focused_slot: &mut usize,
        workplace_id: &WorkplaceId,
        decision_coordinator: &mut WorkerDecisionRouter,
        cwd: &PathBuf,
        provider_kind: ProviderKind,
    ) -> Result<AgentId, String> {
        if slots.len() >= max_slots {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn agent - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "pool_size": slots.len(),
                    "max_slots": max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }

        let agent_id = Self::generate_agent_id(workplace_id, next_agent_index);
        let codename = Self::generate_codename(*next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn",
            "spawned new agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "provider_type": provider_type.label(),
                "pool_size": slots.len(),
                "max_slots": max_slots,
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        if slots.len() == 1 {
            *focused_slot = 0;
            logging::debug_event(
                "pool.focus.change",
                "focus set to first agent after spawn",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "index": 0,
                }),
            );
        }

        // Spawn decision agent for this work agent (if provider supports it)
        if provider_kind != ProviderKind::Mock {
            Self::spawn_decision_agent(slots, decision_coordinator, cwd, &agent_id)?;
        }

        Ok(agent_id)
    }

    /// Spawn a new agent with profile
    ///
    /// Uses the profile to determine provider type.
    pub fn spawn_with_profile(
        slots: &mut Vec<AgentSlot>,
        max_slots: usize,
        next_agent_index: &mut usize,
        focused_slot: &mut usize,
        workplace_id: &WorkplaceId,
        decision_coordinator: &mut WorkerDecisionRouter,
        cwd: &PathBuf,
        profile_store: &crate::provider_profile::ProfileStore,
        profile_id: &ProfileId,
    ) -> Result<AgentId, crate::provider_profile::ProfileError> {
        let profile = get_effective_profile(profile_store, Some(profile_id), ProfileAgentType::Work)?;

        let provider_kind = profile.base_cli.to_provider_kind()
            .ok_or_else(|| crate::provider_profile::ProfileError::UnsupportedCliType(
                profile.base_cli.label().to_string()
            ))?;

        if slots.len() >= max_slots {
            logging::debug_event(
                "pool.agent.spawn_profile.failed",
                "failed to spawn agent with profile - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "profile_id": profile_id,
                }),
            );
            return Err(crate::provider_profile::ProfileError::PersistenceError(
                "Agent pool is full".to_string()
            ));
        }

        let agent_id = Self::generate_agent_id(workplace_id, next_agent_index);
        let codename = Self::generate_codename(*next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let mut slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slot.set_profile_id(profile_id.clone());

        slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn_profile",
            "spawned new agent with profile",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "profile_id": profile_id,
                "provider_type": provider_type.label(),
                "pool_size": slots.len(),
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        if slots.len() == 1 {
            *focused_slot = 0;
        }

        // Spawn decision agent for this work agent
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = Self::spawn_decision_agent(slots, decision_coordinator, cwd, &agent_id) {
                logging::warn_event(
                    "pool.agent.decision_agent_failed",
                    "failed to spawn decision agent for work agent",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": e,
                    }),
                );
            }
        }

        Ok(agent_id)
    }

    /// Spawn a new agent with worktree
    ///
    /// Creates an isolated git worktree workspace for the agent.
    pub fn spawn_with_worktree(
        slots: &mut Vec<AgentSlot>,
        max_slots: usize,
        next_agent_index: &mut usize,
        focused_slot: &mut usize,
        workplace_id: &WorkplaceId,
        decision_coordinator: &mut WorkerDecisionRouter,
        worktree_coordinator: &WorktreeCoordinator,
        cwd: &PathBuf,
        provider_kind: ProviderKind,
        branch_name: Option<String>,
        task_id: Option<String>,
        profile_id: Option<ProfileId>,
    ) -> Result<AgentId, LifecycleError> {
        // Check worktree coordinator is enabled
        if !worktree_coordinator.is_enabled() {
            return Err(LifecycleError::WorktreeNotEnabled);
        }

        // Check capacity
        if slots.len() >= max_slots {
            return Err(LifecycleError::PoolFull);
        }

        let agent_id = Self::generate_agent_id(workplace_id, next_agent_index);
        let codename = Self::generate_codename(*next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);
        let worktree_id = format!("wt-{}", agent_id.as_str());
        let actual_branch = branch_name.unwrap_or_else(|| format!("agent/{}", agent_id.as_str()));

        // Get worktree manager and state store
        let worktree_manager = worktree_coordinator.manager().unwrap();
        let worktree_state_store = worktree_coordinator.state_store().unwrap();

        // Create worktree
        let options = WorktreeCreateOptions {
            path: worktree_manager.worktrees_dir().join(&worktree_id),
            branch: Some(actual_branch.clone()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        let worktree_info = worktree_manager
            .create(&worktree_id, options)
            .map_err(|e| LifecycleError::WorktreeError(e.to_string()))?;

        // Get base commit SHA
        let base_commit = worktree_manager
            .get_current_head()
            .map_err(|e| LifecycleError::WorktreeError(e.to_string()))?;

        // Create worktree state
        let worktree_state = WorktreeState::new(
            worktree_id.clone(),
            worktree_info.path.clone(),
            Some(actual_branch.clone()),
            base_commit,
            task_id,
            agent_id.as_str().to_string(),
        );

        // Save worktree state
        worktree_state_store
            .save(agent_id.as_str(), &worktree_state)
            .map_err(|e| LifecycleError::StateStoreError(e.to_string()))?;

        // Create agent slot with worktree
        let mut slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slot.set_worktree(
            worktree_info.path.clone(),
            Some(actual_branch.clone()),
            worktree_id.clone(),
        );
        if let Some(ref pid) = profile_id {
            slot.set_profile_id(pid.clone());
        }

        slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn_with_worktree",
            "spawned new agent with worktree",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "worktree_id": worktree_id,
                "branch": actual_branch,
                "path": worktree_info.path.to_string_lossy(),
                "profile_id": profile_id,
                "pool_size": slots.len(),
            }),
        );

        // Focus on newly spawned agent if first one
        if slots.len() == 1 {
            *focused_slot = 0;
        }

        // Spawn decision agent
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = Self::spawn_decision_agent(slots, decision_coordinator, cwd, &agent_id) {
                logging::warn_event(
                    "pool.agent.decision_agent_failed",
                    "failed to spawn decision agent",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": e,
                    }),
                );
            }
        }

        Ok(agent_id)
    }

    /// Pause an agent with worktree preservation
    ///
    /// Saves worktree state before pausing for seamless resume.
    pub fn pause_with_worktree(
        slots: &mut Vec<AgentSlot>,
        worktree_coordinator: &WorktreeCoordinator,
        agent_id: &AgentId,
    ) -> Result<(), LifecycleError> {
        if !worktree_coordinator.is_enabled() {
            return Err(LifecycleError::WorktreeNotEnabled);
        }

        // Find the slot
        let slot = slots.iter()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| LifecycleError::AgentNotFound(agent_id.as_str().to_string()))?;

        if !slot.has_worktree() {
            return Err(LifecycleError::NoWorktree(agent_id.as_str().to_string()));
        }

        let worktree_path = slot.cwd();

        if !worktree_path.exists() {
            return Err(LifecycleError::WorktreeNotFound(worktree_path));
        }

        let worktree_state_store = worktree_coordinator.state_store().unwrap();
        let worktree_manager = worktree_coordinator.manager().unwrap();

        // Load existing state
        let mut worktree_state = worktree_state_store
            .load(agent_id.as_str())
            .map_err(|e| LifecycleError::StateStoreError(e.to_string()))?
            .ok_or_else(|| LifecycleError::StateNotFound(agent_id.as_str().to_string()))?;

        // Update state
        worktree_state.path = worktree_path.clone();
        worktree_state.touch();

        // Check uncommitted changes
        let has_changes = worktree_manager
            .has_uncommitted_changes(&worktree_path)
            .map_err(|e| LifecycleError::WorktreeError(e.to_string()))?;
        worktree_state.has_uncommitted_changes = has_changes;

        // Get current HEAD
        if let Some(head) = worktree_manager.get_head_commit(&worktree_path) {
            worktree_state.head_commit = Some(head);
        }

        // Save updated state
        worktree_state_store
            .save(agent_id.as_str(), &worktree_state)
            .map_err(|e| LifecycleError::StateStoreError(e.to_string()))?;

        // Transition slot to paused
        let slot_mut = slots.iter_mut()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| LifecycleError::AgentNotFound(agent_id.as_str().to_string()))?;
        slot_mut
            .transition_to(AgentSlotStatus::paused("worktree preserved"))
            .map_err(|e| LifecycleError::SlotTransitionError(e))?;

        logging::debug_event(
            "pool.agent.pause_with_worktree",
            "paused agent with worktree preservation",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "has_uncommitted_changes": has_changes,
                "worktree_path": worktree_path.to_string_lossy(),
            }),
        );

        Ok(())
    }

    /// Resume a paused agent with worktree verification
    ///
    /// Loads saved worktree state and verifies/recreates worktree if needed.
    pub fn resume_with_worktree(
        slots: &mut Vec<AgentSlot>,
        decision_coordinator: &mut WorkerDecisionRouter,
        worktree_coordinator: &WorktreeCoordinator,
        cwd: &PathBuf,
        agent_id: &AgentId,
    ) -> Result<(), LifecycleError> {
        if !worktree_coordinator.is_enabled() {
            return Err(LifecycleError::WorktreeNotEnabled);
        }

        let worktree_state_store = worktree_coordinator.state_store().unwrap();
        let worktree_manager = worktree_coordinator.manager().unwrap();

        // Load saved state
        let mut worktree_state = worktree_state_store
            .load(agent_id.as_str())
            .map_err(|e| LifecycleError::StateStoreError(e.to_string()))?
            .ok_or_else(|| LifecycleError::StateNotFound(agent_id.as_str().to_string()))?;

        // Find slot and verify paused
        let slot = slots.iter()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| LifecycleError::AgentNotFound(agent_id.as_str().to_string()))?;

        if !slot.status().is_paused() {
            return Err(LifecycleError::AgentNotPaused(agent_id.as_str().to_string()));
        }

        // Verify worktree exists or recreate
        let actual_worktree_path = if worktree_state.exists() {
            worktree_state.path.clone()
        } else {
            // Recreate worktree
            let branch_exists = worktree_state
                .branch
                .as_ref()
                .map(|b| worktree_manager.branch_exists(b).unwrap_or(false))
                .unwrap_or(false);

            let options = WorktreeCreateOptions {
                path: worktree_manager.worktrees_dir().join(&worktree_state.worktree_id),
                branch: worktree_state.branch.clone(),
                create_branch: !branch_exists && worktree_state.branch.is_some(),
                base: if branch_exists {
                    None
                } else {
                    Some(worktree_state.base_commit.clone())
                },
                lock_reason: None,
            };

            let worktree_info = worktree_manager
                .create(&worktree_state.worktree_id, options)
                .map_err(|e| LifecycleError::WorktreeError(e.to_string()))?;

            worktree_state.path = worktree_info.path.clone();

            worktree_state_store
                .save(agent_id.as_str(), &worktree_state)
                .map_err(|e| LifecycleError::StateStoreError(e.to_string()))?;

            logging::debug_event(
                "pool.agent.resume.recreated_worktree",
                "worktree recreated during resume",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "worktree_id": worktree_state.worktree_id,
                    "new_path": worktree_info.path.to_string_lossy(),
                }),
            );

            worktree_info.path
        };

        // Update slot
        {
            let slot_mut = slots.iter_mut()
                .find(|s| s.agent_id() == agent_id)
                .ok_or_else(|| LifecycleError::AgentNotFound(agent_id.as_str().to_string()))?;

            if slot_mut.worktree_path() != Some(&actual_worktree_path) {
                slot_mut.set_worktree(
                    actual_worktree_path.clone(),
                    worktree_state.branch.clone(),
                    worktree_state.worktree_id.clone(),
                );
            }

            slot_mut
                .transition_to(AgentSlotStatus::idle())
                .map_err(|e| LifecycleError::SlotTransitionError(e))?;
        }

        // Ensure decision agent exists
        if !decision_coordinator.has_agent(agent_id) {
            if let Some(slot) = slots.iter().find(|s| s.agent_id() == agent_id) {
                if let Some(provider_kind) = slot.provider_type().to_provider_kind() {
                    if provider_kind != ProviderKind::Mock {
                        if let Err(e) = Self::spawn_decision_agent(slots, decision_coordinator, cwd, agent_id) {
                            logging::warn_event(
                                "pool.resume.decision_agent_failed",
                                "failed to spawn decision agent for resumed agent",
                                serde_json::json!({
                                    "agent_id": agent_id.as_str(),
                                    "error": e,
                                }),
                            );
                        }
                    }
                }
            }
        }

        logging::debug_event(
            "pool.agent.resume_with_worktree",
            "resumed agent with worktree",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "worktree_path": actual_worktree_path.to_string_lossy(),
            }),
        );

        Ok(())
    }

    /// Stop an agent (simple)
    ///
    /// Transitions slot to stopped state and stops decision agent.
    pub fn stop_simple(
        slots: &mut Vec<AgentSlot>,
        decision_coordinator: &mut WorkerDecisionRouter,
        agent_id: &AgentId,
    ) -> Result<usize, String> {
        let index = slots.iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id.as_str()))?;

        let slot = &mut slots[index];
        let codename = slot.codename().clone();
        let reason = "user requested";
        slot.transition_to(AgentSlotStatus::stopped(reason))
            .map_err(|e| format!("Failed to stop agent: {}", e))?;

        // Stop decision agent
        Self::stop_decision_agent(decision_coordinator, agent_id)?;

        logging::debug_event(
            "pool.agent.stop",
            "stopped agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "slot_index": index,
                "reason": reason,
            }),
        );

        Ok(index)
    }

    /// Stop an agent with optional worktree cleanup
    ///
    /// Handles worktree state preservation or cleanup.
    pub fn stop_with_worktree(
        slots: &mut Vec<AgentSlot>,
        decision_coordinator: &mut WorkerDecisionRouter,
        worktree_coordinator: &WorktreeCoordinator,
        agent_id: &AgentId,
        cleanup_worktree: bool,
    ) -> Result<usize, LifecycleError> {
        let index = slots.iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| LifecycleError::AgentNotFound(agent_id.as_str().to_string()))?;

        let slot = &mut slots[index];
        let codename = slot.codename().clone();
        let has_worktree = slot.has_worktree();
        let worktree_id = slot.worktree_id().map(|s| s.clone());

        slot.transition_to(AgentSlotStatus::stopped("user requested"))
            .map_err(|e| LifecycleError::SlotTransitionError(e))?;

        // Stop decision agent
        Self::stop_decision_agent(decision_coordinator, agent_id)
            .map_err(|e| LifecycleError::SlotTransitionError(e))?;

        // Handle worktree cleanup
        if has_worktree && cleanup_worktree && worktree_id.is_some() {
            if let (Some(worktree_manager), Some(worktree_state_store)) =
                (worktree_coordinator.manager(), worktree_coordinator.state_store())
            {
                let wt_id = worktree_id.unwrap();

                let worktree_removed = match worktree_manager.remove(&wt_id, true) {
                    Ok(_) => true,
                    Err(e) => {
                        logging::debug_event(
                            "pool.agent.stop.worktree_remove_failed",
                            "worktree removal failed",
                            serde_json::json!({
                                "agent_id": agent_id.as_str(),
                                "worktree_id": wt_id,
                                "error": e.to_string(),
                            }),
                        );
                        false
                    }
                };

                if worktree_removed {
                    if let Err(e) = worktree_state_store.delete(agent_id.as_str()) {
                        logging::debug_event(
                            "pool.agent.stop.state_delete_failed",
                            "worktree state deletion failed",
                            serde_json::json!({
                                "agent_id": agent_id.as_str(),
                                "error": e.to_string(),
                            }),
                        );
                    }

                    logging::debug_event(
                        "pool.agent.stop.cleanup_worktree",
                        "worktree cleaned up",
                        serde_json::json!({
                            "agent_id": agent_id.as_str(),
                            "worktree_id": wt_id,
                        }),
                    );
                }
            }
        }

        logging::debug_event(
            "pool.agent.stop_with_worktree",
            "stopped agent with worktree handling",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "slot_index": index,
                "has_worktree": has_worktree,
                "cleanup_worktree": cleanup_worktree,
            }),
        );

        Ok(index)
    }

    /// Remove a stopped agent from the pool
    ///
    /// Adjusts focused_slot if necessary.
    pub fn remove_stopped(
        slots: &mut Vec<AgentSlot>,
        decision_coordinator: &mut WorkerDecisionRouter,
        focused_slot: &mut usize,
        agent_id: &AgentId,
    ) -> Result<(), String> {
        let index = slots.iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id.as_str()))?;

        let slot = &slots[index];
        if !slot.status().is_terminal() {
            logging::debug_event(
                "pool.agent.remove.failed",
                "failed to remove agent - not in terminal state",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "current_status": slot.status().label(),
                }),
            );
            return Err(format!(
                "Cannot remove agent with status {} (must be stopped)",
                slot.status().label()
            ));
        }

        let codename = slot.codename().clone();
        slots.remove(index);

        // Remove decision agent
        decision_coordinator.remove_agent(agent_id);

        logging::debug_event(
            "pool.agent.remove",
            "removed agent from pool",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "pool_size_after": slots.len(),
            }),
        );

        // Adjust focus
        if *focused_slot >= slots.len() && !slots.is_empty() {
            *focused_slot = slots.len() - 1;
            if let Some(new_focused) = slots.get(*focused_slot) {
                logging::debug_event(
                    "pool.focus.adjust",
                    "adjusted focus after agent removal",
                    serde_json::json!({
                        "new_index": focused_slot,
                        "new_agent_id": new_focused.agent_id().as_str(),
                    }),
                );
            }
        }

        Ok(())
    }

    /// Spawn decision agent for a work agent
    pub fn spawn_decision_agent(
        slots: &[AgentSlot],
        decision_coordinator: &mut WorkerDecisionRouter,
        cwd: &PathBuf,
        work_agent_id: &AgentId,
    ) -> Result<(), String> {
        // Find work slot
        let work_slot = slots.iter()
            .find(|s| s.agent_id() == work_agent_id)
            .ok_or_else(|| format!("Work agent not found: {}", work_agent_id.as_str()))?;

        let provider_kind_opt = work_slot.provider_type().to_provider_kind();
        let provider_kind = provider_kind_opt.ok_or_else(|| {
            format!(
                "Provider type {} doesn't have a ProviderKind mapping",
                work_slot.provider_type().label()
            )
        })?;

        // Create decision mail channel
        let mail = DecisionMail::new();
        let (sender, receiver) = mail.split();

        // Create decision agent slot
        let mut decision_agent = DecisionAgentSlot::new(
            work_agent_id.as_str().to_string(),
            provider_kind,
            receiver,
            cwd.clone(),
            decision_coordinator.components(),
        );

        // Inject ProviderLLMCaller
        use crate::llm_caller::ProviderLLMCaller;
        use std::sync::Arc;
        let llm_caller = Arc::new(ProviderLLMCaller::new(provider_kind, cwd.clone()));
        decision_agent.set_llm_caller(llm_caller);

        // Store in coordinator
        decision_coordinator.insert_agent(work_agent_id.clone(), decision_agent);
        decision_coordinator.insert_mail_sender(work_agent_id.clone(), sender);

        logging::debug_event(
            "pool.decision_agent.spawn",
            "spawned decision agent for work agent",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
                "provider_kind": provider_kind.label(),
            }),
        );

        Ok(())
    }

    /// Stop decision agent for a work agent
    pub fn stop_decision_agent(
        decision_coordinator: &mut WorkerDecisionRouter,
        work_agent_id: &AgentId,
    ) -> Result<(), String> {
        if let Some(mut decision_agent) = decision_coordinator.remove_agent(work_agent_id) {
            decision_agent.stop("work agent stopping");

            logging::debug_event(
                "pool.decision_agent.stop",
                "stopped decision agent for work agent",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                }),
            );
        }
        Ok(())
    }

    // ===== Helper methods =====

    /// Generate agent ID from workplace and index
    fn generate_agent_id(workplace_id: &WorkplaceId, next_agent_index: &mut usize) -> AgentId {
        let index = *next_agent_index;
        *next_agent_index += 1;
        AgentId::new(&format!("{}_{}", workplace_id.as_str(), index))
    }

    /// Generate codename from index
    fn generate_codename(index: usize) -> AgentCodename {
        AgentCodename::new(&format!("AGENT_{:03}", index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_error_display() {
        let err = LifecycleError::PoolFull;
        assert_eq!(err.to_string(), "Agent pool is full");

        let err = LifecycleError::AgentNotFound("test_1".to_string());
        assert_eq!(err.to_string(), "Agent not found: test_1");

        let err = LifecycleError::WorktreeNotEnabled;
        assert_eq!(err.to_string(), "Worktree support not enabled");
    }

    #[test]
    fn lifecycle_error_from_worktree_error() {
        let wt_err = WorktreeError::GitNotAvailable;
        let lifecycle_err: LifecycleError = wt_err.into();
        assert!(matches!(lifecycle_err, LifecycleError::WorktreeError(_)));
    }
}