//! Overview View State
//!
//! State management for Overview display mode in multi-agent TUI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;

/// Filter mode for Overview agent list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OverviewFilter {
    #[default]
    All,
    BlockedOnly,
    RunningOnly,
}

/// Simplified log message from any agent
#[derive(Debug, Clone)]
pub struct OverviewLogMessage {
    /// Timestamp (HH:MM:SS format stored as u32 for efficiency)
    pub timestamp: u32,
    /// Agent codename
    pub agent: String,
    /// Message type indicator
    pub message_type: OverviewMessageType,
    /// Content text
    pub content: String,
}

/// Message type for scroll log prefix
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewMessageType {
    Progress,
    Blocked,
    Complete,
    Quick,
    Task,
}

impl OverviewMessageType {
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Progress => "●",
            Self::Blocked => "🔶",
            Self::Complete => "✓",
            Self::Quick => "⚡",
            Self::Task => "📋",
        }
    }
}

/// Output verbosity level per agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputVerbosity {
    #[default]
    Concise,
    Full,
    Minimal,
}

/// State specific to Overview view mode
#[derive(Debug, Clone)]
pub struct OverviewViewState {
    /// Number of rows for agent status list (configurable, 3-12)
    pub agent_list_rows: usize,
    /// Currently focused agent index in filtered list
    pub focused_agent_index: usize,
    /// Current filter mode
    pub filter: OverviewFilter,
    /// Scroll position for log area
    pub log_scroll_offset: usize,
    /// Circular buffer for scroll log messages
    pub log_buffer: VecDeque<OverviewLogMessage>,
    /// Maximum log buffer size
    pub max_log_size: usize,
    /// Per-agent output verbosity overrides
    pub agent_verbosity: HashMap<String, OutputVerbosity>,
    /// Global default verbosity
    pub global_verbosity: OutputVerbosity,
    /// Search query for agent selection
    pub search_query: String,
    /// Whether search is active
    pub search_active: bool,
    /// Page offset for agents > agent_list_rows
    pub page_offset: usize,
}

impl Default for OverviewViewState {
    fn default() -> Self {
        Self {
            agent_list_rows: 8,
            focused_agent_index: 0,
            filter: OverviewFilter::default(),
            log_scroll_offset: 0,
            log_buffer: VecDeque::with_capacity(1000),
            max_log_size: 1000,
            agent_verbosity: HashMap::new(),
            global_verbosity: OutputVerbosity::default(),
            search_query: String::new(),
            search_active: false,
            page_offset: 0,
        }
    }
}

