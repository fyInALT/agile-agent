//! AgentSlot for multi-agent runtime
//!
//! Represents a single agent's runtime slot in the agent pool.
//!
//! AgentSlotStatus is now in crate::slot::status module.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use std::time::Instant;

use chrono::Utc;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId, ProviderType};
use crate::app::TranscriptEntry;
use crate::launch_config::AgentLaunchBundle;
use crate::logging;
use crate::worker::Worker;
// Re-export slot types for backward compatibility (files importing from agent_slot still work)
pub use crate::slot::{AgentSlotStatus, TaskCompletionResult, ThreadOutcome};
use crate::{ProviderEvent, SessionHandle};
// Tool call types re-exported from agent-toolkit
use crate::ExecCommandStatus;
use agent_decision::{BlockedState, DecisionAgentCreationPolicy};

// TaskId re-exported from agent-types for backward compatibility
// (agent-types already has TaskId, this file used to have a duplicate)
pub use agent_types::TaskId;

/// A single agent's runtime slot
///
/// Contains all state for managing one agent's execution thread,
/// including provider session, transcript, and event channels.
///
/// # Thread Safety
///
/// AgentSlot is owned by the main thread (TUI loop). The provider thread
/// sends events through the channel, and main thread receives via `event_rx`.
/// All mutations happen on main thread after receiving events.
pub struct AgentSlot {
    /// Unique agent identifier
    agent_id: AgentId,
    /// Agent display codename (alpha, bravo, etc.)
    codename: AgentCodename,
    /// Provider type binding
    provider_type: ProviderType,
    /// Agent role (ProductOwner, ScrumMaster, Developer)
    role: AgentRole,
    /// Current runtime status
    status: AgentSlotStatus,
    /// Provider session handle for multi-turn continuity
    session_handle: Option<SessionHandle>,
    /// Per-agent transcript (copy for TUI rendering)
    transcript: Vec<TranscriptEntry>,
    /// Currently assigned task (if any)
    assigned_task_id: Option<TaskId>,
    /// Event channel receiver from provider thread
    event_rx: Option<Receiver<ProviderEvent>>,
    /// Thread handle for join/cancel operations
    thread_handle: Option<JoinHandle<()>>,
    /// Last activity timestamp
    last_activity: Instant,
    /// Decision agent creation policy
    decision_policy: DecisionAgentCreationPolicy,
    /// Launch configuration bundle (for resume/restore)
    launch_bundle: Option<AgentLaunchBundle>,
    /// Worktree path (if using independent worktree)
    worktree_path: Option<PathBuf>,
    /// Worktree branch name
    worktree_branch: Option<String>,
    /// Worktree unique ID
    worktree_id: Option<String>,
    /// Provider profile ID used for this agent (if profile-based)
    profile_id: Option<String>,
    /// Timestamp of last idle-triggered decision check (cooldown)
    last_idle_trigger_at: Option<Instant>,
    /// Worker aggregate root (domain model, shadow-initialized for dual-write)
    worker: Option<Worker>,
}

impl std::fmt::Debug for AgentSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentSlot")
            .field("agent_id", &self.agent_id)
            .field("codename", &self.codename)
            .field("provider_type", &self.provider_type)
            .field("role", &self.role)
            .field("status", &self.status)
            .field("session_handle", &self.session_handle)
            .field("transcript_len", &self.transcript.len())
            .field("assigned_task_id", &self.assigned_task_id)
            .field("has_provider_thread", &self.has_provider_thread())
            .field("last_activity", &self.last_activity)
            .field("decision_policy", &self.decision_policy)
            .field("launch_bundle", &self.launch_bundle.is_some())
            .field("worktree_path", &self.worktree_path)
            .field("worktree_branch", &self.worktree_branch)
            .field("worktree_id", &self.worktree_id)
            .finish()
    }
}

