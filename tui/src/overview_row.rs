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
        focused: bool,
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

        // Use chars() to handle unicode properly
        let mut chars = self.full.chars();
        let indicator = chars.next().unwrap_or('◎');

        // Skip the space after indicator
        chars.next();

        // Get the rest as a string
        let rest: String = chars.collect();
        let parts: Vec<&str> = rest.split_whitespace().collect();

        // Try to preserve indicator + name prefix
        if parts.is_empty() {
            self.truncated = format!("{}..", indicator);
            self.unicode_width = 3;
            return;
        }

        let name = parts[0];
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

        // Add more of the name if space available
        let remaining = max_width - 3; // indicator + space + name prefix (2)
        let name_chars: String = name.chars().take(remaining.saturating_sub(2)).collect();
        let name_fit = if name.chars().count() > remaining.saturating_sub(2) {
            format!("{}..", name_chars)
        } else {
            name.to_string()
        };

        self.truncated = format!("{} {}", indicator, name_fit);
        self.unicode_width = unicode_width_str(&self.truncated);
    }

    fn status_indicator(status: &AgentSlotStatus) -> &'static str {
        match status {
            AgentSlotStatus::Responding { .. } | AgentSlotStatus::ToolExecuting { .. } => "●",
            AgentSlotStatus::Idle => "○",
            AgentSlotStatus::Stopped { .. } => "◌",
            AgentSlotStatus::Blocked { .. } => "🔶",
            AgentSlotStatus::Starting | AgentSlotStatus::Finishing => "◐",
            AgentSlotStatus::Stopping => "◐",
            AgentSlotStatus::Error { .. } => "⚠",
        }
    }

    fn short_status_label(status: &AgentSlotStatus) -> &'static str {
        match status {
            AgentSlotStatus::Responding { .. } => "run",
            AgentSlotStatus::ToolExecuting { .. } => "run",
            AgentSlotStatus::Idle => "idle",
            AgentSlotStatus::Blocked { .. } => "blk",
            AgentSlotStatus::Stopped { .. } => "stop",
            AgentSlotStatus::Starting => "start",
            AgentSlotStatus::Finishing => "fin",
            AgentSlotStatus::Stopping => "stop",
            AgentSlotStatus::Error { .. } => "err",
        }
    }

    fn task_description(snapshot: &AgentStatusSnapshot) -> String {
        match &snapshot.status {
            AgentSlotStatus::Idle => "Waiting for task".to_string(),
            AgentSlotStatus::Blocked { reason } => reason.clone(),
            AgentSlotStatus::Responding { .. } | AgentSlotStatus::ToolExecuting { .. } => {
                // Would need task info from snapshot
                "Working".to_string()
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
            _ => String::new(),
        }
    }
}

/// Calculate unicode display width of a string
fn unicode_width_str(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
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
    fn unicode_width_ascii() {
        assert_eq!(unicode_width_str("hello"), 5);
    }

    #[test]
    fn unicode_width_unicode() {
        assert_eq!(unicode_width_str("🔶"), 2);
        assert_eq!(unicode_width_str("◎"), 2);
    }
}
