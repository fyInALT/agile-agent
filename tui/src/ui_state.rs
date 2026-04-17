use agent_core::agent_mail::AgentMailbox;
use agent_core::agent_pool::AgentPool;
use agent_core::agent_pool::AgentStatusSnapshot;
use agent_core::agent_role::AgentRole;
use agent_core::agent_runtime::AgentId;
use agent_core::agent_runtime::AgentMeta;
use agent_core::agent_runtime::AgentStatus;
use agent_core::agent_runtime::ProviderSessionId;
use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::app::TranscriptEntry;
use agent_core::event_aggregator::EventAggregator;
use agent_core::logging;
use agent_core::provider::{ProviderEvent, ProviderKind};
use agent_core::runtime_session::RuntimeSession;
use agent_core::shared_state::SharedWorkplaceState;
use agent_core::shutdown_snapshot::AgentShutdownSnapshot;
use agent_core::shutdown_snapshot::ProviderThreadSnapshot;
use agent_core::shutdown_snapshot::ShutdownReason;
use agent_core::shutdown_snapshot::ShutdownSnapshot;
use agent_core::tool_calls::ExecCommandStatus;
use agent_core::tool_calls::McpInvocation;
use agent_core::tool_calls::McpToolCallStatus;
use agent_core::tool_calls::PatchApplyStatus;
use agent_core::tool_calls::PatchChange;
use agent_core::tool_calls::WebSearchAction;
use agent_core::workplace_store::WorkplaceStore;
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::composer::textarea::TextArea;
use crate::composer::textarea::TextAreaState;
use crate::confirmation_overlay::ConfirmationOverlay;
use crate::human_decision_overlay::HumanDecisionOverlay;
use crate::launch_config_overlay::LaunchConfigOverlayState;
use crate::markdown_stream::MarkdownStreamCollector;
use crate::provider_overlay::ProviderSelectionOverlay;
use crate::streaming::AdaptiveChunkingPolicy;
use crate::streaming::QueueSnapshot;
use crate::transcript::cells;
use crate::transcript::overlay::TranscriptOverlayState;
use crate::tui_snapshot::TuiResumeSnapshot;
use crate::view_mode::TuiViewState;

/// Per-agent transcript view state
///
/// Tracks scroll position and follow-tail state for each agent's transcript.
/// Used when switching focus between agents.
#[derive(Debug, Clone)]
pub struct AgentViewState {
    /// Scroll offset for this agent's transcript
    pub scroll_offset: usize,
    /// Whether to follow tail for this agent
    pub follow_tail: bool,
    /// Last rendered cell range for this agent
    pub last_cell_range: Option<(usize, usize)>,
}

impl Default for AgentViewState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            follow_tail: true,
            last_cell_range: None,
        }
    }
}

#[derive(Debug)]
pub struct TuiState {
    pub session: RuntimeSession,
    pub active_cell: Option<ActiveCell>,
    pub active_entries_revision: u64,
    pub composer: TextArea,
    pub composer_state: TextAreaState,
    pub transcript_overlay: Option<TranscriptOverlayState>,
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
    pub transcript_render_width: Option<usize>,
    pub transcript_scroll_offset: usize,
    pub transcript_max_scroll: usize,
    pub transcript_follow_tail: bool,
    pub transcript_rendered_lines: Vec<String>,
    pub transcript_last_cell_range: Option<(usize, usize)>,
    pub busy_started_at: Option<Instant>,
    /// Per-agent view state cache (for multi-agent transcript switching)
    pub agent_view_states: std::collections::HashMap<String, AgentViewState>,
    /// Agent pool for multi-agent management (None in single-agent mode)
    pub agent_pool: Option<AgentPool>,
    /// Event aggregator for polling all agent channels
    pub event_aggregator: EventAggregator,
    /// Mailbox for cross-agent communication
    pub mailbox: AgentMailbox,
    /// View state for different TUI modes
    pub view_state: TuiViewState,
    /// Provider selection overlay (for agent creation)
    pub provider_overlay: Option<ProviderSelectionOverlay>,
    /// Confirmation overlay (for agent stop)
    pub confirmation_overlay: Option<ConfirmationOverlay>,
    /// Human decision overlay (for decision layer)
    pub human_decision_overlay: Option<HumanDecisionOverlay>,
    /// Launch config overlay (for work/decision agent configuration)
    pub launch_config_overlay: Option<LaunchConfigOverlayState>,
}

impl TuiState {
    pub fn from_session(session: RuntimeSession) -> Self {
        let composer = TextArea::from_text(session.app.input.clone());
        Self {
            session,
            active_cell: None,
            active_entries_revision: 0,
            composer,
            composer_state: TextAreaState::default(),
            transcript_overlay: None,
            composer_width: 80,
            transcript_viewport_height: 1,
            transcript_render_width: None,
            transcript_scroll_offset: 0,
            transcript_max_scroll: 0,
            transcript_follow_tail: true,
            transcript_rendered_lines: Vec::new(),
            transcript_last_cell_range: None,
            busy_started_at: None,
            agent_view_states: std::collections::HashMap::new(),
            agent_pool: None,
            event_aggregator: EventAggregator::new(),
            mailbox: AgentMailbox::new(),
            view_state: TuiViewState::new(),
            provider_overlay: None,
            confirmation_overlay: None,
            human_decision_overlay: None,
            launch_config_overlay: None,
        }
    }

    pub fn app(&self) -> &AppState {
        &self.session.app
    }

    pub fn app_mut(&mut self) -> &mut AppState {
        &mut self.session.app
    }

    /// Get workplace (shared state) reference
    pub fn workplace(&self) -> &SharedWorkplaceState {
        self.session.workplace()
    }

    /// Get workplace (shared state) mutable reference
    pub fn workplace_mut(&mut self) -> &mut SharedWorkplaceState {
        self.session.workplace_mut()
    }

    /// Get unread mail count for focused agent
    pub fn focused_unread_mail_count(&self) -> usize {
        if let Some(agent_id) = self.focused_agent_id() {
            self.mailbox.unread_count(&agent_id)
        } else {
            0
        }
    }

    /// Get action-required mail count for focused agent
    pub fn focused_action_required_count(&self) -> usize {
        if let Some(agent_id) = self.focused_agent_id() {
            self.mailbox.action_required_count(&agent_id)
        } else {
            0
        }
    }