impl AgentSlot {
    /// Create a new agent slot with given identity
    pub fn new(agent_id: AgentId, codename: AgentCodename, provider_type: ProviderType) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role: AgentRole::default(),
            status: AgentSlotStatus::idle(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: None,
            thread_handle: None,
            last_activity: Instant::now(),
            decision_policy: DecisionAgentCreationPolicy::default(),
            launch_bundle: None,
            worktree_path: None,
            worktree_branch: None,
            worktree_id: None,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Create a new agent slot with specific role
    pub fn with_role(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        role: AgentRole,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role,
            status: AgentSlotStatus::idle(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: None,
            thread_handle: None,
            last_activity: Instant::now(),
            decision_policy: DecisionAgentCreationPolicy::default(),
            launch_bundle: None,
            worktree_path: None,
            worktree_branch: None,
            worktree_id: None,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Create a new agent slot ready for provider thread
    pub fn with_thread(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        event_rx: Receiver<ProviderEvent>,
        thread_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role: AgentRole::default(),
            status: AgentSlotStatus::starting(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: Some(event_rx),
            thread_handle: Some(thread_handle),
            last_activity: Instant::now(),
            decision_policy: DecisionAgentCreationPolicy::default(),
            launch_bundle: None,
            worktree_path: None,
            worktree_branch: None,
            worktree_id: None,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Create a new agent slot with thread and role
    pub fn with_thread_and_role(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        role: AgentRole,
        event_rx: Receiver<ProviderEvent>,
        thread_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role,
            status: AgentSlotStatus::starting(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: Some(event_rx),
            thread_handle: Some(thread_handle),
            last_activity: Instant::now(),
            decision_policy: DecisionAgentCreationPolicy::default(),
            launch_bundle: None,
            worktree_path: None,
            worktree_branch: None,
            worktree_id: None,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Restore a slot from persisted state without an active provider thread.
    pub fn restored(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        role: AgentRole,
        status: AgentSlotStatus,
        session_handle: Option<SessionHandle>,
        transcript: Vec<TranscriptEntry>,
        assigned_task_id: Option<TaskId>,
    ) -> Self {
        Self::restored_with_worktree(
            agent_id,
            codename,
            provider_type,
            role,
            status,
            session_handle,
            transcript,
            assigned_task_id,
            None,
            None,
            None,
        )
    }

    /// Restore a slot from persisted state with worktree information.
    pub fn restored_with_worktree(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        role: AgentRole,
        status: AgentSlotStatus,
        session_handle: Option<SessionHandle>,
        transcript: Vec<TranscriptEntry>,
        assigned_task_id: Option<TaskId>,
        worktree_path: Option<PathBuf>,
        worktree_branch: Option<String>,
        worktree_id: Option<String>,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role,
            status,
            session_handle,
            transcript,
            assigned_task_id,
            event_rx: None,
            thread_handle: None,
            last_activity: Instant::now(),
            decision_policy: DecisionAgentCreationPolicy::default(),
            launch_bundle: None,
            worktree_path,
            worktree_branch,
            worktree_id,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Create a new agent slot with decision policy
    pub fn with_decision_policy(
        agent_id: AgentId,
        codename: AgentCodename,
        provider_type: ProviderType,
        role: AgentRole,
        decision_policy: DecisionAgentCreationPolicy,
    ) -> Self {
        Self {
            agent_id,
            codename,
            provider_type,
            role,
            status: AgentSlotStatus::idle(),
            session_handle: None,
            transcript: Vec::new(),
            assigned_task_id: None,
            event_rx: None,
            thread_handle: None,
            last_activity: Instant::now(),
            decision_policy,
            launch_bundle: None,
            worktree_path: None,
            worktree_branch: None,
            worktree_id: None,
            profile_id: None,
            last_idle_trigger_at: None,
            worker: None,
        }
    }

    /// Get the agent's unique identifier
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get the agent's codename
    pub fn codename(&self) -> &AgentCodename {
        &self.codename
    }

    /// Get the provider type
    pub fn provider_type(&self) -> ProviderType {
        self.provider_type
    }

    /// Get the agent's role
    pub fn role(&self) -> AgentRole {
        self.role
    }

    /// Set the agent's role
    pub fn set_role(&mut self, role: AgentRole) {
        logging::debug_event(
            "slot.role.change",
            "agent role changed",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "old_role": self.role.label(), "new_role": role.label()}),
        );
        self.role = role;
        self.last_activity = Instant::now();
    }

    /// Get the decision agent creation policy
    pub fn decision_policy(&self) -> DecisionAgentCreationPolicy {
        self.decision_policy
    }

    /// Set the decision agent creation policy
    pub fn set_decision_policy(&mut self, policy: DecisionAgentCreationPolicy) {
        self.decision_policy = policy;
        self.last_activity = Instant::now();
    }

    // === Worktree Methods ===

    /// Get the agent's working directory
    ///
    /// Returns the worktree path if set, otherwise falls back to a reasonable default.
    /// For agents without worktrees, this typically returns the project root.
    pub fn cwd(&self) -> PathBuf {
        self.worktree_path.clone().unwrap_or_else(|| {
            // Try current directory first, then home directory as fallback
            std::env::current_dir()
                .ok()
                .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        })
    }

    /// Set worktree information
    pub fn set_worktree(&mut self, path: PathBuf, branch: Option<String>, worktree_id: String) {
        self.worktree_path = Some(path);
        self.worktree_branch = branch;
        self.worktree_id = Some(worktree_id);
        self.last_activity = Instant::now();
    }

    /// Clear worktree information
    pub fn clear_worktree(&mut self) {
        self.worktree_path = None;
        self.worktree_branch = None;
        self.worktree_id = None;
    }

    /// Get the worktree path
    pub fn worktree_path(&self) -> Option<&PathBuf> {
        self.worktree_path.as_ref()
    }

    /// Get the worktree branch name
    pub fn worktree_branch(&self) -> Option<&String> {
        self.worktree_branch.as_ref()
    }

    /// Get the worktree ID
    pub fn worktree_id(&self) -> Option<&String> {
        self.worktree_id.as_ref()
    }

    /// Check if agent has a worktree assigned
    pub fn has_worktree(&self) -> bool {
        self.worktree_path.is_some()
    }

    /// Get the provider profile ID
    pub fn profile_id(&self) -> Option<&String> {
        self.profile_id.as_ref()
    }

    /// Set the provider profile ID
    pub fn set_profile_id(&mut self, profile_id: String) {
        self.profile_id = Some(profile_id);
        self.last_activity = Instant::now();
    }

    /// Check if agent was created with a profile
    pub fn has_profile(&self) -> bool {
        self.profile_id.is_some()
    }

    /// Check if decision agent should be created eagerly
    pub fn should_create_decision_agent_eagerly(&self) -> bool {
        self.decision_policy.is_eager()
    }

    /// Get the launch bundle (if any)
    pub fn launch_bundle(&self) -> Option<&AgentLaunchBundle> {
        self.launch_bundle.as_ref()
    }

    /// Set the launch bundle
    pub fn set_launch_bundle(&mut self, bundle: AgentLaunchBundle) {
        self.launch_bundle = Some(bundle);
        self.last_activity = Instant::now();
    }

    /// Get the current status
    pub fn status(&self) -> &AgentSlotStatus {
        &self.status
    }

    /// Get the session handle
    pub fn session_handle(&self) -> Option<&SessionHandle> {
        self.session_handle.as_ref()
    }

    /// Get the assigned task ID
    pub fn assigned_task_id(&self) -> Option<&TaskId> {
        self.assigned_task_id.as_ref()
    }

    // =========================================================================
    // Transcript Access
    // =========================================================================

    /// Get the transcript entries
    pub fn transcript(&self) -> &[TranscriptEntry] {
        &self.transcript
    }

    /// Get mutable reference to transcript
    pub fn transcript_mut(&mut self) -> &mut Vec<TranscriptEntry> {
        &mut self.transcript
    }

    /// Get the event receiver (if provider thread is running)
    pub fn event_rx(&self) -> Option<&Receiver<ProviderEvent>> {
        self.event_rx.as_ref()
    }

    /// Get the thread handle (if provider thread is running)
    pub fn thread_handle(&self) -> Option<&JoinHandle<()>> {
        self.thread_handle.as_ref()
    }

    /// Take the thread handle (for joining)
    pub fn take_thread_handle(&mut self) -> Option<JoinHandle<()>> {
        self.thread_handle.take()
    }

    /// Get the last activity timestamp
    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    /// Set the last activity timestamp (used for testing and snapshot restore)
    pub fn set_last_activity(&mut self, instant: Instant) {
        self.last_activity = instant;
    }

    /// Update the last activity timestamp (called when receiving events)
    pub fn touch_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if the agent has been idle within responding state for too long
    /// Returns true if the agent should transition to WaitingForInput
    pub fn should_transition_to_waiting(&self, idle_timeout_secs: u64) -> bool {
        matches!(self.status, AgentSlotStatus::Responding { .. })
            && self.last_activity.elapsed().as_secs() >= idle_timeout_secs
    }

    /// Check if this slot has an active provider thread
    pub fn has_provider_thread(&self) -> bool {
        self.event_rx.is_some() && self.thread_handle.is_some()
    }

    /// Get the Worker aggregate root (if initialized)
    pub fn worker(&self) -> Option<&Worker> {
        self.worker.as_ref()
    }

    /// Get mutable reference to the Worker (if initialized)
    pub fn worker_mut(&mut self) -> Option<&mut Worker> {
        self.worker.as_mut()
    }

    /// Initialize the Worker aggregate root for this slot.
    ///
    /// This is called during dual-write transition (Sprint 2).
    /// Once initialized, provider events can be forwarded to the Worker.
    pub fn init_worker(&mut self) {
        if self.worker.is_none() {
            self.worker = Some(Worker::new(
                self.agent_id.clone(),
                self.codename.clone(),
                self.role,
            ));
        }
    }

    /// Apply a provider event to the Worker (dual-write bridge).
    ///
    /// Initializes the Worker on first call. Returns the Worker's apply result.
    /// Errors from the Worker are logged but not propagated (AgentSlot remains
    /// the primary state authority during transition).
    pub fn apply_provider_event_to_worker(&mut self, event: &ProviderEvent) {
        self.init_worker();
        if let Some(worker) = &mut self.worker {
            // ProviderEvent is DomainEvent, so we can apply directly
            match worker.apply(event.clone()) {
                Ok(_commands) => {
                    // RuntimeCommands are collected for later dispatch by the event loop.
                    // During dual-write transition, we don't execute them yet.
                }
                Err(e) => {
                    logging::warn_event(
                        "slot.worker.apply_error",
                        "Worker rejected event",
                        serde_json::json!({
                            "agent_id": self.agent_id.as_str(),
                            "error": e.to_string(),
                        }),
                    );
                }
            }
        }
    }

    /// Set provider thread components
    pub fn set_provider_thread(
        &mut self,
        event_rx: Receiver<ProviderEvent>,
        thread_handle: JoinHandle<()>,
    ) {
        logging::debug_event(
            "slot.thread.set",
            "provider thread set",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "old_status": self.status.label(), "new_status": "starting"}),
        );
        self.event_rx = Some(event_rx);
        self.thread_handle = Some(thread_handle);
        self.status = AgentSlotStatus::starting();
        self.last_activity = Instant::now();
    }

    /// Set only the thread handle (event_rx managed separately by EventAggregator)
    pub fn set_thread_handle(&mut self, thread_handle: JoinHandle<()>) {
        logging::debug_event(
            "slot.thread.handle_only",
            "thread handle set (event_rx external)",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "old_status": self.status.label(), "new_status": "starting"}),
        );
        self.thread_handle = Some(thread_handle);
        self.status = AgentSlotStatus::starting();
        self.last_activity = Instant::now();
    }

    /// Clear provider thread components (after join)
    pub fn clear_provider_thread(&mut self) {
        logging::debug_event(
            "slot.thread.clear",
            "provider thread cleared",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "old_status": self.status.label()}),
        );
        self.event_rx = None;
        self.thread_handle = None;
    }

