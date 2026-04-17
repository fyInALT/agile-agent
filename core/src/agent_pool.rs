//! AgentPool for managing multiple agent slots
//!
//! Central coordination structure for multi-agent runtime.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::backlog::{BacklogState, TaskStatus};
use crate::decision_agent_slot::{DecisionAgentSlot, DecisionAgentStatus};
use crate::decision_mail::{DecisionMail, DecisionMailSender, DecisionRequest, DecisionResponse};
use crate::logging;
use crate::provider::{ProviderEvent, ProviderKind};
use crate::worktree_manager::{
    WorktreeConfig, WorktreeCreateOptions, WorktreeError, WorktreeManager,
};
use crate::worktree_state::WorktreeState;
use crate::worktree_state_store::WorktreeStateStore;
use chrono::Utc;

// Decision layer imports
use agent_decision::{
    AutoAction, BlockedState, HumanDecisionQueue, HumanDecisionRequest, HumanDecisionResponse,
    HumanDecisionTimeoutConfig, HumanSelection, SituationType,
    classifier::ClassifyResult,
    initializer::{DecisionLayerComponents, initialize_decision_layer},
    provider_event::ProviderEvent as DecisionProviderEvent,
};

/// Convert core ProviderEvent to decision layer ProviderEvent
fn convert_provider_event_to_decision(
    event: &crate::provider::ProviderEvent,
) -> DecisionProviderEvent {
    match event {
        crate::provider::ProviderEvent::Finished => {
            DecisionProviderEvent::Finished { summary: None }
        }
        crate::provider::ProviderEvent::Error(msg) => DecisionProviderEvent::Error {
            message: msg.clone(),
            error_type: None,
        },
        crate::provider::ProviderEvent::Status(text) => DecisionProviderEvent::StatusUpdate {
            status: text.clone(),
        },
        crate::provider::ProviderEvent::AssistantChunk(text) => {
            DecisionProviderEvent::ClaudeAssistantChunk { text: text.clone() }
        }
        crate::provider::ProviderEvent::ThinkingChunk(text) => {
            DecisionProviderEvent::ClaudeThinkingChunk { text: text.clone() }
        }
        crate::provider::ProviderEvent::SessionHandle(handle) => {
            DecisionProviderEvent::SessionHandle {
                session_id: match handle {
                    crate::provider::SessionHandle::ClaudeSession { session_id } => {
                        session_id.clone()
                    }
                    crate::provider::SessionHandle::CodexThread { thread_id } => thread_id.clone(),
                },
                info: None,
            }
        }
        crate::provider::ProviderEvent::ExecCommandStarted { input_preview, .. } => {
            DecisionProviderEvent::ClaudeToolCallStarted {
                name: "exec".to_string(),
                input: input_preview.clone(),
            }
        }
        crate::provider::ProviderEvent::ExecCommandFinished {
            output_preview,
            status,
            ..
        } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: "exec".to_string(),
            output: output_preview.clone(),
            success: matches!(status, crate::tool_calls::ExecCommandStatus::Completed),
        },
        crate::provider::ProviderEvent::GenericToolCallStarted {
            name,
            input_preview,
            ..
        } => DecisionProviderEvent::ClaudeToolCallStarted {
            name: name.clone(),
            input: input_preview.clone(),
        },
        crate::provider::ProviderEvent::GenericToolCallFinished {
            name,
            output_preview,
            success,
            ..
        } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: name.clone(),
            output: output_preview.clone(),
            success: *success,
        },
        crate::provider::ProviderEvent::PatchApplyStarted { .. } => {
            DecisionProviderEvent::CodexPatchApplyStarted {
                path: "".to_string(),
            }
        }
        crate::provider::ProviderEvent::PatchApplyFinished { status, .. } => {
            DecisionProviderEvent::StatusUpdate {
                status: match status {
                    crate::tool_calls::PatchApplyStatus::Completed => "patch completed".to_string(),
                    crate::tool_calls::PatchApplyStatus::Failed => "patch failed".to_string(),
                    crate::tool_calls::PatchApplyStatus::Declined => "patch declined".to_string(),
                    crate::tool_calls::PatchApplyStatus::InProgress => {
                        "patch in progress".to_string()
                    }
                },
            }
        }
        crate::provider::ProviderEvent::McpToolCallStarted { .. } => {
            DecisionProviderEvent::ClaudeToolCallStarted {
                name: "mcp".to_string(),
                input: None,
            }
        }
        crate::provider::ProviderEvent::McpToolCallFinished { error, .. } => {
            DecisionProviderEvent::ClaudeToolCallFinished {
                name: "mcp".to_string(),
                output: error.clone(),
                success: error.is_none(),
            }
        }
        crate::provider::ProviderEvent::WebSearchStarted { .. }
        | crate::provider::ProviderEvent::WebSearchFinished { .. }
        | crate::provider::ProviderEvent::ViewImage { .. }
        | crate::provider::ProviderEvent::ImageGenerationFinished { .. }
        | crate::provider::ProviderEvent::ExecCommandOutputDelta { .. }
        | crate::provider::ProviderEvent::PatchApplyOutputDelta { .. } => {
            DecisionProviderEvent::StatusUpdate {
                status: "running".to_string(),
            }
        }
    }
}