    /// Get formatted unread mail for focused agent's prompt
    pub fn focused_unread_mail_for_prompt(&self) -> String {
        if let Some(agent_id) = self.focused_agent_id() {
            let inbox = self.mailbox.inbox_for(&agent_id);
            if let Some(mails) = inbox {
                let unread = mails.iter().filter(|m| !m.is_read());
                let formatted = unread.map(|m| m.format_for_prompt()).collect::<Vec<_>>();
                if formatted.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n=== Incoming Messages ===\n{}\n=== End Messages ===\n\n",
                        formatted.join("\n")
                    )
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    /// Mark all mail as read for focused agent (keeps mail history)
    pub fn mark_focused_mail_read(&mut self) {
        if let Some(agent_id) = self.focused_agent_id() {
            self.mailbox.mark_all_read(&agent_id);
        }
    }

    /// Save current transcript view state for a specific agent
    ///
    /// Called when switching focus to preserve scroll position.
    pub fn save_agent_view_state(&mut self, agent_id: &str) {
        let state = AgentViewState {
            scroll_offset: self.transcript_scroll_offset,
            follow_tail: self.transcript_follow_tail,
            last_cell_range: self.transcript_last_cell_range,
        };
        self.agent_view_states.insert(agent_id.to_string(), state);
    }

    /// Restore transcript view state for a specific agent
    ///
    /// Called when switching focus to resume scroll position.
    pub fn restore_agent_view_state(&mut self, agent_id: &str) {
        if let Some(state) = self.agent_view_states.get(agent_id).cloned() {
            self.transcript_scroll_offset = state.scroll_offset;
            self.transcript_follow_tail = state.follow_tail;
            self.transcript_last_cell_range = state.last_cell_range;
        } else {
            // New agent - reset to default
            self.transcript_scroll_offset = 0;
            self.transcript_follow_tail = true;
            self.transcript_last_cell_range = None;
        }
    }

    /// Switch focus to a different agent
    ///
    /// Saves current agent's view state and restores the new agent's state.
    /// Uses AgentPool's focus_agent_by_index if available.
    pub fn switch_focus_to_agent(&mut self, new_agent_id: &str) {
        // If we have an agent pool, use its focus management
        if let Some(pool) = self.agent_pool.as_mut() {
            let agent_id = agent_core::agent_runtime::AgentId::new(new_agent_id);
            if pool.focus_agent(&agent_id).is_ok() {
                // Save current scroll state
                self.save_agent_view_state(new_agent_id);
                // Restore new agent's scroll state
                self.restore_agent_view_state(new_agent_id);
            }
        } else {
            // Single-agent mode - just save/restore state
            self.save_agent_view_state(new_agent_id);
            self.restore_agent_view_state(new_agent_id);
        }
    }

    /// Switch focus to the next agent in the pool
    pub fn focus_next_agent(&mut self) -> Option<AgentStatusSnapshot> {
        let (current_id, new_index) = {
            let pool = self.agent_pool.as_ref()?;
            let current_id = pool
                .focused_slot()
                .map(|s| s.agent_id().as_str().to_string());
            let new_index = (pool.focused_slot_index() + 1) % pool.active_count();
            (current_id, new_index)
        };

        // Save current scroll state (no borrow conflict)
        if let Some(id) = current_id.as_ref() {
            self.save_agent_view_state(id);
        }

        // Switch focus in pool
        {
            let pool = self.agent_pool.as_mut()?;
            pool.focus_agent_by_index(new_index).ok()?;
        }

        // Get new focused agent info and restore state
        let (snapshot, new_id) = {
            let pool = self.agent_pool.as_ref()?;
            let focused = pool.focused_slot()?;
            (
                Some(AgentStatusSnapshot {
                    agent_id: focused.agent_id().clone(),
                    codename: focused.codename().clone(),
                    provider_type: focused.provider_type(),
                    role: focused.role(),
                    status: focused.status().clone(),
                    assigned_task_id: focused.assigned_task_id().cloned(),
                    worktree_branch: focused.worktree_branch().cloned(),
                    has_worktree: focused.has_worktree(),
                    worktree_exists: focused.has_worktree() && focused.cwd().exists(),
                }),
                focused.agent_id().as_str().to_string(),
            )
        };

        // Restore new agent's scroll state
        self.restore_agent_view_state(&new_id);

        snapshot
    }

    /// Switch focus to the previous agent in the pool
    pub fn focus_previous_agent(&mut self) -> Option<AgentStatusSnapshot> {
        let (current_id, new_index) = {
            let pool = self.agent_pool.as_ref()?;
            let current_id = pool
                .focused_slot()
                .map(|s| s.agent_id().as_str().to_string());
            let count = pool.active_count();
            let new_index = if pool.focused_slot_index() == 0 {
                count - 1
            } else {
                pool.focused_slot_index() - 1
            };
            (current_id, new_index)
        };

        // Save current scroll state
        if let Some(id) = current_id.as_ref() {
            self.save_agent_view_state(id);
        }

        // Switch focus in pool
        {
            let pool = self.agent_pool.as_mut()?;
            pool.focus_agent_by_index(new_index).ok()?;
        }

        // Get new focused agent info and restore state
        let (snapshot, new_id) = {
            let pool = self.agent_pool.as_ref()?;
            let focused = pool.focused_slot()?;
            (
                Some(AgentStatusSnapshot {
                    agent_id: focused.agent_id().clone(),
                    codename: focused.codename().clone(),
                    provider_type: focused.provider_type(),
                    role: focused.role(),
                    status: focused.status().clone(),
                    assigned_task_id: focused.assigned_task_id().cloned(),
                    worktree_branch: focused.worktree_branch().cloned(),
                    has_worktree: focused.has_worktree(),
                    worktree_exists: focused.has_worktree() && focused.cwd().exists(),
                }),
                focused.agent_id().as_str().to_string(),
            )
        };

        // Restore new agent's scroll state
        self.restore_agent_view_state(&new_id);

        snapshot
    }

    /// Switch focus to a specific agent by index
    pub fn focus_agent_by_index(&mut self, index: usize) -> Option<AgentStatusSnapshot> {
        let current_id = {
            let pool = self.agent_pool.as_ref()?;
            pool.focused_slot()
                .map(|s| s.agent_id().as_str().to_string())
        };

        // Save current scroll state
        if let Some(id) = current_id.as_ref() {
            self.save_agent_view_state(id);
        }

        // Switch focus in pool
        {
            let pool = self.agent_pool.as_mut()?;
            pool.focus_agent_by_index(index).ok()?;
        }

        // Get new focused agent info and restore state
        let (snapshot, new_id) = {
            let pool = self.agent_pool.as_ref()?;
            let focused = pool.focused_slot()?;
            (
                Some(AgentStatusSnapshot {
                    agent_id: focused.agent_id().clone(),
                    codename: focused.codename().clone(),
                    provider_type: focused.provider_type(),
                    role: focused.role(),
                    status: focused.status().clone(),
                    assigned_task_id: focused.assigned_task_id().cloned(),
                    worktree_branch: focused.worktree_branch().cloned(),
                    has_worktree: focused.has_worktree(),
                    worktree_exists: focused.has_worktree() && focused.cwd().exists(),
                }),
                focused.agent_id().as_str().to_string(),
            )
        };

        // Restore new agent's scroll state
        self.restore_agent_view_state(&new_id);

        snapshot
    }

    /// Spawn a new agent in the pool
    ///
    /// If the pool has worktree support, the agent will be created with an
    /// isolated git worktree. Otherwise, falls back to regular agent creation.
    pub fn spawn_agent(
        &mut self,
        provider: ProviderKind,
    ) -> Option<agent_core::agent_runtime::AgentId> {
        // Create agent pool if it doesn't exist
        if self.agent_pool.is_none() {
            self.ensure_overview_agent();
        }

        let pool = self.agent_pool.as_mut()?;
        let has_worktree_support = pool.has_worktree_support();

        let agent_id = if has_worktree_support {
            // Use worktree-enabled spawn
            pool.spawn_agent_with_worktree(provider, None, None)
                .map_err(|e| {
                    logging::warn_event(
                        "tui.agent.spawn_worktree_failed",
                        "failed to spawn agent with worktree",
                        serde_json::json!({
                            "error": e.to_string(),
                        }),
                    );
                    e
                })
                .ok()
        } else {
            // Fallback to regular spawn
            pool.spawn_agent(provider).ok()
        }?;

        // Spawn decision agent for the work agent (if provider supports it)
        // Decision agents don't need worktrees - they only analyze output
        if provider != ProviderKind::Mock {
            if let Some(pool) = self.agent_pool.as_mut() {
                if let Err(e) = pool.spawn_decision_agent_for(&agent_id) {
                    // Log warning but don't fail the work agent spawn
                    logging::warn_event(
                        "decision_agent.spawn_failed",
                        "failed to spawn decision agent",
                        serde_json::json!({
                            "work_agent_id": agent_id.as_str(),
                            "error": e,
                        }),
                    );
                }
            }
        }

        Some(agent_id)
    }

    /// Spawn a new agent with launch configuration
    ///
    /// If the pool has worktree support, the agent will be created with an
    /// isolated git worktree. Otherwise, falls back to regular agent creation.
    pub fn spawn_agent_with_launch_config(
        &mut self,
        provider: ProviderKind,
        work_config: &str,
        decision_config: &str,
    ) -> Option<agent_core::agent_runtime::AgentId> {
        use agent_core::launch_config::parse;
        use agent_core::launch_config::resolve_bundle;

        // Create agent pool if it doesn't exist
        if self.agent_pool.is_none() {
            self.ensure_overview_agent();
        }

        // Parse launch configs
        let work_input = parse(provider, work_config).ok()?;
        let decision_input = parse(provider, decision_config).ok()?;

        // Resolve to get bundle
        let (work_resolved, decision_resolved) = resolve_bundle(work_input.clone(), decision_input.clone()).ok()?;

        let bundle = agent_core::launch_config::AgentLaunchBundle::asymmetric(
            work_input,
            work_resolved,
            decision_input,
            decision_resolved,
        );

        let pool = self.agent_pool.as_mut()?;
        let has_worktree_support = pool.has_worktree_support();

        let agent_id = if has_worktree_support {
            // Use worktree-enabled spawn
            pool.spawn_agent_with_worktree(provider, None, None)
                .map_err(|e| {
                    logging::warn_event(
                        "tui.agent.spawn_worktree_failed",
                        "failed to spawn agent with worktree",
                        serde_json::json!({
                            "error": e.to_string(),
                        }),
                    );
                    e
                })
                .ok()
        } else {
            // Fallback to regular spawn
            pool.spawn_agent(provider).ok()
        }?;

        // Attach launch bundle to the slot
        if let Some(pool) = self.agent_pool.as_mut() {
            if let Some(slot) = pool.get_slot_mut_by_id(&agent_id) {
                slot.set_launch_bundle(bundle);
            }
        }

        // Spawn decision agent for the work agent (if provider supports it)
        // Decision agents don't need worktrees - they only analyze output
        if provider != ProviderKind::Mock {
            if let Some(pool) = self.agent_pool.as_mut() {
                if let Err(e) = pool.spawn_decision_agent_for(&agent_id) {
                    logging::warn_event(
                        "decision_agent.spawn_failed",
                        "failed to spawn decision agent",
                        serde_json::json!({
                            "work_agent_id": agent_id.as_str(),
                            "error": e,
                        }),
                    );
                }
            }
        }

        Some(agent_id)
    }

    /// Focus a specific agent by ID
    pub fn focus_agent(&mut self, agent_id: &AgentId) -> Option<AgentStatusSnapshot> {
        let pool = self.agent_pool.as_mut()?;
        pool.focus_agent(agent_id).ok()?;

        let focused = pool.focused_slot()?;
        Some(AgentStatusSnapshot {
            agent_id: focused.agent_id().clone(),
            codename: focused.codename().clone(),
            provider_type: focused.provider_type(),
            role: focused.role(),
            status: focused.status().clone(),
            assigned_task_id: focused.assigned_task_id().cloned(),
            worktree_branch: focused.worktree_branch().cloned(),
            has_worktree: focused.has_worktree(),
            worktree_exists: focused.has_worktree() && focused.cwd().exists(),
        })
    }

    /// Return overview-mode agent indices in display order.
    ///
    /// OVERVIEW is always pinned first when present. The configured filter is
    /// applied only to worker agents.
    pub fn overview_filtered_agent_indices(&self) -> Vec<usize> {
        let statuses = self.agent_statuses();
        let overview_index = statuses
            .iter()
            .position(|status| status.role == AgentRole::ProductOwner);
        let filter = self.view_state.overview.filter;

        let mut indices = Vec::new();
        if let Some(index) = overview_index {
            indices.push(index);
        }

        for (index, status) in statuses.iter().enumerate() {
            if Some(index) == overview_index {
                continue;
            }

            let included = match filter {
                crate::overview_state::OverviewFilter::All => true,
                crate::overview_state::OverviewFilter::BlockedOnly => status.status.is_blocked(),
                crate::overview_state::OverviewFilter::RunningOnly => status.status.is_active(),
            };

            if included {
                indices.push(index);
            }
        }

        indices
    }

    /// Return the total number of overview pages for the current filter.
    pub fn overview_total_pages(&self) -> usize {
        let filtered = self.overview_filtered_agent_indices();
        if filtered.is_empty() {
            return 0;
        }

        let rows = self.view_state.overview.agent_list_rows.max(1);
        let statuses = self.agent_statuses();
        let has_overview = filtered
            .first()
            .and_then(|index| statuses.get(*index))
            .is_some_and(|status| status.role == AgentRole::ProductOwner);

        if has_overview {
            let worker_count = filtered.len().saturating_sub(1);
            if worker_count == 0 {
                1
            } else {
                let worker_rows = rows.saturating_sub(1).max(1);
                worker_count.div_ceil(worker_rows)
            }
        } else {
            filtered.len().div_ceil(rows)
        }
    }

    /// Return visible overview-mode agent indices for the current page.
    pub fn overview_visible_agent_indices(&self) -> Vec<usize> {
        let filtered = self.overview_filtered_agent_indices();
        if filtered.is_empty() {
            return Vec::new();
        }

        let rows = self.view_state.overview.agent_list_rows.max(1);
        let statuses = self.agent_statuses();
        let has_overview = filtered
            .first()
            .and_then(|index| statuses.get(*index))
            .is_some_and(|status| status.role == AgentRole::ProductOwner);

        if has_overview {
            if rows == 1 {
                return vec![filtered[0]];
            }

            let worker_rows = rows.saturating_sub(1).max(1);
            let page = self
                .view_state
                .overview
                .page_offset
                .min(self.overview_total_pages().saturating_sub(1));
            let start = page * worker_rows;
            let workers = &filtered[1..];
            let end = (start + worker_rows).min(workers.len());

            let mut visible = vec![filtered[0]];
            visible.extend_from_slice(&workers[start..end]);
            visible
        } else {
            let page = self
                .view_state
                .overview
                .page_offset
                .min(self.overview_total_pages().saturating_sub(1));
            let start = page * rows;
            let end = (start + rows).min(filtered.len());
            filtered[start..end].to_vec()
        }
    }

    /// Keep the current overview page aligned to the focused agent when possible.
    pub fn sync_overview_page_to_focus(&mut self) {
        let Some(pool) = self.agent_pool.as_ref() else {
            self.view_state.overview.page_offset = 0;
            return;
        };

        let filtered = self.overview_filtered_agent_indices();
        if filtered.is_empty() {
            self.view_state.overview.page_offset = 0;
            return;
        }

        let focused_index = pool.focused_slot_index();
        if !filtered.contains(&focused_index) {
            self.view_state.overview.page_offset = 0;
            return;
        }

        let rows = self.view_state.overview.agent_list_rows.max(1);
        let statuses = self.agent_statuses();
        let has_overview = filtered
            .first()
            .and_then(|index| statuses.get(*index))
            .is_some_and(|status| status.role == AgentRole::ProductOwner);

        if has_overview {
            if focused_index == filtered[0] {
                self.view_state.overview.page_offset = 0;
                return;
            }

            let worker_rows = rows.saturating_sub(1).max(1);
            if let Some(worker_position) = filtered[1..]
                .iter()
                .position(|index| *index == focused_index)
            {
                self.view_state.overview.page_offset = worker_position / worker_rows;
            }
        } else if let Some(position) = filtered.iter().position(|index| *index == focused_index) {
            self.view_state.overview.page_offset = position / rows;
        }
    }

    /// Ensure the focused agent remains visible under the current overview filter.
    pub fn ensure_overview_focus_visible(&mut self) -> Option<AgentStatusSnapshot> {
        let filtered = self.overview_filtered_agent_indices();
        let focused_index = self.agent_pool.as_ref()?.focused_slot_index();
        if filtered.contains(&focused_index) {
            self.sync_overview_page_to_focus();
            return self.focused_agent_status();
        }

        let next_index = *filtered.first()?;
        let snapshot = self.focus_agent_by_index(next_index)?;
        self.sync_overview_page_to_focus();
        Some(snapshot)
    }

    /// Focus a visible overview agent by number key.
    pub fn focus_overview_agent_by_number(&mut self, n: u8) -> Option<AgentStatusSnapshot> {
        let index = (n as usize).saturating_sub(1);
        let visible = self.overview_visible_agent_indices();
        let original_index = *visible.get(index)?;
        let snapshot = self.focus_agent_by_index(original_index)?;
        self.sync_overview_page_to_focus();
        Some(snapshot)
    }

    /// Focus an overview agent by codename or agent id.
    pub fn focus_overview_agent_by_codename(
        &mut self,
        codename: &str,
    ) -> Option<AgentStatusSnapshot> {
        let index = self.agent_statuses().iter().position(|status| {
            status.codename.as_str() == codename || status.agent_id.as_str() == codename
        })?;
        let snapshot = self.focus_agent_by_index(index)?;
        self.sync_overview_page_to_focus();
        Some(snapshot)
    }

    /// Ensure OVERVIEW agent exists (called on initialization)
    pub fn ensure_overview_agent(&mut self) {
        if self.agent_pool.is_none() {
            let workplace_id = self.session.workplace().workplace_id.clone();
            let cwd = self.session.app.cwd.clone();

            // Get workplace store to access the workplace path (for worktree state storage)
            let workplace_store = WorkplaceStore::for_cwd(&cwd).ok();
            let workplace_path = workplace_store.as_ref().map(|w| w.path().to_path_buf());

            // Try to create pool with worktree support first
            let pool = if let Some(wp_path) = workplace_path {
                AgentPool::new_with_worktrees(
                    workplace_id.clone(),
                    10,
                    cwd.clone(),
                    wp_path,
                )
            } else {
                // Can't get workplace path, use regular pool
                Err(agent_core::worktree_manager::WorktreeError::NotAGitRepository(cwd.clone()))
            };

            let mut pool = match pool {
                Ok(p) => {
                    logging::debug_event(
                        "tui.pool.worktree_enabled",
                        "created agent pool with worktree support",
                        serde_json::json!({
                            "cwd": cwd.display().to_string(),
                        }),
                    );
                    p
                }
                Err(e) => {
                    // Fallback to regular pool if worktree creation fails (e.g., not a git repo)
                    logging::warn_event(
                        "tui.pool.worktree_fallback",
                        "worktree pool creation failed, using regular pool",
                        serde_json::json!({
                            "error": e.to_string(),
                            "cwd": cwd.display().to_string(),
                        }),
                    );
                    AgentPool::with_cwd(workplace_id, 10, cwd)
                }
            };

            // Create OVERVIEW agent with the current provider (OVERVIEW doesn't need worktree)
            let overview_provider = self.session.app.selected_provider;
            pool.spawn_overview_agent(overview_provider).ok();

            self.agent_pool = Some(pool);
        } else if let Some(pool) = self.agent_pool.as_mut() {
            // Check if OVERVIEW agent exists
            if pool.overview_agent().is_none() {
                let overview_provider = self.session.app.selected_provider;
                pool.spawn_overview_agent(overview_provider).ok();
            }
        }
    }

    /// Stop the focused agent in the pool
    pub fn stop_focused_agent(&mut self) -> Option<String> {
        if let Some(pool) = self.agent_pool.as_mut() {
            let focused = pool.focused_slot()?;
            let agent_id = focused.agent_id().clone();
            pool.stop_agent(&agent_id).ok()?;
            Some(agent_id.as_str().to_string())
        } else {
            None
        }
    }

    /// Pause the focused agent with worktree preservation
    /// Pause the focused agent with worktree preservation
    ///
    /// This properly stops the provider thread before pausing to avoid
    /// resource leaks and ensure clean state preservation.
    pub fn pause_focused_agent(&mut self) -> Option<String> {
        // Get agent_id and check worktree requirement first
        let agent_id = {
            let pool = self.agent_pool.as_ref()?;
            let focused = pool.focused_slot()?;

            // Check if agent has a worktree (required for pause)
            if !focused.has_worktree() {
                self.app_mut().push_error_message("Cannot pause agent without worktree");
                return None;
            }

            focused.agent_id().clone()
        };

        // First, unregister the event channel to signal thread to stop
        // The thread will detect channel disconnect and exit
        self.unregister_agent_channel(&agent_id);

        // Get and join the thread handle, then clear provider thread state
        {
            let pool = self.agent_pool.as_mut()?;
            if let Some(slot) = pool.get_slot_mut_by_id(&agent_id) {
                // Take thread handle
                let thread_handle = slot.take_thread_handle();

                // Wait for thread to finish (best effort)
                if let Some(handle) = thread_handle {
                    let _ = handle.join(); // Don't block forever
                }

                // Clear provider thread state
                slot.clear_provider_thread();
            }
        }

        // Now call pool to save worktree state and transition status
        let pool = self.agent_pool.as_mut()?;
        pool.pause_agent_with_worktree(&agent_id).ok()?;

        Some(agent_id.as_str().to_string())
    }

    /// Resume the paused focused agent
    ///
    /// Transitions the agent to Idle status, ready to receive new prompts.
    /// The provider thread needs to be started separately via a prompt.
    pub fn resume_focused_agent(&mut self) -> Option<String> {
        if let Some(pool) = self.agent_pool.as_mut() {
            let focused = pool.focused_slot()?;
            let agent_id = focused.agent_id().clone();
            pool.resume_agent_with_worktree(&agent_id).ok()?;
            Some(agent_id.as_str().to_string())
        } else {
            None
        }
    }

    /// Get all agent statuses from the pool
    pub fn agent_statuses(&self) -> Vec<AgentStatusSnapshot> {
        self.agent_pool
            .as_ref()
            .map(|pool| pool.agent_statuses())
            .unwrap_or_default()
    }

    /// Get the focused agent status
    pub fn focused_agent_status(&self) -> Option<AgentStatusSnapshot> {
        self.agent_pool
            .as_ref()
            .and_then(|pool| pool.focused_slot())
            .map(|s| AgentStatusSnapshot {
                agent_id: s.agent_id().clone(),
                codename: s.codename().clone(),
                provider_type: s.provider_type(),
                role: s.role(),
                status: s.status().clone(),
                assigned_task_id: s.assigned_task_id().cloned(),
                worktree_branch: s.worktree_branch().cloned(),
                has_worktree: s.has_worktree(),
                worktree_exists: s.has_worktree() && s.cwd().exists(),
            })
    }

    /// Check if multi-agent mode is active (agent pool exists with agents)
    pub fn is_multi_agent_mode(&self) -> bool {
        self.agent_pool
            .as_ref()
            .map(|p| p.active_count() > 0)
            .unwrap_or(false)
    }

    /// Get the focused agent codename for display
    pub fn focused_agent_codename(&self) -> &str {
        if let Some(pool) = self.agent_pool.as_ref() {
            pool.focused_slot()
                .map(|s| s.codename().as_str())
                .unwrap_or("alpha")
        } else {
            "alpha"
        }
    }

    /// Get the focused agent ID (if pool exists)
    pub fn focused_agent_id(&self) -> Option<AgentId> {
        self.agent_pool
            .as_ref()
            .and_then(|pool| pool.focused_slot())
            .map(|s| s.agent_id().clone())
    }

    pub fn agent_has_provider_session(&self, agent_id: &AgentId) -> bool {
        self.agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(agent_id))
            .and_then(|slot| slot.session_handle())
            .is_some()
    }

    pub fn append_status_to_agent_transcript(&mut self, agent_id: &AgentId, text: String) {
        if let Some(pool) = self.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(agent_id)
        {
            slot.append_transcript(TranscriptEntry::Status(text));
        }
    }

    pub fn create_shutdown_snapshot(&self, reason: ShutdownReason) -> ShutdownSnapshot {
        let agents = if let Some(pool) = self.agent_pool.as_ref() {
            pool.slots()
                .iter()
                .map(|slot| {
                    let meta = AgentMeta {
                        agent_id: slot.agent_id().clone(),
                        codename: slot.codename().clone(),
                        workplace_id: self.session.workplace().workplace_id.clone(),
                        provider_type: slot.provider_type(),
                        provider_session_id: slot.session_handle().map(|handle| match handle {
                            agent_core::provider::SessionHandle::ClaudeSession { session_id } => {
                                ProviderSessionId::new(session_id)
                            }
                            agent_core::provider::SessionHandle::CodexThread { thread_id } => {
                                ProviderSessionId::new(thread_id)
                            }
                        }),
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        status: if slot.status().is_active() {
                            AgentStatus::Running
                        } else if slot.status().is_terminal() {
                            AgentStatus::Stopped
                        } else {
                            AgentStatus::Idle
                        },
                    };

                    let provider_thread_state = if slot.status().is_active() {
                        Some(ProviderThreadSnapshot::waiting_for_response(
                            None,
                            chrono::Utc::now().to_rfc3339(),
                        ))
                    } else {
                        None
                    };

                    AgentShutdownSnapshot {
                        meta,
                        assigned_task_id: slot
                            .assigned_task_id()
                            .map(|task| task.as_str().to_string()),
                        was_active: slot.status().is_active(),
                        had_error: matches!(
                            slot.status(),
                            agent_core::agent_slot::AgentSlotStatus::Error { .. }
                        ),
                        provider_thread_state,
                        captured_at: chrono::Utc::now().to_rfc3339(),
                    }
                })
                .collect()
        } else {
            let snapshot = if self.session.was_interrupted() {
                AgentShutdownSnapshot::active(
                    self.session.agent_runtime.meta().clone(),
                    self.app().active_task_id.clone(),
                    ProviderThreadSnapshot::waiting_for_response(
                        None,
                        chrono::Utc::now().to_rfc3339(),
                    ),
                )
            } else {
                AgentShutdownSnapshot::idle(self.session.agent_runtime.meta().clone())
            };
            vec![snapshot]
        };

        ShutdownSnapshot::new(
            self.session.workplace().workplace_id.as_str().to_string(),
            agents,
            self.workplace().backlog.clone(),
            self.mailbox.pending_mail_for_snapshot(),
            reason,
        )
    }

    pub fn create_resume_snapshot(&self, reason: ShutdownReason) -> TuiResumeSnapshot {
        TuiResumeSnapshot::from_state(self, reason)
    }

    pub fn restore_from_resume_snapshot(&mut self, snapshot: TuiResumeSnapshot) -> Result<()> {
        self.view_state = TuiViewState::default();
        snapshot.view_state.apply_to(&mut self.view_state);

        self.workplace_mut().backlog = snapshot.backlog.clone();
        self.mailbox = snapshot.mailbox.clone();
        self.app_mut().selected_provider = snapshot.selected_provider;
        self.app_mut().input = snapshot.composer_text.clone();
        self.composer = TextArea::from_text(snapshot.composer_text.clone());
        self.sync_app_input_from_composer();

        let workplace_id = self.session.workplace().workplace_id.clone();
        let cwd = self.session.app.cwd.clone();

        // Try to create pool with worktree support
        let workplace_store = agent_core::workplace_store::WorkplaceStore::for_cwd(&cwd).ok();
        let workplace_path = workplace_store.as_ref().map(|w| w.path().to_path_buf());

        let pool = if let Some(wp_path) = workplace_path {
            AgentPool::new_with_worktrees(
                workplace_id.clone(),
                10,
                cwd.clone(),
                wp_path,
            )
        } else {
            Err(agent_core::worktree_manager::WorktreeError::NotAGitRepository(cwd.clone()))
        };

        let mut pool = match pool {
            Ok(p) => {
                agent_core::logging::debug_event(
                    "tui.restore.worktree_enabled",
                    "restored agent pool with worktree support",
                    serde_json::json!({
                        "cwd": cwd.display().to_string(),
                    }),
                );
                p
            }
            Err(e) => {
                // Fallback to regular pool if worktree creation fails
                agent_core::logging::warn_event(
                    "tui.restore.worktree_fallback",
                    "worktree pool creation failed, using regular pool",
                    serde_json::json!({
                        "error": e.to_string(),
                        "cwd": cwd.display().to_string(),
                    }),
                );
                AgentPool::new(workplace_id, 10)
            }
        };

        for agent in snapshot.agents {
            let mut restored_slot = agent_core::agent_slot::AgentSlot::restored_with_worktree(
                agent.agent_id.clone(),
                agent.codename.clone(),
                agent.provider_type,
                agent.role,
                agent.restore_status(),
                agent.restore_session_handle(),
                agent.transcript.clone(),
                agent.assigned_task_id.clone(),
                agent.worktree_path.clone(),
                agent.worktree_branch.clone(),
                agent.worktree_id.clone(),
            );
            // Attach launch bundle if available
            if let Some(bundle) = agent.launch_bundle {
                restored_slot.set_launch_bundle(bundle);
                agent_core::logging::debug_event(
                    "launch_config.restore",
                    "agent restored with launch bundle",
                    serde_json::json!({
                        "agent_id": agent.agent_id.as_str(),
                        "provider": agent.provider_type,
                        "source": "snapshot",
                    }),
                );
            }
            // Log worktree restoration
            if let Some(wt_path) = &agent.worktree_path {
                agent_core::logging::debug_event(
                    "worktree.restore",
                    "agent restored with worktree",
                    serde_json::json!({
                        "agent_id": agent.agent_id.as_str(),
                        "worktree_path": wt_path.display().to_string(),
                        "worktree_branch": agent.worktree_branch,
                    }),
                );
            }
            pool.restore_slot(restored_slot)
                .map_err(|error| anyhow::anyhow!(error))?;
        }
        self.agent_pool = Some(pool);

        // Verify and recreate worktrees if needed
        self.verify_restored_worktrees()?;

        if let Some(focused_id) = snapshot.focused_agent_id.as_ref() {
            let _ = self.focus_agent(focused_id);
        } else {
            self.ensure_overview_agent();
        }
        self.sync_overview_page_to_focus();

        if let Some(status) = self.focused_agent_status() {
            self.app_mut().selected_provider = status
                .provider_type
                .to_provider_kind()
                .unwrap_or(self.app().selected_provider);
            self.app_mut().transcript = self
                .agent_pool
                .as_ref()
                .and_then(|pool| pool.get_slot_by_id(&status.agent_id))
                .map(|slot| slot.transcript().to_vec())
                .unwrap_or_default();
            self.app_mut().active_task_id = status
                .assigned_task_id
                .map(|task| task.as_str().to_string());
            self.app_mut().status = match self
                .agent_pool
                .as_ref()
                .and_then(|pool| pool.get_slot_by_id(&status.agent_id))
                .map(|slot| slot.status())
            {
                Some(state) if state.is_active() => AppStatus::Responding,
                _ => AppStatus::Idle,
            };
        }

        Ok(())
    }

    /// Verify worktrees exist for restored agents, recreate if missing
    fn verify_restored_worktrees(&mut self) -> Result<()> {
        let pool = self.agent_pool.as_mut().ok_or_else(|| anyhow::anyhow!("No agent pool"))?;
        if !pool.has_worktree_support() {
            return Ok(()); // No worktree support, nothing to verify
        }

        // Collect agents with worktrees that need verification
        let agents_to_verify: Vec<(agent_core::agent_runtime::AgentId, std::path::PathBuf, Option<String>, String)> = pool
            .slots()
            .iter()
            .filter_map(|slot| {
                if slot.has_worktree() {
                    let wt_path = slot.cwd();
                    if !wt_path.exists() {
                        // Worktree directory doesn't exist, needs recreation
                        Some((
                            slot.agent_id().clone(),
                            wt_path,
                            slot.worktree_branch().cloned(),
                            slot.worktree_id().cloned().unwrap_or_default(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Recreate missing worktrees
        for (agent_id, expected_path, branch, worktree_id) in agents_to_verify {
            agent_core::logging::warn_event(
                "worktree.restore.missing",
                "worktree directory missing, will attempt recreation",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "expected_path": expected_path.display().to_string(),
                    "branch": branch,
                }),
            );

            // Use pool's worktree recreation logic
            // Note: resume_agent_with_worktree requires paused state, so we need a different approach
            // For now, we just clear the worktree info if we can't recreate
            if let Err(e) = self.recreate_agent_worktree(&agent_id, &worktree_id, branch.as_deref()) {
                agent_core::logging::warn_event(
                    "worktree.restore.failed",
                    "failed to recreate worktree, clearing worktree info",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "error": e.to_string(),
                    }),
                );
                // Clear worktree info since we couldn't recreate
                if let Some(pool) = self.agent_pool.as_mut() {
                    if let Some(slot) = pool.get_slot_mut_by_id(&agent_id) {
                        slot.clear_worktree();
                    }
                }
            }
        }

        Ok(())
    }

    /// Recreate a worktree for an agent
    fn recreate_agent_worktree(
        &mut self,
        agent_id: &agent_core::agent_runtime::AgentId,
        worktree_id: &str,
        branch: Option<&str>,
    ) -> Result<()> {
        use agent_core::worktree_manager::{WorktreeCreateOptions, WorktreeManager, WorktreeConfig};
        use agent_core::worktree_state_store::WorktreeStateStore;
        use agent_core::worktree_state::WorktreeState;

        let cwd = self.session.app.cwd.clone();
        let workplace_store = agent_core::workplace_store::WorkplaceStore::for_cwd(&cwd)?;
        let workplace_path = workplace_store.path();

        // Create worktree manager
        let worktree_manager = WorktreeManager::new(cwd.clone(), WorktreeConfig::default())
            .map_err(|e| anyhow::anyhow!("Failed to create worktree manager: {}", e))?;

        // Check if branch exists
        let branch_exists = branch
            .map(|b| worktree_manager.branch_exists(b).unwrap_or(false))
            .unwrap_or(false);

        // Get base commit
        let base_commit = worktree_manager.get_current_head()
            .map_err(|e| anyhow::anyhow!("Failed to get HEAD: {}", e))?;

        // Create worktree
        let options = WorktreeCreateOptions {
            path: worktree_manager.worktrees_dir().join(worktree_id),
            branch: branch.map(|b| b.to_string()),
            create_branch: !branch_exists && branch.is_some(),
            base: if branch_exists {
                None
            } else {
                Some(base_commit.clone())
            },
            lock_reason: None,
        };

        let worktree_info = worktree_manager
            .create(worktree_id, options)
            .map_err(|e| anyhow::anyhow!("Failed to create worktree: {}", e))?;

        // Update slot's worktree info
        if let Some(pool) = self.agent_pool.as_mut() {
            if let Some(slot) = pool.get_slot_mut_by_id(agent_id) {
                slot.set_worktree(
                    worktree_info.path.clone(),
                    worktree_info.branch.clone(),
                    worktree_id.to_string(),
                );
            }
        }

        // Save worktree state
        let worktree_state_store = WorktreeStateStore::new(workplace_path.to_path_buf());
        let worktree_state = WorktreeState::new(
            worktree_id.to_string(),
            worktree_info.path.clone(),
            worktree_info.branch.clone(),
            base_commit,
            None,
            agent_id.as_str().to_string(),
        );
        worktree_state_store
            .save(agent_id.as_str(), &worktree_state)
            .map_err(|e| anyhow::anyhow!("Failed to save worktree state: {}", e))?;

        agent_core::logging::debug_event(
            "worktree.restore.recreated",
            "worktree recreated successfully",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "worktree_id": worktree_id,
                "path": worktree_info.path.display().to_string(),
                "branch": worktree_info.branch,
            }),
        );

        Ok(())
    }

    fn unread_mail_for_agent(&self, agent_id: &AgentId) -> String {
        let inbox = self.mailbox.inbox_for(agent_id);
        if let Some(mails) = inbox {
            let unread = mails.iter().filter(|m| !m.is_read());
            let formatted = unread.map(|m| m.format_for_prompt()).collect::<Vec<_>>();
            if formatted.is_empty() {
                String::new()
            } else {
                format!(
                    "\n=== Incoming Messages ===\n{}\n=== End Messages ===\n\n",
                    formatted.join("\n")
                )
            }
        } else {
            String::new()
        }
    }

    fn mark_agent_mail_read(&mut self, agent_id: &AgentId) {
        self.mailbox.mark_all_read(agent_id);
    }

    pub fn build_provider_prompt_for_agent(
        &self,
        agent_id: &AgentId,
        prompt: String,
        inject_mail: bool,
    ) -> Option<String> {
        if !inject_mail {
            return Some(prompt);
        }

        let mail_prefix = self.unread_mail_for_agent(agent_id);
        if mail_prefix.is_empty() {
            Some(prompt)
        } else {
            Some(format!("{}{}", mail_prefix, prompt))
        }
    }

    /// Start provider request for focused agent in pool
    ///
    /// Creates provider thread and registers event channel with AgentSlot.
    /// Returns the event receiver for polling.
    pub fn start_provider_for_focused_agent(
        &mut self,
        prompt: String,
        provider: ProviderKind,
    ) -> Option<std::sync::mpsc::Receiver<agent_core::provider::ProviderEvent>> {
        if self.agent_pool.is_none() {
            let workplace_id = self.session.workplace().workplace_id.clone();
            self.agent_pool = Some(AgentPool::new(workplace_id, 10));
            self.agent_pool.as_mut()?.spawn_agent(provider).ok()?;
        }

        let focused_id = self.focused_agent_id()?;
        self.start_provider_for_agent(&focused_id, prompt)
    }

    /// Start provider request for a specific agent without changing focus.
    pub fn start_provider_for_agent(
        &mut self,
        agent_id: &AgentId,
        prompt: String,
    ) -> Option<std::sync::mpsc::Receiver<agent_core::provider::ProviderEvent>> {
        self.start_provider_for_agent_with_mode(agent_id, prompt, true, true)
    }

    pub fn start_raw_provider_for_agent(
        &mut self,
        agent_id: &AgentId,
        prompt: String,
    ) -> Option<std::sync::mpsc::Receiver<agent_core::provider::ProviderEvent>> {
        self.start_provider_for_agent_with_mode(agent_id, prompt, false, false)
    }

    fn start_provider_for_agent_with_mode(
        &mut self,
        agent_id: &AgentId,
        prompt: String,
        inject_mail: bool,
        record_user_prompt: bool,
    ) -> Option<std::sync::mpsc::Receiver<agent_core::provider::ProviderEvent>> {
        let augmented_prompt =
            self.build_provider_prompt_for_agent(agent_id, prompt.clone(), inject_mail)?;
        if inject_mail {
            self.mark_agent_mail_read(agent_id);
        }

        let (provider_kind, session_handle, cwd, busy_codename) = {
            let pool = self.agent_pool.as_ref()?;
            let slot = pool.get_slot_by_id(agent_id)?;
            if slot.has_provider_thread()
                || slot.status().is_stopping()
                || slot.status().is_active()
            {
                (
                    slot.provider_type()
                        .to_provider_kind()
                        .unwrap_or(self.session.app.selected_provider),
                    slot.session_handle().cloned(),
                    slot.cwd(),
                    Some(slot.codename().as_str().to_string()),
                )
            } else {
                (
                    slot.provider_type()
                        .to_provider_kind()
                        .unwrap_or(self.session.app.selected_provider),
                    slot.session_handle().cloned(),
                    slot.cwd(),
                    None,
                )
            }
        };
        if let Some(codename) = busy_codename {
            self.app_mut()
                .push_error_message(format!("agent {} is already busy", codename));
            return None;
        }

        let thread_name = format!("agent-{}-provider", agent_id.as_str());
        let thread_handle = agent_core::provider::start_provider_with_handle(
            provider_kind,
            augmented_prompt,
            cwd,
            session_handle,
            thread_name,
        );

        match thread_handle {
            Ok(handle) => {
                let (event_rx, join_handle) = handle.into_parts();
                let pool = self.agent_pool.as_mut()?;
                let slot = pool.get_slot_mut_by_id(agent_id)?;
                if record_user_prompt {
                    slot.append_transcript(TranscriptEntry::User(prompt));
                }
                if let Some(jh) = join_handle {
                    slot.set_thread_handle(jh);
                }
                let _ =
                    slot.transition_to(agent_core::agent_slot::AgentSlotStatus::responding_now());
                Some(event_rx)
            }
            Err(e) => {
                self.app_mut()
                    .push_error_message(format!("Failed to start provider: {}", e));
                None
            }
        }
    }

    /// Register an event receiver with the event aggregator
    ///
    /// This is used for polling events from multiple agents.
    pub fn register_agent_channel(&mut self, agent_id: AgentId, rx: Receiver<ProviderEvent>) {
        self.event_aggregator.add_receiver(agent_id.clone(), rx);
        crate::logging::debug_event(
            "tui.agent_channel.register",
            "registered agent event channel",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "total_channels": self.event_aggregator.receiver_count(),
            }),
        );
    }

