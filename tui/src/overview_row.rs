//! Overview Agent Row Formatter
//!
//! Formats agent status rows for display in Overview mode with truncation logic.

use agent_core::agent_pool::AgentStatusSnapshot;
use agent_core::agent_slot::AgentSlotStatus;

/// Format an agent status row for display
pub struct OverviewAgentRow {
    /// Full formatted string
    pub full: String,
    /// Truncated version for narrow widths
    pub truncated: String,
    /// Width in unicode characters
    pub unicode_width: usize,
}

impl OverviewAgentRow {
    /// Format agent status snapshot into display row
    pub fn from_snapshot(
        snapshot: &AgentStatusSnapshot,
        _focused: bool,
        is_overview_agent: bool,
    ) -> Self {
        let indicator = if is_overview_agent {
            "◎" // Overview Agent always uses ◎
        } else {
            Self::status_indicator(&snapshot.status)
        };
        let status_label = if is_overview_agent {
            "ovw" // Overview Agent uses 'ovw' label
        } else {
            Self::short_status_label(&snapshot.status)
        };
        let task_desc = Self::task_description(snapshot);
        let elapsed = Self::elapsed_time(&snapshot.status);

        // Build full row: │ Indicator │ Name │ Status │ Task Description [+ Duration] │
        let full = if elapsed.is_empty() {
            format!(
                "{} {} {} {}",
                indicator,
                snapshot.codename.as_str(),
                status_label,
                task_desc
            )
        } else {
            format!(
                "{} {} {} {} ({})",
                indicator,
                snapshot.codename.as_str(),
                status_label,
                task_desc,
                elapsed
            )
        };

        let unicode_width = unicode_width_str(&full);
        let truncated = full.clone();

        Self {
            full,
            truncated,
            unicode_width,
        }
    }

    /// Truncate row to fit within max_width
    pub fn truncate_to(&mut self, max_width: usize) {
        if self.unicode_width <= max_width {
            return;
        }

        // Truncation order: task description → status → name
        // Minimum: indicator + truncated name (at least 2 chars)
        let min_width = 4; // indicator(1) + space(1) + name prefix(2)
        if max_width < min_width {
            self.truncated = self.full.chars().take(max_width).collect();
            self.unicode_width = max_width;
            return;
        }

        let mut chars = self.full.chars();
        let indicator = chars.next().unwrap_or('◎');
        chars.next();

        let rest: String = chars.collect();
        let mut parts = rest.splitn(3, ' ');
        let name = parts.next().unwrap_or("");
        let status = parts.next().unwrap_or("");
        let task = parts.next().unwrap_or("");

        if name.is_empty() {
            self.truncated = format!("{}..", indicator);
            self.unicode_width = 3;
            return;
        }

        let indicator_name = format!("{} {}", indicator, name);
        let indicator_name_status = if status.is_empty() {
            indicator_name.clone()
        } else {
            format!("{} {}", indicator_name, status)
        };

        if !task.is_empty() {
            let truncated_task = fit_segment(
                task,
                max_width.saturating_sub(unicode_width_str(&indicator_name_status) + 1),
            );
            if !truncated_task.is_empty() {
                self.truncated = format!("{} {}", indicator_name_status, truncated_task);
                self.unicode_width = unicode_width_str(&self.truncated);
                if self.unicode_width <= max_width {
                    return;
                }
            }
        }

        if unicode_width_str(&indicator_name_status) <= max_width {
            self.truncated = indicator_name_status;
            self.unicode_width = unicode_width_str(&self.truncated);
            return;
        }

        let truncated_status = fit_segment(
            status,
            max_width.saturating_sub(unicode_width_str(&indicator_name) + 1),
        );
        if !truncated_status.is_empty() {
            self.truncated = format!("{} {}", indicator_name, truncated_status);
            self.unicode_width = unicode_width_str(&self.truncated);
            if self.unicode_width <= max_width {
                return;
            }
        }

        let name_prefix = if name.chars().take(2).collect::<String>().len() >= 2 {
            name.chars().take(2).collect::<String>()
        } else {
            name.to_string()
        };
        if max_width == min_width {
            self.truncated = format!("{} {}", indicator, name_prefix);
            self.unicode_width = min_width;
            return;
        }

        let available_name_width = max_width.saturating_sub(3);
        let name_fit = fit_segment(name, available_name_width);
        self.truncated = format!("{} {}", indicator, name_fit);
        self.unicode_width = unicode_width_str(&self.truncated);
    }

