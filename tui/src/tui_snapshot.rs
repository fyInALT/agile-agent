//! TUI Shutdown Snapshot
//!
//! Captures TUI-specific state for graceful restore across sessions.

use serde::{Deserialize, Serialize};

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
    /// Timestamp when snapshot was captured
    pub captured_at: String,
}

impl TuiShutdownSnapshot {
    /// Create a new TUI shutdown snapshot
    pub fn new(
        view_mode: ViewMode,
        split_state: Option<SplitViewSnapshot>,
        dashboard_state: Option<DashboardViewSnapshot>,
        mail_state: Option<MailViewSnapshot>,
    ) -> Self {
        use chrono::Utc;
        Self {
            view_mode,
            split_state,
            dashboard_state,
            mail_state,
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

        Self::new(view_state.mode, split_state, dashboard_state, mail_state)
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
    }
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
        );

        let mut view_state = TuiViewState::default();
        snapshot.apply_to(&mut view_state);

        assert_eq!(view_state.mode, ViewMode::Mail);
        assert_eq!(view_state.mail.selected_mail_index, 5);
        assert_eq!(view_state.mail.compose_field, ComposeField::Body);
    }
}
