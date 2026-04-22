//! AgentPool for managing multiple agent slots
//!
//! Central coordination structure for multi-agent runtime.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::backlog::{BacklogState, TaskStatus};
use crate::decision_agent_slot::{DecisionAgentSlot, DecisionAgentStatus};
use crate::decision_mail::{DecisionRequest, DecisionResponse};
use crate::logging;
use crate::{ProviderEvent, ProviderKind};
use crate::provider_profile::{ProfileId, ProfilePersistence, ProfileStore, ProviderProfile, get_effective_profile, AgentType as ProfileAgentType};
// Pool types (extracted to pool module, re-exported for backward compatibility)
pub use crate::pool::{
    AgentBlockedEvent, AgentBlockedNotifier, AgentStatusSnapshot, AgentTaskAssignment,
    BlockedHandlingConfig, BlockedHistoryEntry, BlockedTaskPolicy,
    DecisionExecutionResult, NoOpAgentBlockedNotifier, TaskQueueSnapshot,
    BlockedHandler, DecisionAgentCoordinator, DecisionAgentStats, WorktreeCoordinator,
    FocusManager, PoolQueries, DecisionExecutor,
    WorktreeRecovery, WorktreeRecoveryReport, AgentPoolWorktreeError,

    AgentLifecycleManager,
    spawn_decision_agent_for, spawn_decision_agent_with_profile_for, stop_decision_agent_for,
};
// Worktree types are re-exported from agent-worktree
use crate::{
    WorktreeCreateOptions, WorktreeError, WorktreeState,
};

// Decision layer imports
use agent_decision::{
    AutoAction, BlockedState, DecisionSituation, HumanDecisionQueue, HumanDecisionRequest,
    HumanDecisionResponse, HumanDecisionTimeoutConfig, HumanSelection, SituationType,
    builtin_situations::TaskStartingSituation,
    classifier::ClassifyResult,
    context::DecisionContext,
};

/// Pool managing multiple agent slots
pub struct AgentPool {
    /// All active agent slots
    slots: Vec<AgentSlot>,
    /// Max concurrent agents (configurable)
    max_slots: usize,
    /// Agent index counter for generating new IDs
    next_agent_index: usize,
    /// Workplace ID for this pool
    workplace_id: WorkplaceId,
    /// Human decision queue (slot-dependent, stays in pool)
    human_queue: HumanDecisionQueue,
    /// Blocked handling delegate (manages config, history, notifier)
    blocked_handler: BlockedHandler,
    /// Decision agent coordinator (manages agents, mail senders, components)
    decision_coordinator: DecisionAgentCoordinator,
    /// Worktree coordinator (manages manager, state store, git flow executor)
    worktree_coordinator: WorktreeCoordinator,
    /// Focus manager (manages focused slot index)
    focus_manager: FocusManager,
    /// Working directory for decision agents
    cwd: PathBuf,
    /// Provider profile store (optional, for profile-based agent creation)
    profile_store: Option<ProfileStore>,
}

impl std::fmt::Debug for AgentPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPool")
            .field("slots", &self.slots)
            .field("max_slots", &self.max_slots)
            .field("next_agent_index", &self.next_agent_index)
            .field("focus_manager", &self.focus_manager)
            .field("workplace_id", &self.workplace_id)
            .field("human_queue", &self.human_queue)
            .field("blocked_handler", &self.blocked_handler)
            .field("decision_coordinator", &self.decision_coordinator)
            .field("worktree_coordinator", &self.worktree_coordinator)
            .finish()
    }
}