    /// Unregister an agent's event channel (after agent finishes)
    pub fn unregister_agent_channel(&mut self, agent_id: &AgentId) {
        self.event_aggregator.remove_receiver(agent_id);
    }

    /// Poll all agent event channels
    pub fn poll_agent_events(&self) -> agent_core::event_aggregator::PollResult {
        self.event_aggregator.poll_all()
    }

    /// Poll agent events with timeout
    pub fn poll_agent_events_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> agent_core::event_aggregator::PollResult {
        self.event_aggregator.poll_with_timeout(timeout)
    }

    /// Get count of registered agent channels
    pub fn agent_channel_count(&self) -> usize {
        self.event_aggregator.receiver_count()
    }

    /// Clear all cached agent view states
    pub fn clear_agent_view_states(&mut self) {
        self.agent_view_states.clear();
    }

    /// Open provider selection overlay for agent creation
    pub fn open_provider_overlay(&mut self) {
        self.provider_overlay = Some(ProviderSelectionOverlay::new());
    }

    /// Close provider selection overlay
    pub fn close_provider_overlay(&mut self) {
        self.provider_overlay = None;
    }

    /// Check if provider overlay is open
    pub fn is_provider_overlay_open(&self) -> bool {
        self.provider_overlay.is_some()
    }

    /// Open launch config overlay for selected provider
    pub fn open_launch_config_overlay(&mut self, provider: ProviderKind) {
        self.launch_config_overlay = Some(LaunchConfigOverlayState::new(provider));
    }

