//! Shutdown Snapshot Types
//!
//! Captures complete state at shutdown for graceful restore.

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_mail::AgentMail;
use crate::agent_runtime::AgentMeta;
use crate::backlog::BacklogState;

/// Reason for shutdown
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    /// User requested quit
    UserQuit,
    /// System signal received (SIGTERM, etc.)
    SystemSignal,
    /// Provider timeout exceeded
    ProviderTimeout,
    /// Critical error forced shutdown
    CriticalError { error: String },
    /// Clean exit (all work completed)
    CleanExit,
    /// Interrupted (crash, force kill, etc.)
    Interrupted,
}

/// Snapshot of agent state at shutdown
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentShutdownSnapshot {
    /// Agent metadata
    pub meta: AgentMeta,
    /// Assigned task ID at shutdown
    pub assigned_task_id: Option<String>,
    /// Whether agent was active (running provider)
    pub was_active: bool,
    /// Whether agent had error before shutdown
    pub had_error: bool,
    /// Provider thread state (if running)
    pub provider_thread_state: Option<ProviderThreadSnapshot>,
    /// Timestamp when snapshot was captured
    pub captured_at: String,
}

/// Snapshot of provider thread state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderThreadSnapshot {
    /// Current prompt being processed
    pub current_prompt: Option<String>,
    /// Whether waiting for response
    pub waiting_for_response: bool,
    /// Last event processed
    pub last_event_kind: String,
    /// Thread started at
    pub started_at: String,
}

/// Complete shutdown snapshot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownSnapshot {
    /// Timestamp when shutdown occurred
    pub shutdown_at: String,
    /// Workplace ID
    pub workplace_id: String,
    /// All agent snapshots
    pub agents: Vec<AgentShutdownSnapshot>,
    /// Backlog state
    pub backlog: BacklogState,
    /// Pending cross-agent messages (unprocessed)
    pub pending_mail: Vec<AgentMail>,
    /// Reason for shutdown
    pub shutdown_reason: ShutdownReason,
}

impl ShutdownSnapshot {
    /// Create a new shutdown snapshot
    pub fn new(
        workplace_id: String,
        agents: Vec<AgentShutdownSnapshot>,
        backlog: BacklogState,
        pending_mail: Vec<AgentMail>,
        reason: ShutdownReason,
    ) -> Self {
        Self {
            shutdown_at: Utc::now().to_rfc3339(),
            workplace_id,
            agents,
            backlog,
            pending_mail,
            shutdown_reason: reason,
        }
    }

    /// Create snapshot for interrupted shutdown
    pub fn interrupted(
        workplace_id: String,
        agents: Vec<AgentShutdownSnapshot>,
        backlog: BacklogState,
        pending_mail: Vec<AgentMail>,
    ) -> Self {
        Self::new(workplace_id, agents, backlog, pending_mail, ShutdownReason::Interrupted)
    }

    /// Check if any agents were active at shutdown
    pub fn has_active_agents(&self) -> bool {
        self.agents.iter().any(|a| a.was_active)
    }

    /// Get count of agents that need resume
    pub fn resume_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.was_active || a.assigned_task_id.is_some())
            .count()
    }

    /// Get count of pending mail
    pub fn pending_mail_count(&self) -> usize {
        self.pending_mail.len()
    }
}

impl AgentShutdownSnapshot {
    /// Create snapshot for idle agent
    pub fn idle(meta: AgentMeta) -> Self {
        Self {
            meta,
            assigned_task_id: None,
            was_active: false,
            had_error: false,
            provider_thread_state: None,
            captured_at: Utc::now().to_rfc3339(),
        }
    }

    /// Create snapshot for active agent
    pub fn active(meta: AgentMeta, assigned_task_id: Option<String>, thread_state: ProviderThreadSnapshot) -> Self {
        Self {
            meta,
            assigned_task_id,
            was_active: true,
            had_error: false,
            provider_thread_state: Some(thread_state),
            captured_at: Utc::now().to_rfc3339(),
        }
    }

    /// Create snapshot for agent with error
    pub fn error(meta: AgentMeta, assigned_task_id: Option<String>) -> Self {
        Self {
            meta,
            assigned_task_id,
            was_active: false,
            had_error: true,
            provider_thread_state: None,
            captured_at: Utc::now().to_rfc3339(),
        }
    }