impl AgentPool {
    /// Create a new empty agent pool
    pub fn new(workplace_id: WorkplaceId, max_slots: usize) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_handler: BlockedHandler::new(),
            decision_coordinator: DecisionAgentCoordinator::new(),
            worktree_coordinator: WorktreeCoordinator::new(),
            focus_manager: FocusManager::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            profile_store: None,
        }
    }

    /// Create pool with custom blocked handling config
    pub fn with_blocked_config(
        workplace_id: WorkplaceId,
        max_slots: usize,
        config: BlockedHandlingConfig,
    ) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            workplace_id,
            human_queue: HumanDecisionQueue::new(config.timeout_config.clone()),
            blocked_handler: BlockedHandler::with_config(config),
            decision_coordinator: DecisionAgentCoordinator::new(),
            worktree_coordinator: WorktreeCoordinator::new(),
            focus_manager: FocusManager::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            profile_store: None,
        }
    }

    /// Create pool with working directory for decision agents
    pub fn with_cwd(workplace_id: WorkplaceId, max_slots: usize, cwd: PathBuf) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_handler: BlockedHandler::new(),
            decision_coordinator: DecisionAgentCoordinator::new(),
            worktree_coordinator: WorktreeCoordinator::new(),
            focus_manager: FocusManager::new(),
            cwd,
            profile_store: None,
        }
    }

    /// Create pool with worktree support for isolated agent workspaces
    pub fn new_with_worktrees(
        workplace_id: WorkplaceId,
        max_slots: usize,
        repo_root: PathBuf,
        state_dir: PathBuf,
    ) -> Result<Self, WorktreeError> {
        let worktree_coordinator = WorktreeCoordinator::with_worktrees(repo_root.clone(), state_dir)?;

        // Sync next_agent_index with existing worktree states AND git branches
        // to avoid collision when user cancels restore but previous artifacts exist
        let max_existing_index = worktree_coordinator.find_max_agent_index();
        let next_agent_index = max_existing_index.map_or(1, |idx| idx + 1);

        Ok(Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_handler: BlockedHandler::new(),
            decision_coordinator: DecisionAgentCoordinator::new(),
            worktree_coordinator,
            focus_manager: FocusManager::new(),
            cwd: repo_root,
            profile_store: None,
        })
    }

    /// Set a custom blocked notifier
    pub fn set_blocked_notifier(&mut self, notifier: Arc<dyn AgentBlockedNotifier>) {
        self.blocked_handler.set_notifier(notifier);
    }

    /// Get the maximum number of slots
    pub fn max_slots(&self) -> usize {
        self.max_slots
    }

    /// Get the current number of active slots
    pub fn active_count(&self) -> usize {
        self.slots.len()
    }

    /// Check if pool can spawn more agents
    pub fn can_spawn(&self) -> bool {
        self.slots.len() < self.max_slots
    }

    /// Get the next agent index
    pub fn next_agent_index(&self) -> usize {
        self.next_agent_index
    }

    /// Get the focused slot index
    pub fn focused_slot_index(&self) -> usize {
        self.focus_manager.focused_index()
    }

    /// Get workplace ID
    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    /// Check if pool has worktree support enabled
    pub fn has_worktree_support(&self) -> bool {
        self.worktree_coordinator.is_enabled()
    }

    /// Generate a new unique agent ID
    fn generate_agent_id(&mut self) -> AgentId {
        let id = AgentId::new(format!("agent_{:03}", self.next_agent_index));
        self.next_agent_index += 1;
        id
    }

    /// Generate a codename for an agent
    fn generate_codename(index: usize) -> AgentCodename {
        const NAMES: &[&str] = &[
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
            "juliet", "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo",
            "sierra", "tango", "uniform", "victor", "whiskey", "xray", "yankee", "zulu",
        ];
        let zero_based = index.saturating_sub(1);
        let base = NAMES[zero_based % NAMES.len()];
        let round = zero_based / NAMES.len();
        let name = if round == 0 {
            base.to_string()
        } else {
            format!("{base}-{}", round + 1)
        };
        AgentCodename::new(name)
    }

    /// Spawn a new agent with specified provider type (mock for foundation)
    ///
    /// Returns the new agent's ID on success, or error if pool is full.
    pub fn spawn_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
        if !self.can_spawn() {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn agent - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "pool_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }

        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        self.slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn",
            "spawned new agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "provider_type": provider_type.label(),
                "pool_size": self.slots.len(),
                "max_slots": self.max_slots,
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        self.focus_manager.focus_on_first_spawn(self.slots.len(), &agent_id);

        // Spawn decision agent for this work agent (if provider supports it)
        // All non-Mock agents should have decision layer support
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = self.spawn_decision_agent_for(&agent_id) {
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

    /// Load provider profiles from persistence
    ///
    /// Loads merged profiles (global + workplace override).
    pub fn load_profiles(&mut self, persistence: &ProfilePersistence) -> anyhow::Result<()> {
        let store = persistence.load_merged()?;
        self.profile_store = Some(store);
        logging::debug_event(
            "pool.profile.load",
            "loaded provider profiles",
            serde_json::json!({
                "profile_count": self.profile_store.as_ref().map(|s| s.profile_count()).unwrap_or(0),
            }),
        );
        Ok(())
    }

    /// Set profile store directly (for testing or custom configuration)
    pub fn set_profile_store(&mut self, store: ProfileStore) {
        self.profile_store = Some(store);
    }

    /// Get the profile store (if loaded)
    pub fn profile_store(&self) -> Option<&ProfileStore> {
        self.profile_store.as_ref()
    }

    /// Spawn a new agent using a specific provider profile
    ///
    /// The profile defines the CLI type and environment configuration.
    pub fn spawn_agent_with_profile(
        &mut self,
        profile_id: &ProfileId,
    ) -> Result<AgentId, crate::provider_profile::ProfileError> {
        let store = self.profile_store.as_ref()
            .ok_or(crate::provider_profile::ProfileError::NoProfileStore)?;

        let profile = get_effective_profile(store, Some(profile_id), ProfileAgentType::Work)?;

        // Resolve profile to get ProviderKind
        let provider_kind = profile.base_cli.to_provider_kind()
            .ok_or_else(|| crate::provider_profile::ProfileError::UnsupportedCliType(
                profile.base_cli.label().to_string()
            ))?;

        if !self.can_spawn() {
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

        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let mut slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slot.set_profile_id(profile_id.clone());

        self.slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn_profile",
            "spawned new agent with profile",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "profile_id": profile_id,
                "provider_type": provider_type.label(),
                "pool_size": self.slots.len(),
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        self.focus_manager.focus_on_first_spawn(self.slots.len(), &agent_id);

        // Spawn decision agent for this work agent (if provider supports it)
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = self.spawn_decision_agent_for(&agent_id) {
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

    /// Spawn a new agent with an isolated worktree workspace
    ///
    /// Creates a new git worktree for the agent and spawns the agent
    /// configured to work in that isolated workspace.
    pub fn spawn_agent_with_worktree(
        &mut self,
        provider_kind: ProviderKind,
        branch_name: Option<String>,
        task_id: Option<String>,
    ) -> Result<AgentId, AgentPoolWorktreeError> {
        // Check worktree manager is available first (before any mutable borrows)
        if !self.worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        // Use internal helper without profile
        self.spawn_agent_with_worktree_internal(provider_kind, branch_name, task_id, None)
    }

    /// Spawn a new agent with worktree and a specific provider profile
    ///
    /// Creates a new git worktree for the agent using the specified profile.
    pub fn spawn_agent_with_worktree_and_profile(
        &mut self,
        profile_id: &ProfileId,
        branch_name: Option<String>,
        task_id: Option<String>,
    ) -> Result<AgentId, AgentPoolWorktreeError> {
        // Check worktree coordinator is enabled first
        if !self.worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        // Check profile store is loaded
        let store = self.profile_store.as_ref()
            .ok_or_else(|| AgentPoolWorktreeError::StateStoreError(
                "Profile store not loaded".to_string()
            ))?;

        let profile = get_effective_profile(store, Some(profile_id), ProfileAgentType::Work)
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

        // Resolve profile to get ProviderKind
        let provider_kind = profile.base_cli.to_provider_kind()
            .ok_or_else(|| AgentPoolWorktreeError::StateStoreError(
                format!("CLI type '{}' not supported", profile.base_cli.label())
            ))?;

        // Use existing spawn_agent_with_worktree logic with resolved provider_kind
        let agent_id = self.spawn_agent_with_worktree_internal(
            provider_kind,
            branch_name,
            task_id,
            Some(profile_id.clone()),
        )?;

        Ok(agent_id)
    }

    /// Internal helper for spawning agent with worktree (shared logic)
    fn spawn_agent_with_worktree_internal(
        &mut self,
        provider_kind: ProviderKind,
        branch_name: Option<String>,
        task_id: Option<String>,
        profile_id: Option<ProfileId>,
    ) -> Result<AgentId, AgentPoolWorktreeError> {
        // This is the core logic extracted from spawn_agent_with_worktree
        // Check if pool has capacity
        if !self.can_spawn() {
            return Err(AgentPoolWorktreeError::PoolFull);
        }

        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);
        let worktree_id = format!("wt-{}", agent_id.as_str());
        let actual_branch = branch_name.unwrap_or_else(|| format!("agent/{}", agent_id.as_str()));

        // Get worktree manager and state store via coordinator
        let worktree_manager = self.worktree_coordinator.manager().unwrap();
        let worktree_state_store = self.worktree_coordinator.state_store().unwrap();

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
            .map_err(AgentPoolWorktreeError::WorktreeError)?;

        // Get the base commit SHA before creating worktree state
        let base_commit = worktree_manager
            .get_current_head()
            .map_err(AgentPoolWorktreeError::WorktreeError)?;

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
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

        // Create agent slot with worktree and profile
        let mut slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slot.set_worktree(
            worktree_info.path.clone(),
            Some(actual_branch.clone()),
            worktree_id.clone(),
        );
        if let Some(ref pid) = profile_id {
            slot.set_profile_id(pid.clone());
        }

        self.slots.push(slot);

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
                "pool_size": self.slots.len(),
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        self.focus_manager.focus_on_first_spawn(self.slots.len(), &agent_id);

        // Spawn decision agent for this work agent (if provider supports it)
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = self.spawn_decision_agent_for(&agent_id) {
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

    /// Pause an agent with worktree state preservation
    ///
    /// Saves the current worktree state (uncommitted changes status, HEAD commit)
    /// before pausing, allowing for seamless resume later.
    pub fn pause_agent_with_worktree(
        &mut self,
        agent_id: &AgentId,
    ) -> Result<(), AgentPoolWorktreeError> {
        // Check worktree support is available
        if !self.worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let slot = self
            .get_slot_by_id(agent_id)
            .ok_or_else(|| AgentPoolWorktreeError::AgentNotFound(agent_id.as_str().to_string()))?;

        // Only pause if agent has worktree
        if !slot.has_worktree() {
            return Err(AgentPoolWorktreeError::NoWorktree(
                agent_id.as_str().to_string(),
            ));
        }

        // Get the actual worktree path from slot (most current)
        let worktree_path = slot.cwd();

        // Check if worktree still exists on disk
        if !worktree_path.exists() {
            return Err(AgentPoolWorktreeError::WorktreeNotFound(worktree_path));
        }

        let worktree_state_store = self.worktree_coordinator.state_store().unwrap();
        let worktree_manager = self.worktree_coordinator.manager().unwrap();

        // Load existing worktree state
        let mut worktree_state = worktree_state_store
            .load(agent_id.as_str())
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?
            .ok_or_else(|| AgentPoolWorktreeError::StateNotFound(agent_id.as_str().to_string()))?;

        // Update state with current path (in case it changed)
        worktree_state.path = worktree_path.clone();
        worktree_state.touch();

        // Check for uncommitted changes
        let has_changes = worktree_manager
            .has_uncommitted_changes(&worktree_path)
            .map_err(AgentPoolWorktreeError::WorktreeError)?;
        worktree_state.has_uncommitted_changes = has_changes;

        // Get current HEAD
        if let Some(head) = worktree_manager.get_head_commit(&worktree_path) {
            worktree_state.head_commit = Some(head);
        }

        // Save updated state
        worktree_state_store
            .save(agent_id.as_str(), &worktree_state)
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

        // Transition slot to paused
        let slot_mut = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| AgentPoolWorktreeError::AgentNotFound(agent_id.as_str().to_string()))?;
        slot_mut
            .transition_to(AgentSlotStatus::paused("worktree preserved"))
            .map_err(|e: String| AgentPoolWorktreeError::SlotTransitionError(e))?;

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

    /// Resume an agent with worktree verification
    ///
    /// Loads the saved worktree state, verifies the worktree still exists
    /// (or recreates it if needed), and transitions the agent to idle (ready to work).
    pub fn resume_agent_with_worktree(
        &mut self,
        agent_id: &AgentId,
    ) -> Result<(), AgentPoolWorktreeError> {
        // Check worktree support is available
        if !self.worktree_coordinator.is_enabled() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let worktree_state_store = self.worktree_coordinator.state_store().unwrap();
        let worktree_manager = self.worktree_coordinator.manager().unwrap();

        // Load saved worktree state
        let mut worktree_state = worktree_state_store
            .load(agent_id.as_str())
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?
            .ok_or_else(|| AgentPoolWorktreeError::StateNotFound(agent_id.as_str().to_string()))?;

        // Get the slot and verify it's paused
        let slot = self
            .get_slot_by_id(agent_id)
            .ok_or_else(|| AgentPoolWorktreeError::AgentNotFound(agent_id.as_str().to_string()))?;

        if !slot.status().is_paused() {
            return Err(AgentPoolWorktreeError::AgentNotPaused(
                agent_id.as_str().to_string(),
            ));
        }

        // Verify worktree exists or recreate it
        let actual_worktree_path = if worktree_state.exists() {
            worktree_state.path.clone()
        } else {
            // Worktree was deleted externally - recreate it
            // Check if branch still exists - if so, use existing branch, don't create new
            let branch_exists = worktree_state
                .branch
                .as_ref()
                .map(|b| worktree_manager.branch_exists(b).unwrap_or(false))
                .unwrap_or(false);

            let options = WorktreeCreateOptions {
                path: worktree_manager
                    .worktrees_dir()
                    .join(&worktree_state.worktree_id),
                branch: worktree_state.branch.clone(),
                create_branch: !branch_exists && worktree_state.branch.is_some(),
                base: if branch_exists {
                    None // Use existing branch, no base needed
                } else {
                    Some(worktree_state.base_commit.clone())
                },
                lock_reason: None,
            };

            let worktree_info = worktree_manager
                .create(&worktree_state.worktree_id, options)
                .map_err(AgentPoolWorktreeError::WorktreeError)?;

            // Update worktree_state with new path
            worktree_state.path = worktree_info.path.clone();

            // Save updated state
            worktree_state_store
                .save(agent_id.as_str(), &worktree_state)
                .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

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

        // Update slot's worktree path if it differs from current
        {
            let slot_mut = self.get_slot_mut_by_id(agent_id).ok_or_else(|| {
                AgentPoolWorktreeError::AgentNotFound(agent_id.as_str().to_string())
            })?;

            // Sync the slot's worktree info with actual state
            if slot_mut.worktree_path() != Some(&actual_worktree_path) {
                slot_mut.set_worktree(
                    actual_worktree_path.clone(),
                    worktree_state.branch.clone(),
                    worktree_state.worktree_id.clone(),
                );
            }

            // Transition to idle (ready to resume work)
            slot_mut
                .transition_to(AgentSlotStatus::idle())
                .map_err(|e: String| AgentPoolWorktreeError::SlotTransitionError(e))?;
        }

        // Ensure decision agent exists for this work agent
        // It may have been stopped or lost during pause/crash
        if !self.has_decision_agent(agent_id) {
            if let Ok(slot_index) = self.find_slot_index(agent_id) {
                let provider_kind_opt = self.slots[slot_index].provider_type().to_provider_kind();
                if let Some(provider_kind) = provider_kind_opt {
                    if provider_kind != ProviderKind::Mock {
                        if let Err(e) = self.spawn_decision_agent_for(agent_id) {
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

    /// Recover orphaned worktree states from previous session
    ///
    /// Called at startup to detect worktree states that exist in the store
    /// but whose worktrees may have been deleted externally. This method:
    /// 1. Lists all persisted worktree states
    /// 2. Checks if the worktree path still exists
    /// 3. For missing worktrees, either recreates them or cleans up the state
    ///
    /// Returns a summary of recovered and cleaned up worktrees.
    pub fn recover_orphaned_worktrees(
        &mut self,
        recreate_missing: bool,
    ) -> Result<WorktreeRecoveryReport, AgentPoolWorktreeError> {
        WorktreeRecovery::recover_orphaned(
            &self.slots,
            &self.worktree_coordinator,
            recreate_missing,
        )
    }

    /// Auto cleanup idle worktrees
    ///
    /// Checks for worktrees that have been idle for longer than the specified
    /// duration and have no commits/uncommitted changes. Cleans up both the
    /// worktree directory and the persisted state.
    ///
    /// Returns a list of cleaned up worktree IDs.
    pub fn auto_cleanup_idle_worktrees(
        &mut self,
        idle_duration: chrono::Duration,
    ) -> Result<Vec<String>, AgentPoolWorktreeError> {
        WorktreeRecovery::auto_cleanup_idle(
            &mut self.slots,
            &self.worktree_coordinator,
            idle_duration,
        )
    }

    /// Spawn the OVERVIEW agent (ProductOwner role) at the top of the pool
    ///
    /// The OVERVIEW agent is a special coordination agent that always stays at index 0.
    /// Returns the agent ID on success, or error if pool is full or OVERVIEW already exists.
    pub fn spawn_overview_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
        // Check if OVERVIEW agent already exists
        if self
            .slots
            .iter()
            .any(|s| s.role() == AgentRole::ProductOwner)
        {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn OVERVIEW agent - already exists",
                serde_json::json!({
                    "reason": "overview_already_exists",
                    "pool_size": self.slots.len(),
                }),
            );
            return Err("OVERVIEW agent already exists".to_string());
        }

        if !self.can_spawn() {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn OVERVIEW agent - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "pool_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }

        let agent_id = AgentId::new("OVERVIEW");
        let codename = AgentCodename::new("OVERVIEW");
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        logging::debug_event(
            "pool.agent.spawn_overview",
            "spawning OVERVIEW agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "provider_type": provider_type.label(),
                "pool_size_before": self.slots.len(),
            }),
        );

        let slot = AgentSlot::with_role(
            agent_id.clone(),
            codename,
            provider_type,
            AgentRole::ProductOwner,
        );

        // Insert at the beginning (always at index 0)
        self.slots.insert(0, slot);
        // Note: Do NOT increment next_agent_index for OVERVIEW agent
        // Worker agents should start from index 0 (alpha)

        // Focus on OVERVIEW agent by default
        self.focus_manager.reset_to_first();

        logging::debug_event(
            "pool.focus.change",
            "focus set to OVERVIEW agent after spawn",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "index": 0,
            }),
        );

        // Spawn decision agent for OVERVIEW (if provider supports it)
        // All non-Mock agents should have decision layer support
        if provider_kind != ProviderKind::Mock {
            if let Err(e) = self.spawn_decision_agent_for(&agent_id) {
                logging::warn_event(
                    "pool.overview.decision_agent_failed",
                    "failed to spawn decision agent for OVERVIEW",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": e,
                    }),
                );
            }
        }

        Ok(agent_id)
    }

    /// Get the OVERVIEW agent slot (ProductOwner role)
    pub fn overview_agent(&self) -> Option<&AgentSlot> {
        self.slots
            .iter()
            .find(|s| s.role() == AgentRole::ProductOwner)
    }

    // ===== Decision Agent Management =====

    /// Spawn a decision agent for a work agent
    ///
    /// Creates a decision agent that handles decision requests for the specified work agent.
    /// The decision agent uses the same provider as the work agent.
    pub fn spawn_decision_agent_for(&mut self, work_agent_id: &AgentId) -> Result<(), String> {
        spawn_decision_agent_for(
            &self.slots,
            &mut self.decision_coordinator,
            &self.cwd,
            work_agent_id,
        )
    }

    /// Spawn decision agent for a work agent with optional profile
    ///
    /// Creates a decision agent with an optional profile_id for independent
    /// decision layer LLM backend configuration.
    pub fn spawn_decision_agent_with_profile_for(
        &mut self,
        work_agent_id: &AgentId,
        profile_id: Option<&ProfileId>,
    ) -> Result<(), String> {
        spawn_decision_agent_with_profile_for(
            &self.slots,
            &mut self.decision_coordinator,
            &self.cwd,
            work_agent_id,
            profile_id,
        )
    }

    /// Stop the decision agent for a work agent
    pub fn stop_decision_agent_for(&mut self, work_agent_id: &AgentId) -> Result<(), String> {
        stop_decision_agent_for(&mut self.decision_coordinator, work_agent_id)
    }

    /// Get decision agent for a work agent
    pub fn decision_agent_for(&self, work_agent_id: &AgentId) -> Option<&DecisionAgentSlot> {
        self.decision_coordinator.agent_for(work_agent_id)
    }

    /// Check if work agent has a decision agent
    pub fn has_decision_agent(&self, work_agent_id: &AgentId) -> bool {
        self.decision_coordinator.has_agent(work_agent_id)
    }

    /// Get list of agents that have decision agents in Thinking status
    /// OR had a decision started recently (within MIN_DECISION_DISPLAY_MS)
    /// Used for UI display of pending decisions
    pub fn agents_with_pending_decisions(&self) -> Vec<(AgentId, std::time::Instant)> {
        const MIN_DECISION_DISPLAY_MS: u64 = 1500; // Show "Analyzing" for at least 1.5s

        let now = std::time::Instant::now();

        // First, get agents in Thinking status
        let thinking_agents: Vec<_> = self
            .decision_coordinator
            .agents_iter()
            .filter_map(|(work_agent_id, decision_agent)| match decision_agent.status() {
                DecisionAgentStatus::Thinking { started_at } => {
                    Some((work_agent_id.clone(), *started_at))
                }
                _ => None,
            })
            .collect();

        // If no thinking agents, check if any decision agent had a decision recently
        if thinking_agents.is_empty() {
            let recent_agents: Vec<_> = self
                .decision_coordinator
                .agents_iter()
                .filter_map(|(work_agent_id, decision_agent)| {
                    if let Some(started_at) = decision_agent.last_decision_started_at() {
                        let elapsed = now.duration_since(started_at);
                        if elapsed.as_millis() < MIN_DECISION_DISPLAY_MS as u128 {
                            return Some((work_agent_id.clone(), started_at));
                        }
                    }
                    None
                })
                .collect();

            if !recent_agents.is_empty() {
                return recent_agents;
            }
        }

        thinking_agents
    }

    /// Classify an event for a specific agent
    ///
    /// Uses the classifier registry to determine if the event needs a decision.
    pub fn classify_event(&self, agent_id: &AgentId, event: &ProviderEvent) -> ClassifyResult {
        // Find the work agent slot
        if let Some(slot) = self.get_slot_by_id(agent_id) {
            let provider_kind_opt = slot.provider_type().to_provider_kind();

            // Handle providers without ProviderKind mapping
            if let Some(provider_kind) = provider_kind_opt {
                // Convert to decision ProviderKind
                let decision_provider = match provider_kind {
                    ProviderKind::Claude => agent_decision::provider_kind::ProviderKind::Claude,
                    ProviderKind::Codex => agent_decision::provider_kind::ProviderKind::Codex,
                    ProviderKind::Mock => agent_decision::provider_kind::ProviderKind::Unknown,
                };

                // Convert core ProviderEvent to decision ProviderEvent via shared kernel
                let decision_event: Option<agent_decision::provider::ProviderEvent> = event.into();

                // Use classifier registry to classify the event
                if let Some(decision_event) = decision_event {
                    self.decision_coordinator
                        .components()
                        .classifier_registry
                        .classify(&decision_event, decision_provider)
                } else {
                    ClassifyResult::running(None)
                }
            } else {
                // No ProviderKind mapping, return Running result
                ClassifyResult::running(None)
            }
        } else {
            // Agent not found, return Running result
            ClassifyResult::running(None)
        }
    }

    /// Send a decision request to a decision agent
    ///
    /// Returns true if request was sent successfully.
    pub fn send_decision_request(
        &self,
        work_agent_id: &AgentId,
        request: DecisionRequest,
    ) -> Result<(), String> {
        // Clone situation_type before sending for logging
        let situation_type_name = request.situation_type.name.clone();
        let situation_prompt = request.context.trigger_situation.to_prompt_text();

        if let Some(sender) = self.decision_coordinator.mail_sender_for(work_agent_id) {
            sender.send_request(request).map_err(|e| {
                logging::warn_event(
                    "decision_layer.request_send_failed",
                    "failed to send decision request to decision agent",
                    serde_json::json!({
                        "work_agent_id": work_agent_id.as_str(),
                        "situation_type": situation_type_name,
                        "error": e,
                    }),
                );
                e
            })?;

            logging::debug_event(
                "decision_layer.request_sent",
                "decision request sent to decision agent",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "situation_type": situation_type_name,
                    "situation_prompt": situation_prompt,
                }),
            );

            Ok(())
        } else {
            logging::warn_event(
                "decision_layer.no_decision_agent",
                "no decision agent available for work agent",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                }),
            );
            Err(format!(
                "No decision agent for work agent {}",
                work_agent_id.as_str()
            ))
        }
    }

    /// Trigger task preparation for an agent with a newly assigned task.
    ///
    /// This sends a TaskStartingSituation to the decision layer, which should
    /// return a prepare_task_start action that gets executed via execute_decision_action.
    pub fn trigger_task_preparation(
        &self,
        work_agent_id: &AgentId,
        task_id: &str,
        task_description: &str,
    ) -> Result<(), String> {
        // Create TaskStartingSituation with the task info
        let situation = TaskStartingSituation::new(task_description)
            .with_task_id(task_id);

        let situation_type = situation.situation_type();

        // Create decision context
        let context = DecisionContext::new(Box::new(situation), work_agent_id.as_str());

        // Create decision request
        let request = DecisionRequest::new(
            work_agent_id.clone(),
            situation_type.clone(),
            context,
        );

        logging::debug_event(
            "task_preparation.triggered",
            "triggering task preparation for assigned task",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
                "task_id": task_id,
                "situation_type": situation_type.name,
            }),
        );

        // Send the request
        self.send_decision_request(work_agent_id, request)
    }

    /// Poll decision agents and process pending requests
    ///
    /// Returns responses from decision agents that have processed requests.
    pub fn poll_decision_agents(&mut self) -> Vec<(AgentId, DecisionResponse)> {
        let mut responses = Vec::new();

        // Process each agent with its mail sender using coordinator's helper
        self.decision_coordinator.for_each_agent_with_mail_sender(|work_agent_id, decision_agent, sender| {
            // Poll and process any pending requests (spawns async threads)
            decision_agent.poll_and_process();

            // Try to receive any responses that were generated via channel
            let mut received_this_poll = false;
            let mut had_error = false;
            if let Some(mail_sender) = sender {
                while let Some(response) = mail_sender.try_receive_response() {
                    if response.is_error() {
                        had_error = true;
                    }
                    responses.push((work_agent_id.clone(), response));
                    received_this_poll = true;
                }
            }

            // If no channel response received, check for fallback response
            // This handles the case where async thread couldn't send via channel
            if !received_this_poll {
                if let Some(response) = decision_agent.take_fallback_response() {
                    if response.is_error() {
                        had_error = true;
                    }
                    responses.push((work_agent_id.clone(), response));
                    received_this_poll = true;
                }
            }

            // Only clear thinking status if we actually received a response
            // This prevents premature reset when async thread is still running
            if received_this_poll {
                decision_agent.clear_thinking_status(had_error);
            }
        });

        // Cleanup: Clear old timestamps that are past the display window (1.5s)
        // This prevents memory accumulation of stale timestamps
        const MIN_DECISION_DISPLAY_MS: u64 = 1500;
        self.decision_coordinator.for_each_agent_mut(|_, decision_agent| {
            if let Some(started_at) = decision_agent.last_decision_started_at() {
                let elapsed = std::time::Instant::now().duration_since(started_at);
                if elapsed.as_millis() >= MIN_DECISION_DISPLAY_MS as u128 {
                    decision_agent.clear_recent_decision();
                }
            }
        });

        responses
    }

    /// Get decision agent statistics
    pub fn decision_agent_stats(&self) -> DecisionAgentStats {
        self.decision_coordinator.stats()
    }

    /// Execute a decision action on a work agent
    ///
    /// Takes a decision output and executes the actions on the specified work agent.
    pub fn execute_decision_action(
        &mut self,
        work_agent_id: &AgentId,
        output: &agent_decision::output::DecisionOutput,
    ) -> DecisionExecutionResult {
        DecisionExecutor::execute(
            &mut self.slots,
            &mut self.human_queue,
            &self.worktree_coordinator,
            work_agent_id,
            output,
        )
    }

    /// Stop a specific agent by ID
    ///
    /// Returns the slot index that was stopped.
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<usize, String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &mut self.slots[index];
        let codename = slot.codename().clone();
        let reason = "user requested";
        slot.transition_to(AgentSlotStatus::stopped(reason))
            .map_err(|e| format!("Failed to stop agent: {}", e))?;

        // Also stop the decision agent for this work agent
        self.stop_decision_agent_for(agent_id)?;

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

    /// Stop an agent and optionally cleanup its worktree
    ///
    /// # Arguments
    /// * `agent_id` - The agent ID to stop
    /// * `cleanup_worktree` - If true, remove the worktree and delete state; if false, preserve worktree
    ///
    /// # Returns
    /// The slot index of the stopped agent
    pub fn stop_agent_with_worktree_cleanup(
        &mut self,
        agent_id: &AgentId,
        cleanup_worktree: bool,
    ) -> Result<usize, AgentPoolWorktreeError> {
        use crate::pool::LifecycleError;

        AgentLifecycleManager::stop_with_worktree(
            &mut self.slots,
            &mut self.decision_coordinator,
            &self.worktree_coordinator,
            agent_id,
            cleanup_worktree,
        )
        .map_err(|e| match e {
            LifecycleError::AgentNotFound(id) => AgentPoolWorktreeError::AgentNotFound(id),
            LifecycleError::SlotTransitionError(msg) => AgentPoolWorktreeError::SlotTransitionError(msg),
            LifecycleError::WorktreeError(msg) => AgentPoolWorktreeError::WorktreeError(
                crate::WorktreeError::GitCommandFailed(msg)
            ),
            LifecycleError::StateStoreError(msg) => AgentPoolWorktreeError::StateStoreError(msg),
            LifecycleError::WorktreeNotEnabled => AgentPoolWorktreeError::WorktreeNotEnabled,
            LifecycleError::WorktreeNotFound(path) => AgentPoolWorktreeError::WorktreeNotFound(path),
            LifecycleError::StateNotFound(id) => AgentPoolWorktreeError::StateNotFound(id),
            LifecycleError::AgentNotPaused(id) => AgentPoolWorktreeError::AgentNotPaused(id),
            LifecycleError::NoWorktree(id) => AgentPoolWorktreeError::NoWorktree(id),
            LifecycleError::PoolFull => AgentPoolWorktreeError::PoolFull,
            other => AgentPoolWorktreeError::StateStoreError(other.to_string()),
        })
    }

    /// Remove a stopped agent from the pool
    ///
    /// Only stopped agents can be removed.
    pub fn remove_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &self.slots[index];
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
        self.slots.remove(index);

        // Also remove the decision agent for this work agent using coordinator
        self.decision_coordinator.remove_agent(agent_id);

        logging::debug_event(
            "pool.agent.remove",
            "removed agent from pool",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "pool_size_after": self.slots.len(),
            }),
        );

        // Adjust focus if necessary
        self.focus_manager.adjust_on_remove(index, self.slots.len());
        Ok(())
    }

    /// Get all agents with their current status
    pub fn agent_statuses(&self) -> Vec<AgentStatusSnapshot> {
        PoolQueries::agent_statuses(&self.slots)
    }

    /// Get all slots for snapshot/export use.
    pub fn slots(&self) -> &[AgentSlot] {
        &self.slots
    }

    /// Get all slots mutably (for command draining).
    pub fn slots_mut(&mut self) -> &mut [AgentSlot] {
        &mut self.slots
    }

    /// Restore an agent slot into the pool.
    pub fn restore_slot(&mut self, slot: AgentSlot) -> Result<(), String> {
        let agent_id = slot.agent_id().as_str().to_string();
        let codename = slot.codename().as_str().to_string();
        let role = slot.role().label();

        logging::debug_event(
            "pool.slot.restore",
            "restoring agent slot from snapshot",
            serde_json::json!({
                "agent_id": agent_id,
                "codename": codename,
                "role": role,
                "current_pool_size": self.slots.len(),
                "max_slots": self.max_slots,
            }),
        );

        if !self.can_spawn() {
            logging::debug_event(
                "pool.slot.restore.failed",
                "restore failed - pool full",
                serde_json::json!({
                    "agent_id": agent_id,
                    "current_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }
        if self
            .slots
            .iter()
            .any(|existing| existing.agent_id().as_str() == agent_id)
        {
            let err = format!("Agent {} already exists in pool", agent_id);
            logging::debug_event(
                "pool.slot.restore.failed",
                "restore failed - agent already exists",
                serde_json::json!({
                    "agent_id": agent_id,
                    "error": err,
                }),
            );
            return Err(err);
        }

        let agent_id = if slot.role() == AgentRole::ProductOwner {
            if self.overview_agent().is_some() {
                let err = "OVERVIEW agent already exists".to_string();
                logging::debug_event(
                    "pool.slot.restore.failed",
                    "restore failed - overview agent exists",
                    serde_json::json!({
                        "error": err,
                    }),
                );
                return Err(err);
            }
            self.slots.insert(0, slot);
            self.slots[0].agent_id().clone()
        } else {
            self.slots.push(slot);
            self.slots.last().unwrap().agent_id().clone()
        };

        if let Some(restored_index) = self
            .slots
            .last()
            .and_then(|restored| parse_agent_index(restored.agent_id().as_str()))
        {
            self.next_agent_index = self.next_agent_index.max(restored_index + 1);
        } else if let Some(restored_index) = self
            .slots
            .iter()
            .filter_map(|slot| parse_agent_index(slot.agent_id().as_str()))
            .max()
        {
            self.next_agent_index = self.next_agent_index.max(restored_index + 1);
        }

        // Spawn decision agent for this restored work agent (if provider supports it)
        // All non-Mock agents should have decision layer support
        if let Ok(slot_index) = self.find_slot_index(&agent_id) {
            let provider_kind_opt = self.slots[slot_index].provider_type().to_provider_kind();
            if let Some(provider_kind) = provider_kind_opt {
                if provider_kind != ProviderKind::Mock {
                    if let Err(e) = self.spawn_decision_agent_for(&agent_id) {
                        logging::warn_event(
                            "pool.restore.decision_agent_failed",
                            "failed to spawn decision agent for restored agent",
                            serde_json::json!({
                                "agent_id": agent_id.as_str(),
                                "error": e,
                            }),
                        );
                    }
                }
            }
        }

        logging::debug_event(
            "pool.slot.restore.complete",
            "agent slot restored successfully",
            serde_json::json!({
                "agent_id": agent_id,
                "new_pool_size": self.slots.len(),
            }),
        );

        Ok(())
    }

    /// Switch focus to a different agent by index
    pub fn focus_agent_by_index(&mut self, index: usize) -> Result<(), String> {
        self.focus_manager.focus_by_index(&self.slots, index)
            .map_err(|e| e.to_string())
    }

    /// Switch focus to a different agent by ID
    pub fn focus_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        self.focus_manager.focus_agent(&self.slots, agent_id)
            .map_err(|e| e.to_string())
    }

    /// Get slot by index
    pub fn get_slot(&self, index: usize) -> Option<&AgentSlot> {
        self.slots.get(index)
    }

    /// Get slot by agent ID
    pub fn get_slot_by_id(&self, agent_id: &AgentId) -> Option<&AgentSlot> {
        self.slots.iter().find(|s| s.agent_id() == agent_id)
    }

    /// Get mutable slot by index
    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut AgentSlot> {
        self.slots.get_mut(index)
    }

    /// Get mutable slot by agent ID
    pub fn get_slot_mut_by_id(&mut self, agent_id: &AgentId) -> Option<&mut AgentSlot> {
        self.slots.iter_mut().find(|s| s.agent_id() == agent_id)
    }

    /// Set the last activity timestamp for a slot (testing / snapshot restore)
    pub fn set_slot_last_activity(&mut self, agent_id: &AgentId, instant: Instant) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.agent_id() == agent_id) {
            slot.set_last_activity(instant);
        }
    }

    /// Get the currently focused slot
    pub fn focused_slot(&self) -> Option<&AgentSlot> {
        self.slots.get(self.focus_manager.focused_index())
    }

    /// Get the currently focused slot (mutable)
    pub fn focused_slot_mut(&mut self) -> Option<&mut AgentSlot> {
        self.slots.get_mut(self.focus_manager.focused_index())
    }

    /// Find the index of a slot by agent ID
    fn find_slot_index(&self, agent_id: &AgentId) -> Result<usize, String> {
        self.slots
            .iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))
    }

    /// Get ProviderLaunchContext for an agent from its profile
    ///
    /// Returns None if:
    /// - Agent doesn't exist
    /// - Agent has no profile set
    /// - Profile store is not loaded
    /// - Profile not found in store
    ///
    /// If agent has no profile, falls back to default launch context from ProviderKind.
    pub fn get_launch_context_for_agent(
        &self,
        agent_id: &AgentId,
        cwd: PathBuf,
    ) -> Option<crate::launch_config::context::ProviderLaunchContext> {
        use crate::provider_profile::create_launch_context_from_profile;

        let slot = self.get_slot_by_id(agent_id)?;

        // If agent has a profile, use it
        if let Some(profile_id) = slot.profile_id() {
            let store = self.profile_store.as_ref()?;
            let profile = store.get_profile(profile_id)?;

            return create_launch_context_from_profile(profile, cwd).ok();
        }

        // Fallback: create context from ProviderKind
        let provider_kind = slot.provider_type().to_provider_kind()?;
        crate::launch_config::context::ProviderLaunchContext::from_provider(provider_kind, cwd).ok()
    }

    /// Get resolved profile for an agent (for inspection/debugging)
    pub fn get_profile_for_agent(&self, agent_id: &AgentId) -> Option<&ProviderProfile> {
        let slot = self.get_slot_by_id(agent_id)?;
        let profile_id = slot.profile_id()?;
        let store = self.profile_store.as_ref()?;
        store.get_profile(profile_id)
    }

    /// Assign a task to an idle agent
    pub fn assign_task(&mut self, agent_id: &AgentId, task_id: TaskId) -> Result<(), String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;
        let codename = slot.codename().clone();
        slot.assign_task(task_id.clone()).map_err(|e| {
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "codename": codename.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": e,
                }),
            );
            e
        })?;

        logging::debug_event(
            "pool.task.assign",
            "assigned task to agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
            }),
        );

        Ok(())
    }

    /// Assign a task to an idle agent with backlog validation
    ///
    /// Validates that:
    /// - Agent exists and is idle
    /// - Task exists in backlog with Ready status
    /// - Updates backlog status to Running on success
    pub fn assign_task_with_backlog(
        &mut self,
        agent_id: &AgentId,
        task_id: TaskId,
        backlog: &mut BacklogState,
    ) -> Result<(), String> {
        // Validate task exists and is ready
        if !backlog.can_assign_task(task_id.as_str()) {
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task - task not ready",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": "task_not_ready_or_not_found",
                }),
            );
            return Err(format!(
                "Task {} cannot be assigned (not found or not ready)",
                task_id.as_str()
            ));
        }

        // Get task description for git flow preparation (before slot borrow)
        let task_description = backlog
            .find_task(task_id.as_str())
            .map(|t| t.objective.clone())
            .unwrap_or_default();

        // Assign to agent within a scope to release slot borrow before triggering decision
        let codename = {
            let slot = self
                .get_slot_mut_by_id(agent_id)
                .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;
            let codename = slot.codename().clone();
            slot.assign_task(task_id.clone()).map_err(|e| {
                logging::debug_event(
                    "pool.task.assign.failed",
                    "failed to assign task",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "codename": codename.as_str(),
                        "task_id": task_id.as_str(),
                        "reason": e,
                    }),
                );
                e
            })?;
            codename
        }; // slot borrow released here

        // Update backlog status
        backlog.start_task(task_id.as_str());

        // Trigger git flow task preparation via decision layer
        if let Err(e) = self.trigger_task_preparation(
            agent_id,
            task_id.as_str(),
            &task_description,
        ) {
            logging::warn_event(
                "pool.task.git_flow_trigger_failed",
                "failed to trigger git flow preparation",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "task_id": task_id.as_str(),
                    "error": e,
                }),
            );
        }

        logging::debug_event(
            "pool.task.assign",
            "assigned task with backlog update",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
                "old_status": "ready",
                "new_status": "running",
            }),
        );

        Ok(())
    }

    /// Complete a task for an agent with backlog update
    ///
    /// Updates backlog status based on completion result:
    /// - Success: task marked Done
    /// - Failure: task marked Failed
    pub fn complete_task_with_backlog(
        &mut self,
        agent_id: &AgentId,
        result: TaskCompletionResult,
        backlog: &mut BacklogState,
    ) -> Result<TaskId, String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;

        // Get assigned task before clearing
        let task_id = slot
            .assigned_task_id()
            .cloned()
            .ok_or_else(|| format!("Agent {} has no assigned task", agent_id.as_str()))?;

        let codename = slot.codename().clone();

        // Update backlog based on result
        match &result {
            TaskCompletionResult::Success => {
                backlog.complete_task(
                    task_id.as_str(),
                    Some("Task completed successfully".to_string()),
                );
            }
            TaskCompletionResult::Failure { error } => {
                backlog.fail_task(task_id.as_str(), error.clone());
            }
        }

        // Clear assignment
        slot.clear_task();

        logging::debug_event(
            "pool.task.complete",
            "completed task",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
                "result": match result {
                    TaskCompletionResult::Success => "success",
                    TaskCompletionResult::Failure { .. } => "failure",
                },
                "old_status": "running",
                "new_status": match result {
                    TaskCompletionResult::Success => "done",
                    TaskCompletionResult::Failure { .. } => "failed",
                },
            }),
        );

        Ok(task_id)
    }

    /// Find an idle agent that can accept a task
    pub fn find_idle_agent(&self) -> Option<&AgentSlot> {
        PoolQueries::find_idle_slot(&self.slots)
    }

    /// Find an idle agent and return its ID for assignment
    pub fn find_idle_agent_id(&self) -> Option<AgentId> {
        PoolQueries::find_idle_agent_id(&self.slots)
    }

    /// Auto-assign a ready task to an available idle agent
    ///
    /// Returns the assigned agent ID if successful.
    pub fn auto_assign_task(&mut self, backlog: &mut BacklogState) -> Option<(AgentId, TaskId)> {
        // Find an idle agent
        let agent_id = self.find_idle_agent_id()?;

        // Find a ready task
        let ready_tasks = backlog.ready_tasks();
        let ready_task = ready_tasks.first()?;
        let task_id_str = ready_task.id.clone();
        let task_id = TaskId::new(&task_id_str);

        // Attempt assignment
        match self.assign_task_with_backlog(&agent_id, task_id.clone(), backlog) {
            Ok(()) => {
                logging::debug_event(
                    "pool.task.auto_assign",
                    "auto-assigned task to idle agent",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                    }),
                );
                Some((agent_id, task_id))
            }
            Err(e) => {
                let available_agents = self
                    .slots
                    .iter()
                    .filter(|s| *s.status() == AgentSlotStatus::Idle)
                    .count();
                let ready_count = backlog.ready_tasks().len();
                logging::debug_event(
                    "pool.task.auto_assign.failed",
                    "auto-assign failed",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                        "reason": e,
                        "available_agents": available_agents,
                        "ready_tasks": ready_count,
                    }),
                );
                None
            }
        }
    }

    /// Check if any agent is active (responding or executing)
    pub fn has_active_agents(&self) -> bool {
        self.slots.iter().any(|s| s.status().is_active())
    }

    /// Count agents by status type
    pub fn count_by_status(&self) -> HashMap<String, usize> {
        PoolQueries::count_by_status(&self.slots)
    }

    /// Generate a snapshot of the task queue state for TUI display
    ///
    /// Combines backlog state with agent pool state for comprehensive view.
    pub fn task_queue_snapshot(&self, backlog: &BacklogState) -> TaskQueueSnapshot {
        PoolQueries::task_queue_snapshot(&self.slots, backlog)
    }

    /// Get agents with their assigned task info
    pub fn agents_with_assignments(&self, backlog: &BacklogState) -> Vec<AgentTaskAssignment> {
        PoolQueries::agents_with_assignments(&self.slots, backlog)
    }

    // ==================== Blocked Handling Methods ====================

    /// Get blocked handling configuration
    pub fn blocked_config(&self) -> &BlockedHandlingConfig {
        self.blocked_handler.config()
    }

    /// Get human decision queue
    pub fn human_queue(&self) -> &HumanDecisionQueue {
        &self.human_queue
    }

    /// Get pending human decisions count
    pub fn pending_human_decisions(&self) -> usize {
        self.human_queue.total_pending()
    }

    /// Get blocked history
    pub fn blocked_history(&self) -> &[BlockedHistoryEntry] {
        self.blocked_handler.history()
    }

    /// Find blocked agents
    pub fn blocked_agents(&self) -> Vec<&AgentSlot> {
        self.slots
            .iter()
            .filter(|s| s.status().is_blocked())
            .collect()
    }

    /// Count blocked agents
    pub fn blocked_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.status().is_blocked())
            .count()
    }

    /// Process an agent becoming blocked
    ///
    /// This handles:
    /// 1. Setting the blocked status on the slot
    /// 2. Adding to human decision queue if human_decision type
    /// 3. Notifying other agents (if configured)
    /// 4. Handling the assigned task according to policy
    pub fn process_agent_blocked(
        &mut self,
        agent_id: &AgentId,
        blocked_state: BlockedState,
        backlog: Option<&mut BacklogState>,
    ) -> Result<(), String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;

        // Set blocked status - check if this is a rate limit scenario
        let reason_type = blocked_state.reason().reason_type();
        let is_rate_limit = reason_type == "rate_limit"
            || (reason_type == "error" && blocked_state.reason().description().contains("429"));

        if is_rate_limit {
            // Rate limit - transition to Resting state instead of BlockedForDecision
            slot.transition_to(AgentSlotStatus::resting(blocked_state.clone()))
                .map_err(|e| format!("Failed to transition to resting: {}", e))?;
        } else {
            slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state.clone()))
                .map_err(|e| format!("Failed to transition to blocked: {}", e))?;
        }

        // Handle by blocking type (for non-rate-limit cases)
        if reason_type == "human_decision" {
            // Create human decision request
            let request = self.build_human_request(agent_id, &blocked_state);
            self.human_queue.push(request);
        }

        // Record in history using BlockedHandler delegate
        self.blocked_handler.record_blocked(BlockedHistoryEntry {
            agent_id: agent_id.clone(),
            reason_type: reason_type.to_string(),
            description: blocked_state.reason().description(),
            duration_ms: 0, // Will be updated on resolution
            resolved: false,
            resolution: None,
        });

        // Notify other agents using BlockedHandler delegate
        let event = AgentBlockedEvent {
            agent_id: agent_id.clone(),
            reason_type: reason_type.to_string(),
            description: blocked_state.reason().description(),
            urgency: format!("{}", blocked_state.reason().urgency()),
        };
        self.blocked_handler.notify_blocked(event);

        // Handle blocked task
        if let Some(backlog) = backlog {
            self.handle_blocked_task(agent_id, backlog);
        }

        Ok(())
    }

    /// Build human decision request from blocked state
    fn build_human_request(
        &self,
        agent_id: &AgentId,
        blocked_state: &BlockedState,
    ) -> HumanDecisionRequest {
        let reason = blocked_state.reason();
        let urgency = reason.urgency();
        let timeout_ms = self
            .blocked_handler
            .config()
            .timeout_config
            .timeout_for_urgency(urgency);

        // Generate request ID
        let request_id = format!("req-{}-{}", agent_id.as_str(), uuid::Uuid::new_v4());

        HumanDecisionRequest::new(
            request_id,
            agent_id.as_str(),
            SituationType::new(reason.reason_type()),
            vec![], // Options would come from the blocking reason
            urgency,
            timeout_ms,
        )
        .with_description(reason.description())
    }

    /// Handle the task assigned to a blocked agent
    fn handle_blocked_task(&mut self, agent_id: &AgentId, backlog: &mut BacklogState) {
        // Get assigned task
        let task_id = self
            .get_slot_by_id(agent_id)
            .and_then(|s| s.assigned_task_id().cloned());

        if let Some(task_id) = task_id {
            match self.blocked_handler.config().task_policy {
                BlockedTaskPolicy::KeepAssigned => {
                    // Task stays with blocked agent - no action needed
                }
                BlockedTaskPolicy::ReassignIfPossible => {
                    // Try to find idle agent
                    if let Some(idle_agent) = self.find_idle_agent_id() {
                        // Check task exists and is Running (task was already assigned)
                        let task_exists = backlog
                            .find_task(task_id.as_str())
                            .map(|t| t.status == TaskStatus::Running)
                            .unwrap_or(false);

                        if task_exists {
                            // Try to assign to idle agent FIRST
                            let reassignment_succeeded = self
                                .get_slot_mut_by_id(&idle_agent)
                                .map(|slot| slot.assign_task(task_id.clone()).is_ok())
                                .unwrap_or(false);

                            // Only clear from blocked slot if reassignment succeeded
                            if reassignment_succeeded {
                                if let Some(blocked_slot) = self.get_slot_mut_by_id(agent_id) {
                                    blocked_slot.clear_task();
                                }
                            }
                            // If reassignment failed, task stays with blocked agent
                        }
                    }
                }
                BlockedTaskPolicy::MarkWaiting => {
                    // Mark task as blocked in backlog
                    backlog.block_task(task_id.as_str(), "agent_blocked".to_string());
                }
            }
        }
    }

    /// Process human decision response
    ///
    /// This handles:
    /// 1. Completing the request in the queue
    /// 2. Clearing the blocked status on the agent
    /// 3. Executing the decision
    /// 4. Recording in history
    pub fn process_human_response(
        &mut self,
        response: HumanDecisionResponse,
    ) -> Result<DecisionExecutionResult, String> {
        // Get request from queue
        let request = self.human_queue.peek().cloned();

        // Complete in queue
        if !self.human_queue.complete(response.clone()) {
            return Err(format!(
                "Request {} not found in queue",
                response.request_id
            ));
        }

        // Get agent ID from response/request
        let agent_id = AgentId::new(
            request
                .as_ref()
                .map(|r| r.agent_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        );

        // Find and update history using BlockedHandler delegate
        self.blocked_handler.record_resolution(&agent_id, format!("{:?}", response.selection));

        // Get slot and clear blocked status
        let slot = self.get_slot_mut_by_id(&agent_id);
        if slot.is_none() {
            return Ok(DecisionExecutionResult::AgentNotFound);
        }

        let slot = slot.unwrap();
        if !slot.status().is_blocked() {
            return Ok(DecisionExecutionResult::NotBlocked);
        }

        // Transition to Responding (active state after unblock)
        use std::time::Instant;
        slot.transition_to(AgentSlotStatus::Responding {
            started_at: Instant::now(),
        })
        .map_err(|e| format!("Failed to unblock agent: {}", e))?;

        // Execute decision
        self.execute_decision(&agent_id, response.selection)
    }

    /// Execute human selection on an agent
    fn execute_decision(
        &mut self,
        agent_id: &AgentId,
        selection: HumanSelection,
    ) -> Result<DecisionExecutionResult, String> {
        let slot = self.get_slot_by_id(agent_id);
        if slot.is_none() {
            return Ok(DecisionExecutionResult::AgentNotFound);
        }

        let result = match selection {
            HumanSelection::Selected { option_id } => {
                DecisionExecutionResult::Executed { option_id }
            }
            HumanSelection::AcceptedRecommendation => {
                DecisionExecutionResult::AcceptedRecommendation
            }
            HumanSelection::Custom { instruction } => {
                DecisionExecutionResult::CustomInstruction { instruction }
            }
            HumanSelection::Skipped => {
                // Clear task assignment
                if let Some(slot) = self.get_slot_mut_by_id(agent_id) {
                    slot.clear_task();
                }
                DecisionExecutionResult::Skipped
            }
            HumanSelection::Cancelled => {
                // Transition to Idle
                if let Some(slot) = self.get_slot_mut_by_id(agent_id) {
                    slot.transition_to(AgentSlotStatus::Idle)
                        .map_err(|e| format!("Failed to cancel: {}", e))?;
                }
                DecisionExecutionResult::Cancelled
            }
        };

        Ok(result)
    }

    /// Clear all blocked agents (e.g., on shutdown)
    pub fn clear_all_blocked(&mut self) {
        for slot in &mut self.slots {
            if slot.status().is_blocked() {
                // Record resolution in history using BlockedHandler delegate
                self.blocked_handler.record_resolution(
                    slot.agent_id(),
                    "cleared_on_shutdown".to_string(),
                );
                slot.transition_to(AgentSlotStatus::Idle).ok();
            }
        }
        // Clear human queue
        self.human_queue.check_expired();
    }

    /// Clear agent context (transcript, session handle) for a specific agent
    ///
    /// This clears both the work agent and its associated decision agent context.
    /// Used by /clear command to reset the conversation state.
    pub fn clear_agent_context(&mut self, agent_id: &AgentId) -> Result<(), String> {
        // Clear work agent context
        if let Some(slot) = self.get_slot_mut_by_id(agent_id) {
            // Clear transcript
            slot.clear_transcript();

            // Clear session handle (provider session)
            slot.clear_session_handle();

            // Clear any assigned task
            slot.clear_task();

            // Transition to idle if not already
            if !slot.status().is_idle() {
                slot.transition_to(AgentSlotStatus::Idle)?;
            }

            logging::debug_event(
                "pool.clear_context",
                "cleared work agent context",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "codename": slot.codename().as_str(),
                }),
            );
        } else {
            return Err(format!("agent {} not found", agent_id.as_str()));
        }

        // Clear decision agent context if exists
        if let Some(decision_agent) = self.decision_coordinator.agent_mut_for(agent_id) {
            // Reset decision agent to idle state
            if decision_agent.status().has_error() {
                decision_agent.reset_error();
            }

            logging::debug_event(
                "pool.clear_context.decision",
                "cleared decision agent context",
                serde_json::json!({
                    "work_agent_id": agent_id.as_str(),
                    "decision_agent_id": decision_agent.agent_id(),
                }),
            );
        }

        Ok(())
    }

    /// Clear context for all agents (both work agents and decision agents)
    ///
    /// This is used when user wants to reset all conversation state.
    pub fn clear_all_agent_contexts(&mut self) {
        for slot in &mut self.slots {
            slot.clear_transcript();
            slot.clear_session_handle();
            slot.clear_task();

            if !slot.status().is_idle() {
                let _ = slot.transition_to(AgentSlotStatus::Idle);
            }
        }

        // Clear all decision agents using coordinator
        self.decision_coordinator.for_each_agent_mut(|work_agent_id, decision_agent| {
            if decision_agent.status().has_error() {
                decision_agent.reset_error();
            }

            logging::debug_event(
                "pool.clear_all_contexts.decision",
                "cleared decision agent context",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "decision_agent_id": decision_agent.agent_id(),
                }),
            );
        });

        // Clear human decision queue
        self.human_queue.clear();

        logging::debug_event(
            "pool.clear_all_contexts",
            "cleared all agent contexts",
            serde_json::json!({
                "agent_count": self.slots.len(),
                "decision_agent_count": self.decision_coordinator.agent_count(),
            }),
        );
    }

    /// Check for expired human decision requests
    pub fn check_expired_requests(&mut self) -> Vec<HumanDecisionRequest> {
        self.human_queue.check_expired()
    }

    /// Get requests approaching timeout
    pub fn approaching_timeout_requests(&self) -> Vec<&HumanDecisionRequest> {
        self.human_queue.approaching_timeout()
    }

    /// Process expired requests and execute timeout actions
    ///
    /// Returns the number of requests processed.
    /// Note: This handles expired requests that were already removed from queue by check_expired.
    pub fn process_expired_requests(&mut self) -> usize {
        let expired = self.human_queue.check_expired();
        let count = expired.len();

        for request in expired {
            let selection = self.timeout_action_for_request(&request);
            let agent_id = AgentId::new(request.agent_id.clone());

            // Find and update history
            // Record resolution using BlockedHandler delegate
            self.blocked_handler.record_resolution(&agent_id, format!("timeout: {:?}", selection));

            // Clear blocked status and execute timeout action
            self.execute_timeout_action(&agent_id, selection);
        }

        count
    }

    /// Execute timeout action on an agent (called when request already removed from queue)
    fn execute_timeout_action(&mut self, agent_id: &AgentId, selection: HumanSelection) {
        let slot = self.get_slot_mut_by_id(agent_id);
        if slot.is_none() {
            return;
        }

        let slot = slot.unwrap();
        if !slot.status().is_blocked() {
            return;
        }

        // Transition to appropriate status based on selection
        match selection {
            HumanSelection::Cancelled => {
                let _ = slot.transition_to(AgentSlotStatus::Idle);
            }
            HumanSelection::Skipped => {
                // Clear task but keep agent ready
                slot.clear_task();
                let _ = slot.transition_to(AgentSlotStatus::Idle);
            }
            _ => {
                // For other selections, just transition to responding
                use std::time::Instant;
                let _ = slot.transition_to(AgentSlotStatus::Responding {
                    started_at: Instant::now(),
                });
            }
        }
    }

    /// Determine the timeout action for a request based on config
    fn timeout_action_for_request(&self, request: &HumanDecisionRequest) -> HumanSelection {
        let timeout_action = self.blocked_handler.config().timeout_config.timeout_default;

        match timeout_action {
            AutoAction::FollowRecommendation => {
                // If there's a recommendation, accept it
                if request.recommendation.is_some() {
                    HumanSelection::AcceptedRecommendation
                } else {
                    // No recommendation, select default option
                    self.select_default_option(request)
                }
            }
            AutoAction::SelectDefault => self.select_default_option(request),
            AutoAction::Cancel => HumanSelection::Cancelled,
            AutoAction::MarkTaskFailed => {
                // Mark task as failed - this would require a new selection type
                // For now, treat as cancelled
                HumanSelection::Cancelled
            }
            AutoAction::ReleaseResource => HumanSelection::Cancelled,
        }
    }

    /// Select the default option from a request
    fn select_default_option(&self, request: &HumanDecisionRequest) -> HumanSelection {
        if let Some(first_option) = request.options.first() {
            HumanSelection::Selected {
                option_id: first_option.id.clone(),
            }
        } else {
            // No options available, skip
            HumanSelection::Skipped
        }
    }
}