    fn status_indicator(status: &AgentSlotStatus) -> &'static str {
        match status {
            AgentSlotStatus::Responding { .. } | AgentSlotStatus::ToolExecuting { .. } => "●",
            AgentSlotStatus::Idle => "○",
            AgentSlotStatus::Paused { .. } => "◈", // Paused with worktree preserved
            AgentSlotStatus::Stopped { .. } => "◌",
            AgentSlotStatus::Blocked { .. } => "🔶",
            AgentSlotStatus::BlockedForDecision { .. } => "🔶",
            AgentSlotStatus::Starting | AgentSlotStatus::Finishing => "◐",
            AgentSlotStatus::Stopping => "◐",
            AgentSlotStatus::Error { .. } => "⚠",
            AgentSlotStatus::WaitingForInput { .. } => "◉", // Waiting for user input
        }
    }

    fn short_status_label(status: &AgentSlotStatus) -> &'static str {
        match status {
            AgentSlotStatus::Responding { .. } => "run",
            AgentSlotStatus::ToolExecuting { .. } => "run",
            AgentSlotStatus::Idle => "idle",
            AgentSlotStatus::Paused { .. } => "pause",
            AgentSlotStatus::Blocked { .. } => "blk",
            AgentSlotStatus::BlockedForDecision { .. } => "dec",
            AgentSlotStatus::Stopped { .. } => "stop",
            AgentSlotStatus::Starting => "start",
            AgentSlotStatus::Finishing => "fin",
            AgentSlotStatus::Stopping => "stop",
            AgentSlotStatus::Error { .. } => "err",
            AgentSlotStatus::WaitingForInput { .. } => "wait",
        }
    }

    fn task_description(snapshot: &AgentStatusSnapshot) -> String {
        match &snapshot.status {
            AgentSlotStatus::Idle => {
                if snapshot.has_worktree {
                    if let Some(branch) = &snapshot.worktree_branch {
                        format!("wt:{}", branch)
                    } else {
                        "wt:detached".to_string()
                    }
                } else {
                    "Waiting for task".to_string()
                }
            }
            AgentSlotStatus::Paused { reason } => {
                if let Some(branch) = &snapshot.worktree_branch {
                    format!("wt:{} ({})", branch, reason)
                } else {
                    format!("paused ({})", reason)
                }
            }
            AgentSlotStatus::Blocked { reason } => reason.clone(),
            AgentSlotStatus::Responding { .. } | AgentSlotStatus::ToolExecuting { .. } => {
                if snapshot.has_worktree {
                    if let Some(branch) = &snapshot.worktree_branch {
                        format!("Working [wt:{}]", branch)
                    } else {
                        "Working".to_string()
                    }
                } else {
                    "Working".to_string()
                }
            }
            AgentSlotStatus::WaitingForInput { .. } => {
                if snapshot.has_worktree {
                    if let Some(branch) = &snapshot.worktree_branch {
                        format!("Waiting for input [wt:{}]", branch)
                    } else {
                        "Waiting for input".to_string()
                    }
                } else {
                    "Waiting for input".to_string()
                }
            }
            _ => String::new(),
        }
    }

    fn elapsed_time(status: &AgentSlotStatus) -> String {
        match status {
            AgentSlotStatus::Responding { started_at } => {
                let elapsed = started_at.elapsed().as_secs();
                let mins = elapsed / 60;
                let secs = elapsed % 60;
                format!("{}m{}s", mins, secs)
            }
            AgentSlotStatus::WaitingForInput { started_at } => {
                let elapsed = started_at.elapsed().as_secs();
                let mins = elapsed / 60;
                let secs = elapsed % 60;
                format!("{}m{}s", mins, secs)
            }
            _ => String::new(),
        }
    }
}