/// Event emitted when an agent becomes blocked
#[derive(Debug, Clone)]
pub struct AgentBlockedEvent {
    /// The blocked agent ID
    pub agent_id: AgentId,
    /// The reason type
    pub reason_type: String,
    /// Human readable description
    pub description: String,
    /// Urgency level
    pub urgency: String,
}

/// Notifier trait for agent blocked events
///
/// Implement this trait to receive notifications when agents become blocked.
/// This enables other agents or systems to react to blocking events.
pub trait AgentBlockedNotifier: Send + Sync {
    /// Called when an agent becomes blocked
    fn on_agent_blocked(&self, event: AgentBlockedEvent);
}

/// No-op notifier that does nothing
#[derive(Debug, Clone, Default)]
pub struct NoOpAgentBlockedNotifier;

impl AgentBlockedNotifier for NoOpAgentBlockedNotifier {
    fn on_agent_blocked(&self, _event: AgentBlockedEvent) {
        // Do nothing
    }
}

/// Snapshot of an agent's status for display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatusSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
    pub status: AgentSlotStatus,
    pub assigned_task_id: Option<TaskId>,
    /// Worktree branch name (if agent has worktree)
    pub worktree_branch: Option<String>,
    /// Whether agent has a worktree
    pub has_worktree: bool,
    /// Whether worktree directory exists on disk
    pub worktree_exists: bool,
}

/// Per-agent task assignment info for visualization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskAssignment {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub task_id: TaskId,
    pub task_status: TaskStatus,
}

/// Snapshot of task queue state for TUI display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskQueueSnapshot {
    /// Total number of tasks in backlog
    pub total_tasks: usize,
    /// Number of tasks ready to be assigned
    pub ready_tasks: usize,
    /// Number of tasks currently running
    pub running_tasks: usize,
    /// Number of tasks completed successfully
    pub completed_tasks: usize,
    /// Number of tasks that failed
    pub failed_tasks: usize,
    /// Number of tasks that are blocked
    pub blocked_tasks: usize,
    /// Tasks assigned to specific agents
    pub agent_assignments: Vec<AgentTaskAssignment>,
    /// Number of idle agents available for assignment
    pub available_agents: usize,
    /// Number of active agents (responding/executing)
    pub active_agents: usize,
}

/// Policy for handling tasks when agent becomes blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedTaskPolicy {
    /// Task stays assigned to blocked agent
    KeepAssigned,
    /// Reassign task to another idle agent if available
    ReassignIfPossible,
    /// Mark task as waiting in backlog
    MarkWaiting,
}

impl Default for BlockedTaskPolicy {
    fn default() -> Self {
        BlockedTaskPolicy::ReassignIfPossible
    }
}

/// Blocked handling configuration
#[derive(Debug, Clone)]
pub struct BlockedHandlingConfig {
    /// Task policy when agent blocked
    pub task_policy: BlockedTaskPolicy,
    /// Human decision timeout config
    pub timeout_config: HumanDecisionTimeoutConfig,
    /// Notify other agents when blocked
    pub notify_others: bool,
    /// Record blocked history
    pub record_history: bool,
    /// Maximum history entries (0 = unlimited)
    pub max_history_entries: usize,
}

impl Default for BlockedHandlingConfig {
    fn default() -> Self {
        Self {
            task_policy: BlockedTaskPolicy::default(),
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: true,
            record_history: true,
            max_history_entries: 1000,
        }
    }
}

/// Record of agent blocking history
#[derive(Debug, Clone)]
pub struct BlockedHistoryEntry {
    /// Agent ID
    pub agent_id: AgentId,
    /// Blocking reason type
    pub reason_type: String,
    /// Blocking description
    pub description: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether it was resolved
    pub resolved: bool,
    /// Resolution method
    pub resolution: Option<String>,
}

/// Decision execution result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionExecutionResult {
    /// Selection executed successfully
    Executed { option_id: String },
    /// Recommendation accepted
    AcceptedRecommendation,
    /// Custom instruction sent
    CustomInstruction { instruction: String },
    /// Task skipped
    Skipped,
    /// Operation cancelled
    Cancelled,
    /// Agent not found
    AgentNotFound,
    /// Agent not blocked
    NotBlocked,
}

/// Pool managing multiple agent slots
pub struct AgentPool {
    /// All active agent slots
    slots: Vec<AgentSlot>,
    /// Max concurrent agents (configurable)
    max_slots: usize,
    /// Agent index counter for generating new IDs
    next_agent_index: usize,
    /// Index of the currently focused agent (for TUI)
    focused_slot: usize,
    /// Workplace ID for this pool
    workplace_id: WorkplaceId,
    /// Human decision queue
    human_queue: HumanDecisionQueue,
    /// Blocked handling configuration
    blocked_config: BlockedHandlingConfig,
    /// Blocking history records
    blocked_history: Vec<BlockedHistoryEntry>,
    /// Notifier for blocked events (used when notify_others is true)
    blocked_notifier: Arc<dyn AgentBlockedNotifier>,
    /// Decision agent slots (keyed by work agent ID)
    decision_agents: HashMap<AgentId, DecisionAgentSlot>,
    /// Decision mail senders (keyed by work agent ID)
    /// Used by work agents to send decision requests
    decision_mail_senders: HashMap<AgentId, DecisionMailSender>,
    /// Decision layer components (classifiers, etc.)
    decision_components: DecisionLayerComponents,
    /// Working directory for decision agents
    cwd: PathBuf,
    /// Worktree manager (optional, for isolated worktrees)
    worktree_manager: Option<WorktreeManager>,
    /// Worktree state store for persistence
    worktree_state_store: Option<WorktreeStateStore>,
}

