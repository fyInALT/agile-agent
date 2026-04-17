//! TUI Shutdown Snapshot
//!
//! Captures TUI-specific state for graceful restore across sessions.

use std::fs;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use agent_core::agent_mail::AgentMailbox;
use agent_core::agent_role::AgentRole;
use agent_core::agent_runtime::{AgentCodename, AgentId, ProviderType};
use agent_core::agent_slot::{AgentSlotStatus, TaskId};
use agent_core::app::TranscriptEntry;
use agent_core::backlog::BacklogState;
use agent_core::launch_config::AgentLaunchBundle;
use agent_core::provider::{ProviderKind, SessionHandle};
use agent_core::shutdown_snapshot::ShutdownReason;
use agent_core::workplace_store::WorkplaceStore;

use crate::overview_state::OverviewFilter;
use crate::view_mode::{ComposeField, ViewMode};

/// Snapshot of TUI state at shutdown
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuiShutdownSnapshot {
    /// View mode at shutdown
    pub view_mode: ViewMode,
    /// Split view state (if in split mode)
    pub split_state: Option<SplitViewSnapshot>,
    /// Dashboard state (if in dashboard mode)
    pub dashboard_state: Option<DashboardViewSnapshot>,
    /// Mail state (if in mail mode)
    pub mail_state: Option<MailViewSnapshot>,
    /// Overview state (if in overview mode)
    pub overview_state: Option<OverviewViewSnapshot>,
    /// Timestamp when snapshot was captured
    pub captured_at: String,
}

/// Complete TUI resume snapshot for restoring a multi-agent TUI session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuiResumeSnapshot {
    /// Snapshot version for migration support (V2 adds launch_bundle).
    #[serde(default = "default_snapshot_version")]
    pub version: String,
    /// Timestamp when the snapshot was captured.
    pub captured_at: String,
    /// Why the session was shut down.
    pub shutdown_reason: ShutdownReason,
    /// Current composer contents.
    pub composer_text: String,
    /// Selected provider for new work.
    pub selected_provider: ProviderKind,
    /// Shared backlog state.
    pub backlog: BacklogState,
    /// Full mailbox state.
    pub mailbox: AgentMailbox,
    /// All agents visible in the TUI.
    pub agents: Vec<PersistedAgentSnapshot>,
    /// Focused agent at shutdown.
    pub focused_agent_id: Option<AgentId>,
    /// TUI view state.
    pub view_state: TuiShutdownSnapshot,
}

fn default_snapshot_version() -> String {
    "v3".to_string()
}

/// Persisted view of one TUI agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedAgentSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
    pub status: PersistedAgentStatus,
    pub provider_session_id: Option<String>,
    pub transcript: Vec<TranscriptEntry>,
    pub assigned_task_id: Option<TaskId>,
    /// Launch configuration bundle for this agent (added in V2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_bundle: Option<AgentLaunchBundle>,
    /// Worktree path for isolated agent workspace (added in V3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<std::path::PathBuf>,
    /// Worktree branch name (added in V3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
    /// Worktree ID (added in V3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_id: Option<String>,
}

/// Serializable snapshot of a live agent status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistedAgentStatus {
    Idle,
    Active,
    Blocked { reason: String },
    Paused { reason: String },
    Stopped { reason: String },
    Error { message: String },
    WaitingForInput,
}

impl TuiShutdownSnapshot {
    /// Create a new TUI shutdown snapshot
    pub fn new(
        view_mode: ViewMode,
        split_state: Option<SplitViewSnapshot>,
        dashboard_state: Option<DashboardViewSnapshot>,
        mail_state: Option<MailViewSnapshot>,
        overview_state: Option<OverviewViewSnapshot>,
    ) -> Self {
        use chrono::Utc;
        Self {
            view_mode,
            split_state,
            dashboard_state,
            mail_state,
            overview_state,
            captured_at: Utc::now().to_rfc3339(),
        }
    }