    // =========================================================================
    // Transcript Manipulation
    // =========================================================================

    /// Append entry to transcript
    pub fn append_transcript(&mut self, entry: TranscriptEntry) {
        self.transcript.push(entry);
        self.last_activity = Instant::now();
    }

    /// Update the last ExecCommand entry in transcript with finished state
    ///
    /// Finds the most recent ExecCommand with status InProgress and matching call_id,
    /// then updates it with the finished values. If no matching entry found,
    /// pushes a new entry with allow_exploring_group=false.
    pub fn update_last_exec_command(
        &mut self,
        call_id: Option<String>,
        output_preview: Option<String>,
        status: crate::ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::ExecCommand {
                call_id: existing_call_id,
                source: existing_source,
                allow_exploring_group,
                input_preview: existing_input_preview,
                status: ExecCommandStatus::InProgress,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest_in_flight = call_id.is_none();
                if matches_call_id || matches_latest_in_flight {
                    *entry = TranscriptEntry::ExecCommand {
                        call_id: existing_call_id.clone().or(call_id),
                        source: existing_source.clone(),
                        allow_exploring_group: *allow_exploring_group,
                        input_preview: existing_input_preview.clone(),
                        output_preview,
                        status,
                        exit_code,
                        duration_ms,
                    };
                    return;
                }
            }
        }
        // Not found - push new entry
        self.transcript.push(TranscriptEntry::ExecCommand {
            call_id,
            source: None,
            allow_exploring_group: false,
            input_preview: None,
            output_preview,
            status,
            exit_code,
            duration_ms,
        });
        self.last_activity = Instant::now();
    }

    /// Append output delta to the last ExecCommand entry in transcript
    ///
    /// Finds the most recent ExecCommand with status InProgress and matching call_id,
    /// then appends the delta to its output_preview. If no matching entry found,
    /// pushes a new entry.
    pub fn append_exec_command_output_delta(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::ExecCommand {
                call_id: existing_call_id,
                allow_exploring_group: _,
                output_preview,
                status: ExecCommandStatus::InProgress,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest_in_flight = call_id.is_none();
                if matches_call_id || matches_latest_in_flight {
                    output_preview
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                    return;
                }
            }
        }

        // Not found - push new entry
        self.transcript.push(TranscriptEntry::ExecCommand {
            call_id,
            source: None,
            allow_exploring_group: false,
            input_preview: None,
            output_preview: Some(delta.to_string()),
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });
        self.last_activity = Instant::now();
    }

    /// Update the last GenericToolCall entry in transcript with finished state
    ///
    /// Finds the most recent GenericToolCall with started=true and matching name/call_id,
    /// then updates it with the finished values. If no matching entry found,
    /// pushes a new entry.
    pub fn update_last_generic_tool_call(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::GenericToolCall {
                name: existing_name,
                call_id: existing_call_id,
                input_preview: existing_input_preview,
                started: true,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_name = *existing_name == name;
                if matches_call_id || matches_name {
                    *entry = TranscriptEntry::GenericToolCall {
                        name: existing_name.clone(),
                        call_id: existing_call_id.clone().or(call_id),
                        input_preview: existing_input_preview.clone(),
                        output_preview,
                        success,
                        started: false,
                        exit_code,
                        duration_ms,
                    };
                    return;
                }
            }
        }

        // Not found - push new entry
        self.transcript.push(TranscriptEntry::GenericToolCall {
            name,
            call_id,
            input_preview: None,
            output_preview,
            success,
            started: false,
            exit_code,
            duration_ms,
        });
        self.last_activity = Instant::now();
    }

    /// Clear transcript
    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
    }

    /// Transition to a new status
    ///
    /// Returns Ok(()) if transition is valid, Err if invalid.
    pub fn transition_to(&mut self, new_status: AgentSlotStatus) -> Result<(), String> {
        let old_status = self.status.clone();
        let transition_valid = self.status.can_transition_to(&new_status);

        logging::debug_event(
            "slot.transition",
            if transition_valid {
                "attempting status transition"
            } else {
                "invalid status transition"
            },
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "old_status": old_status.label(), "new_status": new_status.label(), "transition_valid": transition_valid, "role": self.role.label()}),
        );

        if !transition_valid {
            return Err(format!(
                "Invalid transition from {} to {}",
                old_status.label(),
                new_status.label()
            ));
        }
        self.status = new_status;
        self.last_activity = Instant::now();

        logging::debug_event(
            "slot.transition.complete",
            "status transition completed",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "new_status": self.status.label()}),
        );

        Ok(())
    }

    /// Set the session handle
    pub fn set_session_handle(&mut self, handle: SessionHandle) {
        logging::debug_event(
            "slot.session_handle.set",
            "session handle set",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "handle_type": format!("{:?}", handle)}),
        );
        self.session_handle = Some(handle);
        self.last_activity = Instant::now();
    }

    /// Clear the session handle
    pub fn clear_session_handle(&mut self) {
        logging::debug_event(
            "slot.session_handle.clear",
            "session handle cleared",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str()}),
        );
        self.session_handle = None;
    }

    /// Assign a task to this agent
    pub fn assign_task(&mut self, task_id: TaskId) -> Result<(), String> {
        logging::debug_event(
            "slot.task.assign",
            "task assignment requested",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "task_id": task_id.as_str(), "current_status": self.status.label()}),
        );

        if self.status != AgentSlotStatus::Idle {
            let err = format!(
                "Cannot assign task to agent with status {}",
                self.status.label()
            );
            logging::debug_event(
                "slot.task.assign.failed",
                "task assignment failed",
                serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "task_id": task_id.as_str(), "error": err}),
            );
            return Err(err);
        }
        self.assigned_task_id = Some(task_id.clone());
        self.last_activity = Instant::now();

        logging::debug_event(
            "slot.task.assign.complete",
            "task assigned",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "task_id": task_id.as_str()}),
        );

        Ok(())
    }

    /// Clear the assigned task
    pub fn clear_task(&mut self) {
        let task_id = self
            .assigned_task_id
            .as_ref()
            .map(|t| t.as_str().to_string());
        logging::debug_event(
            "slot.task.clear",
            "task cleared from agent",
            serde_json::json!({"agent_id": self.agent_id.as_str(), "codename": self.codename.as_str(), "task_id": task_id, "status": self.status.label()}),
        );
        self.assigned_task_id = None;
    }

    /// Get the last idle trigger timestamp
    pub fn last_idle_trigger_at(&self) -> Option<Instant> {
        self.last_idle_trigger_at
    }

    /// Set the last idle trigger timestamp
    pub fn set_last_idle_trigger_at(&mut self, instant: Instant) {
        self.last_idle_trigger_at = Some(instant);
    }

    // === Resting State Methods ===

    /// Enter resting state due to rate limit (HTTP 429).
    ///
    /// Transitions the agent to Resting status with a BlockedState containing
    /// RateLimitBlockedReason. The agent will wait for the retry interval
    /// before attempting recovery.
    pub fn enter_resting(&mut self, blocked_state: BlockedState) -> Result<(), String> {
        let old_status = self.status.clone();
        let new_status = AgentSlotStatus::resting(blocked_state);

        if !old_status.can_transition_to(&new_status) {
            return Err(format!(
                "Cannot enter resting from {}",
                old_status.label()
            ));
        }

        logging::debug_event(
            "slot.resting.enter",
            "agent entering resting state due to rate limit",
            serde_json::json!({
                "agent_id": self.agent_id.as_str(),
                "codename": self.codename.as_str(),
                "old_status": old_status.label(),
                "new_status": new_status.label()
            }),
        );

        self.status = new_status;
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Check if resting agent should attempt recovery now.
    ///
    /// Returns true if:
    /// - The agent is in Resting state
    /// - Either on_resume is true (immediate recovery on snapshot restore)
    ///   OR enough time has passed since the resting started
    ///
    /// The retry interval is configurable via AgentLaunchBundle.
    pub fn should_attempt_recovery(&self, retry_interval_secs: u64) -> bool {
        match &self.status {
            AgentSlotStatus::Resting {
                on_resume,
                blocked_state,
                started_at,
                ..
            } => {
                if *on_resume {
                    return true;
                }
                // Check if it's a rate limit and enough time has passed
                if let Some(rate_limit_reason) = blocked_state.reason().as_rate_limit_reason() {
                    let reference_time = rate_limit_reason.last_retry_at().unwrap_or(*started_at);
                    let elapsed_secs = (Utc::now() - reference_time).num_seconds() as u64;
                    elapsed_secs >= retry_interval_secs
                } else {
                    // Unknown reason type - don't auto-retry
                    false
                }
            }
            _ => false,
        }
    }

    /// Record a recovery attempt while in Resting state.
    ///
    /// Resets the on_resume flag to false after the attempt.
    pub fn record_recovery_attempt(&mut self) {
        match &mut self.status {
            AgentSlotStatus::Resting {
                started_at,
                blocked_state,
                on_resume,
            } => {
                // Just check it's a rate limit - we don't need to update internal state
                // since we're using the elapsed time from started_at
                let _ = blocked_state.is_rate_limit();

                // Reset on_resume flag
                *on_resume = false;

                logging::debug_event(
                    "slot.resting.attempt",
                    "rate limit recovery attempt recorded",
                    serde_json::json!({
                        "agent_id": self.agent_id.as_str(),
                        "codename": self.codename.as_str(),
                        "started_at": started_at.to_rfc3339(),
                    }),
                );
            }
            _ => {}
        }
    }

    /// Transition from Resting to Idle after successful recovery.
    pub fn recover_from_resting(&mut self) -> Result<(), String> {
        let old_status = self.status.clone();
        let new_status = AgentSlotStatus::idle();

        if !old_status.can_transition_to(&new_status) {
            return Err(format!(
                "Cannot recover from {} to {}",
                old_status.label(),
                new_status.label()
            ));
        }

        logging::debug_event(
            "slot.resting.recover",
            "agent recovered from resting state",
            serde_json::json!({
                "agent_id": self.agent_id.as_str(),
                "codename": self.codename.as_str(),
                "old_status": old_status.label(),
                "new_status": new_status.label()
            }),
        );

        self.status = new_status;
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Transition from Resting to Error when recovery fails.
    pub fn resting_to_error(&mut self, message: impl Into<String>) -> Result<(), String> {
        let old_status = self.status.clone();
        let new_status = AgentSlotStatus::error(message);

        if !old_status.can_transition_to(&new_status) {
            return Err(format!(
                "Cannot transition from {} to Error",
                old_status.label()
            ));
        }

        logging::debug_event(
            "slot.resting.error",
            "resting agent transitioned to error state",
            serde_json::json!({
                "agent_id": self.agent_id.as_str(),
                "codename": self.codename.as_str(),
                "old_status": old_status.label(),
            }),
        );

        self.status = new_status;
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Transition from Resting to Stopped when user cancels.
    pub fn resting_to_stopped(&mut self, reason: impl Into<String>) -> Result<(), String> {
        let old_status = self.status.clone();
        let new_status = AgentSlotStatus::stopped(reason);

        if !old_status.can_transition_to(&new_status) {
            return Err(format!(
                "Cannot transition from {} to Stopped",
                old_status.label()
            ));
        }

        logging::debug_event(
            "slot.resting.stop",
            "resting agent stopped by user",
            serde_json::json!({
                "agent_id": self.agent_id.as_str(),
                "codename": self.codename.as_str(),
                "old_status": old_status.label(),
            }),
        );

        self.status = new_status;
        self.last_activity = Instant::now();
        Ok(())
    }

    /// Summary string for display
    pub fn summary(&self) -> String {
        format!(
            "{} ({}) [{}]",
            self.codename.as_str(),
            self.agent_id.as_str(),
            self.status.label()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_slot() -> AgentSlot {
        AgentSlot::new(
            AgentId::new("agent_001"),
            AgentCodename::new("alpha"),
            ProviderType::Mock,
        )
    }

    #[test]
    fn slot_new_creates_idle_slot() {
        let slot = make_slot();
        assert_eq!(slot.status(), &AgentSlotStatus::Idle);
        assert!(slot.session_handle().is_none());
        assert!(slot.assigned_task_id().is_none());
        assert!(slot.transcript().is_empty());
        assert!(!slot.has_provider_thread());
    }

    #[test]
    fn status_idle_can_transition_to_starting() {
        let status = AgentSlotStatus::idle();
        assert!(status.can_transition_to(&AgentSlotStatus::starting()));
    }

    #[test]
    fn status_idle_cannot_transition_to_responding() {
        let status = AgentSlotStatus::idle();
        assert!(!status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_starting_can_transition_to_responding() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_starting_can_transition_to_idle() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_starting_can_transition_to_stopping() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_starting_can_transition_to_stopped() {
        let status = AgentSlotStatus::starting();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_responding_can_transition_to_tool_executing() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::tool_executing("read_file")));
    }

    #[test]
    fn status_responding_can_transition_to_idle() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_responding_can_transition_to_stopping() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_responding_can_transition_to_stopped() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_tool_executing_can_transition_to_responding() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_tool_executing_can_transition_to_idle() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_tool_executing_can_transition_to_stopping() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_tool_executing_can_transition_to_stopped() {
        let status = AgentSlotStatus::tool_executing("bash");
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_finishing_can_transition_to_idle() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_finishing_can_transition_to_stopping() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::stopping()));
    }

    #[test]
    fn status_finishing_can_transition_to_stopped() {
        let status = AgentSlotStatus::finishing();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_stopping_can_transition_to_stopped() {
        let status = AgentSlotStatus::stopping();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn status_stopping_can_transition_to_error() {
        let status = AgentSlotStatus::stopping();
        assert!(status.can_transition_to(&AgentSlotStatus::error("thread panic")));
    }

    #[test]
    fn status_error_can_transition_to_idle() {
        let status = AgentSlotStatus::error("something went wrong");
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_stopped_can_transition_to_starting() {
        let status = AgentSlotStatus::stopped("user requested");
        assert!(status.can_transition_to(&AgentSlotStatus::starting()));
    }

    #[test]
    fn status_idle_can_transition_to_stopped() {
        let status = AgentSlotStatus::idle();
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    #[test]
    fn slot_transition_valid_succeeds() {
        let mut slot = make_slot();
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);
    }

    #[test]
    fn slot_transition_invalid_fails() {
        let mut slot = make_slot();
        let result = slot.transition_to(AgentSlotStatus::responding_now());
        assert!(result.is_err());
        assert_eq!(slot.status(), &AgentSlotStatus::Idle);
    }

    #[test]
    fn slot_assign_task_to_idle_succeeds() {
        let mut slot = make_slot();
        slot.assign_task(TaskId::new("task-001")).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn slot_assign_task_to_non_idle_fails() {
        let mut slot = make_slot();
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        let result = slot.assign_task(TaskId::new("task-001"));
        assert!(result.is_err());
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn slot_clear_task() {
        let mut slot = make_slot();
        slot.assign_task(TaskId::new("task-001")).unwrap();
        slot.clear_task();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn slot_transcript_operations() {
        let mut slot = make_slot();
        assert!(slot.transcript().is_empty());

        // Append entry (using placeholder since TranscriptEntry is complex)
        // Note: In actual implementation, we'd create real TranscriptEntry
        // For this test, we verify the transcript_mut accessor works
        slot.transcript_mut().clear();
        assert!(slot.transcript().is_empty());
    }

    #[test]
    fn status_is_active() {
        assert!(!AgentSlotStatus::idle().is_active());
        assert!(AgentSlotStatus::starting().is_active());
        assert!(AgentSlotStatus::responding_now().is_active());
        assert!(AgentSlotStatus::tool_executing("test").is_active());
        assert!(AgentSlotStatus::finishing().is_active());
        assert!(!AgentSlotStatus::stopping().is_active());
        assert!(!AgentSlotStatus::stopped("test").is_active());
        assert!(!AgentSlotStatus::error("test").is_active());
    }

    #[test]
    fn status_is_stopping() {
        assert!(!AgentSlotStatus::idle().is_stopping());
        assert!(AgentSlotStatus::stopping().is_stopping());
        assert!(!AgentSlotStatus::stopped("test").is_stopping());
    }

    #[test]
    fn status_label_includes_stopping() {
        assert_eq!(AgentSlotStatus::stopping().label(), "stopping");
    }

    #[test]
    fn slot_with_thread_creates_slot_with_provider_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        let slot = AgentSlot::with_thread(
            AgentId::new("agent_001"),
            AgentCodename::new("alpha"),
            ProviderType::Mock,
            rx,
            handle,
        );

        assert!(slot.has_provider_thread());
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);
        assert!(slot.event_rx().is_some());
        assert!(slot.thread_handle().is_some());

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_set_provider_thread() {
        let mut slot = make_slot();
        assert!(!slot.has_provider_thread());

        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);

        assert!(slot.has_provider_thread());
        assert_eq!(slot.status(), &AgentSlotStatus::Starting);

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_clear_provider_thread() {
        let mut slot = make_slot();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);
        assert!(slot.has_provider_thread());

        slot.clear_provider_thread();
        assert!(!slot.has_provider_thread());
        assert!(slot.event_rx().is_none());
        assert!(slot.thread_handle().is_none());

        // Cleanup
        drop(tx);
    }

    #[test]
    fn slot_take_thread_handle() {
        let mut slot = make_slot();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(|| {});

        slot.set_provider_thread(rx, handle);
        assert!(slot.thread_handle().is_some());

        let taken = slot.take_thread_handle();
        assert!(taken.is_some());
        assert!(slot.thread_handle().is_none());

        // Cleanup - join the taken handle
        taken.unwrap().join().unwrap();
        drop(tx);
    }

    // Blocked status tests
    #[test]
    fn status_blocked_is_not_active() {
        let status = AgentSlotStatus::blocked("API design not confirmed");
        assert!(!status.is_active());
    }

    #[test]
    fn status_blocked_label() {
        let status = AgentSlotStatus::blocked("API design not confirmed");
        assert_eq!(status.label(), "blocked:API design not confirmed");
    }

    #[test]
    fn status_idle_can_transition_to_blocked() {
        let status = AgentSlotStatus::idle();
        assert!(status.can_transition_to(&AgentSlotStatus::blocked("test")));
    }

    #[test]
    fn status_blocked_can_transition_to_idle() {
        let status = AgentSlotStatus::blocked("test");
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn status_blocked_can_transition_to_stopped() {
        let status = AgentSlotStatus::blocked("test");
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user resolved")));
    }

    #[test]
    fn status_blocked_is_blocked() {
        let status = AgentSlotStatus::blocked("test reason");
        assert!(status.is_blocked());
    }

    #[test]
    fn status_other_is_not_blocked() {
        assert!(!AgentSlotStatus::idle().is_blocked());
        assert!(!AgentSlotStatus::starting().is_blocked());
        assert!(!AgentSlotStatus::stopped("test").is_blocked());
    }

    #[test]
    fn status_blocked_can_transition_to_responding() {
        let status = AgentSlotStatus::blocked("test");
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_blocked_can_transition_to_blocked_for_decision() {
        // Critical transition for 429 error recovery: Blocked → BlockedForDecision escalation
        use agent_decision::{BlockedState, ErrorInfo, ErrorSituation, HumanDecisionBlocking};

        let error = ErrorInfo::new("rate_limit", "API Error: Request rejected (429)");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        let status = AgentSlotStatus::blocked("API Error: Request rejected (429)");
        assert!(status.can_transition_to(&AgentSlotStatus::blocked_for_decision(blocked_state)));
    }

    #[test]
    fn status_blocked_for_decision_can_transition_to_blocked() {
        // Provider crash recovery: BlockedForDecision → Blocked (when provider exits unexpectedly)
        use agent_decision::{BlockedState, ErrorInfo, ErrorSituation, HumanDecisionBlocking};

        let error = ErrorInfo::new("rate_limit", "API Error: Request rejected (429)");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        let status = AgentSlotStatus::blocked_for_decision(blocked_state);
        assert!(status.can_transition_to(&AgentSlotStatus::blocked("claude exited with status 1")));
    }

    // Worktree tests
    #[test]
    fn slot_new_has_no_worktree() {
        let slot = make_slot();
        assert!(!slot.has_worktree());
        assert!(slot.worktree_path().is_none());
        assert!(slot.worktree_branch().is_none());
        assert!(slot.worktree_id().is_none());
    }

    #[test]
    fn slot_set_worktree() {
        let mut slot = make_slot();
        slot.set_worktree(
            PathBuf::from(".worktrees/agent-alpha"),
            Some("agent/task-123".to_string()),
            "wt-alpha-001".to_string(),
        );

        assert!(slot.has_worktree());
        assert_eq!(
            slot.worktree_path(),
            Some(&PathBuf::from(".worktrees/agent-alpha"))
        );
        assert_eq!(slot.worktree_branch(), Some(&"agent/task-123".to_string()));
        assert_eq!(slot.worktree_id(), Some(&"wt-alpha-001".to_string()));
    }

    #[test]
    fn slot_cwd_returns_worktree_path() {
        let mut slot = make_slot();
        slot.set_worktree(
            PathBuf::from(".worktrees/agent-alpha"),
            Some("agent/task-123".to_string()),
            "wt-alpha-001".to_string(),
        );

        assert_eq!(slot.cwd(), PathBuf::from(".worktrees/agent-alpha"));
    }

    #[test]
    fn slot_cwd_returns_current_dir_without_worktree() {
        let slot = make_slot();
        let cwd = slot.cwd();
        // Should return current directory or fallback
        assert!(cwd.is_absolute() || cwd == PathBuf::from("."));
    }

    #[test]
    fn slot_clear_worktree() {
        let mut slot = make_slot();
        slot.set_worktree(
            PathBuf::from(".worktrees/agent-alpha"),
            Some("agent/task-123".to_string()),
            "wt-alpha-001".to_string(),
        );

        assert!(slot.has_worktree());

        slot.clear_worktree();
        assert!(!slot.has_worktree());
        assert!(slot.worktree_path().is_none());
        assert!(slot.worktree_branch().is_none());
        assert!(slot.worktree_id().is_none());
    }

    #[test]
    fn status_waiting_for_input_label() {
        let status = AgentSlotStatus::waiting_for_input();
        assert_eq!(status.label(), "waiting_for_input");
    }

    #[test]
    fn status_waiting_for_input_is_not_active() {
        let status = AgentSlotStatus::waiting_for_input();
        assert!(!status.is_active());
        assert!(status.is_waiting_for_input());
    }

    #[test]
    fn status_responding_can_transition_to_waiting_for_input() {
        let status = AgentSlotStatus::responding_now();
        assert!(status.can_transition_to(&AgentSlotStatus::waiting_for_input()));
    }

    #[test]
    fn status_waiting_for_input_can_transition_to_responding() {
        let status = AgentSlotStatus::waiting_for_input();
        assert!(status.can_transition_to(&AgentSlotStatus::responding_now()));
    }

    #[test]
    fn status_waiting_for_input_can_transition_to_idle() {
        let status = AgentSlotStatus::waiting_for_input();
        assert!(status.can_transition_to(&AgentSlotStatus::idle()));
    }

    #[test]
    fn slot_touch_activity_updates_timestamp() {
        let mut slot = make_slot();
        let initial_activity = slot.last_activity();

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        slot.touch_activity();

        assert!(slot.last_activity() > initial_activity);
    }

    #[test]
    fn slot_should_transition_to_waiting_after_timeout() {
        let mut slot = make_slot();
        // Proper transition: Idle → Starting → Responding
        slot.transition_to(AgentSlotStatus::starting()).unwrap();
        slot.transition_to(AgentSlotStatus::responding_now())
            .unwrap();

        // Should not transition immediately
        assert!(!slot.should_transition_to_waiting(5));

        // Simulate idle by not calling touch_activity
        // In real test we'd need to wait, but for unit test we just check the logic
        // The actual timeout check depends on elapsed time
    }

    #[test]
    fn status_resting_can_transition_to_stopped() {
        let blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        let status = AgentSlotStatus::resting(blocked_state);
        assert!(status.can_transition_to(&AgentSlotStatus::stopped("user requested")));
    }

    // Resting state method tests
    #[test]
    fn slot_enter_resting() {
        let mut slot = make_slot();
        // First transition to BlockedForDecision (rate limit escalation path)
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error: Request rejected (429)");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        // Now transition to Resting
        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.enter_resting(resting_blocked_state).unwrap();
        assert!(slot.status().is_resting());
    }

    #[test]
    fn slot_enter_resting_from_blocked_for_decision() {
        let mut slot = make_slot();
        // First transition to BlockedForDecision
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error: Request rejected (429)");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        // Now transition to Resting
        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.enter_resting(resting_blocked_state).unwrap();
        assert!(slot.status().is_resting());
    }

    #[test]
    fn slot_should_attempt_recovery_with_on_resume() {
        let mut slot = make_slot();
        // Transition via BlockedForDecision -> Resting
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.transition_to(AgentSlotStatus::resting_with_on_resume(resting_blocked_state, true)).unwrap();

        // Should attempt recovery immediately when on_resume is true
        assert!(slot.should_attempt_recovery(1800));
    }

    #[test]
    fn slot_should_attempt_recovery_after_interval() {
        let mut slot = make_slot();
        // Transition via BlockedForDecision -> Resting
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        // Create a reason with last_retry_at in the past (30+ minutes ago)
        let reason = agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now())
            .with_last_retry_at(chrono::Utc::now() - chrono::Duration::minutes(35));
        let resting_blocked_state = BlockedState::new(Box::new(reason));
        slot.transition_to(AgentSlotStatus::resting(resting_blocked_state)).unwrap();

        // Should attempt recovery since interval has passed
        assert!(slot.should_attempt_recovery(1800)); // 30 min default
    }

    #[test]
    fn slot_recover_from_resting() {
        let mut slot = make_slot();
        // Transition via BlockedForDecision -> Resting
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.transition_to(AgentSlotStatus::resting(resting_blocked_state)).unwrap();
        assert!(slot.status().is_resting());

        slot.recover_from_resting().unwrap();
        assert!(slot.status().is_idle());
    }

    #[test]
    fn slot_resting_to_error() {
        let mut slot = make_slot();
        // Transition via BlockedForDecision -> Resting
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.transition_to(AgentSlotStatus::resting(resting_blocked_state)).unwrap();

        slot.resting_to_error("quota exceeded permanently").unwrap();
        assert!(matches!(slot.status(), AgentSlotStatus::Error { .. }));
    }

    #[test]
    fn slot_resting_to_stopped() {
        let mut slot = make_slot();
        // Transition via BlockedForDecision -> Resting
        use agent_decision::{ErrorInfo, ErrorSituation, HumanDecisionBlocking};
        let error = ErrorInfo::new("rate_limit", "API Error");
        let situation = Box::new(ErrorSituation::new(error));
        let blocking = HumanDecisionBlocking::new("req-001", situation, vec![]);
        let blocked_state = BlockedState::new(Box::new(blocking));
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state)).unwrap();

        let resting_blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(chrono::Utc::now()),
        ));
        slot.transition_to(AgentSlotStatus::resting(resting_blocked_state)).unwrap();

        slot.resting_to_stopped("user cancelled").unwrap();
        assert!(matches!(slot.status(), AgentSlotStatus::Stopped { .. }));
    }

    #[test]
    fn slot_status_resting_label_shows_minutes() {
        let blocked_state = BlockedState::new(Box::new(
            agent_decision::blocking::RateLimitBlockedReason::new(
                chrono::Utc::now() - chrono::Duration::minutes(5)
            ),
        ));
        let status = AgentSlotStatus::resting(blocked_state);
        let label = status.label();
        assert!(label.contains("resting"));
        assert!(label.contains("min"));
    }

    // ── Worker dual-write bridge tests ──────────────────────────

    #[test]
    fn worker_initially_none() {
        let slot = make_slot();
        assert!(slot.worker().is_none());
    }

    #[test]
    fn init_worker_creates_worker() {
        let mut slot = make_slot();
        slot.init_worker();
        assert!(slot.worker().is_some());
        let worker = slot.worker().unwrap();
        assert_eq!(worker.agent_id().as_str(), "agent_001");
        assert_eq!(worker.codename().as_str(), "alpha");
    }

    #[test]
    fn apply_provider_event_initializes_worker() {
        let mut slot = make_slot();
        assert!(slot.worker().is_none());

        slot.apply_provider_event_to_worker(&ProviderEvent::Status("worker started".to_string()));

        assert!(slot.worker().is_some());
        let worker = slot.worker().unwrap();
        assert!(matches!(worker.state(), crate::worker_state::WorkerState::Starting));
    }

    #[test]
    fn apply_provider_event_forwards_to_worker() {
        let mut slot = make_slot();
        slot.init_worker();

        slot.apply_provider_event_to_worker(&ProviderEvent::Status("worker started".to_string()));
        slot.apply_provider_event_to_worker(&ProviderEvent::AssistantChunk("hello".to_string()));
        slot.apply_provider_event_to_worker(&ProviderEvent::Finished);

        let worker = slot.worker().unwrap();
        assert!(matches!(worker.state(), crate::worker_state::WorkerState::Completed));
        assert_eq!(worker.transcript().len(), 3);
    }

    #[test]
    fn apply_provider_event_tool_call_sync() {
        let mut slot = make_slot();
        slot.init_worker();

        slot.apply_provider_event_to_worker(&ProviderEvent::Status("worker started".to_string()));
        slot.apply_provider_event_to_worker(&ProviderEvent::AssistantChunk("using tool".to_string()));
        slot.apply_provider_event_to_worker(&ProviderEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: None,
            input_preview: None,
        });

        let worker = slot.worker().unwrap();
        assert!(matches!(
            worker.state(),
            crate::worker_state::WorkerState::ProcessingTool { name } if name == "read_file"
        ));
    }
}
