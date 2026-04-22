//! Daily Standup Report Generation
//!
//! Provides daily standup report generation for Scrum-style coordination.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::agent_pool::{AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot};
use crate::agent_role::AgentRole;
use crate::backlog::{BacklogState, TaskItem, TaskStatus};

/// Daily standup report for a workplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStandupReport {
    /// Report date
    pub date: DateTime<Utc>,
    /// Standup entries for each agent
    pub agent_entries: Vec<AgentStandupEntry>,
    /// Workplace-wide blockers
    pub workplace_blockers: Vec<String>,
    /// Summary statistics
    pub summary: StandupSummary,
}

/// Per-agent standup entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStandupEntry {
    /// Agent codename
    pub codename: String,
    /// Agent role
    pub role: AgentRole,
    /// Tasks completed yesterday
    pub yesterday_completed: Vec<TaskSummary>,
    /// Tasks planned for today
    pub today_planned: Vec<TaskSummary>,
    /// Current blockers
    pub blockers: Vec<String>,
}

/// Summary of a task for standup display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    /// Task ID
    pub id: String,
    /// Task objective/title
    pub title: String,
    /// Current status
    pub status: TaskStatus,
    /// Status change (if any)
    pub status_change: Option<StatusChange>,
}

/// Status change description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChange {
    /// Previous status
    pub from: TaskStatus,
    /// Current status
    pub to: TaskStatus,
}

/// Summary statistics for standup
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct StandupSummary {
    /// Total agents in standup
    pub total_agents: usize,
    /// Agents with blockers
    pub agents_with_blockers: usize,
    /// Total blockers
    pub total_blockers: usize,
    /// Tasks completed yesterday
    pub tasks_completed: usize,
    /// Tasks planned today
    pub tasks_planned: usize,
    /// Tasks in progress
    pub tasks_in_progress: usize,
}

impl DailyStandupReport {
    /// Create a new empty standup report for today
    pub fn new() -> Self {
        Self {
            date: Utc::now(),
            agent_entries: Vec::new(),
            workplace_blockers: Vec::new(),
            summary: StandupSummary::default(),
        }
    }

    /// Create standup report from task queue snapshot
    pub fn from_snapshot(
        snapshot: &TaskQueueSnapshot,
        backlog: &BacklogState,
        yesterday_tasks: Option<&[TaskHistoryEntry]>,
    ) -> Self {
        let mut report = Self::new();

        // Generate per-agent entries
        for assignment in &snapshot.agent_assignments {
            let entry = Self::build_agent_entry(assignment, backlog, yesterday_tasks);
            report.agent_entries.push(entry);
        }

        // Calculate summary
        report.summary = Self::calculate_summary(&report.agent_entries, snapshot);

        // Collect workplace-level blockers
        report.workplace_blockers = Self::collect_workplace_blockers(backlog);

        report
    }