/// Calculate unicode display width of a string
fn unicode_width_str(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

fn fit_segment(segment: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if unicode_width_str(segment) <= max_width {
        return segment.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut fitted = String::new();
    for ch in segment.chars() {
        let next = format!("{fitted}{ch}");
        if unicode_width_str(&next) + 3 > max_width {
            break;
        }
        fitted.push(ch);
    }

    if fitted.is_empty() {
        ".".repeat(max_width)
    } else {
        format!("{fitted}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::agent_role::AgentRole;
    use agent_core::agent_runtime::{AgentCodename, AgentId, ProviderType};

    fn make_snapshot(status: AgentSlotStatus) -> AgentStatusSnapshot {
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent_001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Mock,
            role: AgentRole::Developer,
            status,
            assigned_task_id: None,
            worktree_branch: None,
            has_worktree: false,
            worktree_exists: false,
        }
    }

    fn make_snapshot_with_worktree(
        status: AgentSlotStatus,
        branch: Option<String>,
        exists: bool,
    ) -> AgentStatusSnapshot {
        AgentStatusSnapshot {
            agent_id: AgentId::new("agent_001"),
            codename: AgentCodename::new("alpha"),
            provider_type: ProviderType::Mock,
            role: AgentRole::Developer,
            status,
            assigned_task_id: None,
            worktree_branch: branch,
            has_worktree: true,
            worktree_exists: exists,
        }
    }

    #[test]
    fn row_format_idle_agent() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("○"));
        assert!(row.full.contains("alpha"));
        assert!(row.full.contains("idle"));
    }

    #[test]
    fn row_format_blocked_agent() {
        let snapshot = make_snapshot(AgentSlotStatus::blocked("API design not confirmed"));
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("🔶"));
        assert!(row.full.contains("blk"));
        assert!(row.full.contains("API design not confirmed"));
    }

    #[test]
    fn row_format_overview_agent() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, true);
        assert!(row.full.contains("◎"));
        assert!(row.full.contains("alpha"));
        assert!(row.full.contains("ovw"));
    }

    #[test]
    fn row_truncate_preserves_indicator() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        row.truncate_to(5);
        assert!(row.truncated.starts_with("○"));
    }

    #[test]
    fn row_truncate_preserves_overview_indicator() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false, true);
        row.truncate_to(5);
        assert!(row.truncated.starts_with("◎"));
    }

    #[test]
    fn row_truncate_preserves_name_prefix() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        row.truncate_to(4);
        assert!(row.truncated.contains("al")); // "alpha" prefix
    }

    #[test]
    fn row_truncate_minimum_width() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        row.truncate_to(2);
        assert!(row.unicode_width <= 2);
    }

    #[test]
    fn row_truncate_keeps_status_before_falling_back_to_name_only() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        row.truncate_to(18);
        assert!(row.truncated.contains("idle"));
    }

    #[test]
    fn unicode_width_ascii() {
        assert_eq!(unicode_width_str("hello"), 5);
    }

    #[test]
    fn unicode_width_unicode() {
        assert_eq!(unicode_width_str("🔶"), 2);
        assert_eq!(unicode_width_str("◎"), 2);
    }

    #[test]
    fn row_format_paused_agent() {
        let snapshot = make_snapshot_with_worktree(
            AgentSlotStatus::paused("worktree preserved"),
            Some("feature/test".to_string()),
            true,
        );
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("◈")); // Paused indicator
        assert!(row.full.contains("pause"));
        assert!(row.full.contains("wt:feature/test"));
    }

    #[test]
    fn row_format_agent_with_worktree_shows_branch() {
        let snapshot = make_snapshot_with_worktree(
            AgentSlotStatus::Idle,
            Some("feature/my-task".to_string()),
            true,
        );
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("wt:feature/my-task"));
    }

    #[test]
    fn row_format_agent_without_worktree_waiting() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("Waiting for task"));
        assert!(!row.full.contains("wt:"));
    }

    #[test]
    fn row_format_working_agent_with_worktree() {
        let snapshot = make_snapshot_with_worktree(
            AgentSlotStatus::Responding {
                started_at: std::time::Instant::now(),
            },
            Some("dev/123".to_string()),
            true,
        );
        let row = OverviewAgentRow::from_snapshot(&snapshot, false, false);
        assert!(row.full.contains("[wt:dev/123]"));
    }
}