    /// Close launch config overlay
    pub fn close_launch_config_overlay(&mut self) {
        self.launch_config_overlay = None;
    }

    /// Check if launch config overlay is open
    pub fn is_launch_config_overlay_open(&self) -> bool {
        self.launch_config_overlay.is_some()
    }

    /// Check if any overlay is open (transcript or provider)
    pub fn is_any_overlay_open(&self) -> bool {
        self.is_overlay_open()
            || self.is_provider_overlay_open()
            || self.is_confirmation_overlay_open()
            || self.is_human_decision_overlay_open()
            || self.is_launch_config_overlay_open()
    }

    /// Open confirmation overlay for stopping agent
    pub fn open_stop_confirmation(&mut self, agent_name: &str) {
        self.confirmation_overlay = Some(ConfirmationOverlay::for_stop_agent(agent_name));
    }

    /// Close confirmation overlay
    pub fn close_confirmation_overlay(&mut self) {
        self.confirmation_overlay = None;
    }

    /// Check if confirmation overlay is open
    pub fn is_confirmation_overlay_open(&self) -> bool {
        self.confirmation_overlay.is_some()
    }

    /// Open human decision overlay for decision request
    pub fn open_human_decision_overlay(&mut self, request: agent_decision::HumanDecisionRequest) {
        self.human_decision_overlay = Some(HumanDecisionOverlay::new(request));
    }

    /// Close human decision overlay
    pub fn close_human_decision_overlay(&mut self) {
        self.human_decision_overlay = None;
    }

    /// Check if human decision overlay is open
    pub fn is_human_decision_overlay_open(&self) -> bool {
        self.human_decision_overlay.is_some()
    }

