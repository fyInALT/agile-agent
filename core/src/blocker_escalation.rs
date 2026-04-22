//! Blocker Escalation Flow
//!
//! Provides blocker detection and escalation for Scrum-style coordination.
//! Helps ScrumMaster role agents identify and resolve blockers.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::agent_mail::{AgentMail, MailBody, MailId, MailSubject, MailTarget};
use crate::pool::AgentStatusSnapshot;
use crate::agent_role::AgentRole;
use crate::agent_runtime::AgentId;
use crate::agent_slot::TaskId;
use crate::backlog::{BacklogState, TaskStatus};

/// Blocker escalation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerEscalation {
    /// Unique escalation ID
    pub escalation_id: String,
    /// Task that is blocked
    pub task_id: String,
    /// Agent who reported the blocker
    pub blocked_agent_id: AgentId,
    /// Blocker reason/description
    pub reason: String,
    /// When the blocker was detected
    pub detected_at: DateTime<Utc>,
    /// When the blocker was escalated
    pub escalated_at: Option<DateTime<Utc>>,
    /// When the blocker was resolved
    pub resolved_at: Option<DateTime<Utc>>,
    /// Agent who resolved the blocker (ScrumMaster)
    pub resolved_by: Option<AgentId>,
    /// Escalation status
    pub status: EscalationStatus,
    /// Resolution time in minutes (if resolved)
    pub resolution_time_minutes: Option<u64>,
    /// Mail ID used for escalation
    pub mail_id: Option<MailId>,
}

/// Status of an escalation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EscalationStatus {
    /// Blocker detected but not yet escalated
    #[default]
    Detected,
    /// Blocker escalated to ScrumMaster
    Escalated,
    /// ScrumMaster acknowledged the blocker
    Acknowledged,
    /// Blocker resolved
    Resolved,
    /// Escalation cancelled (blocker no longer relevant)
    Cancelled,
}


impl BlockerEscalation {
    /// Create a new blocker escalation record
    pub fn new(task_id: String, blocked_agent_id: AgentId, reason: String) -> Self {
        Self {
            escalation_id: format!("escalation-{}", Utc::now().timestamp_millis()),
            task_id,
            blocked_agent_id,
            reason,
            detected_at: Utc::now(),
            escalated_at: None,
            resolved_at: None,
            resolved_by: None,
            status: EscalationStatus::default(),
            resolution_time_minutes: None,
            mail_id: None,
        }
    }

    /// Escalate this blocker (send to ScrumMaster)
    pub fn escalate(&mut self, mail_id: MailId) {
        self.escalated_at = Some(Utc::now());
        self.mail_id = Some(mail_id);
        self.status = EscalationStatus::Escalated;
    }

    /// Mark escalation as acknowledged by ScrumMaster
    pub fn acknowledge(&mut self) {
        self.status = EscalationStatus::Acknowledged;
    }

    /// Mark escalation as resolved
    pub fn resolve(&mut self, resolved_by: AgentId) {
        self.resolved_at = Some(Utc::now());
        self.resolved_by = Some(resolved_by);
        self.status = EscalationStatus::Resolved;

        // Calculate resolution time from escalation if available, otherwise from detection
        let start_time = self.escalated_at.unwrap_or(self.detected_at);
        let duration = self.resolved_at.unwrap() - start_time;
        self.resolution_time_minutes = Some(duration.num_minutes() as u64);
    }

    /// Cancel this escalation
    pub fn cancel(&mut self) {
        self.status = EscalationStatus::Cancelled;
    }

    /// Check if escalation is active (not resolved or cancelled)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            EscalationStatus::Detected
                | EscalationStatus::Escalated
                | EscalationStatus::Acknowledged
        )
    }

    /// Check if escalation is resolved
    pub fn is_resolved(&self) -> bool {
        self.status == EscalationStatus::Resolved
    }

    /// Get time since detection (in minutes)
    pub fn minutes_since_detection(&self) -> i64 {
        let now = Utc::now();
        (now - self.detected_at).num_minutes()
    }

    /// Get summary for display
    pub fn summary(&self) -> String {
        format!(
            "Blocker: {} - {} (status: {:?}, {} min)",
            self.task_id,
            self.reason,
            self.status,
            self.minutes_since_detection()
        )
    }
}