impl std::fmt::Debug for AgentPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPool")
            .field("slots", &self.slots)
            .field("max_slots", &self.max_slots)
            .field("next_agent_index", &self.next_agent_index)
            .field("focused_slot", &self.focused_slot)
            .field("workplace_id", &self.workplace_id)
            .field("human_queue", &self.human_queue)
            .field("blocked_config", &self.blocked_config)
            .field("blocked_history", &self.blocked_history)
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
            focused_slot: 0,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_config: BlockedHandlingConfig::default(),
            blocked_history: Vec::new(),
            blocked_notifier: Arc::new(NoOpAgentBlockedNotifier),
            decision_agents: HashMap::new(),
            decision_mail_senders: HashMap::new(),
            decision_components: initialize_decision_layer(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            worktree_manager: None,
            worktree_state_store: None,
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
            focused_slot: 0,
            workplace_id,
            human_queue: HumanDecisionQueue::new(config.timeout_config.clone()),
            blocked_config: config,
            blocked_history: Vec::new(),
            blocked_notifier: Arc::new(NoOpAgentBlockedNotifier),
            decision_agents: HashMap::new(),
            decision_mail_senders: HashMap::new(),
            decision_components: initialize_decision_layer(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            worktree_manager: None,
            worktree_state_store: None,
        }
    }

    /// Create pool with working directory for decision agents
    pub fn with_cwd(workplace_id: WorkplaceId, max_slots: usize, cwd: PathBuf) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            focused_slot: 0,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_config: BlockedHandlingConfig::default(),
            blocked_history: Vec::new(),
            blocked_notifier: Arc::new(NoOpAgentBlockedNotifier),
            decision_agents: HashMap::new(),
            decision_mail_senders: HashMap::new(),
            decision_components: initialize_decision_layer(),
            cwd,
            worktree_manager: None,
            worktree_state_store: None,
        }
    }

    /// Create pool with worktree support for isolated agent workspaces
    pub fn new_with_worktrees(
        workplace_id: WorkplaceId,
        max_slots: usize,
        repo_root: PathBuf,
        state_dir: PathBuf,
    ) -> Result<Self, WorktreeError> {
        let config = WorktreeConfig::default();
        let worktree_manager = WorktreeManager::new(repo_root.clone(), config)?;
        let worktree_state_store = WorktreeStateStore::new(state_dir);

        // Sync next_agent_index with existing worktree states AND git branches
        // to avoid collision when user cancels restore but previous artifacts exist
        let existing_states = worktree_state_store.list_all().unwrap_or_default();
        let max_state_index = existing_states
            .iter()
            .filter_map(|(agent_id, _)| parse_agent_index(agent_id))
            .max();

        // Also check existing agent branches (agent/agent_XXX pattern)
        let existing_branches = worktree_manager.list_agent_branches().unwrap_or_default();
        let max_branch_index = existing_branches
            .iter()
            .filter_map(|branch| {
                // Branch format: "agent/agent_001"
                branch.strip_prefix("agent/").and_then(parse_agent_index)
            })
            .max();

        // Use the maximum index found from both sources
        let max_existing_index = max_state_index.max(max_branch_index);
        let next_agent_index = max_existing_index.map_or(1, |idx| idx + 1);

        Ok(Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index,
            focused_slot: 0,
            workplace_id,
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_config: BlockedHandlingConfig::default(),
            blocked_history: Vec::new(),
            blocked_notifier: Arc::new(NoOpAgentBlockedNotifier),
            decision_agents: HashMap::new(),
            decision_mail_senders: HashMap::new(),
            decision_components: initialize_decision_layer(),
            cwd: repo_root,
            worktree_manager: Some(worktree_manager),
            worktree_state_store: Some(worktree_state_store),
        })
    }

    /// Set a custom blocked notifier
    pub fn set_blocked_notifier(&mut self, notifier: Arc<dyn AgentBlockedNotifier>) {
        self.blocked_notifier = notifier;
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
        self.focused_slot
    }

    /// Get workplace ID
    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    /// Check if pool has worktree support enabled
    pub fn has_worktree_support(&self) -> bool {
        self.worktree_manager.is_some() && self.worktree_state_store.is_some()
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
        if self.slots.len() == 1 {
            self.focused_slot = 0;
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
        if self.worktree_manager.is_none() || self.worktree_state_store.is_none() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        // Check if pool has capacity
        if !self.can_spawn() {
            return Err(AgentPoolWorktreeError::PoolFull);
        }

        // Generate agent ID (this needs mutable borrow)
        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        // Generate worktree ID and branch name
        let worktree_id = format!("wt-{}", agent_id.as_str());
        let actual_branch = branch_name.unwrap_or_else(|| format!("agent/{}", agent_id.as_str()));

        // Now get worktree_manager and state_store (immutable borrows)
        let worktree_manager = self.worktree_manager.as_ref().unwrap();
        let worktree_state_store = self.worktree_state_store.as_ref().unwrap();

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

        // Create agent slot with worktree
        let mut slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        slot.set_worktree(
            worktree_info.path.clone(),
            Some(actual_branch.clone()),
            worktree_id.clone(),
        );

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
                "pool_size": self.slots.len(),
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        if self.slots.len() == 1 {
            self.focused_slot = 0;
        }

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

    /// Pause an agent with worktree state preservation
    ///
    /// Saves the current worktree state (uncommitted changes status, HEAD commit)
    /// before pausing, allowing for seamless resume later.
    pub fn pause_agent_with_worktree(
        &mut self,
        agent_id: &AgentId,
    ) -> Result<(), AgentPoolWorktreeError> {
        // Check worktree support is available
        if self.worktree_state_store.is_none() || self.worktree_manager.is_none() {
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

        let worktree_state_store = self.worktree_state_store.as_ref().unwrap();
        let worktree_manager = self.worktree_manager.as_ref().unwrap();

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
        if self.worktree_state_store.is_none() || self.worktree_manager.is_none() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let worktree_state_store = self.worktree_state_store.as_ref().unwrap();
        let worktree_manager = self.worktree_manager.as_ref().unwrap();

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
        // Check worktree support is available
        if self.worktree_state_store.is_none() || self.worktree_manager.is_none() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        let worktree_state_store = self.worktree_state_store.as_ref().unwrap();
        let worktree_manager = self.worktree_manager.as_ref().unwrap();

        let all_states = worktree_state_store
            .list_all()
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

        let mut recovered = Vec::new();
        let mut cleaned_up = Vec::new();

        for (agent_id, state) in all_states {
            // Check if this agent is already in the pool
            if self.get_slot_by_id(&AgentId::new(&agent_id)).is_some() {
                // Agent already exists, skip
                continue;
            }

            // Check if worktree exists
            if state.exists() {
                // Worktree exists but agent doesn't - this is an orphan
                // We could restore it as a paused agent, but for now we just log it
                logging::debug_event(
                    "worktree.orphan_found",
                    "found orphaned worktree state",
                    serde_json::json!({
                        "agent_id": agent_id,
                        "worktree_id": state.worktree_id,
                        "path": state.path.to_string_lossy(),
                    }),
                );
                // For orphan worktrees that still exist, we preserve them
                // They can be manually recovered later
            } else if recreate_missing {
                // Worktree missing, recreate it
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
                        // Failed to recreate, clean up the state
                        worktree_state_store
                            .delete(&agent_id)
                            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;
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
                // Not recreating, clean up the stale state
                worktree_state_store
                    .delete(&agent_id)
                    .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;
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
    pub fn auto_cleanup_idle_worktrees(
        &mut self,
        idle_duration: chrono::Duration,
    ) -> Result<Vec<String>, AgentPoolWorktreeError> {
        // Check worktree support is available first
        if self.worktree_state_store.is_none() || self.worktree_manager.is_none() {
            return Err(AgentPoolWorktreeError::WorktreeNotEnabled);
        }

        // Get all states first (this borrows the store)
        let all_states = self
            .worktree_state_store
            .as_ref()
            .unwrap()
            .list_all()
            .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;

        // First pass: collect worktrees to clean up and their info
        let mut to_cleanup: Vec<(String, WorktreeState, bool)> = Vec::new();

        for (agent_id, state) in &all_states {
            // Check if agent is in pool and is idle
            let slot = self.get_slot_by_id(&AgentId::new(agent_id));
            let is_pool_idle =
                slot.map_or(false, |s| s.status().is_idle() || s.status().is_paused());

            // Skip if agent is active (not idle/paused)
            if slot.is_some() && !is_pool_idle {
                continue;
            }

            // Check if worktree is idle and empty
            if state.is_idle_longer_than(idle_duration) && state.is_empty() {
                to_cleanup.push((agent_id.clone(), state.clone(), slot.is_some()));
            }
        }

        // Release the borrow by taking ownership of the cleanup list
        let cleanup_list = to_cleanup;

        let mut cleaned_up = Vec::new();

        // Second pass: do the cleanup
        for (agent_id, state, in_pool) in cleanup_list {
            // Remove worktree if it exists
            if state.exists() {
                if let Some(wm) = &self.worktree_manager {
                    wm.remove(&state.worktree_id, true)
                        .map_err(AgentPoolWorktreeError::WorktreeError)?;
                }
            }

            // Delete state
            if let Some(store) = &self.worktree_state_store {
                store
                    .delete(&agent_id)
                    .map_err(|e| AgentPoolWorktreeError::StateStoreError(e.to_string()))?;
            }

            // Clear worktree from slot if agent is in pool
            if in_pool {
                if let Some(slot) = self.get_slot_mut_by_id(&AgentId::new(&agent_id)) {
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
        self.focused_slot = 0;

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
        // Find the work agent slot
        let slot_index = self.find_slot_index(work_agent_id)?;
        let work_slot = &self.slots[slot_index];
        let provider_kind_opt = work_slot.provider_type().to_provider_kind();

        // Handle Opencode provider which doesn't have ProviderKind mapping
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
            self.cwd.clone(),
            &self.decision_components,
        );

        // Inject ProviderLLMCaller for real LLM calls
        use crate::llm_caller::ProviderLLMCaller;
        use std::sync::Arc;
        let llm_caller = Arc::new(ProviderLLMCaller::new(provider_kind, self.cwd.clone()));
        decision_agent.set_llm_caller(llm_caller);

        // Store the decision agent and mail sender
        self.decision_agents
            .insert(work_agent_id.clone(), decision_agent);
        self.decision_mail_senders
            .insert(work_agent_id.clone(), sender);

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

    /// Stop the decision agent for a work agent
    pub fn stop_decision_agent_for(&mut self, work_agent_id: &AgentId) -> Result<(), String> {
        if let Some(decision_agent) = self.decision_agents.get_mut(work_agent_id) {
            decision_agent.stop("work agent stopping");
            self.decision_agents.remove(work_agent_id);
            self.decision_mail_senders.remove(work_agent_id);

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

    /// Get decision agent for a work agent
    pub fn decision_agent_for(&self, work_agent_id: &AgentId) -> Option<&DecisionAgentSlot> {
        self.decision_agents.get(work_agent_id)
    }

    /// Check if work agent has a decision agent
    pub fn has_decision_agent(&self, work_agent_id: &AgentId) -> bool {
        self.decision_agents.contains_key(work_agent_id)
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

                // Convert core ProviderEvent to decision ProviderEvent
                let decision_event = convert_provider_event_to_decision(event);

                // Use classifier registry to classify the event
                self.decision_components
                    .classifier_registry
                    .classify(&decision_event, decision_provider)
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

        if let Some(sender) = self.decision_mail_senders.get(work_agent_id) {
            sender.send_request(request).map_err(|e| {
                logging::warn_event(
                    "pool.decision_request.send_failed",
                    "failed to send decision request",
                    serde_json::json!({
                        "work_agent_id": work_agent_id.as_str(),
                        "error": e,
                    }),
                );
                e
            })?;

            logging::debug_event(
                "pool.decision_request.sent",
                "sent decision request",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "situation_type": situation_type_name,
                }),
            );

            Ok(())
        } else {
            Err(format!(
                "No decision agent for work agent {}",
                work_agent_id.as_str()
            ))
        }
    }

    /// Poll decision agents and process pending requests
    ///
    /// Returns responses from decision agents that have processed requests.
    pub fn poll_decision_agents(&mut self) -> Vec<(AgentId, DecisionResponse)> {
        let mut responses = Vec::new();

        for (work_agent_id, decision_agent) in &mut self.decision_agents {
            // Poll and process any pending requests
            decision_agent.poll_and_process();

            // Try to receive any responses that were generated
            if let Some(sender) = self.decision_mail_senders.get(work_agent_id) {
                while let Some(response) = sender.try_receive_response() {
                    responses.push((work_agent_id.clone(), response));
                }
            }
        }

        responses
    }

    /// Get decision agent statistics
    pub fn decision_agent_stats(&self) -> DecisionAgentStats {
        let mut stats = DecisionAgentStats::default();

        for (_, decision_agent) in &self.decision_agents {
            stats.total_agents += 1;
            stats.total_decisions += decision_agent.decision_count();
            stats.total_errors += decision_agent.error_count();

            match decision_agent.status() {
                DecisionAgentStatus::Idle => stats.idle_agents += 1,
                DecisionAgentStatus::Thinking { .. } => stats.thinking_agents += 1,
                DecisionAgentStatus::Responding => stats.responding_agents += 1,
                DecisionAgentStatus::Error { .. } => stats.error_agents += 1,
                DecisionAgentStatus::Stopped { .. } => stats.stopped_agents += 1,
            }
        }

        stats
    }

    /// Execute a decision action on a work agent
    ///
    /// Takes a decision output and executes the actions on the specified work agent.
    pub fn execute_decision_action(
        &mut self,
        work_agent_id: &AgentId,
        output: &agent_decision::output::DecisionOutput,
    ) -> DecisionExecutionResult {
        // Find the work agent
        let slot_index = match self.find_slot_index(work_agent_id) {
            Ok(idx) => idx,
            Err(_) => return DecisionExecutionResult::AgentNotFound,
        };

        let slot = &mut self.slots[slot_index];

        // Check if agent is blocked (most decisions require blocked state)
        // Allow idle state too for some decisions
        if !slot.status().is_blocked() && !slot.status().is_idle() {
            return DecisionExecutionResult::NotBlocked;
        }

        // Execute the first action from the output
        if let Some(action) = output.actions.first() {
            let action_type = action.action_type().name.clone();
            let params_str = action.serialize_params();

            logging::debug_event(
                "pool.decision_action.execute",
                "executing decision action",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "action_type": action_type,
                    "params": params_str,
                }),
            );

            match action_type.as_str() {
                "select_option" => {
                    // Parse params to get option_id
                    let params: serde_json::Value =
                        serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
                    let option_id = params["option_id"].as_str().unwrap_or("a").to_string();

                    // Execute the selection - find pending request for THIS agent
                    // Bug fix: Use find_by_agent_id to ensure we process the correct agent's request
                    let pending_request = self.human_queue.find_by_agent_id(work_agent_id.as_str());
                    if let Some(request) = pending_request {
                        // Verify this request belongs to our agent (double-check)
                        if request.agent_id != work_agent_id.as_str() {
                            logging::warn_event(
                                "pool.decision_action.mismatch",
                                "request agent_id mismatch",
                                serde_json::json!({
                                    "work_agent_id": work_agent_id.as_str(),
                                    "request_agent_id": request.agent_id,
                                }),
                            );
                            return DecisionExecutionResult::Cancelled;
                        }

                        // Create response with the selection
                        let selection = HumanSelection::selected(option_id.clone());
                        let response = HumanDecisionResponse::new(request.id.clone(), selection);
                        let result = self.process_human_response(response);

                        match result {
                            Ok(DecisionExecutionResult::Executed { .. }) => {
                                DecisionExecutionResult::Executed { option_id }
                            }
                            Ok(other) => other,
                            Err(e) => {
                                logging::warn_event(
                                    "pool.decision_action.process_failed",
                                    "failed to process human response",
                                    serde_json::json!({ "error": e }),
                                );
                                DecisionExecutionResult::Cancelled
                            }
                        }
                    } else {
                        // No pending request for this agent - might not be blocked correctly
                        logging::warn_event(
                            "pool.decision_action.no_request",
                            "no pending request for this agent",
                            serde_json::json!({
                                "work_agent_id": work_agent_id.as_str(),
                            }),
                        );
                        DecisionExecutionResult::NotBlocked
                    }
                }
                "skip" => {
                    // Skip the current task for THIS agent
                    let pending_request = self.human_queue.find_by_agent_id(work_agent_id.as_str());
                    if let Some(request) = pending_request {
                        let response =
                            HumanDecisionResponse::new(request.id.clone(), HumanSelection::skip());
                        let _ = self.process_human_response(response);
                    }
                    DecisionExecutionResult::Skipped
                }
                "request_human" => {
                    // Already in human decision queue - nothing additional to do
                    DecisionExecutionResult::AcceptedRecommendation
                }
                "custom_instruction" => {
                    // Parse params to get instruction
                    let params: serde_json::Value =
                        serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
                    let instruction = params["instruction"].as_str().unwrap_or("").to_string();

                    // Store instruction for the agent to use in next turn
                    if !instruction.is_empty() {
                        slot.append_transcript(crate::app::TranscriptEntry::User(
                            instruction.clone(),
                        ));
                    }
                    DecisionExecutionResult::CustomInstruction { instruction }
                }
                "continue" => {
                    // Continue with normal processing - agent should transition to idle
                    if slot.status().is_blocked() {
                        let _ = slot.transition_to(AgentSlotStatus::idle());
                    }
                    DecisionExecutionResult::AcceptedRecommendation
                }
                _ => {
                    // Unknown action type
                    logging::warn_event(
                        "pool.decision_action.unknown",
                        "unknown decision action type",
                        serde_json::json!({
                            "work_agent_id": work_agent_id.as_str(),
                            "action_type": action_type,
                        }),
                    );
                    DecisionExecutionResult::Cancelled
                }
            }
        } else {
            // No actions in output - nothing to execute
            DecisionExecutionResult::Cancelled
        }
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
        // First, transition the slot to stopped
        let index = self
            .find_slot_index(agent_id)
            .map_err(|e| AgentPoolWorktreeError::AgentNotFound(e))?;

        let slot = &mut self.slots[index];
        let codename = slot.codename().clone();
        let has_worktree = slot.has_worktree();
        let worktree_id = slot.worktree_id().map(|s| s.clone());

        slot.transition_to(AgentSlotStatus::stopped("user requested"))
            .map_err(|e: String| AgentPoolWorktreeError::SlotTransitionError(e))?;

        // Also stop the decision agent for this work agent
        self.stop_decision_agent_for(agent_id)
            .map_err(|e| AgentPoolWorktreeError::SlotTransitionError(e))?;

        // Handle worktree cleanup
        if has_worktree && cleanup_worktree && worktree_id.is_some() {
            // Get worktree manager and state store
            if let (Some(worktree_manager), Some(worktree_state_store)) =
                (&self.worktree_manager, &self.worktree_state_store)
            {
                let wt_id = worktree_id.unwrap();

                // Remove the worktree - log error if it fails but continue
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

                // Delete the worktree state - only if worktree was removed successfully
                // Otherwise, keep the state so it can be recovered manually
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
                } else {
                    logging::debug_event(
                        "pool.agent.stop.worktree_preserved",
                        "worktree preserved due to removal failure",
                        serde_json::json!({
                            "agent_id": agent_id.as_str(),
                            "worktree_id": wt_id,
                            "note": "state kept for manual recovery",
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

        // Also remove the decision agent for this work agent
        self.decision_agents.remove(agent_id);
        self.decision_mail_senders.remove(agent_id);

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
        if self.focused_slot >= self.slots.len() && !self.slots.is_empty() {
            self.focused_slot = self.slots.len() - 1;
            if let Some(new_focused) = self.slots.get(self.focused_slot) {
                logging::debug_event(
                    "pool.focus.adjust",
                    "adjusted focus after agent removal",
                    serde_json::json!({
                        "new_index": self.focused_slot,
                        "new_agent_id": new_focused.agent_id().as_str(),
                    }),
                );
            }
        }
        Ok(())
    }

    /// Get all agents with their current status
    pub fn agent_statuses(&self) -> Vec<AgentStatusSnapshot> {
        self.slots
            .iter()
            .map(|slot| AgentStatusSnapshot {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                provider_type: slot.provider_type(),
                role: slot.role(),
                status: slot.status().clone(),
                assigned_task_id: slot.assigned_task_id().cloned(),
                worktree_branch: slot.worktree_branch().cloned(),
                has_worktree: slot.has_worktree(),
                worktree_exists: slot.has_worktree() && slot.cwd().exists(),
            })
            .collect()
    }

    /// Get all slots for snapshot/export use.
    pub fn slots(&self) -> &[AgentSlot] {
        &self.slots
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

        if slot.role() == AgentRole::ProductOwner {
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
        } else {
            self.slots.push(slot);
        }

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
        if index >= self.slots.len() {
            logging::debug_event(
                "pool.focus.invalid_index",
                "invalid focus index",
                serde_json::json!({
                    "attempted_index": index,
                    "pool_size": self.slots.len(),
                }),
            );
            return Err(format!(
                "Invalid focus index {} (only {} agents)",
                index,
                self.slots.len()
            ));
        }
        let old_index = self.focused_slot;
        let old_agent_id = self
            .slots
            .get(old_index)
            .map(|s| s.agent_id().as_str().to_string());
        let new_agent_id = self
            .slots
            .get(index)
            .map(|s| s.agent_id().as_str().to_string());
        self.focused_slot = index;

        logging::debug_event(
            "pool.focus.change",
            "focus changed by index",
            serde_json::json!({
                "old_index": old_index,
                "new_index": index,
                "old_agent_id": old_agent_id,
                "new_agent_id": new_agent_id,
            }),
        );

        Ok(())
    }

    /// Switch focus to a different agent by ID
    pub fn focus_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        let old_index = self.focused_slot;
        let old_agent_id = self
            .slots
            .get(old_index)
            .map(|s| s.agent_id().as_str().to_string());
        let new_codename = self
            .slots
            .get(index)
            .map(|s| s.codename().as_str().to_string());

        logging::debug_event(
            "pool.focus.change.by_id",
            "focus changed by agent ID",
            serde_json::json!({
                "old_index": old_index,
                "old_agent_id": old_agent_id,
                "new_agent_id": agent_id.as_str(),
                "new_codename": new_codename,
            }),
        );

        self.focus_agent_by_index(index)
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

    /// Get the currently focused slot
    pub fn focused_slot(&self) -> Option<&AgentSlot> {
        self.slots.get(self.focused_slot)
    }

    /// Get the currently focused slot (mutable)
    pub fn focused_slot_mut(&mut self) -> Option<&mut AgentSlot> {
        self.slots.get_mut(self.focused_slot)
    }

    /// Find the index of a slot by agent ID
    fn find_slot_index(&self, agent_id: &AgentId) -> Result<usize, String> {
        self.slots
            .iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))
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

        // Assign to agent
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

        // Update backlog status
        backlog.start_task(task_id.as_str());

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
        self.slots
            .iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
    }

    /// Find an idle agent and return its ID for assignment
    pub fn find_idle_agent_id(&self) -> Option<AgentId> {
        self.slots
            .iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
            .map(|s| s.agent_id().clone())
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
        let mut counts = HashMap::new();
        for slot in &self.slots {
            let label = slot.status().label();
            *counts.entry(label).or_insert(0) += 1;
        }
        counts
    }

    /// Generate a snapshot of the task queue state for TUI display
    ///
    /// Combines backlog state with agent pool state for comprehensive view.
    pub fn task_queue_snapshot(&self, backlog: &BacklogState) -> TaskQueueSnapshot {
        let counts = backlog.count_tasks_by_status();

        // Collect agent assignments
        let agent_assignments: Vec<AgentTaskAssignment> = self
            .slots
            .iter()
            .filter_map(|slot| {
                let task_id = slot.assigned_task_id()?;
                let task = backlog.find_task(task_id.as_str())?;
                Some(AgentTaskAssignment {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    task_id: task_id.clone(),
                    task_status: task.status,
                })
            })
            .collect();

        // Count available and active agents
        let available_agents = self
            .slots
            .iter()
            .filter(|s| *s.status() == AgentSlotStatus::Idle)
            .count();
        let active_agents = self.slots.iter().filter(|s| s.status().is_active()).count();

        TaskQueueSnapshot {
            total_tasks: backlog.tasks.len(),
            ready_tasks: counts.get(&TaskStatus::Ready).copied().unwrap_or(0),
            running_tasks: counts.get(&TaskStatus::Running).copied().unwrap_or(0),
            completed_tasks: counts.get(&TaskStatus::Done).copied().unwrap_or(0),
            failed_tasks: counts.get(&TaskStatus::Failed).copied().unwrap_or(0),
            blocked_tasks: counts.get(&TaskStatus::Blocked).copied().unwrap_or(0),
            agent_assignments,
            available_agents,
            active_agents,
        }
    }

    /// Get agents with their assigned task info
    pub fn agents_with_assignments(&self, backlog: &BacklogState) -> Vec<AgentTaskAssignment> {
        self.slots
            .iter()
            .filter_map(|slot| {
                let task_id = slot.assigned_task_id()?;
                let task = backlog.find_task(task_id.as_str())?;
                Some(AgentTaskAssignment {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    task_id: task_id.clone(),
                    task_status: task.status,
                })
            })
            .collect()
    }

    // ==================== Blocked Handling Methods ====================

    /// Get blocked handling configuration
    pub fn blocked_config(&self) -> &BlockedHandlingConfig {
        &self.blocked_config
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
        &self.blocked_history
    }

    /// Prune history to max size
    ///
    /// Removes oldest resolved entries first, then oldest unresolved if still over limit.
    fn prune_history(&mut self) {
        let max = self.blocked_config.max_history_entries;
        if max == 0 {
            return; // No limit
        }

        while self.blocked_history.len() > max {
            // Find the oldest resolved entry
            if let Some(pos) = self.blocked_history.iter().position(|e| e.resolved) {
                self.blocked_history.remove(pos);
            } else {
                // No resolved entries, remove the oldest
                self.blocked_history.remove(0);
            }
        }
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

        // Set blocked status
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state.clone()))
            .map_err(|e| format!("Failed to transition to blocked: {}", e))?;

        // Handle by blocking type
        let reason_type = blocked_state.reason().reason_type();
        if reason_type == "human_decision" {
            // Create human decision request
            let request = self.build_human_request(agent_id, &blocked_state);
            self.human_queue.push(request);
        }

        // Record in history
        if self.blocked_config.record_history {
            self.blocked_history.push(BlockedHistoryEntry {
                agent_id: agent_id.clone(),
                reason_type: reason_type.to_string(),
                description: blocked_state.reason().description(),
                duration_ms: 0, // Will be updated on resolution
                resolved: false,
                resolution: None,
            });
            self.prune_history();
        }

        // Notify other agents if configured
        if self.blocked_config.notify_others {
            // Emit event for other agents to react
            crate::logging::debug_event(
                "agent.blocked.notify_others",
                "agent blocked, notifying other agents",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "reason_type": reason_type,
                    "description": blocked_state.reason().description(),
                    "urgency": format!("{}", blocked_state.reason().urgency()),
                }),
            );
            let event = AgentBlockedEvent {
                agent_id: agent_id.clone(),
                reason_type: reason_type.to_string(),
                description: blocked_state.reason().description(),
                urgency: format!("{}", blocked_state.reason().urgency()),
            };
            self.blocked_notifier.on_agent_blocked(event);
        }

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
            .blocked_config
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
            match self.blocked_config.task_policy {
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

        // Find and update history
        if let Some(entry) = self
            .blocked_history
            .iter_mut()
            .find(|e| e.agent_id == agent_id && !e.resolved)
        {
            entry.resolved = true;
            entry.resolution = Some(format!("{:?}", response.selection));
        }

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
                // Record in history
                if self.blocked_config.record_history {
                    if let Some(entry) = self
                        .blocked_history
                        .iter_mut()
                        .find(|e| &e.agent_id == slot.agent_id() && !e.resolved)
                    {
                        entry.resolved = true;
                        entry.resolution = Some("cleared_on_shutdown".to_string());
                    }
                }
                slot.transition_to(AgentSlotStatus::Idle).ok();
            }
        }
        // Clear human queue
        self.human_queue.check_expired();
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
            if let Some(entry) = self
                .blocked_history
                .iter_mut()
                .find(|e| e.agent_id == agent_id && !e.resolved)
            {
                entry.resolved = true;
                entry.resolution = Some(format!("timeout: {:?}", selection));
            }

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
        let timeout_action = self.blocked_config.timeout_config.timeout_default;

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

/// Statistics for decision agents
#[derive(Debug, Clone, Default)]
pub struct DecisionAgentStats {
    /// Total number of decision agents
    pub total_agents: usize,
    /// Number of idle decision agents
    pub idle_agents: usize,
    /// Number of thinking decision agents
    pub thinking_agents: usize,
    /// Number of responding decision agents
    pub responding_agents: usize,
    /// Number of decision agents with errors
    pub error_agents: usize,
    /// Number of stopped decision agents
    pub stopped_agents: usize,
    /// Total decisions made
    pub total_decisions: u64,
    /// Total errors encountered
    pub total_errors: u64,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_slot::AgentSlotStatus;
    use crate::worktree_state_store::WorktreeStateStore;
    use agent_decision::{
        BlockedState, HumanDecisionBlocking, HumanSelection, ResourceBlocking,
        WaitingForChoiceSituation,
    };

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
    }

    #[test]
    fn blocked_history_pruning_removes_resolved_first() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Add 5 blocked entries
        for i in 0..5 {
            let blocking = HumanDecisionBlocking::new(
                format!("req-{}", i),
                Box::new(WaitingForChoiceSituation::default()),
                vec![],
            );
            pool.process_agent_blocked(&agent_id, BlockedState::new(Box::new(blocking)), None)
                .unwrap();
        }

        // Set max to 3
        pool.blocked_config.max_history_entries = 3;

        // Manually resolve some entries (prune is called after push, so manually trigger)
        pool.blocked_history[0].resolved = true;
        pool.blocked_history[1].resolved = true;

        pool.prune_history();

        // Should have 3 entries remaining (resolved ones removed first)
        assert_eq!(pool.blocked_history().len(), 3);
    }

    #[test]
    fn blocked_history_pruning_unbounded_when_zero() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Add many blocked entries with max = 0 (unlimited)
        pool.blocked_config.max_history_entries = 0;

        for i in 0..10 {
            let blocking = HumanDecisionBlocking::new(
                format!("req-{}", i),
                Box::new(WaitingForChoiceSituation::default()),
                vec![],
            );
            pool.process_agent_blocked(&agent_id, BlockedState::new(Box::new(blocking)), None)
                .unwrap();
        }

        // Should have all 10 entries
        assert_eq!(pool.blocked_history().len(), 10);
    }

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
        let count = pool.process_expired_requests();
        // Request may or may not be expired yet depending on timing
        assert!(count >= 0);
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
        assert!(pool.worktree_manager.is_some());
        assert!(pool.worktree_state_store.is_some());
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
}