    /// Create snapshot from current TuiViewState
    pub fn from_view_state(view_state: &crate::view_mode::TuiViewState) -> Self {
        let split_state = Some(SplitViewSnapshot {
            left_agent_index: view_state.split.left_agent_index,
            right_agent_index: view_state.split.right_agent_index,
            focused_side: view_state.split.focused_side,
            split_ratio: view_state.split.split_ratio,
        });

        let dashboard_state = Some(DashboardViewSnapshot {
            selected_card_index: view_state.dashboard.selected_card_index,
            scroll_offset: view_state.dashboard.scroll_offset,
        });

        let mail_state = Some(MailViewSnapshot {
            selected_mail_index: view_state.mail.selected_mail_index,
            compose_field: view_state.mail.compose_field,
        });

        let overview_state = Some(OverviewViewSnapshot {
            filter: view_state.overview.filter,
            page_offset: view_state.overview.page_offset,
            agent_list_rows: view_state.overview.agent_list_rows,
        });

        Self::new(
            view_state.mode,
            split_state,
            dashboard_state,
            mail_state,
            overview_state,
        )
    }

    /// Apply snapshot to TuiViewState
    pub fn apply_to(&self, view_state: &mut crate::view_mode::TuiViewState) {
        view_state.mode = self.view_mode;

        if let Some(split) = &self.split_state {
            view_state.split.left_agent_index = split.left_agent_index;
            view_state.split.right_agent_index = split.right_agent_index;
            view_state.split.focused_side = split.focused_side;
            view_state.split.split_ratio = split.split_ratio;
        }

        if let Some(dashboard) = &self.dashboard_state {
            view_state.dashboard.selected_card_index = dashboard.selected_card_index;
            view_state.dashboard.scroll_offset = dashboard.scroll_offset;
        }

        if let Some(mail) = &self.mail_state {
            view_state.mail.selected_mail_index = mail.selected_mail_index;
            view_state.mail.compose_field = mail.compose_field;
        }

        if let Some(overview) = &self.overview_state {
            view_state.overview.filter = overview.filter;
            view_state.overview.page_offset = overview.page_offset;
            view_state.overview.agent_list_rows = overview.agent_list_rows;
        }
    }
}