/// Blocker escalation tracker for workplace
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockerEscalationTracker {
    /// All escalations (active and resolved)
    escalations: Vec<BlockerEscalation>,
    /// Threshold in minutes before auto-escalation
    auto_escalation_threshold: u64,
}

impl BlockerEscalationTracker {
    /// Create a new escalation tracker
    pub fn new() -> Self {
        Self {
            escalations: Vec::new(),
            auto_escalation_threshold: 30, // 30 minutes default
        }
    }

    /// Set the auto-escalation threshold
    pub fn with_threshold(mut self, threshold_minutes: u64) -> Self {
        self.auto_escalation_threshold = threshold_minutes;
        self
    }

    /// Detect blocked agents from status snapshots
    pub fn detect_blocked_agents(
        &mut self,
        statuses: &[AgentStatusSnapshot],
        backlog: &BacklogState,
    ) -> Vec<BlockerEscalation> {
        let mut new_escalations = Vec::new();

        for status in statuses {
            // Check if agent has an assigned task that is blocked
            if let Some(task_id) = &status.assigned_task_id
                && let Some(task) = backlog.find_task(task_id.as_str())
                    && task.status == TaskStatus::Blocked {
                        // Check if we already have an escalation for this task
                        if !self
                            .escalations
                            .iter()
                            .any(|e| e.task_id == task_id.as_str() && e.is_active())
                        {
                            // Create new escalation
                            let reason = task
                                .result_summary
                                .clone()
                                .unwrap_or_else(|| "Unknown blocker".to_string());

                            let escalation = BlockerEscalation::new(
                                task_id.as_str().to_string(),
                                status.agent_id.clone(),
                                reason,
                            );

                            new_escalations.push(escalation.clone());
                            self.escalations.push(escalation);
                        }
                    }
        }

        new_escalations
    }

    /// Check for escalations that need auto-escalation
    pub fn check_auto_escalation(&mut self) -> Vec<&mut BlockerEscalation> {
        self.escalations
            .iter_mut()
            .filter(|e| {
                e.status == EscalationStatus::Detected
                    && e.minutes_since_detection() as u64 >= self.auto_escalation_threshold
            })
            .collect()
    }

    /// Get all active escalations
    pub fn active_escalations(&self) -> Vec<&BlockerEscalation> {
        self.escalations.iter().filter(|e| e.is_active()).collect()
    }

    /// Get all resolved escalations
    pub fn resolved_escalations(&self) -> Vec<&BlockerEscalation> {
        self.escalations
            .iter()
            .filter(|e| e.is_resolved())
            .collect()
    }

    /// Get escalation by task ID
    pub fn get_escalation_for_task(&self, task_id: &str) -> Option<&BlockerEscalation> {
        self.escalations
            .iter()
            .find(|e| e.task_id == task_id && e.is_active())
    }

    /// Mark escalation as resolved by task ID
    pub fn resolve_escalation(&mut self, task_id: &str, resolved_by: AgentId) -> bool {
        if let Some(escalation) = self
            .escalations
            .iter_mut()
            .find(|e| e.task_id == task_id && e.is_active())
        {
            escalation.resolve(resolved_by);
            true
        } else {
            false
        }
    }

    /// Calculate average resolution time (in minutes)
    pub fn average_resolution_time(&self) -> Option<u64> {
        let resolved: Vec<_> = self
            .escalations
            .iter()
            .filter(|e| e.resolution_time_minutes.is_some())
            .collect();

        if resolved.is_empty() {
            return None;
        }

        let total = resolved
            .iter()
            .map(|e| e.resolution_time_minutes.unwrap())
            .sum::<u64>();

        Some(total / resolved.len() as u64)
    }