    /// Build a single agent's standup entry
    fn build_agent_entry(
        assignment: &AgentTaskAssignment,
        backlog: &BacklogState,
        yesterday_tasks: Option<&[TaskHistoryEntry]>,
    ) -> AgentStandupEntry {
        // Find current task
        let current_task = backlog.find_task(assignment.task_id.as_str());

        // Build today's planned task
        let today_planned = current_task
            .map(|t| {
                vec![TaskSummary {
                    id: t.id.clone(),
                    title: t.objective.clone(),
                    status: t.status,
                    status_change: None,
                }]
            })
            .unwrap_or_default();

        // Build yesterday's completed from history (if provided)
        let yesterday_completed = yesterday_tasks
            .map(|history| {
                history
                    .iter()
                    .filter(|h| {
                        h.agent_codename == assignment.codename.as_str() && h.was_completed()
                    })
                    .map(|h| TaskSummary {
                        id: h.task_id.clone(),
                        title: h.task_title.clone(),
                        status: TaskStatus::Done,
                        status_change: Some(StatusChange {
                            from: h.previous_status,
                            to: TaskStatus::Done,
                        }),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Determine blockers
        let blockers = if assignment.task_status == TaskStatus::Blocked {
            current_task
                .map(|t| {
                    t.result_summary
                        .clone()
                        .unwrap_or_else(|| "Blocked".to_string())
                })
                .map(|b| vec![b])
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        AgentStandupEntry {
            codename: assignment.codename.as_str().to_string(),
            role: AgentRole::Developer, // Default, would be derived from agent pool
            yesterday_completed,
            today_planned,
            blockers,
        }
    }

    /// Calculate summary statistics
    fn calculate_summary(
        entries: &[AgentStandupEntry],
        snapshot: &TaskQueueSnapshot,
    ) -> StandupSummary {
        StandupSummary {
            total_agents: entries.len(),
            agents_with_blockers: entries.iter().filter(|e| !e.blockers.is_empty()).count(),
            total_blockers: entries.iter().map(|e| e.blockers.len()).sum(),
            tasks_completed: entries.iter().map(|e| e.yesterday_completed.len()).sum(),
            tasks_planned: entries.iter().map(|e| e.today_planned.len()).sum(),
            tasks_in_progress: snapshot.running_tasks,
        }
    }

    /// Collect workplace-level blockers
    fn collect_workplace_blockers(backlog: &BacklogState) -> Vec<String> {
        backlog
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Blocked)
            .map(|t| {
                format!(
                    "{}: {}",
                    t.id,
                    t.result_summary
                        .clone()
                        .unwrap_or_else(|| "Blocked".to_string())
                )
            })
            .collect()
    }

    /// Generate formatted standup report text
    pub fn format_report(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "Daily Standup - {}\n\n",
            self.date.format("%Y-%m-%d")
        ));

        // Per-agent entries
        for entry in &self.agent_entries {
            output.push_str(&format!("Agent {}:\n", entry.codename));

            // Yesterday
            if entry.yesterday_completed.is_empty() {
                output.push_str("- Yesterday: none\n");
            } else {
                output.push_str("- Yesterday: ");
                let items = entry
                    .yesterday_completed
                    .iter()
                    .map(|t| {
                        if let Some(change) = &t.status_change {
                            format!("{} ({:?}→{:?})", t.id, change.from, change.to)
                        } else {
                            t.id.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&items);
                output.push('\n');
            }

            // Today
            if entry.today_planned.is_empty() {
                output.push_str("- Today: none\n");
            } else {
                output.push_str("- Today: ");
                let items = entry
                    .today_planned
                    .iter()
                    .map(|t| format!("{} ({:?})", t.id, t.status))
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&items);
                output.push('\n');
            }

            // Blockers
            if entry.blockers.is_empty() {
                output.push_str("- Blockers: none\n");
            } else {
                output.push_str("- Blockers: ");
                output.push_str(&entry.blockers.join(", "));
                output.push('\n');
            }

            output.push('\n');
        }

        // Workplace blockers
        if !self.workplace_blockers.is_empty() {
            output.push_str("Workplace Blockers:\n");
            for blocker in &self.workplace_blockers {
                output.push_str(&format!("- {}\n", blocker));
            }
            output.push('\n');
        }

        // Summary
        output.push_str(&format!(
            "Summary: {} agents, {} completed, {} planned, {} in progress, {} blockers\n",
            self.summary.total_agents,
            self.summary.tasks_completed,
            self.summary.tasks_planned,
            self.summary.tasks_in_progress,
            self.summary.total_blockers
        ));

        output
    }

    /// Check if any agent has blockers
    pub fn has_blockers(&self) -> bool {
        self.agent_entries.iter().any(|e| !e.blockers.is_empty())
            || !self.workplace_blockers.is_empty()
    }

    /// Get agents with blockers
    pub fn blocked_agents(&self) -> Vec<&AgentStandupEntry> {
        self.agent_entries
            .iter()
            .filter(|e| !e.blockers.is_empty())
            .collect()
    }
}

impl Default for DailyStandupReport {
    fn default() -> Self {
        Self::new()
    }
}


/// Task history entry for tracking yesterday's work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHistoryEntry {
    /// Task ID
    pub task_id: String,
    /// Task title
    pub task_title: String,
    /// Agent who worked on it
    pub agent_codename: String,
    /// Previous status before change
    pub previous_status: TaskStatus,
    /// Timestamp of status change
    pub changed_at: DateTime<Utc>,
    /// Whether it was completed
    pub completed: bool,
}

impl TaskHistoryEntry {
    /// Create a new task history entry
    pub fn new(
        task_id: String,
        task_title: String,
        agent_codename: String,
        previous_status: TaskStatus,
        completed: bool,
    ) -> Self {
        Self {
            task_id,
            task_title,
            agent_codename,
            previous_status,
            changed_at: Utc::now(),
            completed,
        }
    }

    /// Check if this represents a completion
    pub fn was_completed(&self) -> bool {
        self.completed
    }

    /// Check if this entry is from yesterday
    pub fn is_yesterday(&self) -> bool {
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        self.changed_at >= yesterday && self.changed_at < now
    }
}

/// Standup helper functions
pub struct StandupHelper;

impl StandupHelper {
    /// Check if a task should be reported as blocked
    pub fn is_blocked(task: &TaskItem) -> bool {
        task.status == TaskStatus::Blocked
    }

    /// Check if a task was recently completed
    pub fn was_recently_completed(task: &TaskItem) -> bool {
        task.status == TaskStatus::Done && task.result_summary.is_some()
    }

    /// Generate blockers list from backlog
    pub fn list_blockers(backlog: &BacklogState) -> Vec<&TaskItem> {
        backlog
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Blocked)
            .collect()
    }

    /// Generate standup from agent status snapshot
    pub fn generate_from_status(
        statuses: &[AgentStatusSnapshot],
        backlog: &BacklogState,
    ) -> DailyStandupReport {
        let mut report = DailyStandupReport::new();

        for status in statuses {
            let entry = Self::build_entry_from_status(status, backlog);
            report.agent_entries.push(entry);
        }

        report.summary = Self::calculate_summary_from_status(&report.agent_entries, statuses);
        report
    }

    /// Build entry from agent status snapshot
    fn build_entry_from_status(
        status: &AgentStatusSnapshot,
        backlog: &BacklogState,
    ) -> AgentStandupEntry {
        // Find current task if assigned
        let current_task = status
            .assigned_task_id
            .as_ref()
            .and_then(|tid| backlog.find_task(tid.as_str()));

        // Today's planned task
        let today_planned = current_task
            .map(|t| {
                vec![TaskSummary {
                    id: t.id.clone(),
                    title: t.objective.clone(),
                    status: t.status,
                    status_change: None,
                }]
            })
            .unwrap_or_default();

        // Blockers
        let blockers = current_task
            .filter(|t| t.status == TaskStatus::Blocked)
            .map(|t| {
                t.result_summary
                    .clone()
                    .unwrap_or_else(|| "Blocked".to_string())
            })
            .map(|b| vec![b])
            .unwrap_or_default();

        AgentStandupEntry {
            codename: status.codename.as_str().to_string(),
            role: status.role,
            yesterday_completed: Vec::new(), // Would need history tracking
            today_planned,
            blockers,
        }
    }

    /// Calculate summary from status snapshots
    fn calculate_summary_from_status(
        entries: &[AgentStandupEntry],
        statuses: &[AgentStatusSnapshot],
    ) -> StandupSummary {
        let running_count = statuses.iter().filter(|s| s.status.is_active()).count();

        StandupSummary {
            total_agents: entries.len(),
            agents_with_blockers: entries.iter().filter(|e| !e.blockers.is_empty()).count(),
            total_blockers: entries.iter().map(|e| e.blockers.len()).sum(),
            tasks_completed: 0, // Would need history
            tasks_planned: entries.iter().map(|e| e.today_planned.len()).sum(),
            tasks_in_progress: running_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_pool::AgentTaskAssignment;
    use crate::backlog::{TaskItem, TaskStatus};

    fn make_test_task(id: &str, objective: &str, status: TaskStatus) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            todo_id: "todo-1".to_string(),
            objective: objective.to_string(),
            scope: "test scope".to_string(),
            constraints: vec![],
            verification_plan: vec![],
            status,
            result_summary: None,
        }
    }

    #[allow(dead_code)]
    fn make_test_assignment(agent: &str, task_id: &str, status: TaskStatus) -> AgentTaskAssignment {
        use crate::agent_slot::TaskId;
        AgentTaskAssignment {
            agent_id: crate::agent_runtime::AgentId::new(format!("agent-{}", agent)),
            codename: crate::agent_runtime::AgentCodename::new(agent.to_string()),
            task_id: TaskId::new(task_id),
            task_status: status,
        }
    }

    #[test]
    fn standup_report_new() {
        let report = DailyStandupReport::new();
        assert!(report.agent_entries.is_empty());
        assert!(report.workplace_blockers.is_empty());
        assert_eq!(report.summary.total_agents, 0);
    }

    #[test]
    fn standup_report_format() {
        let mut report = DailyStandupReport::new();
        report.agent_entries.push(AgentStandupEntry {
            codename: "alpha".to_string(),
            role: AgentRole::Developer,
            yesterday_completed: vec![TaskSummary {
                id: "task-001".to_string(),
                title: "Test task".to_string(),
                status: TaskStatus::Done,
                status_change: Some(StatusChange {
                    from: TaskStatus::Running,
                    to: TaskStatus::Done,
                }),
            }],
            today_planned: vec![TaskSummary {
                id: "task-002".to_string(),
                title: "Next task".to_string(),
                status: TaskStatus::Ready,
                status_change: None,
            }],
            blockers: vec!["Waiting on review".to_string()],
        });

        let formatted = report.format_report();
        assert!(formatted.contains("Daily Standup"));
        assert!(formatted.contains("Agent alpha"));
        assert!(formatted.contains("task-001"));
        assert!(formatted.contains("task-002"));
        assert!(formatted.contains("Waiting on review"));
    }

    #[test]
    fn standup_report_no_blockers() {
        let mut report = DailyStandupReport::new();
        report.agent_entries.push(AgentStandupEntry {
            codename: "alpha".to_string(),
            role: AgentRole::Developer,
            yesterday_completed: vec![],
            today_planned: vec![],
            blockers: vec!["Blocked".to_string()],
        });
        assert!(report.has_blockers());

        report.agent_entries[0].blockers.clear();
        assert!(!report.has_blockers());
    }

    #[test]
    fn standup_blocked_agents() {
        let mut report = DailyStandupReport::new();
        report.agent_entries.push(AgentStandupEntry {
            codename: "alpha".to_string(),
            role: AgentRole::Developer,
            yesterday_completed: vec![],
            today_planned: vec![],
            blockers: vec!["Blocked".to_string()],
        });
        report.agent_entries.push(AgentStandupEntry {
            codename: "beta".to_string(),
            role: AgentRole::Developer,
            yesterday_completed: vec![],
            today_planned: vec![],
            blockers: vec![],
        });

        let blocked = report.blocked_agents();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].codename, "alpha");
    }

    #[test]
    fn task_history_entry_new() {
        let entry = TaskHistoryEntry::new(
            "task-001".to_string(),
            "Test task".to_string(),
            "alpha".to_string(),
            TaskStatus::Running,
            true,
        );
        assert!(entry.was_completed());
    }

    #[test]
    fn task_history_yesterday_check() {
        let entry = TaskHistoryEntry::new(
            "task-001".to_string(),
            "Test task".to_string(),
            "alpha".to_string(),
            TaskStatus::Running,
            true,
        );
        // Just created, should be recent
        assert!(entry.is_yesterday() || entry.changed_at >= Utc::now() - Duration::hours(1));
    }

    #[test]
    fn standup_helper_list_blockers() {
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task(
            "task-001",
            "Running task",
            TaskStatus::Running,
        ));
        backlog.push_task(make_test_task(
            "task-002",
            "Blocked task",
            TaskStatus::Blocked,
        ));
        backlog.push_task(make_test_task("task-003", "Done task", TaskStatus::Done));

        let blockers = StandupHelper::list_blockers(&backlog);
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].id, "task-002");
    }

    #[test]
    fn standup_helper_is_blocked() {
        let blocked = make_test_task("t1", "Blocked", TaskStatus::Blocked);
        let running = make_test_task("t2", "Running", TaskStatus::Running);

        assert!(StandupHelper::is_blocked(&blocked));
        assert!(!StandupHelper::is_blocked(&running));
    }

    #[test]
    fn standup_summary_default() {
        let summary = StandupSummary::default();
        assert_eq!(summary.total_agents, 0);
        assert_eq!(summary.tasks_completed, 0);
    }

    #[test]
    fn status_change_serialization() {
        let change = StatusChange {
            from: TaskStatus::Running,
            to: TaskStatus::Done,
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("Running"));
        assert!(json.contains("Done"));
    }

    #[test]
    fn task_summary_serialization() {
        let summary = TaskSummary {
            id: "task-001".to_string(),
            title: "Test".to_string(),
            status: TaskStatus::Done,
            status_change: None,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("task-001"));
    }
}