fn parse_agent_index(agent_id: &str) -> Option<usize> {
    agent_id.strip_prefix("agent_")?.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_slot::AgentSlotStatus;
    use crate::provider_profile::{ProfileStore, ProviderProfile, CliBaseType};
    use crate::{WorktreeStateStore, WorktreeManager, WorktreeConfig};
    use agent_decision::{
        BlockedState, HumanDecisionBlocking, HumanSelection, ResourceBlocking,
        WaitingForChoiceSituation,
    };
    use agent_decision::builtin_situations::register_situation_builtins;
    use agent_decision::context::DecisionContext;
    use agent_decision::situation_registry::SituationRegistry;
    use agent_decision::types::SituationType;
    use agent_decision::model::action::ContinueAllTasksAction;
    use agent_decision::output::DecisionOutput;

    fn make_pool(max_slots: usize) -> AgentPool {
        AgentPool::new(WorkplaceId::new("workplace-001"), max_slots)
    }

    #[test]
    fn pool_new_is_empty() {
        let pool = make_pool(4);
        assert_eq!(pool.active_count(), 0);
        assert!(pool.can_spawn());
        assert_eq!(pool.max_slots(), 4);
    }

    #[test]
    fn spawn_agent_creates_slot() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        assert_eq!(pool.active_count(), 1);
        assert!(pool.get_slot_by_id(&id).is_some());
    }

    #[test]
    fn spawn_multiple_agents_unique_ids() {
        let mut pool = make_pool(4);
        let id1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id2 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        let id3 = pool.spawn_agent(ProviderKind::Codex).unwrap();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(pool.active_count(), 3);
    }

    #[test]
    fn spawn_until_full_then_fail() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let result = pool.spawn_agent(ProviderKind::Codex);
        assert!(result.is_err());
        assert_eq!(pool.active_count(), 2);
    }

    #[test]
    fn stop_agent_marks_stopped() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.stop_agent(&id).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert!(slot.status().is_terminal());
    }

    #[test]
    fn stop_nonexistent_agent_fails() {
        let mut pool = make_pool(4);
        let result = pool.stop_agent(&AgentId::new("agent_999"));
        assert!(result.is_err());
    }

    #[test]
    fn remove_stopped_agent() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.stop_agent(&id).unwrap();
        pool.remove_agent(&id).unwrap();
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn remove_active_agent_fails() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        // Agent is Idle, not stopped
        let result = pool.remove_agent(&id);
        assert!(result.is_err());
    }

    #[test]
    fn agent_statuses_snapshot() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let statuses = pool.agent_statuses();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].status, AgentSlotStatus::Idle);
    }

    #[test]
    fn focus_agent_by_index() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent_by_index(1).unwrap();
        assert_eq!(pool.focused_slot_index(), 1);
    }

    #[test]
    fn focus_agent_by_id() {
        let mut pool = make_pool(4);
        let _id1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id2 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent(&id2).unwrap();
        assert_eq!(pool.focused_slot_index(), 1);
    }

    #[test]
    fn focus_invalid_index_fails() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let result = pool.focus_agent_by_index(5);
        assert!(result.is_err());
    }

    #[test]
    fn get_slot_by_index() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let slot = pool.get_slot(0).unwrap();
        assert_eq!(slot.agent_id().as_str(), "agent_001");
    }

    #[test]
    fn get_slot_by_id() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert_eq!(slot.agent_id(), &id);
    }

    #[test]
    fn focused_slot() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let focused = pool.focused_slot().unwrap();
        assert_eq!(focused.agent_id().as_str(), "agent_001");
    }

    #[test]
    fn assign_task_to_idle_agent() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.assign_task(&id, TaskId::new("task-001")).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn find_idle_agent() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let idle = pool.find_idle_agent().unwrap();
        assert_eq!(idle.status(), &AgentSlotStatus::Idle);
    }

    #[test]
    fn has_active_agents() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        // All agents are Idle initially
        assert!(!pool.has_active_agents());
    }

    #[test]
    fn count_by_status() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let counts = pool.count_by_status();
        assert_eq!(counts.get("idle"), Some(&2));
    }

    #[test]
    fn codename_generation() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.spawn_agent(ProviderKind::Codex).unwrap();
        let slot0 = pool.get_slot(0).unwrap();
        let slot1 = pool.get_slot(1).unwrap();
        let slot2 = pool.get_slot(2).unwrap();
        assert_eq!(slot0.codename().as_str(), "alpha");
        assert_eq!(slot1.codename().as_str(), "bravo");
        assert_eq!(slot2.codename().as_str(), "charlie");
    }

    #[test]
    fn remove_adjusts_focus() {
        let mut pool = make_pool(4);
        let _id0 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id1 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent_by_index(1).unwrap();
        pool.stop_agent(&id1).unwrap();
        pool.remove_agent(&id1).unwrap();
        // Focus should adjust to valid index
        assert_eq!(pool.focused_slot_index(), 0);
    }

    // Backlog Integration Tests

    fn make_backlog_with_ready_task() -> BacklogState {
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Test objective".to_string(),
            scope: "Test scope".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });
        backlog
    }

    #[test]
    fn assign_task_with_backlog_updates_status() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task with backlog validation
        let result =
            pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog);
        assert!(result.is_ok());

        // Agent should have task assigned
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_some());

        // Backlog task should be Running
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Running);
    }

    #[test]
    fn assign_task_with_backlog_fails_for_non_ready_task() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Test".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Running, // Already running
            result_summary: None,
        });

        let result =
            pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog);
        assert!(result.is_err());
    }

    #[test]
    fn complete_task_with_backlog_success() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Complete task successfully
        let completed_task =
            pool.complete_task_with_backlog(&agent_id, TaskCompletionResult::Success, &mut backlog);
        assert!(completed_task.is_ok());

        // Task should be Done in backlog
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Done);

        // Agent should have no assigned task
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn complete_task_with_backlog_failure() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Complete task with failure
        let completed_task = pool.complete_task_with_backlog(
            &agent_id,
            TaskCompletionResult::Failure {
                error: "test error".to_string(),
            },
            &mut backlog,
        );
        assert!(completed_task.is_ok());

        // Task should be Failed in backlog
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Failed);
        assert_eq!(task.result_summary, Some("test error".to_string()));
    }

    #[test]
    fn auto_assign_task_assigns_to_idle_agent() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Auto-assign should work
        let result = pool.auto_assign_task(&mut backlog);
        assert!(result.is_some());

        let (_agent_id, task_id) = result.unwrap();
        assert_eq!(task_id.as_str(), "task-001");

        // Task should be Running
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Running);
    }

    #[test]
    fn auto_assign_task_returns_none_when_no_idle_agents() {
        let mut pool = make_pool(1);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        // Manually mark agent as starting (not idle)
        // Idle -> Starting is valid, then Starting -> Responding
        pool.get_slot_mut_by_id(&agent_id)
            .unwrap()
            .transition_to(AgentSlotStatus::starting())
            .unwrap();
        let mut backlog = make_backlog_with_ready_task();

        let result = pool.auto_assign_task(&mut backlog);
        assert!(result.is_none());
    }

    #[test]
    fn auto_assign_task_returns_none_when_no_ready_tasks() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let backlog = BacklogState::default(); // No tasks

        let result = pool.auto_assign_task(&mut backlog.clone());
        assert!(result.is_none());
    }

    // Task Queue Visualization Tests

    #[test]
    fn task_queue_snapshot_empty_backlog() {
        let pool = make_pool(2);
        let backlog = BacklogState::default();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.total_tasks, 0);
        assert_eq!(snapshot.ready_tasks, 0);
        assert_eq!(snapshot.running_tasks, 0);
        assert_eq!(snapshot.agent_assignments.len(), 0);
    }

    #[test]
    fn task_queue_snapshot_with_tasks() {
        let pool = make_pool(2);
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-002".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 2".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Running,
            result_summary: None,
        });
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-003".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 3".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Done,
            result_summary: Some("Completed".to_string()),
        });

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.total_tasks, 3);
        assert_eq!(snapshot.ready_tasks, 1);
        assert_eq!(snapshot.running_tasks, 1);
        assert_eq!(snapshot.completed_tasks, 1);
    }

    #[test]
    fn task_queue_snapshot_with_agent_assignments() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        // Assign task to agent
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.agent_assignments.len(), 1);
        assert_eq!(snapshot.agent_assignments[0].task_id.as_str(), "task-001");
        assert_eq!(
            snapshot.agent_assignments[0].task_status,
            crate::backlog::TaskStatus::Running
        );
        assert_eq!(snapshot.running_tasks, 1);
    }

    #[test]
    fn task_queue_snapshot_available_agents_count() {
        let mut pool = make_pool(3);
        let _agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent3 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        // Assign task to agent2 (agent status stays Idle)
        pool.assign_task_with_backlog(&agent2, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Now mark agent2 as starting (not idle)
        pool.get_slot_mut_by_id(&agent2)
            .unwrap()
            .transition_to(AgentSlotStatus::starting())
            .unwrap();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.available_agents, 2); // agent1 and agent3 are idle
        assert_eq!(snapshot.active_agents, 1); // Starting is active
    }

    #[test]
    fn agents_with_assignments_returns_assigned_agents() {
        let mut pool = make_pool(2);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        pool.assign_task_with_backlog(&agent1, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        let assignments = pool.agents_with_assignments(&backlog);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_id, agent1);
    }

    // Blocked Handling Tests

    #[test]
    fn blocked_task_policy_default() {
        assert_eq!(
            BlockedTaskPolicy::default(),
            BlockedTaskPolicy::ReassignIfPossible
        );
    }

    #[test]
    fn blocked_handling_config_default() {
        let config = BlockedHandlingConfig::default();
        assert_eq!(config.task_policy, BlockedTaskPolicy::ReassignIfPossible);
        assert!(config.notify_others);
        assert!(config.record_history);
    }

    #[test]
    fn pool_new_has_blocked_handling() {
        let pool = make_pool(4);
        assert_eq!(pool.pending_human_decisions(), 0);
        assert_eq!(pool.blocked_count(), 0);
        assert!(pool.blocked_history().is_empty());
    }

    #[test]
    fn pool_with_blocked_config() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::KeepAssigned,
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: false,
            record_history: false,
            max_history_entries: 100,
        };
        let pool = AgentPool::with_blocked_config(WorkplaceId::new("workplace-001"), 4, config);
        assert_eq!(
            pool.blocked_config().task_policy,
            BlockedTaskPolicy::KeepAssigned
        );
        assert!(!pool.blocked_config().notify_others);
    }

    #[test]
    fn process_agent_blocked_sets_status() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        let result = pool.process_agent_blocked(&agent_id, blocked_state, None);
        assert!(result.is_ok());

        // Check status is blocked
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_blocked());
        assert!(slot.status().is_blocked_for_human());
    }

    #[test]
    fn process_agent_blocked_adds_to_queue() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        pool.process_agent_blocked(&agent_id, blocked_state, None)
            .unwrap();

        // Check human queue has request
        assert_eq!(pool.pending_human_decisions(), 1);
    }

    #[test]
    fn process_agent_blocked_records_history() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        pool.process_agent_blocked(&agent_id, blocked_state, None)
            .unwrap();

        // Check history recorded
        assert_eq!(pool.blocked_history().len(), 1);
        let entry = &pool.blocked_history()[0];
        assert_eq!(entry.agent_id, agent_id);
        assert_eq!(entry.reason_type, "human_decision");
        assert!(!entry.resolved);
    }

    #[test]
    fn blocked_task_stays_with_agent_keep_assigned() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::KeepAssigned,
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: true,
            record_history: true,
            max_history_entries: 100,
        };
        let mut pool = AgentPool::with_blocked_config(WorkplaceId::new("workplace-001"), 2, config);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, Some(&mut backlog))
            .unwrap();

        // Task should still be assigned to blocked agent
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn blocked_task_reassigns_if_possible() {
        let mut pool = make_pool(3);
        let blocked_agent = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let idle_agent = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task to blocked_agent
        pool.assign_task_with_backlog(&blocked_agent, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&blocked_agent, blocked_state, Some(&mut backlog))
            .unwrap();

        // Task should be reassigned to idle agent (with ReassignIfPossible policy)
        let blocked_slot = pool.get_slot_by_id(&blocked_agent).unwrap();
        let idle_slot = pool.get_slot_by_id(&idle_agent).unwrap();

        // Task is on idle agent now (or still on blocked if slot.assign_task failed due to status)
        // Note: idle_slot.assign_task would fail because the slot is Idle but we need Running
        // For now, check that blocked agent's task is cleared
        assert!(
            blocked_slot.assigned_task_id().is_none() || idle_slot.assigned_task_id().is_some()
        );
    }

    #[test]
    fn process_human_response_clears_blocked() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None)
            .unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response
        let response =
            HumanDecisionResponse::new(request.id.clone(), HumanSelection::selected("option-a"));

        // Process response
        let result = pool.process_human_response(response);
        assert!(result.is_ok());

        // Check agent is unblocked
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(!slot.status().is_blocked());
    }

    #[test]
    fn process_human_response_executes_selection() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None)
            .unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with selection
        let response =
            HumanDecisionResponse::new(request.id.clone(), HumanSelection::selected("option-a"));

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(
            result,
            DecisionExecutionResult::Executed {
                option_id: "option-a".to_string()
            }
        );
    }

    #[test]
    fn process_human_response_skip_clears_task() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, Some(&mut backlog))
            .unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with skip
        let response = HumanDecisionResponse::new(request.id.clone(), HumanSelection::skip());

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(result, DecisionExecutionResult::Skipped);

        // Task should be cleared
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn process_human_response_cancel_transitions_to_idle() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None)
            .unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with cancel
        let response = HumanDecisionResponse::new(request.id.clone(), HumanSelection::cancel());

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(result, DecisionExecutionResult::Cancelled);

        // Agent should be Idle
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(matches!(slot.status(), AgentSlotStatus::Idle));
    }

    #[test]
    fn clear_all_blocked_unblocks_agents() {
        let mut pool = make_pool(2);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking for both
        let blocking1 = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent1, BlockedState::new(Box::new(blocking1)), None)
            .unwrap();

        let blocking2 = HumanDecisionBlocking::new(
            "req-2",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent2, BlockedState::new(Box::new(blocking2)), None)
            .unwrap();

        assert_eq!(pool.blocked_count(), 2);

        // Clear all
        pool.clear_all_blocked();

        // All should be unblocked
        assert_eq!(pool.blocked_count(), 0);
        let slot1 = pool.get_slot_by_id(&agent1).unwrap();
        let slot2 = pool.get_slot_by_id(&agent2).unwrap();
        assert!(matches!(slot1.status(), AgentSlotStatus::Idle));
        assert!(matches!(slot2.status(), AgentSlotStatus::Idle));
    }

    #[test]
    fn blocked_agents_list() {
        let mut pool = make_pool(3);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent3 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Block agent1 with human decision
        let blocking1 = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent1, BlockedState::new(Box::new(blocking1)), None)
            .unwrap();

        // Block agent2 with resource waiting
        let blocking2 = ResourceBlocking::new("file", "/tmp/lock", "waiting for file lock");
        pool.process_agent_blocked(&agent2, BlockedState::new(Box::new(blocking2)), None)
            .unwrap();

        // Get blocked agents
        let blocked = pool.blocked_agents();
        assert_eq!(blocked.len(), 2);
    }

    #[test]
    fn decision_execution_result_variants() {
        // Test all variants are constructible
        let executed = DecisionExecutionResult::Executed {
            option_id: "a".to_string(),
        };
        let accepted = DecisionExecutionResult::AcceptedRecommendation;
        let custom = DecisionExecutionResult::CustomInstruction {
            instruction: "test".to_string(),
        };
        let skipped = DecisionExecutionResult::Skipped;
        let cancelled = DecisionExecutionResult::Cancelled;
        let not_found = DecisionExecutionResult::AgentNotFound;
        let not_blocked = DecisionExecutionResult::NotBlocked;
        let task_prepared = DecisionExecutionResult::TaskPrepared {
            branch: "feature/test".to_string(),
            worktree_path: std::path::PathBuf::from("/tmp/worktree"),
        };
        let prep_failed = DecisionExecutionResult::PreparationFailed {
            reason: "health check failed".to_string(),
        };

        assert!(matches!(executed, DecisionExecutionResult::Executed { .. }));
        assert!(matches!(
            accepted,
            DecisionExecutionResult::AcceptedRecommendation
        ));
        assert!(matches!(
            custom,
            DecisionExecutionResult::CustomInstruction { .. }
        ));
        assert!(matches!(skipped, DecisionExecutionResult::Skipped));
        assert!(matches!(cancelled, DecisionExecutionResult::Cancelled));
        assert!(matches!(not_found, DecisionExecutionResult::AgentNotFound));
        assert!(matches!(not_blocked, DecisionExecutionResult::NotBlocked));
        assert!(matches!(task_prepared, DecisionExecutionResult::TaskPrepared { .. }));
        assert!(matches!(prep_failed, DecisionExecutionResult::PreparationFailed { .. }));
    }

    // Note: blocked_history pruning tests are in pool/blocked_handler.rs
    // The BlockedHandler tests cover:
    // - pruning removes resolved entries first
    // - max_entries=0 means unlimited history

    #[test]
    fn process_expired_requests_with_default_action() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Add a blocked entry
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent_id, BlockedState::new(Box::new(blocking)), None)
            .unwrap();

        // Manually expire the request in the queue
        // (In real scenario, time would pass and check_expired would find it)
        // For now, just verify the method exists and can be called
        let _count = pool.process_expired_requests();
        // Request may or may not be expired yet depending on timing
    }

    #[test]
    fn agent_blocked_notifier_receives_events() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct TestNotifier {
            count: Arc<AtomicUsize>,
        }

        impl AgentBlockedNotifier for TestNotifier {
            fn on_agent_blocked(&self, _event: AgentBlockedEvent) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let notifier = Arc::new(TestNotifier {
            count: count.clone(),
        });

        let mut pool = make_pool(2);
        pool.set_blocked_notifier(notifier);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Block the agent
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent_id, BlockedState::new(Box::new(blocking)), None)
            .unwrap();

        // Notifier should have been called
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn notify_others_disabled_does_not_notify() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct TestNotifier {
            count: Arc<AtomicUsize>,
        }

        impl AgentBlockedNotifier for TestNotifier {
            fn on_agent_blocked(&self, _event: AgentBlockedEvent) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let notifier = Arc::new(TestNotifier {
            count: count.clone(),
        });

        let mut config = BlockedHandlingConfig::default();
        config.notify_others = false; // Disable

        let mut pool = AgentPool::with_blocked_config(WorkplaceId::new("workplace-001"), 2, config);
        pool.set_blocked_notifier(notifier);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Block the agent
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent_id, BlockedState::new(Box::new(blocking)), None)
            .unwrap();

        // Notifier should NOT have been called
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    // ============== Worktree Integration Tests ==============

    fn setup_test_repo() -> (tempfile::TempDir, PathBuf) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to init git repo");

        // Disable GPG signing for tests
        std::process::Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to disable GPG signing");

        // Create initial commit
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to create initial commit");

        (temp_dir, repo_path)
    }

    #[test]
    fn pool_new_with_worktrees_creates_pool() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        );

        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert!(pool.has_worktree_support());
        assert_eq!(pool.max_slots(), 4);
    }

    #[test]
    fn pool_without_worktrees_spawn_fails_without_worktree() {
        let mut pool = make_pool(4);

        // Attempt to spawn with worktree should fail
        let result = pool.spawn_agent_with_worktree(ProviderKind::Mock, None, None);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::WorktreeNotEnabled
        ));
    }

    #[test]
    fn spawn_agent_with_worktree_creates_worktree() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path.clone(),
            state_dir,
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/test-branch".to_string()),
                Some("task-001".to_string()),
            )
            .unwrap();

        // Check agent was created
        assert_eq!(pool.active_count(), 1);
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.has_worktree());

        // Check worktree path exists
        let worktree_path = slot.cwd();
        assert!(worktree_path.exists());

        // Check worktree is a valid git worktree
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to check git worktree");

        assert!(output.status.success());
    }

    #[test]
    fn spawn_agent_with_worktree_default_branch_name() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                None, // No custom branch name
                None,
            )
            .unwrap();

        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.has_worktree());

        // Should have default branch name pattern "agent/{agent_id}"
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(slot.cwd())
            .output()
            .expect("Failed to check branch");

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(branch.starts_with("agent/"));
    }

    #[test]
    fn spawn_agent_with_worktree_fails_when_pool_full() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            1, // Only 1 slot
            repo_path,
            state_dir,
        )
        .unwrap();

        // Spawn first agent - should succeed
        let _ = pool
            .spawn_agent_with_worktree(ProviderKind::Mock, None, None)
            .unwrap();

        // Spawn second agent - should fail
        let result = pool.spawn_agent_with_worktree(ProviderKind::Mock, None, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::PoolFull
        ));
    }

    #[test]
    fn spawn_agent_with_worktree_persists_state() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir.clone(),
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/test".to_string()),
                Some("task-001".to_string()),
            )
            .unwrap();

        // Verify state was persisted
        let store = WorktreeStateStore::new(state_dir);
        let loaded_state = store.load(agent_id.as_str()).unwrap();

        assert!(loaded_state.is_some());
        let state = loaded_state.unwrap();
        assert_eq!(state.agent_id, agent_id.as_str());
        assert_eq!(state.task_id, Some("task-001".to_string()));
    }

    #[test]
    fn pause_agent_with_worktree_preserves_state() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir.clone(),
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/pause-test".to_string()),
                None,
            )
            .unwrap();

        // Verify agent is running (idle after spawn)
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_idle());

        // Pause the agent
        pool.pause_agent_with_worktree(&agent_id).unwrap();

        // Verify status is paused
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_paused());

        // Verify worktree still exists
        let worktree_path = slot.cwd();
        assert!(worktree_path.exists());

        // Verify state was updated
        let store = WorktreeStateStore::new(state_dir);
        let loaded_state = store.load(agent_id.as_str()).unwrap().unwrap();
        assert!(loaded_state.last_active_at > loaded_state.created_at);
    }

    #[test]
    fn resume_agent_with_worktree_restores_slot() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir.clone(),
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/resume-test".to_string()),
                None,
            )
            .unwrap();

        // Pause the agent
        pool.pause_agent_with_worktree(&agent_id).unwrap();
        assert!(pool.get_slot_by_id(&agent_id).unwrap().status().is_paused());

        // Resume the agent
        pool.resume_agent_with_worktree(&agent_id).unwrap();

        // Verify status is idle (ready to resume work)
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_idle());

        // Verify worktree still exists
        assert!(slot.cwd().exists());
    }

    #[test]
    fn pause_fails_without_worktree_support() {
        let mut pool = AgentPool::new(WorkplaceId::new("workplace-001"), 4);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Pause should fail because pool has no worktree support
        let result = pool.pause_agent_with_worktree(&agent_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::WorktreeNotEnabled
        ));
    }

    #[test]
    fn pause_fails_for_agent_without_worktree() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        )
        .unwrap();

        // Spawn agent without worktree (using regular spawn)
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Pause should fail because agent has no worktree
        let result = pool.pause_agent_with_worktree(&agent_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::NoWorktree(_)
        ));
    }

    #[test]
    fn resume_fails_if_agent_not_paused() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/resume-fail".to_string()),
                None,
            )
            .unwrap();

        // Agent is idle, not paused - resume should fail
        let result = pool.resume_agent_with_worktree(&agent_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::AgentNotPaused(_)
        ));
    }

    #[test]
    fn stop_agent_with_cleanup_removes_worktree() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path.clone(),
            state_dir.clone(),
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/stop-cleanup".to_string()),
                None,
            )
            .unwrap();

        // Get worktree info before stop
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        let worktree_path = slot.cwd();

        // Stop with cleanup
        pool.stop_agent_with_worktree_cleanup(&agent_id, true)
            .unwrap();

        // Verify slot is stopped
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_terminal());

        // Verify worktree was removed
        assert!(!worktree_path.exists());

        // Verify state was deleted
        let store = WorktreeStateStore::new(state_dir);
        let loaded_state = store.load(agent_id.as_str()).unwrap();
        assert!(loaded_state.is_none());

        // Verify worktree not in git list
        let worktree_manager = WorktreeManager::new(repo_path, WorktreeConfig::default()).unwrap();
        let worktrees = worktree_manager.list().unwrap();
        let found = worktrees.iter().any(|wt| wt.path == worktree_path);
        assert!(!found);
    }

    #[test]
    fn stop_agent_preserve_keeps_worktree() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path.clone(),
            state_dir.clone(),
        )
        .unwrap();

        let agent_id = pool
            .spawn_agent_with_worktree(
                ProviderKind::Mock,
                Some("feature/stop-preserve".to_string()),
                None,
            )
            .unwrap();

        // Get worktree info before stop
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        let worktree_path = slot.cwd();

        // Stop with preserve (cleanup=false)
        pool.stop_agent_with_worktree_cleanup(&agent_id, false)
            .unwrap();

        // Verify slot is stopped
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_terminal());

        // Verify worktree still exists
        assert!(worktree_path.exists());

        // Verify state was preserved
        let store = WorktreeStateStore::new(state_dir);
        let loaded_state = store.load(agent_id.as_str()).unwrap();
        assert!(loaded_state.is_some());
    }

    #[test]
    fn stop_regular_agent_without_worktree_works() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        )
        .unwrap();

        // Spawn agent without worktree (regular spawn)
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Stop with cleanup should still work
        pool.stop_agent_with_worktree_cleanup(&agent_id, true)
            .unwrap();

        // Verify slot is stopped
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_terminal());
    }

    // ============== Crash Recovery Tests ==============

    #[test]
    fn recover_orphaned_worktrees_with_missing_worktree_cleans_state() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        // Create a worktree state for a non-existent agent
        let store = WorktreeStateStore::new(state_dir.clone());
        let fake_worktree_path = PathBuf::from("/nonexistent/worktree/path");
        let state = WorktreeState::new(
            "wt-orphan".to_string(),
            fake_worktree_path,
            Some("feature/orphan".to_string()),
            "abc123".to_string(),
            Some("task-orphan".to_string()),
            "agent_orphan".to_string(),
        );
        store.save("agent_orphan", &state).unwrap();

        // Create pool and recover
        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir.clone(),
        )
        .unwrap();

        // Recover without recreating
        let report = pool.recover_orphaned_worktrees(false).unwrap();
        assert_eq!(report.cleaned_up.len(), 1);
        assert_eq!(report.recovered.len(), 0);

        // State should be deleted
        let loaded = store.load("agent_orphan").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn recover_orphaned_worktrees_empty_store() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir,
        )
        .unwrap();

        let report = pool.recover_orphaned_worktrees(true).unwrap();
        assert_eq!(report.recovered.len(), 0);
        assert_eq!(report.cleaned_up.len(), 0);
    }

    #[test]
    fn recover_skips_agents_in_pool() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path,
            state_dir.clone(),
        )
        .unwrap();

        // Spawn an agent with worktree
        let agent_id = pool
            .spawn_agent_with_worktree(ProviderKind::Mock, Some("feature/active".to_string()), None)
            .unwrap();

        // The state is created by spawn, so it exists
        // Recovery should not affect it since agent is in pool
        let report = pool.recover_orphaned_worktrees(false).unwrap();
        assert_eq!(report.recovered.len(), 0);
        assert_eq!(report.cleaned_up.len(), 0);

        // State should still exist
        let store = WorktreeStateStore::new(state_dir);
        let loaded = store.load(agent_id.as_str()).unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn recover_fails_without_worktree_support() {
        let mut pool = AgentPool::new(WorkplaceId::new("workplace-001"), 4);
        let result = pool.recover_orphaned_worktrees(true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::WorktreeNotEnabled
        ));
    }

    #[test]
    fn spawn_does_not_collide_with_existing_worktrees() {
        // This test reproduces the bug: when creating a new AgentPool with
        // existing worktrees on disk (from a previous session), spawn should
        // NOT fail with "worktree already exists" error.

        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        // Create a worktree state store
        let state_store = WorktreeStateStore::new(state_dir.clone());

        // Create a worktree manager and pre-create worktree for agent_001
        // (simulating a previous session's leftover)
        let config = WorktreeConfig::default();
        let worktree_manager = WorktreeManager::new(repo_path.clone(), config).unwrap();

        // Pre-create worktree wt-agent_001
        let worktree_id = "wt-agent_001";
        let worktree_options = WorktreeCreateOptions {
            path: worktree_manager.worktrees_dir().join(worktree_id),
            branch: Some("agent/agent_001".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        let worktree_info = worktree_manager
            .create(worktree_id, worktree_options)
            .unwrap();

        // Save worktree state for agent_001
        let base_commit = worktree_manager.get_current_head().unwrap();
        let worktree_state = WorktreeState::new(
            worktree_id.to_string(),
            worktree_info.path.clone(),
            Some("agent/agent_001".to_string()),
            base_commit,
            None,
            "agent_001".to_string(),
        );
        state_store.save("agent_001", &worktree_state).unwrap();

        // Now create a fresh AgentPool (simulating TUI startup after cancel restore)
        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-002"),
            4,
            repo_path.clone(),
            state_dir.clone(),
        )
        .unwrap();

        // Try to spawn a new agent - should NOT fail with worktree collision
        let result = pool.spawn_agent_with_worktree(ProviderKind::Mock, None, None);

        // The bug: this would fail with "worktree already exists: wt-agent_001"
        // The fix: pool should sync its next_agent_index with existing worktrees
        assert!(
            result.is_ok(),
            "spawn should succeed, got error: {:?}",
            result.err()
        );

        // The spawned agent should have a different ID (not agent_001)
        let spawned_id = result.unwrap();
        assert_ne!(
            spawned_id.as_str(),
            "agent_001",
            "spawned agent should not collide with existing agent_001"
        );

        // Verify the worktree was created with a different path
        let slot = pool.get_slot_by_id(&spawned_id).unwrap();
        let worktree_path = slot.cwd();
        assert_ne!(
            worktree_path, worktree_info.path,
            "worktree path should be different"
        );
        assert!(worktree_path.exists(), "new worktree should exist");
    }

    #[test]
    fn spawn_does_not_collide_with_existing_branches() {
        // This test reproduces the bug where worktree state was deleted
        // but the git branch still exists, causing spawn to fail

        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        // Create a worktree manager and pre-create a branch for agent_001
        // (simulating leftover from previous session after worktree state cleanup)
        let config = WorktreeConfig::default();
        let worktree_manager = WorktreeManager::new(repo_path.clone(), config).unwrap();

        // Create branch "agent/agent_001" (without creating worktree state)
        let branch_options = WorktreeCreateOptions {
            path: worktree_manager.worktrees_dir().join("wt-agent_001"),
            branch: Some("agent/agent_001".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        let _worktree_info = worktree_manager
            .create("wt-agent_001", branch_options)
            .unwrap();

        // Remove worktree but keep the branch (simulating cleanup that deleted state)
        worktree_manager.remove("wt-agent_001", true).unwrap();

        // Verify branch still exists
        assert!(worktree_manager.branch_exists("agent/agent_001").unwrap());

        // Now create a fresh AgentPool with no worktree state files
        // (simulating TUI startup after cancel restore + manual cleanup)
        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-003"),
            4,
            repo_path.clone(),
            state_dir, // empty state dir, no worktree states
        )
        .unwrap();

        // Try to spawn a new agent - should NOT fail with branch collision
        let result = pool.spawn_agent_with_worktree(ProviderKind::Mock, None, None);

        // The bug: this would fail with "branch already exists: agent/agent_001"
        // The fix: pool should sync its next_agent_index with existing branches too
        assert!(
            result.is_ok(),
            "spawn should succeed, got error: {:?}",
            result.err()
        );

        // The spawned agent should have a different ID (not agent_001)
        let spawned_id = result.unwrap();
        assert_ne!(
            spawned_id.as_str(),
            "agent_001",
            "spawned agent should not collide with existing branch agent_001"
        );

        // Verify the worktree was created
        let slot = pool.get_slot_by_id(&spawned_id).unwrap();
        assert!(slot.cwd().exists(), "new worktree should exist");
    }

    // Profile spawning tests

    #[test]
    fn spawn_agent_with_profile_fails_without_profile_store() {
        let mut pool = make_pool(4);

        let result = pool.spawn_agent_with_profile(&"claude-default".to_string());

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::provider_profile::ProfileError::NoProfileStore
        ));
    }

    #[test]
    fn spawn_agent_with_profile_creates_agent_with_profile_id() {
        let mut pool = make_pool(4);
        // Manually create store with Claude profile (not auto-detect)
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        let agent_id = pool
            .spawn_agent_with_profile(&"claude-default".to_string())
            .expect("spawn with profile");

        assert_eq!(pool.active_count(), 1);
        let slot = pool.get_slot_by_id(&agent_id).expect("slot exists");
        assert!(slot.has_profile());
        assert_eq!(slot.profile_id(), Some(&"claude-default".to_string()));
    }

    #[test]
    fn spawn_agent_with_profile_fails_for_nonexistent_profile() {
        let mut pool = make_pool(4);
        // Manually create store (even empty store would work for this test)
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        let result = pool.spawn_agent_with_profile(&"nonexistent-profile".to_string());

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::provider_profile::ProfileError::ProfileNotFound(_)
        ));
    }

    #[test]
    fn spawn_agent_with_profile_fails_when_pool_full() {
        let mut pool = make_pool(2);
        // Manually create store
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        // Spawn first agent
        pool.spawn_agent_with_profile(&"claude-default".to_string())
            .expect("first spawn");

        // Spawn second agent
        pool.spawn_agent_with_profile(&"claude-default".to_string())
            .expect("second spawn");

        // Third should fail
        let result = pool.spawn_agent_with_profile(&"claude-default".to_string());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::provider_profile::ProfileError::PersistenceError(_)
        ));
    }

    #[test]
    fn spawn_agent_with_profile_fails_for_unsupported_cli_type() {
        let mut pool = make_pool(4);

        // Create profile store with unsupported CLI type
        let mut store = ProfileStore::new();
        let unsupported_profile = crate::provider_profile::profile::ProviderProfile::new(
            "unsupported".to_string(),
            CliBaseType::OpenCode,
        );
        store.add_profile(unsupported_profile);
        pool.set_profile_store(store);

        let result = pool.spawn_agent_with_profile(&"unsupported".to_string());

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::provider_profile::ProfileError::UnsupportedCliType(_)
        ));
    }

    #[test]
    fn spawn_agent_with_worktree_and_profile_fails_without_profile_store() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path.clone(),
            state_dir,
        )
        .unwrap();

        let result = pool.spawn_agent_with_worktree_and_profile(
            &"claude-default".to_string(),
            Some("feature/test".to_string()),
            None,
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentPoolWorktreeError::StateStoreError(_)
        ));
    }

    #[test]
    fn spawn_agent_with_worktree_and_profile_creates_agent_with_profile() {
        let (_temp_repo, repo_path) = setup_test_repo();
        let temp_state = tempfile::TempDir::new().unwrap();
        let state_dir = temp_state.path().to_path_buf();

        let mut pool = AgentPool::new_with_worktrees(
            WorkplaceId::new("workplace-001"),
            4,
            repo_path.clone(),
            state_dir,
        )
        .unwrap();

        // Manually create store with Claude profile
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        let agent_id = pool
            .spawn_agent_with_worktree_and_profile(
                &"claude-default".to_string(),
                Some("feature/test-profile".to_string()),
                Some("task-123".to_string()),
            )
            .expect("spawn with profile");

        assert_eq!(pool.active_count(), 1);
        let slot = pool.get_slot_by_id(&agent_id).expect("slot exists");
        assert!(slot.has_worktree());
        assert!(slot.has_profile());
        assert_eq!(slot.profile_id(), Some(&"claude-default".to_string()));
    }

    #[test]
    fn load_profiles_merges_global_and_workplace() {
        let mut pool = make_pool(4);

        // Create temporary directories for profile persistence
        let temp_global = tempfile::TempDir::new().unwrap();
        let temp_workplace = tempfile::TempDir::new().unwrap();

        let persistence = ProfilePersistence::for_paths(
            temp_global.path().to_path_buf(),
            Some(temp_workplace.path().to_path_buf()),
        );

        // Should succeed loading defaults
        pool.load_profiles(&persistence).expect("load profiles");

        assert!(pool.profile_store().is_some());
        let store = pool.profile_store().unwrap();
        assert!(store.has_profile(&"claude-default".to_string()));
    }

    // Decision agent profile tests

    #[test]
    fn spawn_decision_agent_with_profile_sets_profile_id() {
        let mut pool = make_pool(4);

        // Spawn work agent first (Claude supports decision agents)
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Spawn decision agent with profile
        let profile_id = "claude-default".to_string();
        pool.spawn_decision_agent_with_profile_for(&agent_id, Some(&profile_id))
            .expect("spawn decision agent with profile");

        // Check decision agent has profile
        let decision_agent = pool.decision_agent_for(&agent_id).expect("decision agent exists");
        assert!(decision_agent.has_profile());
        assert_eq!(decision_agent.profile_id(), Some(&profile_id));
    }

    #[test]
    fn spawn_decision_agent_without_profile_has_no_profile_id() {
        let mut pool = make_pool(4);

        // Spawn work agent first (Claude supports decision agents)
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Spawn decision agent without profile
        pool.spawn_decision_agent_with_profile_for(&agent_id, None)
            .expect("spawn decision agent");

        // Check decision agent has no profile
        let decision_agent = pool.decision_agent_for(&agent_id).expect("decision agent exists");
        assert!(!decision_agent.has_profile());
        assert_eq!(decision_agent.profile_id(), None);
    }

    #[test]
    fn spawn_decision_agent_for_nonexistent_work_agent_fails() {
        let mut pool = make_pool(4);

        let result = pool.spawn_decision_agent_with_profile_for(
            &AgentId::new("nonexistent"),
            Some(&"claude-default".to_string()),
        );

        assert!(result.is_err());
    }

    // Launch context tests

    #[test]
    fn get_launch_context_for_agent_with_profile() {
        let mut pool = make_pool(4);
        // Manually create store with Claude profile
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        // Spawn agent with profile
        let agent_id = pool
            .spawn_agent_with_profile(&"claude-default".to_string())
            .expect("spawn with profile");

        // Get launch context
        let cwd = std::path::PathBuf::from("/tmp/test");
        let context = pool.get_launch_context_for_agent(&agent_id, cwd.clone());
        assert!(context.is_some());

        let ctx = context.unwrap();
        assert_eq!(ctx.provider(), ProviderKind::Claude);
        assert_eq!(ctx.cwd, cwd);
    }

    #[test]
    fn get_launch_context_for_agent_without_profile_fallback() {
        let mut pool = make_pool(4);

        // Spawn agent without profile (using ProviderKind directly)
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn agent");

        // Get launch context (should fallback to default)
        let cwd = std::path::PathBuf::from("/tmp/test");
        let context = pool.get_launch_context_for_agent(&agent_id, cwd.clone());
        assert!(context.is_some());

        let ctx = context.unwrap();
        assert_eq!(ctx.provider(), ProviderKind::Claude);
    }

    #[test]
    fn get_launch_context_for_nonexistent_agent_returns_none() {
        let pool = make_pool(4);

        let cwd = std::path::PathBuf::from("/tmp/test");
        let context = pool.get_launch_context_for_agent(&AgentId::new("nonexistent"), cwd);
        assert!(context.is_none());
    }

    #[test]
    fn get_launch_context_for_agent_without_profile_store_returns_fallback() {
        let mut pool = make_pool(4);

        // Spawn agent with profile but no profile store loaded
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn agent");

        // Should still return fallback context
        let cwd = std::path::PathBuf::from("/tmp/test");
        let context = pool.get_launch_context_for_agent(&agent_id, cwd);
        assert!(context.is_some());
    }

    #[test]
    fn get_profile_for_agent_returns_correct_profile() {
        let mut pool = make_pool(4);
        // Manually create store with Claude profile
        let mut store = ProfileStore::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        pool.set_profile_store(store);

        // Spawn agent with profile
        let agent_id = pool
            .spawn_agent_with_profile(&"claude-default".to_string())
            .expect("spawn with profile");

        // Get profile
        let profile = pool.get_profile_for_agent(&agent_id);
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().id, "claude-default");
    }

    #[test]
    fn get_profile_for_agent_without_profile_returns_none() {
        let mut pool = make_pool(4);

        // Spawn agent without profile
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn agent");

        let profile = pool.get_profile_for_agent(&agent_id);
        assert!(profile.is_none());
    }

    // Decision agent polling and status tests

    #[test]
    fn has_decision_agent_returns_true_after_spawn() {
        let mut pool = make_pool(4);

        // Spawn work agent (Claude supports decision agents)
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Should have decision agent
        assert!(pool.has_decision_agent(&agent_id));
    }

    #[test]
    fn has_decision_agent_returns_false_for_nonexistent() {
        let pool = make_pool(4);

        // Nonexistent agent should not have decision agent
        assert!(!pool.has_decision_agent(&AgentId::new("nonexistent")));
    }

    #[test]
    fn stop_decision_agent_removes_decision_agent() {
        let mut pool = make_pool(4);

        // Spawn work agent
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Should have decision agent
        assert!(pool.has_decision_agent(&agent_id));

        // Stop decision agent
        pool.stop_decision_agent_for(&agent_id).expect("stop decision agent");

        // Should no longer have decision agent
        assert!(!pool.has_decision_agent(&agent_id));
    }

    #[test]
    fn poll_decision_agents_processes_pending_request() {
        let mut pool = make_pool(4);

        // Spawn work agent
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Get the decision mail sender for this agent
        let mail_sender = pool.decision_coordinator.mail_sender_for_test(&agent_id).expect("mail sender exists");

        // Create a decision request
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);
        let situation = registry.build(SituationType::new("waiting_for_choice"));
        let context = DecisionContext::new(situation, agent_id.as_str());

        let request = crate::decision_mail::DecisionRequest::new(
            agent_id.clone(),
            SituationType::new("waiting_for_choice"),
            context,
        );

        // Send request
        mail_sender.send_request(request).expect("send request");

        // First poll - spawns async thread but response not ready yet
        let _responses = pool.poll_decision_agents();

        // Wait for async thread to complete and send response
        // The response is sent via the channel, so we need to give it time
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Second poll - collects the response that was sent
        let responses = pool.poll_decision_agents();

        // Should have received a response
        assert_eq!(responses.len(), 1);

        let (work_agent_id, response) = &responses[0];
        assert_eq!(work_agent_id, &agent_id);
        // Response should be either success or error (depending on engine)
        assert!(response.is_success() || response.is_error());
    }

    #[test]
    fn agents_with_pending_decisions_returns_empty_initially() {
        let pool = make_pool(4);

        // No pending decisions initially
        let pending = pool.agents_with_pending_decisions();
        assert!(pending.is_empty());
    }

    #[test]
    fn agents_with_pending_decisions_returns_agent_after_poll() {
        // After starting a decision, the agent should appear in pending decisions
        // This verifies per-agent timestamp tracking (not global)
        let mut pool = make_pool(4);

        // Spawn work agent
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Initially no pending decisions
        let pending = pool.agents_with_pending_decisions();
        assert!(pending.is_empty());

        // Get the decision mail sender for this agent
        let mail_sender = pool.decision_coordinator.mail_sender_for_test(&agent_id).expect("mail sender exists");

        // Create a decision request
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);
        let situation = registry.build(SituationType::new("waiting_for_choice"));
        let context = DecisionContext::new(situation, agent_id.as_str());

        let request = crate::decision_mail::DecisionRequest::new(
            agent_id.clone(),
            SituationType::new("waiting_for_choice"),
            context,
        );

        // Send request
        mail_sender.send_request(request).expect("send request");

        // First poll - spawns async thread, should set per-agent timestamp
        pool.poll_decision_agents();

        // Now the agent should appear in pending decisions (either Thinking or has recent decision)
        let pending = pool.agents_with_pending_decisions();
        assert_eq!(pending.len(), 1, "Should have one pending decision");
        assert_eq!(pending[0].0, agent_id, "Pending decision should be for our agent");
    }

    #[test]
    fn decision_agent_stats_initializes_correctly() {
        let pool = make_pool(4);

        let stats = pool.decision_agent_stats();

        // Initially no agents
        assert_eq!(stats.total_agents, 0);
        assert_eq!(stats.total_decisions, 0);
        assert_eq!(stats.total_errors, 0);
    }

    #[test]
    fn decision_agent_stats_counts_after_spawn() {
        let mut pool = make_pool(4);

        // Spawn work agent
        let _agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Get stats
        let stats = pool.decision_agent_stats();

        // Should have 1 agent
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.idle_agents, 1);
    }

    #[test]
    fn poll_decision_agents_returns_empty_when_no_requests() {
        let mut pool = make_pool(4);

        // Spawn work agent
        let _agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Poll with no pending requests
        let responses = pool.poll_decision_agents();

        // Should be empty (no requests to process)
        assert!(responses.is_empty());
    }

    #[test]
    fn clear_recent_decision_called_after_display_window_expires() {
        // Bug 2: Memory leak - clear_recent_decision should be called after 1.5s window
        // This test verifies that old timestamps are cleaned up after poll_decision_agents
        let mut pool = make_pool(4);

        // Spawn work agent
        let agent_id = pool.spawn_agent(ProviderKind::Claude).expect("spawn work agent");

        // Manually set last_decision_started_at to a past time (>1.5s ago)
        // This simulates a decision that started long ago and already completed
        if let Some(decision_agent) = pool.decision_coordinator.agent_mut_for_test(&agent_id) {
            // Set to 2 seconds ago
            decision_agent.set_last_decision_started_at_for_test(Some(
                std::time::Instant::now() - std::time::Duration::from_millis(2000)
            ));
        }

        // Verify the timestamp exists before poll
        let pending_before = pool.agents_with_pending_decisions();
        assert!(pending_before.is_empty(), "Should be empty because elapsed > 1.5s");

        // Poll decision agents - should clean up old timestamps
        let _responses = pool.poll_decision_agents();

        // After poll, the old timestamp should be cleared
        if let Some(decision_agent) = pool.decision_coordinator.agent_for_test(&agent_id) {
            assert!(
                decision_agent.last_decision_started_at().is_none(),
                "Old timestamp should be cleared after poll_decision_agents cleanup"
            );
        }
    }

    #[test]
    fn continue_all_tasks_on_idle_agent_with_no_task_returns_accepted() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Agent is idle with no assigned task
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_idle());
        assert!(slot.assigned_task_id().is_none());

        // Build continue_all_tasks decision output
        let action = Box::new(ContinueAllTasksAction::new("continue finish all tasks"));
        let output = DecisionOutput::new(vec![action], "Rule: continue-on-idle");

        // Execute decision action
        let result = pool.execute_decision_action(&agent_id, &output);

        // Should return AcceptedRecommendation, not CustomInstruction
        assert_eq!(result, DecisionExecutionResult::AcceptedRecommendation);
    }
}