    fn active_tool_ref(&self) -> Option<&ActiveTool> {
        match self.active_cell.as_ref() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            _ => None,
        }
    }

    fn active_tool_mut(&mut self) -> Option<&mut ActiveTool> {
        match self.active_cell.as_mut() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            _ => None,
        }
    }

    fn take_active_tool(&mut self) -> Option<ActiveTool> {
        match self.active_cell.take() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            Some(cell) => {
                self.active_cell = Some(cell);
                None
            }
            None => None,
        }
    }

    pub(crate) fn set_active_tool(&mut self, tool: ActiveTool) {
        self.active_cell = Some(ActiveCell::Tool(tool));
    }

    fn active_stream_ref(&self) -> Option<&ActiveStream> {
        match self.active_cell.as_ref() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            _ => None,
        }
    }

    fn active_stream_mut(&mut self) -> Option<&mut ActiveStream> {
        match self.active_cell.as_mut() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            _ => None,
        }
    }

    fn take_active_stream(&mut self) -> Option<ActiveStream> {
        match self.active_cell.take() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            Some(cell) => {
                self.active_cell = Some(cell);
                None
            }
            None => None,
        }
    }

    pub(crate) fn set_active_stream(&mut self, stream: ActiveStream) {
        self.active_cell = Some(ActiveCell::Stream(stream));
    }

    pub fn active_entries_for_display(&self) -> Vec<TranscriptEntry> {
        self.active_cell
            .as_ref()
            .map(ActiveCell::as_transcript_entries)
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn overlay_entries_for_display(&self) -> Vec<TranscriptEntry> {
        let mut entries = self.session.app.transcript.clone();
        entries.extend(self.active_entries_for_display());
        entries
    }

    #[cfg(test)]
    pub fn set_active_entry_for_test(&mut self, entry: TranscriptEntry) {
        self.active_cell = match entry {
            TranscriptEntry::ExecCommand {
                call_id,
                source,
                allow_exploring_group,
                input_preview,
                output_preview,
                status,
                exit_code,
                duration_ms,
            } => Some(ActiveCell::Tool(ActiveTool::Exec(vec![ActiveExecCall {
                call_id,
                source,
                allow_exploring_group,
                input_preview,
                output_preview,
                status,
                exit_code,
                duration_ms,
            }]))),
            TranscriptEntry::GenericToolCall {
                name,
                call_id,
                input_preview,
                output_preview,
                success,
                started,
                exit_code,
                duration_ms,
            } => Some(ActiveCell::Tool(ActiveTool::Generic(
                ActiveGenericToolCall {
                    name,
                    call_id,
                    input_preview,
                    output_preview,
                    success,
                    started,
                    exit_code,
                    duration_ms,
                },
            ))),
            TranscriptEntry::PatchApply {
                call_id,
                changes,
                status,
                output_preview,
            } => Some(ActiveCell::Tool(ActiveTool::Patch(ActivePatchApply {
                call_id,
                changes,
                status,
                output_preview,
            }))),
            TranscriptEntry::WebSearch {
                call_id,
                query,
                action,
                started,
            } => Some(ActiveCell::Tool(ActiveTool::WebSearch(ActiveWebSearch {
                call_id,
                query,
                action,
                started,
            }))),
            TranscriptEntry::McpToolCall {
                call_id,
                invocation,
                result_blocks,
                error,
                status,
                is_error,
            } => Some(ActiveCell::Tool(ActiveTool::Mcp(ActiveMcpToolCall {
                call_id,
                invocation,
                result_blocks,
                error,
                status,
                is_error,
            }))),
            TranscriptEntry::Assistant(text) => {
                self.set_active_stream(ActiveStream {
                    kind: StreamTextKind::Assistant,
                    tail: text,
                    pending_commits: VecDeque::new(),
                    collector: MarkdownStreamCollector::new(
                        self.transcript_render_width,
                        self.app().cwd.as_path(),
                    ),
                    policy: AdaptiveChunkingPolicy::default(),
                });
                self.bump_active_entries_revision();
                return;
            }
            TranscriptEntry::Thinking(text) => {
                self.set_active_stream(ActiveStream {
                    kind: StreamTextKind::Thinking,
                    tail: text,
                    pending_commits: VecDeque::new(),
                    collector: MarkdownStreamCollector::new(
                        self.transcript_render_width,
                        self.app().cwd.as_path(),
                    ),
                    policy: AdaptiveChunkingPolicy::default(),
                });
                self.bump_active_entries_revision();
                return;
            }
            other => panic!("unsupported active test entry: {other:?}"),
        };
        self.bump_active_entries_revision();
    }

    pub fn active_cell_transcript_key(&self) -> Option<ActiveCellTranscriptKey> {
        self.active_cell.as_ref().map(|_| ActiveCellTranscriptKey {
            revision: self.active_entries_revision,
            is_stream_continuation: self.live_tail_is_stream_continuation(),
        })
    }

    pub fn active_cell_transcript_lines(
        &self,
        width: u16,
    ) -> Option<Vec<ratatui::text::Line<'static>>> {
        let entries = self.active_entries_for_display();
        let lines = cells::flatten_cells(&cells::build_overlay_cells(&entries, width));
        (!lines.is_empty()).then_some(lines)
    }

    pub fn active_cell_preview_cells(&self, width: u16) -> Vec<cells::TranscriptCell> {
        cells::build_live_tail_cells(&self.active_entries_for_display(), width)
    }

    #[cfg(test)]
    pub(crate) fn active_tool_is_empty(&self) -> bool {
        self.active_tool_ref().is_none()
    }

    #[cfg(test)]
    fn active_tool_entries_len(&self) -> usize {
        self.active_tool_ref()
            .map(|tool| tool.as_transcript_entries().len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn active_stream_for_test(&self) -> Option<&ActiveStream> {
        self.active_stream_ref()
    }

    pub fn live_tail_is_stream_continuation(&self) -> bool {
        matches!(
            (
                self.app().transcript.last(),
                self.active_stream_ref().map(|stream| stream.kind),
            ),
            (
                Some(TranscriptEntry::Assistant(_)),
                Some(StreamTextKind::Assistant)
            ) | (
                Some(TranscriptEntry::Thinking(_)),
                Some(StreamTextKind::Thinking)
            )
        )
    }

    pub fn has_pending_active_stream_commits(&self) -> bool {
        self.active_stream_ref()
            .is_some_and(|stream| !stream.pending_commits.is_empty())
    }

    fn bump_active_entries_revision(&mut self) {
        self.active_entries_revision = self.active_entries_revision.wrapping_add(1);
    }

    pub fn append_active_assistant_chunk(&mut self, chunk: &str) {
        self.append_streaming_text_chunk(StreamTextKind::Assistant, chunk);
    }

    pub fn append_active_thinking_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.append_streaming_text_chunk(StreamTextKind::Thinking, chunk);
    }

    fn append_streaming_text_chunk(&mut self, kind: StreamTextKind, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        if matches!(self.active_cell, Some(ActiveCell::Tool(_))) {
            self.flush_active_entries_to_transcript();
        }

        let mut committed = None;
        let stream = self.ensure_active_stream(kind);
        stream.collector.push_delta(chunk);
        stream.tail.push_str(chunk);
        if let Some(split_index) = stream.tail.rfind('\n').map(|index| index + 1) {
            let remainder = stream.tail.split_off(split_index);
            let finished = std::mem::replace(&mut stream.tail, remainder);
            if !finished.is_empty() {
                committed = Some(finished);
            }
        }

        if let Some(committed) = committed {
            let rendered_lines = stream.collector.commit_complete_lines().len().max(1);
            stream.pending_commits.push_back(QueuedStreamCommit {
                text: committed,
                rendered_lines,
                enqueued_at: Instant::now(),
            });
        }
        self.drop_empty_active_stream();
        self.bump_active_entries_revision();
    }

    fn ensure_active_stream(&mut self, kind: StreamTextKind) -> &mut ActiveStream {
        if self
            .active_stream_ref()
            .is_some_and(|stream| stream.kind != kind && !stream.tail.is_empty())
            && let Some(stream) = self.take_active_stream()
        {
            self.flush_stream_to_transcript(stream);
        }

        if self.active_stream_ref().is_none() {
            self.set_active_stream(ActiveStream {
                kind,
                tail: String::new(),
                pending_commits: VecDeque::new(),
                collector: MarkdownStreamCollector::new(
                    self.transcript_render_width,
                    self.app().cwd.as_path(),
                ),
                policy: AdaptiveChunkingPolicy::default(),
            });
        }
        let stream = self.active_stream_mut().expect("active stream exists");
        stream.kind = kind;
        stream
    }

    fn drop_empty_active_stream(&mut self) {
        if self
            .active_stream_ref()
            .is_some_and(|stream| stream.tail.is_empty() && stream.pending_commits.is_empty())
        {
            self.active_cell = None;
        }
    }

    pub fn drain_active_stream_commit_tick(&mut self) -> bool {
        let now = Instant::now();
        let next = self.active_stream_mut().and_then(|stream| {
            let snapshot = QueueSnapshot {
                queued_lines: stream
                    .pending_commits
                    .iter()
                    .map(|commit| commit.rendered_lines)
                    .sum(),
                oldest_age: stream.oldest_queued_age(now),
            };
            let decision = stream.policy.decide(snapshot, now);
            let mut remaining = decision.drain_lines;
            if remaining == 0 {
                return None;
            }
            let mut drained = Vec::new();
            while remaining > 0 {
                let Some(commit) = stream.pending_commits.pop_front() else {
                    break;
                };
                remaining = remaining.saturating_sub(commit.rendered_lines);
                drained.push(commit.text);
            }
            if drained.is_empty() {
                return None;
            }
            Some((stream.kind, drained))
        });
        let Some((kind, drained)) = next else {
            return false;
        };
        for text in drained {
            self.commit_stream_text(kind, &text);
        }
        self.drop_empty_active_stream();
        self.bump_active_entries_revision();
        true
    }

    pub fn push_active_exec_started(
        &mut self,
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Exec);
        self.flush_active_stream_to_transcript();
        let call = ActiveExecCall {
            call_id,
            source,
            allow_exploring_group: true,
            input_preview,
            output_preview: None,
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        };
        match self.active_tool_mut() {
            Some(ActiveTool::Exec(group)) => {
                group.retain(|existing| {
                    !(call.call_id.is_some() && existing.call_id == call.call_id)
                });
                group.push(call);
            }
            _ => {
                self.set_active_tool(ActiveTool::Exec(vec![call]));
            }
        }
        self.bump_active_entries_revision();
    }

    pub fn append_active_exec_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(ActiveTool::Exec(group)) = self.active_tool_mut() {
            for entry in group.iter_mut().rev() {
                let matches_call_id = call_id.is_some() && entry.call_id == call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    entry
                        .output_preview
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                    self.bump_active_entries_revision();
                    return;
                }
            }
        }
    }

    pub fn finish_active_exec(
        &mut self,
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    ) {
        if let Some(ActiveTool::Exec(mut group)) = self.take_active_tool()
            && let Some(index) = group
                .iter()
                .rposition(|entry| call_id.is_some() && entry.call_id == call_id)
        {
            let entry = group.remove(index);
            self.session
                .app
                .transcript
                .push(TranscriptEntry::ExecCommand {
                    call_id: entry.call_id,
                    source: entry.source.or(source),
                    allow_exploring_group: entry.allow_exploring_group,
                    input_preview: entry.input_preview,
                    output_preview: output_preview.or(entry.output_preview),
                    status,
                    exit_code,
                    duration_ms,
                });
            if group.is_empty() {
                self.active_cell = None;
            } else {
                self.set_active_tool(ActiveTool::Exec(group));
            }
            self.bump_active_entries_revision();
            return;
        } else if let Some(tool) = self.take_active_tool() {
            self.set_active_tool(tool);
        }

        self.session.app.push_exec_command_finished(
            call_id,
            output_preview,
            status,
            exit_code,
            duration_ms,
            source,
        );
    }

    pub fn push_active_generic_tool_call_started(
        &mut self,
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Generic(ActiveGenericToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
            exit_code: None,
            duration_ms: None,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_generic_tool_call(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        if let Some(ActiveTool::Generic(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_name = entry.name == name;
            if matches_call_id || matches_name {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::GenericToolCall {
                        name: entry.name,
                        call_id: entry.call_id.or(call_id),
                        input_preview: entry.input_preview,
                        output_preview: output_preview.or(entry.output_preview),
                        success,
                        started: false,
                        exit_code,
                        duration_ms,
                    });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Generic(entry));
        }

        self.session.app.push_generic_tool_call_finished(
            name,
            call_id,
            output_preview,
            success,
            exit_code,
            duration_ms,
        );
    }

    pub fn push_active_patch_apply_started(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Patch(ActivePatchApply {
            call_id,
            changes,
            status: PatchApplyStatus::InProgress,
            output_preview: None,
        }));
        self.bump_active_entries_revision();
    }

    pub fn append_active_patch_apply_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(ActiveTool::Patch(entry)) = self.active_tool_mut() {
            {
                let existing_call_id = &entry.call_id;
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    entry
                        .output_preview
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                    self.bump_active_entries_revision();
                    return;
                }
            }
        }

        self.session.app.append_patch_apply_output(call_id, delta);
    }

    pub fn finish_active_patch_apply(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
    ) {
        if let Some(ActiveTool::Patch(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::PatchApply {
                        call_id: entry.call_id.or(call_id),
                        changes: if changes.is_empty() {
                            entry.changes
                        } else {
                            changes
                        },
                        status,
                        output_preview: entry.output_preview,
                    });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Patch(entry));
        }

        self.session
            .app
            .push_patch_apply_finished(call_id, changes, status);
    }

    pub fn push_active_web_search_started(&mut self, call_id: Option<String>, query: String) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::WebSearch(ActiveWebSearch {
            call_id,
            query,
            action: None,
            started: true,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_web_search(
        &mut self,
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    ) {
        if let Some(ActiveTool::WebSearch(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::WebSearch {
                        call_id: entry.call_id.or(call_id),
                        query,
                        action,
                        started: false,
                    });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::WebSearch(entry));
        }

        self.session
            .app
            .push_web_search_finished(call_id, query, action);
    }

    pub fn push_active_mcp_tool_call_started(
        &mut self,
        call_id: Option<String>,
        invocation: McpInvocation,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Mcp(ActiveMcpToolCall {
            call_id,
            invocation,
            result_blocks: Vec::new(),
            error: None,
            status: McpToolCallStatus::InProgress,
            is_error: false,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_mcp_tool_call(
        &mut self,
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    ) {
        if let Some(ActiveTool::Mcp(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::McpToolCall {
                        call_id: entry.call_id.or(call_id),
                        invocation,
                        result_blocks,
                        error,
                        status,
                        is_error,
                    });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Mcp(entry));
        }

        self.session.app.push_mcp_tool_call_finished(
            call_id,
            invocation,
            result_blocks,
            error,
            status,
            is_error,
        );
    }

    pub fn flush_active_entries_to_transcript(&mut self) {
        self.drain_active_entries(None);
    }

    pub fn finalize_active_entries_after_failure(&mut self, reason: Option<&str>) {
        self.drain_active_entries(reason);
        self.mark_in_progress_transcript_entries_failed(reason);
    }

    pub fn sync_app_input_from_composer(&mut self) {
        self.session.app.input = self.composer.text().to_string();
    }

    #[allow(dead_code)]
    pub fn replace_transcript(&mut self, transcript: Vec<TranscriptEntry>) {
        self.session.app.transcript = transcript;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        self.transcript_max_scroll = 0;
        if self.transcript_follow_tail {
            self.transcript_scroll_offset = 0;
        } else {
            self.transcript_scroll_offset = self
                .transcript_scroll_offset
                .min(self.transcript_max_scroll);
        }
    }

    pub fn into_app_state(mut self) -> AppState {
        self.sync_app_input_from_composer();
        self.session.app
    }

    pub fn persist_if_changed(&mut self) -> Result<()> {
        self.session.persist_if_changed()
    }

    pub fn is_overlay_open(&self) -> bool {
        self.transcript_overlay.is_some()
    }

    pub fn open_transcript_overlay(&mut self) {
        if self.transcript_overlay.is_none() {
            self.transcript_overlay = Some(TranscriptOverlayState::pinned_to_bottom());
        }
    }

    pub fn close_transcript_overlay(&mut self) {
        self.transcript_overlay = None;
    }

    pub fn scroll_transcript_up(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_sub(rows);
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_down(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_add(rows);
        if rows > 0 {
            self.transcript_follow_tail =
                self.transcript_scroll_offset >= self.transcript_max_scroll;
        }
    }

    pub fn scroll_transcript_home(&mut self) {
        self.transcript_scroll_offset = 0;
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_end(&mut self) {
        self.transcript_follow_tail = true;
    }

    pub fn sync_busy_started_at(&mut self) {
        if self.is_busy() {
            if self.busy_started_at.is_none() {
                self.busy_started_at = Some(Instant::now());
            }
        } else {
            self.busy_started_at = None;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.session.app.status == AppStatus::Responding
            || !matches!(self.session.app.loop_phase, LoopPhase::Idle)
            || self
                .agent_pool
                .as_ref()
                .is_some_and(|pool| pool.has_active_agents())
    }

    pub fn switch_to_new_agent(
        &mut self,
        provider_kind: agent_core::provider::ProviderKind,
    ) -> Result<String> {
        self.sync_app_input_from_composer();
        let summary = self.session.switch_agent(provider_kind)?;
        logging::debug_event(
            "tui.provider_switch",
            "switched to sibling agent from TUI state",
            serde_json::json!({
                "provider": provider_kind.label(),
                "summary": summary,
            }),
        );
        self.composer = TextArea::new();
        self.composer_state = TextAreaState::default();
        self.transcript_overlay = None;
        self.active_cell = None;
        self.bump_active_entries_revision();
        self.transcript_scroll_offset = 0;
        self.transcript_max_scroll = 0;
        self.transcript_follow_tail = true;
        self.transcript_render_width = None;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        self.busy_started_at = None;
        Ok(summary)
    }
}

impl TuiState {
    fn drain_active_entries(&mut self, failure_reason: Option<&str>) {
        if self.active_cell.is_none() {
            return;
        }
        if let Some(cell) = self.active_cell.take() {
            match (failure_reason, cell) {
                (_, ActiveCell::Stream(stream)) => self.flush_stream_to_transcript(stream),
                (Some(_), ActiveCell::Tool(ActiveTool::Exec(group))) => {
                    for entry in group {
                        self.session
                            .app
                            .transcript
                            .push(TranscriptEntry::ExecCommand {
                                call_id: entry.call_id,
                                source: entry.source,
                                allow_exploring_group: entry.allow_exploring_group,
                                input_preview: entry.input_preview,
                                output_preview: entry.output_preview,
                                status: ExecCommandStatus::Failed,
                                exit_code: entry.exit_code,
                                duration_ms: entry.duration_ms,
                            });
                    }
                }
                (Some(_), ActiveCell::Tool(ActiveTool::Generic(entry))) => {
                    self.session
                        .app
                        .transcript
                        .push(TranscriptEntry::GenericToolCall {
                            name: entry.name,
                            call_id: entry.call_id,
                            input_preview: entry.input_preview,
                            output_preview: entry.output_preview,
                            success: false,
                            started: false,
                            exit_code: None,
                            duration_ms: None,
                        });
                }
                (Some(_), ActiveCell::Tool(ActiveTool::Patch(entry))) => {
                    self.session
                        .app
                        .transcript
                        .push(TranscriptEntry::PatchApply {
                            call_id: entry.call_id,
                            changes: entry.changes,
                            status: PatchApplyStatus::Failed,
                            output_preview: entry.output_preview,
                        });
                }
                (Some(_), ActiveCell::Tool(ActiveTool::WebSearch(entry))) => {
                    self.session
                        .app
                        .transcript
                        .push(TranscriptEntry::WebSearch {
                            call_id: entry.call_id,
                            query: entry.query,
                            action: entry.action,
                            started: false,
                        });
                }
                (Some(reason), ActiveCell::Tool(ActiveTool::Mcp(entry))) => {
                    self.session
                        .app
                        .transcript
                        .push(TranscriptEntry::McpToolCall {
                            call_id: entry.call_id,
                            invocation: entry.invocation,
                            result_blocks: entry.result_blocks,
                            error: entry.error.or_else(|| Some(reason.to_string())),
                            status: McpToolCallStatus::Failed,
                            is_error: true,
                        });
                }
                (_, ActiveCell::Tool(tool)) => {
                    for entry in tool.as_transcript_entries() {
                        self.session.app.transcript.push(entry);
                    }
                }
            }
        }
        self.bump_active_entries_revision();
    }

    fn commit_stream_text(&mut self, kind: StreamTextKind, text: &str) {
        match kind {
            StreamTextKind::Assistant => self.session.app.append_assistant_chunk(text),
            StreamTextKind::Thinking => self.session.app.append_thinking_chunk(text),
        }
    }

    fn mark_in_progress_transcript_entries_failed(&mut self, reason: Option<&str>) {
        for entry in &mut self.session.app.transcript {
            match entry {
                TranscriptEntry::ExecCommand {
                    status: exec_status,
                    ..
                } if matches!(*exec_status, ExecCommandStatus::InProgress) => {
                    *exec_status = ExecCommandStatus::Failed;
                }
                TranscriptEntry::GenericToolCall {
                    success, started, ..
                } if *started => {
                    *success = false;
                    *started = false;
                }
                TranscriptEntry::PatchApply { status, .. }
                    if matches!(*status, PatchApplyStatus::InProgress) =>
                {
                    *status = PatchApplyStatus::Failed;
                }
                TranscriptEntry::WebSearch { started, .. } if *started => {
                    *started = false;
                }
                TranscriptEntry::McpToolCall {
                    error,
                    status,
                    is_error,
                    ..
                } if matches!(*status, McpToolCallStatus::InProgress) => {
                    *status = McpToolCallStatus::Failed;
                    *is_error = true;
                    if error.is_none() {
                        *error = reason.map(ToOwned::to_owned);
                    }
                }
                _ => {}
            }
        }
    }

    fn flush_stream_to_transcript(&mut self, stream: ActiveStream) {
        let ActiveStream {
            kind,
            tail,
            pending_commits,
            mut collector,
            ..
        } = stream;
        for commit in pending_commits {
            self.commit_stream_text(kind, &commit.text);
        }
        if !tail.is_empty() {
            self.commit_stream_text(kind, &tail);
        }
        let _ = collector.finalize_and_drain();
    }

    fn flush_active_stream_to_transcript(&mut self) {
        if let Some(stream) = self.take_active_stream() {
            self.flush_stream_to_transcript(stream);
            self.bump_active_entries_revision();
        }
    }

    fn prepare_for_active_tool_start(&mut self, start: ActiveToolStart) {
        let should_flush = match start {
            ActiveToolStart::Exec => {
                matches!(self.active_tool_ref(), Some(tool) if !matches!(tool, ActiveTool::Exec(_)))
            }
            ActiveToolStart::Other => self.active_tool_ref().is_some(),
        };
        if should_flush {
            self.flush_active_entries_to_transcript();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTool {
    Exec(Vec<ActiveExecCall>),
    Generic(ActiveGenericToolCall),
    Patch(ActivePatchApply),
    WebSearch(ActiveWebSearch),
    Mcp(ActiveMcpToolCall),
}

impl ActiveTool {
    fn as_transcript_entries(&self) -> Vec<TranscriptEntry> {
        match self {
            ActiveTool::Exec(group) => group
                .iter()
                .map(|entry| TranscriptEntry::ExecCommand {
                    call_id: entry.call_id.clone(),
                    source: entry.source.clone(),
                    allow_exploring_group: entry.allow_exploring_group,
                    input_preview: entry.input_preview.clone(),
                    output_preview: entry.output_preview.clone(),
                    status: entry.status,
                    exit_code: entry.exit_code,
                    duration_ms: entry.duration_ms,
                })
                .collect(),
            ActiveTool::Generic(entry) => vec![TranscriptEntry::GenericToolCall {
                name: entry.name.clone(),
                call_id: entry.call_id.clone(),
                input_preview: entry.input_preview.clone(),
                output_preview: entry.output_preview.clone(),
                success: entry.success,
                started: entry.started,
                exit_code: entry.exit_code,
                duration_ms: entry.duration_ms,
            }],
            ActiveTool::Patch(entry) => vec![TranscriptEntry::PatchApply {
                call_id: entry.call_id.clone(),
                changes: entry.changes.clone(),
                status: entry.status,
                output_preview: entry.output_preview.clone(),
            }],
            ActiveTool::WebSearch(entry) => vec![TranscriptEntry::WebSearch {
                call_id: entry.call_id.clone(),
                query: entry.query.clone(),
                action: entry.action.clone(),
                started: entry.started,
            }],
            ActiveTool::Mcp(entry) => vec![TranscriptEntry::McpToolCall {
                call_id: entry.call_id.clone(),
                invocation: entry.invocation.clone(),
                result_blocks: entry.result_blocks.clone(),
                error: entry.error.clone(),
                status: entry.status,
                is_error: entry.is_error,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveCell {
    Tool(ActiveTool),
    Stream(ActiveStream),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveCellTranscriptKey {
    pub revision: u64,
    pub is_stream_continuation: bool,
}

impl ActiveCell {
    fn as_transcript_entries(&self) -> Vec<TranscriptEntry> {
        match self {
            ActiveCell::Tool(tool) => tool.as_transcript_entries(),
            ActiveCell::Stream(stream) => vec![stream.as_transcript_entry()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveExecCall {
    pub(crate) call_id: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) allow_exploring_group: bool,
    pub(crate) input_preview: Option<String>,
    pub(crate) output_preview: Option<String>,
    pub(crate) status: ExecCommandStatus,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGenericToolCall {
    pub(crate) name: String,
    pub(crate) call_id: Option<String>,
    pub(crate) input_preview: Option<String>,
    pub(crate) output_preview: Option<String>,
    pub(crate) success: bool,
    pub(crate) started: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePatchApply {
    pub(crate) call_id: Option<String>,
    pub(crate) changes: Vec<PatchChange>,
    pub(crate) status: PatchApplyStatus,
    pub(crate) output_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWebSearch {
    pub(crate) call_id: Option<String>,
    pub(crate) query: String,
    pub(crate) action: Option<WebSearchAction>,
    pub(crate) started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMcpToolCall {
    pub(crate) call_id: Option<String>,
    pub(crate) invocation: McpInvocation,
    pub(crate) result_blocks: Vec<serde_json::Value>,
    pub(crate) error: Option<String>,
    pub(crate) status: McpToolCallStatus,
    pub(crate) is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveStream {
    pub(crate) kind: StreamTextKind,
    pub(crate) tail: String,
    pub(crate) pending_commits: VecDeque<QueuedStreamCommit>,
    pub(crate) collector: MarkdownStreamCollector,
    pub(crate) policy: AdaptiveChunkingPolicy,
}

impl ActiveStream {
    fn as_transcript_entry(&self) -> TranscriptEntry {
        match self.kind {
            StreamTextKind::Assistant => TranscriptEntry::Assistant(self.tail.clone()),
            StreamTextKind::Thinking => TranscriptEntry::Thinking(self.tail.clone()),
        }
    }

    fn oldest_queued_age(&self, now: Instant) -> Option<std::time::Duration> {
        self.pending_commits
            .front()
            .map(|commit| now.saturating_duration_since(commit.enqueued_at))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedStreamCommit {
    pub(crate) text: String,
    pub(crate) rendered_lines: usize,
    pub(crate) enqueued_at: Instant,
}

/// Parsed @ command result for agent routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtCommandResult {
    /// Send to single agent
    Single { agent: String, message: String },
    /// Broadcast to multiple agents
    Broadcast {
        agents: Vec<String>,
        message: String,
    },
    /// No @ command, normal input
    Normal(String),
    /// Malformed @ command
    Invalid { error: String },
}

/// Parse input for @ command syntax
///
/// Supports:
/// - `@alpha hello` -> Single { agent: "alpha", message: "hello" }
/// - `@alpha,bravo hello` -> Broadcast { agents: ["alpha", "bravo"], message: "hello" }
/// - `@alpha, bravo hello` -> Broadcast { agents: ["alpha", "bravo"], message: "hello" } (space after comma)
/// - `hello world` -> Normal("hello world")
pub fn parse_at_command(input: &str) -> AtCommandResult {
    let trimmed = input.trim();

    if !trimmed.starts_with('@') {
        return AtCommandResult::Normal(input.to_string());
    }

    // Find the message part after agents
    let rest = &trimmed[1..]; // Skip the '@'

    // Collect all words
    let words: Vec<&str> = rest.split(' ').filter(|s| !s.is_empty()).collect();

    if words.is_empty() {
        return AtCommandResult::Invalid {
            error: "No agent specified".to_string(),
        };
    }

    // Find where agent specs end
    // - Words ending with comma are agent names (more agents follow)
    // - A word not ending with comma after a comma-ending word is still an agent
    // - The message starts after all agent names are collected
    let mut agent_words: Vec<&str> = Vec::new();
    let mut message_words: Vec<&str> = Vec::new();
    let mut expecting_more_agents = false;

    for word in &words {
        if expecting_more_agents {
            // Previous word ended with comma, so this word is an agent name
            agent_words.push(word);
            expecting_more_agents = word.ends_with(',');
        } else if agent_words.is_empty() {
            // First word - always an agent
            agent_words.push(word);
            expecting_more_agents = word.ends_with(',');
        } else {
            // Not expecting more agents, this is the message
            message_words.push(word);
        }
    }

    // Check if we have any agents
    if agent_words.is_empty() {
        return AtCommandResult::Invalid {
            error: "No agent specified".to_string(),
        };
    }

    // Join agent words and split by comma
    let agent_spec = agent_words.join("");
    let agents: Vec<String> = agent_spec
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if agents.is_empty() {
        return AtCommandResult::Invalid {
            error: "No agent specified".to_string(),
        };
    }

    // Check if we have a message
    let message = message_words.join(" ");
    if message.is_empty() {
        return AtCommandResult::Invalid {
            error: "Missing message after agent name".to_string(),
        };
    }

    if agents.len() == 1 {
        AtCommandResult::Single {
            agent: agents.into_iter().next().unwrap(),
            message,
        }
    } else {
        AtCommandResult::Broadcast { agents, message }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamTextKind {
    Assistant,
    Thinking,
}

#[derive(Clone, Copy)]
enum ActiveToolStart {
    Exec,
    Other,
}

#[cfg(test)]
mod tests {
    use super::ActiveCellTranscriptKey;
    use super::ActiveStream;
    use super::AtCommandResult;
    use super::StreamTextKind;
    use super::TuiState;
    use super::parse_at_command;
    use agent_core::agent_runtime::AgentId;
    use agent_core::agent_slot::TaskId;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use agent_core::tool_calls::ExecCommandStatus;
    use agent_core::tool_calls::McpInvocation;
    use agent_core::tool_calls::McpToolCallStatus;
    use agent_core::tool_calls::PatchApplyStatus;
    use agent_core::tool_calls::PatchChange;
    use agent_core::tool_calls::PatchChangeKind;
    use agent_core::tool_calls::WebSearchAction;
    use tempfile::TempDir;

    #[test]
    fn switching_provider_creates_new_agent_runtime() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
                .expect("bootstrap");
        session.app.push_status_message("existing transcript");

        let mut state = TuiState::from_session(session);
        let previous_agent_id = state.session.agent_runtime.agent_id().as_str().to_string();

        let summary = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert_ne!(
            state.session.agent_runtime.agent_id().as_str(),
            previous_agent_id
        );
        assert_eq!(state.session.app.selected_provider, ProviderKind::Codex);
        assert!(summary.contains("agent_"));
        assert!(matches!(
            state.session.app.transcript.first(),
            Some(TranscriptEntry::Status(text)) if text.contains("created agent:")
        ));
    }

    #[test]
    fn scrolling_down_to_known_tail_restores_follow_mode() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_scroll_offset = 6;
        state.transcript_max_scroll = 6;
        state.transcript_follow_tail = false;

        state.scroll_transcript_up(2);
        assert!(!state.transcript_follow_tail);

        state.scroll_transcript_down(2);

        assert_eq!(state.transcript_scroll_offset, 6);
        assert!(state.transcript_follow_tail);
    }

    #[test]
    fn switch_to_new_agent_clears_active_entries() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.set_active_entry_for_test(TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

        let _ = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert!(state.active_tool_is_empty());
    }

    #[test]
    fn active_cell_transcript_key_is_none_without_active_cell() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        assert_eq!(state.active_cell_transcript_key(), None);
    }

    #[test]
    fn active_cell_transcript_key_reflects_revision_and_stream_continuation() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state
            .app_mut()
            .transcript
            .push(TranscriptEntry::Assistant("committed".to_string()));
        state.append_active_assistant_chunk("tail");

        assert_eq!(
            state.active_cell_transcript_key(),
            Some(ActiveCellTranscriptKey {
                revision: state.active_entries_revision,
                is_stream_continuation: true,
            })
        );
    }

    #[test]
    fn active_cell_transcript_lines_render_current_live_tail() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.append_active_assistant_chunk("tail");

        let rendered = state
            .active_cell_transcript_lines(80)
            .expect("active lines")
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered.iter().any(|line| line == "tail"));
    }

    #[test]
    fn active_cell_transcript_lines_render_exec_group_as_command_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("ls -la".to_string()),
            Some("agent".to_string()),
        );
        state.push_active_exec_started(
            Some("call-2".to_string()),
            Some("cat src/lib.rs".to_string()),
            Some("agent".to_string()),
        );

        let rendered = state
            .active_cell_transcript_lines(80)
            .expect("active lines")
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            rendered.iter().any(|line| line.contains("$ ls -la")),
            "{rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("$ cat src/lib.rs")),
            "{rendered:?}"
        );
    }

    #[test]
    fn active_cell_preview_cells_render_active_exec() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.set_active_entry_for_test(TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

        let rendered = state
            .active_cell_preview_cells(80)
            .into_iter()
            .flat_map(|cell| cell.lines)
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered.iter().any(|line| line == "• Running printf hello"));
    }

    #[test]
    fn active_exec_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.append_active_exec_output(Some("call-1".to_string()), "hello\n");
        state.finish_active_exec(
            Some("call-1".to_string()),
            None,
            agent_core::tool_calls::ExecCommandStatus::Completed,
            Some(0),
            Some(5),
            Some("agent".to_string()),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                output_preview,
                status,
                ..
            })
            if call_id.as_deref() == Some("call-1")
                && output_preview.as_deref() == Some("hello\n")
                && *status == ExecCommandStatus::Completed
        ));
    }

    #[test]
    fn active_generic_tool_call_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_generic_tool_call_started(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("{\"cmd\":\"git status\"}".to_string()),
        );
        state.finish_active_generic_tool_call(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("On branch main".to_string()),
            true,
            Some(0),
            Some(20),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::GenericToolCall {
                name,
                call_id,
                output_preview,
                success,
                started,
                ..
            })
            if name == "shell"
                && call_id.as_deref() == Some("tool-1")
                && output_preview.as_deref() == Some("On branch main")
                && *success
                && !started
        ));
    }

    #[test]
    fn active_patch_apply_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let changes = vec![PatchChange {
            path: "README.md".to_string(),
            move_path: None,
            kind: PatchChangeKind::Update,
            diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
            added: 1,
            removed: 1,
        }];

        state.push_active_patch_apply_started(Some("patch-1".to_string()), changes.clone());
        state.append_active_patch_apply_output(Some("patch-1".to_string()), "applied");
        state.finish_active_patch_apply(
            Some("patch-1".to_string()),
            changes.clone(),
            PatchApplyStatus::Completed,
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::PatchApply {
                call_id,
                changes: committed_changes,
                status,
                output_preview,
            })
            if call_id.as_deref() == Some("patch-1")
                && committed_changes == &changes
                && *status == PatchApplyStatus::Completed
                && output_preview.as_deref() == Some("applied")
        ));
    }

    #[test]
    fn active_web_search_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let action = Some(WebSearchAction::OpenPage {
            url: Some("https://example.com".to_string()),
        });

        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );
        state.finish_active_web_search(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
            action.clone(),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::WebSearch {
                call_id,
                query,
                action: committed_action,
                started,
            })
            if call_id.as_deref() == Some("search-1")
                && query == "ratatui styling"
                && committed_action == &action
                && !started
        ));
    }

    #[test]
    fn active_mcp_tool_call_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let invocation = McpInvocation {
            server: "search".to_string(),
            tool: "find_docs".to_string(),
            arguments: Some(serde_json::json!({
                "query": "ratatui styling",
                "limit": 3
            })),
        };
        let result_blocks = vec![serde_json::json!({
            "type": "text",
            "text": "doc-1"
        })];

        state.push_active_mcp_tool_call_started(Some("mcp-1".to_string()), invocation.clone());
        state.finish_active_mcp_tool_call(
            Some("mcp-1".to_string()),
            invocation.clone(),
            result_blocks.clone(),
            None,
            McpToolCallStatus::Completed,
            false,
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::McpToolCall {
                call_id,
                invocation: committed_invocation,
                result_blocks: committed_result_blocks,
                error,
                status,
                is_error,
            })
            if call_id.as_deref() == Some("mcp-1")
                && committed_invocation == &invocation
                && committed_result_blocks == &result_blocks
                && error.is_none()
                && *status == McpToolCallStatus::Completed
                && !is_error
        ));
    }

    #[test]
    fn transcript_overlay_opens_pinned_to_bottom() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.open_transcript_overlay();

        assert_eq!(
            state
                .transcript_overlay
                .as_ref()
                .expect("overlay")
                .scroll_offset,
            usize::MAX
        );
    }

    #[test]
    fn replace_transcript_swaps_committed_history_and_clears_scroll_anchors() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_rendered_lines = vec!["stale".to_string()];
        state.transcript_last_cell_range = Some((10, 2));

        state.replace_transcript(vec![TranscriptEntry::Status("replaced".to_string())]);

        assert_eq!(
            state.app().transcript,
            vec![TranscriptEntry::Status("replaced".to_string())]
        );
        assert!(state.transcript_rendered_lines.is_empty());
        assert!(state.transcript_last_cell_range.is_none());
        assert_eq!(state.transcript_scroll_offset, 0);
    }

    #[test]
    fn replace_transcript_clamps_manual_scroll_state() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_follow_tail = false;
        state.transcript_scroll_offset = 99;
        state.transcript_max_scroll = 99;

        state.replace_transcript(vec![TranscriptEntry::Status("replaced".to_string())]);

        assert_eq!(state.transcript_scroll_offset, 0);
        assert_eq!(state.transcript_max_scroll, 0);
    }

    #[test]
    fn active_assistant_chunks_stay_in_live_tail_until_finalize() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "hello world"
        ));
        assert!(!state.app().transcript.iter().any(
            |entry| matches!(entry, TranscriptEntry::Assistant(text) if text == "hello world")
        ));

        state.finalize_active_entries_after_failure(None);

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "hello world"
        ));
    }

    #[test]
    fn assistant_chunks_commit_completed_lines_and_keep_partial_tail_active() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world\nnext");

        assert!(!state.app().transcript.iter().any(
            |entry| matches!(entry, TranscriptEntry::Assistant(text) if text == "hello world\n")
        ));
        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn assistant_stream_flushes_active_exec_entries_into_committed_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.append_active_assistant_chunk("answer");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "answer"
        ));
    }

    #[test]
    fn starting_exec_flushes_active_assistant_stream_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("streaming answer");
        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );

        assert!(state.active_stream_for_test().is_none());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "streaming answer"
        ));
        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
    }

    #[test]
    fn starting_web_search_flushes_active_exec_live_tail_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );

        assert_eq!(state.active_tool_entries_len(), 1);
        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::WebSearch { call_id, started, .. })
                if call_id.as_deref() == Some("search-1") && *started
        ));
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
    }

    #[test]
    fn active_thinking_chunks_stay_in_live_tail_until_finalize() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_thinking_chunk("step 1 ");
        state.append_active_thinking_chunk("step 2");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Thinking,
                tail,
                ..
            }) if tail == "step 1 step 2"
        ));
        assert!(!state.app().transcript.iter().any(
            |entry| matches!(entry, TranscriptEntry::Thinking(text) if text == "step 1 step 2")
        ));

        state.finalize_active_entries_after_failure(None);

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Thinking(text)) if text == "step 1 step 2"
        ));
    }

    #[test]
    fn flush_active_entries_to_transcript_commits_live_tail_without_failure_semantics() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("tail");
        state.flush_active_entries_to_transcript();

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "tail"
        ));
    }

    #[test]
    fn thinking_chunks_commit_completed_lines_and_keep_partial_tail_active() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_thinking_chunk("step 1 ");
        state.append_active_thinking_chunk("step 2\nnext");

        assert!(!state.app().transcript.iter().any(
            |entry| matches!(entry, TranscriptEntry::Thinking(text) if text == "step 1 step 2\n")
        ));
        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Thinking,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn active_stream_commit_tick_drains_queued_assistant_lines() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world\nnext");

        assert!(state.drain_active_stream_commit_tick());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "hello world\n"
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn active_stream_commit_tick_catches_up_large_backlog() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        for index in 1..=8 {
            state.append_active_assistant_chunk(&format!("line {index}\n"));
        }
        state.append_active_assistant_chunk("tail");

        assert!(state.drain_active_stream_commit_tick());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text))
                if text == "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\n"
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "tail"
        ));
        assert!(!state.drain_active_stream_commit_tick());
    }

    #[test]
    fn active_stream_snapshots_render_width_for_commit_line_count() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_render_width = Some(8);

        state.append_active_assistant_chunk("123456789\n");

        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream { pending_commits, .. })
                if pending_commits.front().is_some_and(|commit| commit.rendered_lines == 2)
        ));
    }

    #[test]
    fn finalizing_active_entries_after_failure_commits_failed_history_and_clears_live_tail() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let patch_changes = vec![PatchChange {
            path: "README.md".to_string(),
            move_path: None,
            kind: PatchChangeKind::Update,
            diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
            added: 1,
            removed: 1,
        }];
        let invocation = McpInvocation {
            server: "search".to_string(),
            tool: "find_docs".to_string(),
            arguments: Some(serde_json::json!({ "query": "ratatui styling" })),
        };

        state.push_active_exec_started(
            Some("exec-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.push_active_generic_tool_call_started(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("{\"cmd\":\"git status\"}".to_string()),
        );
        state.push_active_patch_apply_started(Some("patch-1".to_string()), patch_changes.clone());
        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );
        state.push_active_mcp_tool_call_started(Some("mcp-1".to_string()), invocation.clone());

        state.finalize_active_entries_after_failure(Some("provider failed"));

        assert!(state.active_tool_is_empty());
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::ExecCommand {
                    call_id,
                    status: ExecCommandStatus::Failed,
                    ..
                } if call_id.as_deref() == Some("exec-1")
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::GenericToolCall {
                    call_id,
                    success,
                    started,
                    ..
                } if call_id.as_deref() == Some("tool-1") && !success && !started
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::PatchApply {
                    call_id,
                    changes,
                    status: PatchApplyStatus::Failed,
                    ..
                } if call_id.as_deref() == Some("patch-1") && changes == &patch_changes
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::WebSearch {
                    call_id,
                    started,
                    ..
                } if call_id.as_deref() == Some("search-1") && !started
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::McpToolCall {
                    call_id,
                    invocation: committed_invocation,
                    error,
                    status: McpToolCallStatus::Failed,
                    is_error,
                    ..
                } if call_id.as_deref() == Some("mcp-1")
                    && committed_invocation == &invocation
                    && error.as_deref() == Some("provider failed")
                    && *is_error
            )
        }));
    }

    #[test]
    fn agent_view_state_default_values() {
        use super::AgentViewState;

        let state = AgentViewState::default();
        assert_eq!(state.scroll_offset, 0);
        assert!(state.follow_tail);
        assert!(state.last_cell_range.is_none());
    }

    #[test]
    fn save_and_restore_agent_view_state() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Set some view state
        state.transcript_scroll_offset = 10;
        state.transcript_follow_tail = false;
        state.transcript_last_cell_range = Some((5, 15));

        // Save it
        state.save_agent_view_state("agent-001");

        // Modify current state
        state.transcript_scroll_offset = 20;
        state.transcript_follow_tail = true;

        // Restore it
        state.restore_agent_view_state("agent-001");

        assert_eq!(state.transcript_scroll_offset, 10);
        assert!(!state.transcript_follow_tail);
        assert_eq!(state.transcript_last_cell_range, Some((5, 15)));
    }

    #[test]
    fn restore_new_agent_state_defaults() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Set some view state
        state.transcript_scroll_offset = 10;
        state.transcript_follow_tail = false;

        // Restore for unknown agent (should reset to defaults)
        state.restore_agent_view_state("unknown-agent");

        assert_eq!(state.transcript_scroll_offset, 0);
        assert!(state.transcript_follow_tail);
    }

    #[test]
    fn switch_focus_preserves_view_state() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn two agents to enable multi-agent mode
        state.spawn_agent(ProviderKind::Mock);
        state.spawn_agent(ProviderKind::Mock);

        // Set scroll offset for current view
        state.transcript_scroll_offset = 10;

        // Switch to next agent
        let _ = state.focus_next_agent();

        // Original scroll offset should be saved in view states
        // (though we can't verify exact agent ID since pool generates them)
        // Verify that the scroll offset changed to default for new agent
        assert_eq!(state.transcript_scroll_offset, 0);
    }

    #[test]
    fn spawn_agent_creates_pool_and_agent() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Initially no pool
        assert!(state.agent_pool.is_none());

        // Spawn creates pool with OVERVIEW agent + worker agent
        let agent_id = state.spawn_agent(ProviderKind::Claude);
        assert!(agent_id.is_some());
        assert!(state.agent_pool.is_some());
        // OVERVIEW agent + 1 worker agent = 2
        let pool = state.agent_pool.as_ref().unwrap();
        assert_eq!(pool.active_count(), 2);
        // OVERVIEW agent should be first (focused)
        let focused = pool.focused_slot().expect("focused slot");
        assert_eq!(focused.codename().as_str(), "OVERVIEW");
    }

    #[test]
    fn focus_next_agent_cycles_pool() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn multiple agents
        state.spawn_agent(ProviderKind::Mock);
        state.spawn_agent(ProviderKind::Mock);
        state.spawn_agent(ProviderKind::Mock);

        // Focus should cycle
        let first_codename = state.focused_agent_codename().to_string();
        state.focus_next_agent();
        let second_codename = state.focused_agent_codename().to_string();
        state.focus_next_agent();
        let third_codename = state.focused_agent_codename().to_string();

        // All should have different codenames
        assert_ne!(first_codename, second_codename);
        assert_ne!(second_codename, third_codename);
    }

    #[test]
    fn stop_focused_agent_marks_stopped() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.spawn_agent(ProviderKind::Mock);
        let agent_id = state.stop_focused_agent();
        assert!(agent_id.is_some());

        // Agent should be stopped (terminal status)
        let pool = state.agent_pool.as_ref().unwrap();
        let slot = pool.focused_slot().unwrap();
        assert!(slot.status().is_terminal());
    }

    #[test]
    fn multi_agent_provider_request_registers_channel_with_aggregator() {
        use agent_core::provider::ProviderEvent;
        use std::sync::mpsc;

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent to activate multi-agent mode
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");
        assert!(state.is_multi_agent_mode());

        // Start provider for focused agent
        let event_rx = state
            .start_provider_for_focused_agent("test prompt".to_string(), ProviderKind::Mock)
            .expect("start provider");

        // Register channel with aggregator
        state.register_agent_channel(agent_id.clone(), event_rx);

        // Verify aggregator has the channel
        assert_eq!(state.agent_channel_count(), 1);

        // Poll should return empty channel (no events yet)
        let poll_result = state.poll_agent_events();
        assert!(poll_result.empty_channels.contains(&agent_id));
    }

    #[test]
    fn multi_agent_event_routing_updates_agent_slot_transcript() {
        use agent_core::provider::ProviderEvent;
        use std::sync::mpsc;

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Setup multi-agent
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");
        let (event_tx, event_rx) = mpsc::channel();
        state.register_agent_channel(agent_id.clone(), event_rx);

        // Simulate provider events
        event_tx
            .send(ProviderEvent::AssistantChunk(
                "Hello from agent".to_string(),
            ))
            .unwrap();
        event_tx
            .send(ProviderEvent::Status("Working".to_string()))
            .unwrap();

        // Poll events
        let poll_result = state.poll_agent_events();
        assert_eq!(poll_result.events.len(), 2);

        // Verify events are tagged with agent_id
        for event in &poll_result.events {
            assert_eq!(event.agent_id(), &agent_id);
        }
    }

    #[test]
    fn focused_unread_mail_count_empty_when_no_agent() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        // No agent pool, no focused agent
        assert_eq!(state.focused_unread_mail_count(), 0);
    }

    #[test]
    fn focused_unread_mail_count_with_mail() {
        use agent_core::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent (creates OVERVIEW at index 0, worker at index 1)
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");

        // Focus the spawned worker agent so mail checks work on it
        state.focus_agent(&agent_id);

        // Send mail to that agent
        let mail = AgentMail::new(
            AgentId::new("sender"),
            MailTarget::Direct(agent_id.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        state.mailbox.send_mail(mail);
        state.mailbox.process_pending();

        assert_eq!(state.focused_unread_mail_count(), 1);
    }

    #[test]
    fn focused_unread_mail_for_prompt_formats_correctly() {
        use agent_core::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent (creates OVERVIEW at index 0, worker at index 1)
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");

        // Focus the spawned worker agent so mail checks work on it
        state.focus_agent(&agent_id);

        // Send mail with action required
        let mail = AgentMail::new(
            AgentId::new("sender"),
            MailTarget::Direct(agent_id.clone()),
            MailSubject::TaskHelpRequest {
                task_id: TaskId::new("task-1"),
            },
            MailBody::Text("Need help".to_string()),
        )
        .with_action_required();
        state.mailbox.send_mail(mail);
        state.mailbox.process_pending();

        let formatted = state.focused_unread_mail_for_prompt();
        assert!(formatted.contains("=== Incoming Messages ==="));
        assert!(formatted.contains("[ACTION REQUIRED]"));
        assert!(formatted.contains("Help requested for task-1"));
        assert!(formatted.contains("=== End Messages ==="));
    }

    #[test]
    fn focused_unread_mail_for_prompt_empty_when_no_mail() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent but no mail
        state.spawn_agent(ProviderKind::Mock);

        let formatted = state.focused_unread_mail_for_prompt();
        assert!(formatted.is_empty());
    }

    #[test]
    fn mark_focused_mail_read_keeps_mail_history() {
        use agent_core::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent (creates OVERVIEW at index 0, worker at index 1)
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");

        // Focus the spawned worker agent so mail checks work on it
        state.focus_agent(&agent_id);

        let mail = AgentMail::new(
            AgentId::new("sender"),
            MailTarget::Direct(agent_id.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        state.mailbox.send_mail(mail);
        state.mailbox.process_pending();

        // Verify unread
        assert_eq!(state.focused_unread_mail_count(), 1);

        // Mark read
        state.mark_focused_mail_read();

        // Verify unread is 0 but mail still exists
        assert_eq!(state.focused_unread_mail_count(), 0);
        let inbox = state.mailbox.inbox_for(&agent_id);
        assert!(inbox.is_some());
        assert_eq!(inbox.unwrap().len(), 1); // Mail preserved
        assert!(inbox.unwrap()[0].is_read());
    }

    #[test]
    fn mail_injection_prepends_to_prompt() {
        use agent_core::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn agent (creates OVERVIEW at index 0, worker at index 1)
        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");

        // Focus the spawned worker agent so mail checks work on it
        state.focus_agent(&agent_id);

        let mail = AgentMail::new(
            AgentId::new("helper"),
            MailTarget::Direct(agent_id.clone()),
            MailSubject::InfoRequest {
                query: "What is the status?".to_string(),
            },
            MailBody::Text("Please respond".to_string()),
        );
        state.mailbox.send_mail(mail);
        state.mailbox.process_pending();

        // Get mail prefix before starting provider
        let mail_prefix = state.focused_unread_mail_for_prompt();
        assert!(!mail_prefix.is_empty());

        // The augmented prompt would be mail_prefix + user_prompt
        let user_prompt = "Write tests for feature X";
        let augmented = if mail_prefix.is_empty() {
            user_prompt.to_string()
        } else {
            format!("{}{}", mail_prefix, user_prompt)
        };

        assert!(augmented.contains("=== Incoming Messages ==="));
        assert!(augmented.contains("Write tests for feature X"));
        assert!(augmented.starts_with("\n=== Incoming Messages ==="));
    }

    #[test]
    fn raw_provider_prompt_preview_does_not_inject_or_consume_mail() {
        use agent_core::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        let agent_id = state.spawn_agent(ProviderKind::Mock).expect("spawn agent");
        state.focus_agent(&agent_id);

        let mail = AgentMail::new(
            AgentId::new("helper"),
            MailTarget::Direct(agent_id.clone()),
            MailSubject::InfoRequest {
                query: "What is the status?".to_string(),
            },
            MailBody::Text("Please respond".to_string()),
        );
        state.mailbox.send_mail(mail);
        state.mailbox.process_pending();
        assert_eq!(state.focused_unread_mail_count(), 1);

        let preview = state
            .build_provider_prompt_for_agent(&agent_id, "/status".to_string(), false)
            .expect("preview");

        assert_eq!(preview, "/status");
        assert_eq!(state.focused_unread_mail_count(), 1);
    }

    #[test]
    fn parse_single_agent() {
        let result = parse_at_command("@alpha hello world");
        assert!(matches!(result, AtCommandResult::Single { agent, message }
            if agent == "alpha" && message == "hello world"));
    }

    #[test]
    fn parse_comma_separated() {
        let result = parse_at_command("@alpha,bravo hello");
        assert!(
            matches!(result, AtCommandResult::Broadcast { agents, message }
            if agents == vec!["alpha", "bravo"] && message == "hello")
        );
    }

    #[test]
    fn parse_normal_input() {
        let result = parse_at_command("hello world");
        assert!(matches!(result, AtCommandResult::Normal(s) if s == "hello world"));
    }

    #[test]
    fn parse_invalid_no_message() {
        let result = parse_at_command("@alpha");
        assert!(matches!(result, AtCommandResult::Invalid { .. }));
    }

    #[test]
    fn parse_invalid_no_agent() {
        let result = parse_at_command("@ hello");
        assert!(matches!(result, AtCommandResult::Invalid { .. }));
    }

    #[test]
    fn parse_agents_with_spaces() {
        // Test: @alpha, bravo hello - space after comma should be trimmed
        let result = parse_at_command("@alpha, bravo hello");
        assert!(
            matches!(result, AtCommandResult::Broadcast { agents, message }
            if agents == vec!["alpha", "bravo"] && message == "hello")
        );
    }

    #[test]
    fn parse_message_with_leading_spaces() {
        let result = parse_at_command("@alpha   hello world");
        assert!(matches!(result, AtCommandResult::Single { agent, message }
            if agent == "alpha" && message == "hello world"));
    }

    #[test]
    fn parse_empty_message_after_trim() {
        let result = parse_at_command("@alpha   ");
        assert!(matches!(result, AtCommandResult::Invalid { .. }));
    }

    #[test]
    fn parse_input_with_leading_spaces() {
        let result = parse_at_command("  hello world");
        assert!(matches!(result, AtCommandResult::Normal(s) if s == "  hello world"));
    }
}