impl TuiResumeSnapshot {
    pub fn from_state(state: &crate::ui_state::TuiState, reason: ShutdownReason) -> Self {
        let agents = state
            .agent_pool
            .as_ref()
            .map(|pool| {
                pool.slots()
                    .iter()
                    .map(PersistedAgentSnapshot::from_slot)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            version: default_snapshot_version(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            shutdown_reason: reason,
            composer_text: state.composer.text().to_string(),
            selected_provider: state.app().selected_provider,
            backlog: state.workplace().backlog.clone(),
            mailbox: state.mailbox.clone(),
            agents,
            focused_agent_id: state.focused_agent_id(),
            view_state: TuiShutdownSnapshot::from_view_state(&state.view_state),
        }
    }
}

impl PersistedAgentSnapshot {
    pub fn from_slot(slot: &agent_core::agent_slot::AgentSlot) -> Self {
        Self {
            agent_id: slot.agent_id().clone(),
            codename: slot.codename().clone(),
            provider_type: slot.provider_type(),
            role: slot.role(),
            status: PersistedAgentStatus::from_slot_status(slot.status()),
            provider_session_id: slot.session_handle().map(|handle| match handle {
                SessionHandle::ClaudeSession { session_id } => session_id.clone(),
                SessionHandle::CodexThread { thread_id } => thread_id.clone(),
            }),
            transcript: slot.transcript().to_vec(),
            assigned_task_id: slot.assigned_task_id().cloned(),
            launch_bundle: None,
            worktree_path: slot.worktree_path().cloned(),
            worktree_branch: slot.worktree_branch().cloned(),
            worktree_id: slot.worktree_id().cloned(),
        }
    }

    pub fn restore_status(&self) -> AgentSlotStatus {
        match &self.status {
            PersistedAgentStatus::Idle => AgentSlotStatus::idle(),
            PersistedAgentStatus::Active => AgentSlotStatus::idle(),
            PersistedAgentStatus::Blocked { reason } => AgentSlotStatus::blocked(reason.clone()),
            PersistedAgentStatus::Paused { reason } => AgentSlotStatus::paused(reason.clone()),
            PersistedAgentStatus::Stopped { reason } => AgentSlotStatus::stopped(reason.clone()),
            PersistedAgentStatus::Error { message } => AgentSlotStatus::error(message.clone()),
            PersistedAgentStatus::WaitingForInput => AgentSlotStatus::waiting_for_input(),
        }
    }

    pub fn restore_session_handle(&self) -> Option<SessionHandle> {
        self.provider_session_id
            .as_ref()
            .map(|value| match self.provider_type {
                ProviderType::Codex => SessionHandle::CodexThread {
                    thread_id: value.clone(),
                },
                _ => SessionHandle::ClaudeSession {
                    session_id: value.clone(),
                },
            })
    }
}

impl PersistedAgentStatus {
    fn from_slot_status(status: &AgentSlotStatus) -> Self {
        match status {
            AgentSlotStatus::Idle => Self::Idle,
            AgentSlotStatus::Blocked { reason } => Self::Blocked {
                reason: reason.clone(),
            },
            AgentSlotStatus::Stopped { reason } => Self::Stopped {
                reason: reason.clone(),
            },
            AgentSlotStatus::Error { message } => Self::Error {
                message: message.clone(),
            },
            AgentSlotStatus::BlockedForDecision { blocked_state } => Self::Blocked {
                reason: blocked_state.reason().description(),
            },
            AgentSlotStatus::Paused { reason } => Self::Paused {
                reason: reason.clone(),
            },
            AgentSlotStatus::WaitingForInput { .. } => Self::WaitingForInput,
            AgentSlotStatus::Starting
            | AgentSlotStatus::Responding { .. }
            | AgentSlotStatus::ToolExecuting { .. }
            | AgentSlotStatus::Finishing
            | AgentSlotStatus::Stopping => Self::Active,
        }
    }
}

pub fn resume_snapshot_path(workplace: &WorkplaceStore) -> std::path::PathBuf {
    workplace.path().join("tui_shutdown_snapshot.json")
}

pub fn save_resume_snapshot(
    workplace: &WorkplaceStore,
    snapshot: &TuiResumeSnapshot,
) -> Result<std::path::PathBuf> {
    let path = resume_snapshot_path(workplace);
    let payload = serde_json::to_string_pretty(snapshot)
        .context("failed to serialize TUI resume snapshot")?;
    fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn load_resume_snapshot(workplace: &WorkplaceStore) -> Result<Option<TuiResumeSnapshot>> {
    let path = resume_snapshot_path(workplace);
    if !path.exists() {
        return Ok(None);
    }
    let payload =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let snapshot = deserialize_with_migration(&payload)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(snapshot))
}

/// Deserialize with migration support for older snapshot versions.
///
/// V1 snapshots (before launch_bundle field) are migrated to V2 by adding
/// default launch_bundle: None to all agents.
fn deserialize_with_migration(json: &str) -> Result<TuiResumeSnapshot> {
    // First, parse as generic JSON to check version
    let value: serde_json::Value = serde_json::from_str(json)?;

    // Check if version field exists
    let version = value.get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v1");

    if version == "v1" {
        // Migration needed: add version field and launch_bundle fields
        let mut migrated: TuiResumeSnapshot = serde_json::from_str(json)?;

        // The serde default will already handle launch_bundle being None
        // Just need to ensure version is set
        migrated.version = "v2".to_string();

        // Note: V1 snapshots are migrated to V2 with empty launch bundles
        Ok(migrated)
    } else {
        // Already v2 or later, parse normally
        serde_json::from_str(json).map_err(Into::into)
    }
}

pub fn clear_resume_snapshot(workplace: &WorkplaceStore) -> Result<()> {
    let path = resume_snapshot_path(workplace);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Snapshot of split view state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitViewSnapshot {
    /// Left agent index
    pub left_agent_index: usize,
    /// Right agent index
    pub right_agent_index: usize,
    /// Focused side (0=left, 1=right)
    pub focused_side: usize,
    /// Split ratio (0.0-1.0)
    #[serde(with = "split_ratio_serde")]
    pub split_ratio: f32,
}

/// Snapshot of dashboard view state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardViewSnapshot {
    /// Selected card index
    pub selected_card_index: usize,
    /// Scroll offset
    pub scroll_offset: usize,
}

/// Snapshot of mail view state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailViewSnapshot {
    /// Selected mail index
    pub selected_mail_index: usize,
    /// Compose field focus
    pub compose_field: ComposeField,
}

/// Snapshot of overview view state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewViewSnapshot {
    /// Active overview filter
    pub filter: OverviewFilter,
    /// Current page offset
    pub page_offset: usize,
    /// Configured agent rows
    pub agent_list_rows: usize,
}