    /// Get escalation statistics
    pub fn statistics(&self) -> EscalationStats {
        EscalationStats {
            total_escalations: self.escalations.len(),
            active_count: self.active_escalations().len(),
            resolved_count: self.resolved_escalations().len(),
            average_resolution_minutes: self.average_resolution_time(),
        }
    }
}

/// Statistics for escalation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct EscalationStats {
    /// Total number of escalations
    pub total_escalations: usize,
    /// Currently active escalations
    pub active_count: usize,
    /// Resolved escalations
    pub resolved_count: usize,
    /// Average resolution time in minutes
    pub average_resolution_minutes: Option<u64>,
}


/// Blocker escalation helper functions
pub struct BlockerHelper;

impl BlockerHelper {
    /// Create escalation mail for ScrumMaster
    pub fn create_escalation_mail(
        escalation: &BlockerEscalation,
        scrum_master_id: &AgentId,
    ) -> AgentMail {
        AgentMail::new(
            escalation.blocked_agent_id.clone(),
            MailTarget::Direct(scrum_master_id.clone()),
            MailSubject::TaskBlocked {
                task_id: TaskId::new(&escalation.task_id),
                reason: escalation.reason.clone(),
            },
            MailBody::TaskContext {
                summary: escalation.summary(),
                details: format!(
                    "Detected at: {}\nTask: {}\nAgent: {}\n",
                    escalation.detected_at.to_rfc3339(),
                    escalation.task_id,
                    escalation.blocked_agent_id.as_str()
                ),
            },
        )
        .with_action_required()
        .with_deadline(
            // Set deadline as 1 hour from now
            (Utc::now() + Duration::hours(1)).to_rfc3339(),
        )
    }

    /// Find ScrumMaster agent from pool status
    pub fn find_scrum_master(statuses: &[AgentStatusSnapshot]) -> Option<&AgentStatusSnapshot> {
        statuses.iter().find(|s| s.role == AgentRole::ScrumMaster)
    }

    /// Check if agent is blocked based on status
    pub fn is_agent_blocked(status: &AgentStatusSnapshot, backlog: &BacklogState) -> bool {
        status
            .assigned_task_id
            .as_ref()
            .and_then(|tid| backlog.find_task(tid.as_str()))
            .map(|t| t.status == TaskStatus::Blocked)
            .unwrap_or(false)
    }