impl OverviewViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set agent list rows (clamped to 3-12)
    pub fn set_agent_list_rows(&mut self, rows: usize) {
        self.agent_list_rows = rows.clamp(3, 12);
    }

    /// Cycle filter mode
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            OverviewFilter::All => OverviewFilter::BlockedOnly,
            OverviewFilter::BlockedOnly => OverviewFilter::RunningOnly,
            OverviewFilter::RunningOnly => OverviewFilter::All,
        };
    }

    /// Add message to scroll log (evicts oldest if full)
    pub fn push_log_message(&mut self, message: OverviewLogMessage) {
        if self.log_buffer.len() >= self.max_log_size {
            self.log_buffer.pop_front();
            // Adjust scroll offset if near end
            if self.log_scroll_offset > 0 {
                self.log_scroll_offset -= 1;
            }
        }
        self.log_buffer.push_back(message);
    }

    /// Clear scroll log
    pub fn clear_log(&mut self) {
        self.log_buffer.clear();
        self.log_scroll_offset = 0;
    }

    /// Get verbosity for specific agent
    pub fn verbosity_for(&self, agent: &str) -> OutputVerbosity {
        self.agent_verbosity
            .get(agent)
            .copied()
            .unwrap_or(self.global_verbosity)
    }

    /// Set verbosity for specific agent
    pub fn set_agent_verbosity(&mut self, agent: &str, level: OutputVerbosity) {
        self.agent_verbosity.insert(agent.to_string(), level);
    }

    /// Scroll log up
    pub fn scroll_log_up(&mut self, step: usize) {
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(step);
    }

    /// Scroll log down
    pub fn scroll_log_down(&mut self, step: usize) {
        let max_scroll = self.log_buffer.len().saturating_sub(1);
        self.log_scroll_offset = (self.log_scroll_offset + step).min(max_scroll);
    }

    /// Focus next agent in filtered list
    pub fn focus_next(&mut self, filtered_count: usize) {
        if filtered_count > 0 {
            self.focused_agent_index = (self.focused_agent_index + 1) % filtered_count;
        }
    }

    /// Focus previous agent in filtered list
    pub fn focus_prev(&mut self, filtered_count: usize) {
        if filtered_count > 0 {
            self.focused_agent_index =
                (self.focused_agent_index + filtered_count - 1) % filtered_count;
        }
    }

    /// Focus agent by number (1-9)
    pub fn focus_by_number(&mut self, n: u8, filtered_count: usize) {
        let index = (n as usize).saturating_sub(1);
        if index < filtered_count {
            self.focused_agent_index = index;
        }
    }

    /// Clamp focus to valid range
    pub fn clamp_focus(&mut self, filtered_count: usize) {
        if filtered_count == 0 {
            self.focused_agent_index = 0;
        } else {
            self.focused_agent_index = self.focused_agent_index.min(filtered_count - 1);
        }
    }

    /// Page navigation for agents > agent_list_rows
    pub fn page_up(&mut self, total_pages: usize) {
        if total_pages > 0 {
            self.page_offset = self.page_offset.saturating_sub(1);
        }
    }

    pub fn page_down(&mut self, total_pages: usize) {
        if self.page_offset < total_pages.saturating_sub(1) {
            self.page_offset += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_filter_default_is_all() {
        assert_eq!(OverviewFilter::default(), OverviewFilter::All);
    }

    #[test]
    fn overview_message_type_indicators() {
        assert_eq!(OverviewMessageType::Progress.indicator(), "●");
        assert_eq!(OverviewMessageType::Blocked.indicator(), "🔶");
        assert_eq!(OverviewMessageType::Complete.indicator(), "✓");
    }

    #[test]
    fn output_verbosity_default_is_concise() {
        assert_eq!(OutputVerbosity::default(), OutputVerbosity::Concise);
    }

    #[test]
    fn overview_state_default_agent_list_rows() {
        let state = OverviewViewState::default();
        assert_eq!(state.agent_list_rows, 8);
    }

    #[test]
    fn overview_state_set_agent_list_rows_clamped() {
        let mut state = OverviewViewState::default();
        state.set_agent_list_rows(1);
        assert_eq!(state.agent_list_rows, 3);
        state.set_agent_list_rows(20);
        assert_eq!(state.agent_list_rows, 12);
    }

    #[test]
    fn overview_state_cycle_filter() {
        let mut state = OverviewViewState::default();
        assert_eq!(state.filter, OverviewFilter::All);
        state.cycle_filter();
        assert_eq!(state.filter, OverviewFilter::BlockedOnly);
        state.cycle_filter();
        assert_eq!(state.filter, OverviewFilter::RunningOnly);
        state.cycle_filter();
        assert_eq!(state.filter, OverviewFilter::All);
    }

    #[test]
    fn overview_state_push_log_evicts_old() {
        let mut state = OverviewViewState {
            max_log_size: 3,
            ..Default::default()
        };
        for i in 0..5 {
            state.push_log_message(OverviewLogMessage {
                timestamp: i,
                agent: "alpha".to_string(),
                message_type: OverviewMessageType::Progress,
                content: format!("msg {}", i),
            });
        }
        assert_eq!(state.log_buffer.len(), 3);
        // Should have messages 2, 3, 4
        assert_eq!(state.log_buffer.front().unwrap().timestamp, 2);
        assert_eq!(state.log_buffer.back().unwrap().timestamp, 4);
    }

    #[test]
    fn overview_state_focus_navigation() {
        let mut state = OverviewViewState::default();
        state.focus_next(5);
        assert_eq!(state.focused_agent_index, 1);
        state.focus_prev(5);
        assert_eq!(state.focused_agent_index, 0);
        state.focus_by_number(3, 5);
        assert_eq!(state.focused_agent_index, 2);
    }

    #[test]
    fn overview_state_verbosity_override() {
        let mut state = OverviewViewState {
            global_verbosity: OutputVerbosity::Minimal,
            ..Default::default()
        };
        state.set_agent_verbosity("alpha", OutputVerbosity::Full);
        assert_eq!(state.verbosity_for("alpha"), OutputVerbosity::Full);
        assert_eq!(state.verbosity_for("bravo"), OutputVerbosity::Minimal);
    }
}