/// Serde module for f32 split ratio
mod split_ratio_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &f32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as string to avoid precision issues
        serializer.serialize_str(&format!("{:.4}", value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        s.parse::<f32>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_state::TuiState;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use agent_core::shutdown_snapshot::ShutdownReason;
    use tempfile::TempDir;

    #[test]
    fn tui_snapshot_serializes_view_mode() {
        let snapshot = TuiShutdownSnapshot::new(
            ViewMode::Split,
            Some(SplitViewSnapshot {
                left_agent_index: 0,
                right_agent_index: 1,
                focused_side: 0,
                split_ratio: 0.5,
            }),
            None,
            None,
            None,
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"view_mode\":\"split\""));
        assert!(json.contains("\"left_agent_index\":0"));
    }

    #[test]
    fn tui_snapshot_deserializes() {
        let json = r#"{
            "view_mode": "dashboard",
            "split_state": null,
            "dashboard_state": {"selected_card_index": 2, "scroll_offset": 1},
            "mail_state": null,
            "overview_state": null,
            "captured_at": "2026-04-14T00:00:00Z"
        }"#;

        let snapshot: TuiShutdownSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.view_mode, ViewMode::Dashboard);
        assert!(snapshot.split_state.is_none());
        assert!(snapshot.dashboard_state.is_some());
    }

    #[test]
    fn tui_snapshot_apply_restores_state() {
        use crate::view_mode::TuiViewState;

        let snapshot = TuiShutdownSnapshot::new(
            ViewMode::Mail,
            None,
            None,
            Some(MailViewSnapshot {
                selected_mail_index: 5,
                compose_field: ComposeField::Body,
            }),
            None,
        );

        let mut view_state = TuiViewState::default();
        snapshot.apply_to(&mut view_state);

        assert_eq!(view_state.mode, ViewMode::Mail);
        assert_eq!(view_state.mail.selected_mail_index, 5);
        assert_eq!(view_state.mail.compose_field, ComposeField::Body);
    }

    #[test]
    fn resume_snapshot_round_trip_restores_multi_agent_state() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.ensure_overview_agent();
        let alpha_id = state.spawn_agent(ProviderKind::Mock).expect("spawn alpha");
        state.view_state.mode = ViewMode::Overview;
        state.view_state.overview.filter = crate::overview_state::OverviewFilter::RunningOnly;
        state.focus_agent(&alpha_id);
        if let Some(pool) = state.agent_pool.as_mut()
            && let Some(slot) = pool.get_slot_mut_by_id(&alpha_id)
        {
            slot.append_transcript(agent_core::app::TranscriptEntry::Assistant(
                "restored worker output".to_string(),
            ));
        }

        let snapshot = TuiResumeSnapshot::from_state(&state, ShutdownReason::UserQuit);

        let fresh = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap fresh");
        let mut restored = TuiState::from_session(fresh);
        restored
            .restore_from_resume_snapshot(snapshot)
            .expect("restore snapshot");

        assert_eq!(restored.view_state.mode, ViewMode::Overview);
        assert_eq!(
            restored.view_state.overview.filter,
            crate::overview_state::OverviewFilter::RunningOnly
        );
        assert_eq!(restored.focused_agent_codename(), "alpha");
        let alpha_slot = restored
            .agent_pool
            .as_ref()
            .and_then(|pool| pool.get_slot_by_id(&alpha_id))
            .expect("alpha slot");
        assert!(alpha_slot.transcript().iter().any(|entry| {
            matches!(entry, agent_core::app::TranscriptEntry::Assistant(text) if text == "restored worker output")
        }));
    }

    #[test]
    fn persisted_agent_snapshot_preserves_worktree_info() {
        use agent_core::agent_slot::AgentSlot;
        use agent_core::agent_runtime::{AgentId, AgentCodename, ProviderType};
        use std::path::PathBuf;

        // Create a slot with worktree info
        let mut slot = AgentSlot::new(
            AgentId::new("agent_001"),
            AgentCodename::new("alpha"),
            ProviderType::Claude,
        );
        slot.set_worktree(
            PathBuf::from("/path/to/worktrees/wt-agent_001"),
            Some("agent/agent_001".to_string()),
            "wt-agent_001".to_string(),
        );

        // Convert to snapshot
        let snapshot = PersistedAgentSnapshot::from_slot(&slot);

        // Verify worktree info is captured
        assert_eq!(snapshot.worktree_path, Some(PathBuf::from("/path/to/worktrees/wt-agent_001")));
        assert_eq!(snapshot.worktree_branch, Some("agent/agent_001".to_string()));
        assert_eq!(snapshot.worktree_id, Some("wt-agent_001".to_string()));

        // Verify serialization includes worktree fields
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("worktree_path"));
        assert!(json.contains("worktree_branch"));
        assert!(json.contains("worktree_id"));
    }
}