    /// Generate blockers summary for standup
    pub fn blockers_summary(tracker: &BlockerEscalationTracker) -> String {
        let active = tracker.active_escalations();
        if active.is_empty() {
            return "No active blockers".to_string();
        }

        let mut summary = String::from("Active Blockers:\n");
        for escalation in active {
            summary.push_str(&format!(
                "- {} (agent: {}, {} min)\n",
                escalation.task_id,
                escalation.blocked_agent_id.as_str(),
                escalation.minutes_since_detection()
            ));
        }

        let stats = tracker.statistics();
        summary.push_str(&format!(
            "\nStats: {} total, {} resolved, avg {} min resolution\n",
            stats.total_escalations,
            stats.resolved_count,
            stats.average_resolution_minutes.unwrap_or(0)
        ));

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::TaskItem;

    fn make_test_task(id: &str, status: TaskStatus) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            todo_id: "todo-1".to_string(),
            objective: "Test task".to_string(),
            scope: "test".to_string(),
            constraints: vec![],
            verification_plan: vec![],
            status,
            result_summary: if status == TaskStatus::Blocked {
                Some("Blocked reason".to_string())
            } else {
                None
            },
        }
    }

    fn make_test_agent_status(
        id: &str,
        role: AgentRole,
        task_id: Option<&str>,
    ) -> AgentStatusSnapshot {
        use crate::agent_runtime::{AgentCodename, ProviderType};
        use crate::agent_slot::{AgentSlotStatus, TaskId};

        AgentStatusSnapshot {
            agent_id: AgentId::new(id.to_string()),
            codename: AgentCodename::new(format!("codename-{}", id)),
            provider_type: ProviderType::Claude,
            role,
            status: AgentSlotStatus::idle(),
            assigned_task_id: task_id.map(|t| TaskId::new(t)),
            worktree_branch: None,
            has_worktree: false,
            worktree_exists: false,
        }
    }

    #[test]
    fn escalation_new() {
        let escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        assert_eq!(escalation.status, EscalationStatus::Detected);
        assert!(escalation.escalated_at.is_none());
        assert!(escalation.is_active());
        assert!(!escalation.is_resolved());
    }

    #[test]
    fn escalation_escalate() {
        let mut escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        let mail_id = MailId::new();
        escalation.escalate(mail_id.clone());

        assert_eq!(escalation.status, EscalationStatus::Escalated);
        assert!(escalation.escalated_at.is_some());
        assert!(escalation.mail_id.is_some());
        assert!(escalation.is_active());
    }

    #[test]
    fn escalation_resolve() {
        let mut escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        escalation.escalate(MailId::new());
        escalation.resolve(AgentId::new("scrum-master".to_string()));

        assert_eq!(escalation.status, EscalationStatus::Resolved);
        assert!(escalation.resolved_at.is_some());
        assert!(escalation.resolved_by.is_some());
        assert!(escalation.resolution_time_minutes.is_some());
        assert!(!escalation.is_active());
        assert!(escalation.is_resolved());
    }

    #[test]
    fn escalation_resolve_without_escalation() {
        // Test resolving directly from Detected status (without escalate)
        let mut escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        // Resolve directly without escalating first
        escalation.resolve(AgentId::new("scrum-master".to_string()));

        assert_eq!(escalation.status, EscalationStatus::Resolved);
        assert!(escalation.resolved_at.is_some());
        assert!(escalation.escalated_at.is_none());
        // Resolution time should still be calculated from detected_at
        assert!(escalation.resolution_time_minutes.is_some());
        assert!(escalation.is_resolved());
    }

    #[test]
    fn escalation_cancel() {
        let mut escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        escalation.cancel();

        assert_eq!(escalation.status, EscalationStatus::Cancelled);
        assert!(!escalation.is_active());
    }

    #[test]
    fn escalation_minutes_since_detection() {
        let escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        // Should be 0 or very small since just created
        assert!(escalation.minutes_since_detection() >= 0);
        assert!(escalation.minutes_since_detection() < 1);
    }

    #[test]
    fn escalation_tracker_new() {
        let tracker = BlockerEscalationTracker::new();
        assert!(tracker.escalations.is_empty());
        assert_eq!(tracker.auto_escalation_threshold, 30);
    }

    #[test]
    fn escalation_tracker_with_threshold() {
        let tracker = BlockerEscalationTracker::new().with_threshold(60);
        assert_eq!(tracker.auto_escalation_threshold, 60);
    }

    #[test]
    fn escalation_tracker_detect_blocked() {
        let mut tracker = BlockerEscalationTracker::new();
        let mut backlog = BacklogState::default();

        backlog.push_task(make_test_task("task-001", TaskStatus::Blocked));
        backlog.push_task(make_test_task("task-002", TaskStatus::Running));

        let statuses = vec![
            make_test_agent_status("agent-1", AgentRole::Developer, Some("task-001")),
            make_test_agent_status("agent-2", AgentRole::Developer, Some("task-002")),
        ];

        let new_escalations = tracker.detect_blocked_agents(&statuses, &backlog);

        assert_eq!(new_escalations.len(), 1);
        assert_eq!(new_escalations[0].task_id, "task-001");
        assert_eq!(tracker.escalations.len(), 1);
    }

    #[test]
    fn escalation_tracker_no_duplicate() {
        let mut tracker = BlockerEscalationTracker::new();
        let mut backlog = BacklogState::default();

        backlog.push_task(make_test_task("task-001", TaskStatus::Blocked));

        let statuses = vec![make_test_agent_status(
            "agent-1",
            AgentRole::Developer,
            Some("task-001"),
        )];

        // First detection
        tracker.detect_blocked_agents(&statuses, &backlog);
        assert_eq!(tracker.escalations.len(), 1);

        // Second detection should not create duplicate
        tracker.detect_blocked_agents(&statuses, &backlog);
        assert_eq!(tracker.escalations.len(), 1);
    }

    #[test]
    fn escalation_tracker_resolve() {
        let mut tracker = BlockerEscalationTracker::new();
        let mut backlog = BacklogState::default();

        backlog.push_task(make_test_task("task-001", TaskStatus::Blocked));

        let statuses = vec![make_test_agent_status(
            "agent-1",
            AgentRole::Developer,
            Some("task-001"),
        )];

        tracker.detect_blocked_agents(&statuses, &backlog);

        // Resolve the escalation
        let resolved =
            tracker.resolve_escalation("task-001", AgentId::new("scrum-master".to_string()));

        assert!(resolved);
        assert_eq!(tracker.active_escalations().len(), 0);
        assert_eq!(tracker.resolved_escalations().len(), 1);
    }

    #[test]
    fn escalation_tracker_statistics() {
        let mut tracker = BlockerEscalationTracker::new();
        let mut backlog = BacklogState::default();

        backlog.push_task(make_test_task("task-001", TaskStatus::Blocked));
        backlog.push_task(make_test_task("task-002", TaskStatus::Blocked));

        let statuses = vec![
            make_test_agent_status("agent-1", AgentRole::Developer, Some("task-001")),
            make_test_agent_status("agent-2", AgentRole::Developer, Some("task-002")),
        ];

        tracker.detect_blocked_agents(&statuses, &backlog);
        tracker.resolve_escalation("task-001", AgentId::new("sm".to_string()));

        let stats = tracker.statistics();
        assert_eq!(stats.total_escalations, 2);
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.resolved_count, 1);
    }

    #[test]
    fn blocker_helper_create_mail() {
        let escalation = BlockerEscalation::new(
            "task-001".to_string(),
            AgentId::new("agent-1".to_string()),
            "Waiting on dependency".to_string(),
        );

        let mail = BlockerHelper::create_escalation_mail(
            &escalation,
            &AgentId::new("scrum-master".to_string()),
        );

        assert!(mail.requires_action);
        assert!(mail.deadline.is_some());
    }

    #[test]
    fn blocker_helper_find_scrum_master() {
        let statuses = vec![
            make_test_agent_status("agent-1", AgentRole::Developer, None),
            make_test_agent_status("agent-2", AgentRole::ScrumMaster, None),
            make_test_agent_status("agent-3", AgentRole::ProductOwner, None),
        ];

        let sm = BlockerHelper::find_scrum_master(&statuses);
        assert!(sm.is_some());
        assert_eq!(sm.unwrap().role, AgentRole::ScrumMaster);
    }

    #[test]
    fn blocker_helper_is_agent_blocked() {
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task("task-001", TaskStatus::Blocked));
        backlog.push_task(make_test_task("task-002", TaskStatus::Running));

        let blocked_status =
            make_test_agent_status("agent-1", AgentRole::Developer, Some("task-001"));
        let running_status =
            make_test_agent_status("agent-2", AgentRole::Developer, Some("task-002"));

        assert!(BlockerHelper::is_agent_blocked(&blocked_status, &backlog));
        assert!(!BlockerHelper::is_agent_blocked(&running_status, &backlog));
    }

    #[test]
    fn escalation_status_serialization() {
        let status = EscalationStatus::Escalated;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"escalated\"");
        let parsed: EscalationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, EscalationStatus::Escalated);
    }

    #[test]
    fn escalation_stats_default() {
        let stats = EscalationStats::default();
        assert_eq!(stats.total_escalations, 0);
        assert_eq!(stats.active_count, 0);
    }
}