    /// Check if agent needs to resume work
    pub fn needs_resume(&self) -> bool {
        self.was_active || self.assigned_task_id.is_some()
    }
}

impl ProviderThreadSnapshot {
    /// Create snapshot for thread waiting for response
    pub fn waiting_for_response(current_prompt: Option<String>, started_at: String) -> Self {
        Self {
            current_prompt,
            waiting_for_response: true,
            last_event_kind: "waiting".to_string(),
            started_at,
        }
    }

    /// Create snapshot for thread processing events
    pub fn processing(last_event_kind: String, started_at: String) -> Self {
        Self {
            current_prompt: None,
            waiting_for_response: false,
            last_event_kind,
            started_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentShutdownSnapshot;
    use super::ShutdownReason;
    use super::ShutdownSnapshot;
    use crate::agent_runtime::AgentCodename;
    use crate::agent_runtime::AgentId;
    use crate::agent_runtime::AgentMeta;
    use crate::agent_runtime::AgentStatus;
    use crate::agent_runtime::ProviderType;
    use crate::agent_runtime::WorkplaceId;
    use crate::backlog::BacklogState;

    fn test_meta(id: &str) -> AgentMeta {
        AgentMeta {
            agent_id: AgentId::new(id),
            codename: AgentCodename::new("alpha"),
            workplace_id: WorkplaceId::new("wp_test"),
            provider_type: ProviderType::Mock,
            provider_session_id: None,
            created_at: "2026-04-14T00:00:00Z".to_string(),
            updated_at: "2026-04-14T00:00:00Z".to_string(),
            status: AgentStatus::Idle,
        }
    }

    #[test]
    fn shutdown_snapshot_counts_active_agents() {
        let idle = AgentShutdownSnapshot::idle(test_meta("agent_001"));
        let active = AgentShutdownSnapshot::active(
            test_meta("agent_002"),
            Some("task-1".to_string()),
            super::ProviderThreadSnapshot::waiting_for_response(Some("prompt".to_string()), "2026-04-14T00:00:00Z".to_string()),
        );

        let snapshot = ShutdownSnapshot::new(
            "wp_test".to_string(),
            vec![idle, active],
            BacklogState::default(),
            vec![], // no pending mail
            ShutdownReason::UserQuit,
        );

        assert!(snapshot.has_active_agents());
        assert_eq!(snapshot.resume_count(), 1);
        assert_eq!(snapshot.pending_mail_count(), 0);
    }

    #[test]
    fn agent_snapshot_needs_resume_when_active() {
        let active = AgentShutdownSnapshot::active(
            test_meta("agent_001"),
            None,
            super::ProviderThreadSnapshot::waiting_for_response(None, "2026-04-14T00:00:00Z".to_string()),
        );

        assert!(active.needs_resume());
    }

    #[test]
    fn agent_snapshot_needs_resume_when_has_task() {
        let with_task = AgentShutdownSnapshot {
            meta: test_meta("agent_001"),
            assigned_task_id: Some("task-1".to_string()),
            was_active: false,
            had_error: false,
            provider_thread_state: None,
            captured_at: "2026-04-14T00:00:00Z".to_string(),
        };

        assert!(with_task.needs_resume());
    }

    #[test]
    fn interrupted_snapshot_has_correct_reason() {
        let snapshot = ShutdownSnapshot::interrupted(
            "wp_test".to_string(),
            vec![AgentShutdownSnapshot::idle(test_meta("agent_001"))],
            BacklogState::default(),
            vec![], // no pending mail
        );

        assert_eq!(snapshot.shutdown_reason, ShutdownReason::Interrupted);
    }

    #[test]
    fn shutdown_snapshot_with_pending_mail() {
        use crate::agent_mail::{AgentMail, MailBody, MailSubject, MailTarget};

        let mail = AgentMail::new(
            AgentId::new("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom { label: "Announcement".to_string() },
            MailBody::Text("Hello all".to_string()),
        );

        let snapshot = ShutdownSnapshot::interrupted(
            "wp_test".to_string(),
            vec![AgentShutdownSnapshot::idle(test_meta("agent_001"))],
            BacklogState::default(),
            vec![mail],
        );

        assert_eq!(snapshot.pending_mail_count(), 1);
    }
}