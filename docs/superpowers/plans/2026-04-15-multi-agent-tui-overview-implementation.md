# Multi-Agent TUI Overview Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Overview display mode for multi-agent TUI, where Overview Agent serves as the primary entry point for coordinating worker agents.

**Architecture:** Add Overview as a new ViewMode variant, with a dedicated state structure (OverviewViewState) managing agent list display, scroll log aggregation, and focus/filter state. The layout uses a fixed-height agent status list (top) and scrollable log/transcript area (middle), sharing the existing composer (bottom).

**Tech Stack:** Rust, ratatui (TUI framework), existing agent_pool/agent_slot infrastructure

---

## Implementation Challenges and Risk Mitigation

### Challenge 1: Width Truncation Logic
**Risk:** Agent rows can overflow on narrow terminals, causing layout breakage.
**Mitigation:** Implement truncation in order: task description → status → name (preserve prefix). Use `unicode_width` crate for accurate width calculation, not `str.len()`. Minimum width = indicator + truncated name prefix.

### Challenge 2: Scroll Log Buffer Memory
**Risk:** Unbounded scroll log can consume excessive memory with many agents.
**Mitigation:** Implement configurable buffer size (default: 1000 messages). Use circular buffer (`VecDeque`) with automatic eviction. Messages from same minute can share timestamp.

### Challenge 3: Multi-Agent Event Polling Race Condition
**Risk:** EventAggregator polls multiple agent channels; missed events can cause stale display.
**Mitigation:** Ensure polling happens before each render cycle. Store event revision counter to detect staleness.

### Challenge 4: Focus State Consistency
**Risk:** Focus switching while agents are being spawned/stopped can cause index mismatch.
**Mitigation:** Always clamp focus index after any pool mutation. Use pool's `focused_slot_index()` as authoritative source, not local copy.

### Challenge 5: @ Command Parsing Edge Cases
**Risk:** `@alpha,bravo` vs `@alpha @bravo` syntax ambiguity.
**Mitigation:** Support both comma-separated and space-separated syntax. Parse before routing; reject malformed commands with user feedback.

### Challenge 6: Simplified Output Level Consistency
**Risk:** Different agents may have different output levels, causing inconsistent log density.
**Mitigation:** Per-agent level config stored in OverviewViewState. Default to global level; override per-agent when explicitly set.

---

## File Structure

| File | Purpose |
|------|---------|
| `tui/src/view_mode.rs` | Add `Overview` variant to ViewMode enum |
| `tui/src/ui_state.rs` | Add `OverviewViewState` struct, scroll log buffer |
| `tui/src/overview_log.rs` | New file: scroll log message types, aggregation logic |
| `tui/src/render.rs` | Add `render_overview_view`, agent status list, scroll log rendering |
| `tui/src/input.rs` | Add Overview-specific key handling (filtering, search) |
| `tui/src/overview_row.rs` | New file: agent row formatting, truncation logic |
| `tui/src/app_loop.rs` | Add @ command routing logic |
| `tui/src/lib.rs` | Export new modules |
| `core/src/agent_slot.rs` | Add `Blocked` status variant (if not present) |

---

## Task 1: Add Blocked Status to AgentSlotStatus

**Files:**
- Modify: `core/src/agent_slot.rs`
- Test: `core/src/agent_slot.rs` (inline tests)

**Context:** The design spec requires a `blk` (blocked) status for agents that need human intervention. Current AgentSlotStatus has Idle, Starting, Responding, ToolExecuting, Finishing, Stopping, Stopped, Error - but no Blocked.

- [ ] **Step 1: Write the failing test**

```rust
// In core/src/agent_slot.rs tests module

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package agent-core status_blocked --no-fail-fast`
Expected: FAIL with "no variant or associated item named `blocked` found"

- [ ] **Step 3: Add Blocked variant to AgentSlotStatus enum**

```rust
// In core/src/agent_slot.rs AgentSlotStatus enum

pub enum AgentSlotStatus {
    /// Agent is idle, waiting for task assignment
    Idle,
    /// Agent is starting up
    Starting,
    /// Agent is generating response (thinking/streaming)
    Responding { started_at: Instant },
    /// Agent is executing a tool call
    ToolExecuting { tool_name: String },
    /// Agent is finishing its current work
    Finishing,
    /// Agent is being stopped gracefully (not yet joined)
    Stopping,
    /// Agent has been stopped intentionally
    Stopped { reason: String },
    /// Agent encountered an error
    Error { message: String },
    /// Agent is blocked, waiting for human intervention
    Blocked { reason: String },
}
```

- [ ] **Step 4: Add blocked() constructor**

```rust
// In AgentSlotStatus impl block

/// Create a new Blocked status
pub fn blocked(reason: impl Into<String>) -> Self {
    Self::Blocked { reason: reason.into() }
}
```

- [ ] **Step 5: Update can_transition_to for Blocked**

```rust
// In can_transition_to match block, add:

// Idle can go to Blocked
(Self::Idle, Self::Blocked { .. }) => true,
// Blocked can go to Idle (user resolved), Responding (user provided input), or Stopped
(Self::Blocked { .. }, Self::Idle) => true,
(Self::Blocked { .. }, Self::Responding { .. }) => true,
(Self::Blocked { .. }, Self::Stopped { .. }) => true,
```

- [ ] **Step 6: Update label() for Blocked**

```rust
// In label() match block, add:

Self::Blocked { reason } => format!("blocked:{}", reason),
```

- [ ] **Step 7: Add is_blocked() helper**

```rust
// In AgentSlotStatus impl block

/// Check if agent is blocked
pub fn is_blocked(&self) -> bool {
    matches!(self, Self::Blocked { .. })
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cargo test --package agent-core status_blocked`
Expected: PASS (all blocked tests)

- [ ] **Step 9: Commit**

```bash
git add core/src/agent_slot.rs
git commit -m "feat(core): add Blocked status to AgentSlotStatus"
```

---

## Task 2: Add Overview Variant to ViewMode

**Files:**
- Modify: `tui/src/view_mode.rs`
- Test: `tui/src/view_mode.rs` (inline tests)

**Context:** Add `Overview` as a new ViewMode, updating cycle order and helper methods.

- [ ] **Step 1: Write the failing test**

```rust
// In tui/src/view_mode.rs tests module

#[test]
fn view_mode_overview_label() {
    assert_eq!(ViewMode::Overview.label(), "Overview");
}

#[test]
fn view_mode_overview_key_hint() {
    assert_eq!(ViewMode::Overview.key_hint(), "Ctrl+V 6");
}

#[test]
fn view_mode_overview_from_number() {
    assert_eq!(ViewMode::from_number(6), Some(ViewMode::Overview));
}

#[test]
fn view_mode_overview_cycle() {
    // TaskMatrix -> Overview -> Focused
    assert_eq!(ViewMode::TaskMatrix.next(), ViewMode::Overview);
    assert_eq!(ViewMode::Overview.next(), ViewMode::Focused);
    assert_eq!(ViewMode::Overview.prev(), ViewMode::TaskMatrix);
}

#[test]
fn view_mode_overview_shows_multiple() {
    assert!(ViewMode::Overview.shows_multiple_agents());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package agent-tui view_mode_overview`
Expected: FAIL with "no variant named `Overview`"

- [ ] **Step 3: Add Overview variant to ViewMode enum**

```rust
// In tui/src/view_mode.rs ViewMode enum

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    #[default]
    Focused,
    Split,
    Dashboard,
    Mail,
    TaskMatrix,
    Overview,
}
```

- [ ] **Step 4: Update label() method**

```rust
pub fn label(&self) -> &'static str {
    match self {
        Self::Focused => "Focused",
        Self::Split => "Split",
        Self::Dashboard => "Dashboard",
        Self::Mail => "Mail",
        Self::TaskMatrix => "Tasks",
        Self::Overview => "Overview",
    }
}
```

- [ ] **Step 5: Update key_hint() method**

```rust
pub fn key_hint(&self) -> &'static str {
    match self {
        Self::Focused => "Ctrl+V 1",
        Self::Split => "Ctrl+V 2",
        Self::Dashboard => "Ctrl+V 3",
        Self::Mail => "Ctrl+V 4",
        Self::TaskMatrix => "Ctrl+V 5",
        Self::Overview => "Ctrl+V 6",
    }
}
```

- [ ] **Step 6: Update from_number() method**

```rust
pub fn from_number(n: u8) -> Option<Self> {
    match n {
        1 => Some(Self::Focused),
        2 => Some(Self::Split),
        3 => Some(Self::Dashboard),
        4 => Some(Self::Mail),
        5 => Some(Self::TaskMatrix),
        6 => Some(Self::Overview),
        _ => None,
    }
}
```

- [ ] **Step 7: Update next() and prev() cycle methods**

```rust
pub fn next(self) -> Self {
    match self {
        Self::Focused => Self::Split,
        Self::Split => Self::Dashboard,
        Self::Dashboard => Self::Mail,
        Self::Mail => Self::TaskMatrix,
        Self::TaskMatrix => Self::Overview,
        Self::Overview => Self::Focused,
    }
}

pub fn prev(self) -> Self {
    match self {
        Self::Focused => Self::Overview,
        Self::Split => Self::Focused,
        Self::Dashboard => Self::Split,
        Self::Mail => Self::Dashboard,
        Self::TaskMatrix => Self::Mail,
        Self::Overview => Self::TaskMatrix,
    }
}
```

- [ ] **Step 8: Update shows_multiple_agents()**

```rust
pub fn shows_multiple_agents(self) -> bool {
    matches!(self, Self::Split | Self::Dashboard | Self::TaskMatrix | Self::Overview)
}
```

- [ ] **Step 9: Run test to verify it passes**

Run: `cargo test --package agent-tui view_mode_overview`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add tui/src/view_mode.rs
git commit -m "feat(tui): add Overview variant to ViewMode"
```

---

## Task 3: Create OverviewViewState Structure

**Files:**
- Create: `tui/src/overview_state.rs`
- Modify: `tui/src/view_mode.rs` (add OverviewViewState to TuiViewState)
- Test: `tui/src/overview_state.rs` (inline tests)

**Context:** Create state structure for Overview mode, including agent list rows config, focus state, filter state, and scroll log buffer.

- [ ] **Step 1: Write the failing test (in new file)**

```rust
// Create tui/src/overview_state.rs

use std::collections::VecDeque;

/// Filter mode for Overview agent list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
}
```

- [ ] **Step 2: Run test to verify file structure**

Run: `cargo test --package agent-tui overview_state`
Expected: PASS (basic structure tests)

- [ ] **Step 3: Add OverviewViewState struct**

```rust
// Add to tui/src/overview_state.rs

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
    pub agent_verbosity: std::collections::HashMap<String, OutputVerbosity>,
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
            agent_verbosity: std::collections::HashMap::new(),
            global_verbosity: OutputVerbosity::default(),
            search_query: String::new(),
            search_active: false,
            page_offset: 0,
        }
    }
}
```

- [ ] **Step 4: Add OverviewViewState helper methods**

```rust
// Add to OverviewViewState impl block

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
        self.agent_verbosity.get(agent).copied().unwrap_or(self.global_verbosity)
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
            self.focused_agent_index = (self.focused_agent_index + filtered_count - 1) % filtered_count;
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
```

- [ ] **Step 5: Add tests for OverviewViewState**

```rust
// Add to tests module

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
    let mut state = OverviewViewState::default();
    state.max_log_size = 3;
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
    let mut state = OverviewViewState::default();
    state.global_verbosity = OutputVerbosity::Minimal;
    state.set_agent_verbosity("alpha", OutputVerbosity::Full);
    assert_eq!(state.verbosity_for("alpha"), OutputVerbosity::Full);
    assert_eq!(state.verbosity_for("bravo"), OutputVerbosity::Minimal);
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --package agent-tui overview_state`
Expected: PASS

- [ ] **Step 7: Add OverviewViewState to TuiViewState**

```rust
// In tui/src/view_mode.rs TuiViewState struct

use crate::overview_state::OverviewViewState;

#[derive(Debug, Clone, Default)]
pub struct TuiViewState {
    pub mode: ViewMode,
    pub split: SplitViewState,
    pub dashboard: DashboardViewState,
    pub mail: MailViewState,
    pub task_matrix: TaskMatrixViewState,
    pub overview: OverviewViewState,
}
```

- [ ] **Step 8: Update TuiViewState::new()**

```rust
// In TuiViewState impl

pub fn new() -> Self {
    Self::default()
}
```

- [ ] **Step 9: Export module in lib.rs**

```rust
// In tui/src/lib.rs

pub mod overview_state;
```

- [ ] **Step 10: Run all tests**

Run: `cargo test --package agent-tui`
Expected: PASS

- [ ] **Step 11: Commit**

```bash
git add tui/src/overview_state.rs tui/src/view_mode.rs tui/src/lib.rs
git commit -m "feat(tui): add OverviewViewState for Overview mode"
```

---

## Task 4: Create Agent Row Formatter with Truncation

**Files:**
- Create: `tui/src/overview_row.rs`
- Test: `tui/src/overview_row.rs` (inline tests)

**Context:** Implement agent row formatting with truncation logic. Truncation order: task description → status → name (preserve prefix). Use unicode width for accurate calculation.

- [ ] **Step 1: Write the failing test**

```rust
// Create tui/src/overview_row.rs

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
    pub fn from_snapshot(snapshot: &AgentStatusSnapshot, focused: bool) -> Self {
        let indicator = Self::status_indicator(&snapshot.status);
        let status_label = Self::short_status_label(&snapshot.status);
        let task_desc = Self::task_description(snapshot);
        let elapsed = Self::elapsed_time(&snapshot.status);

        // Build full row: │ Indicator │ Name │ Status │ Task Description [+ Duration] │
        let full = format!(
            "{} {} {}{}{}",
            indicator,
            snapshot.codename.as_str(),
            status_label,
            if task_desc.is_empty() { "" } else { " " },
            if elapsed.is_empty() {
                task_desc
            } else {
                format!("{} ({})", task_desc, elapsed)
            }
        );

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

        let indicator = self.full.chars().next().unwrap_or('◎');
        let parts: Vec<&str> = self.full[2..].split_whitespace().collect();

        // Try to preserve indicator + name prefix
        if parts.is_empty() {
            self.truncated = format!("{}..", indicator);
            self.unicode_width = 3;
            return;
        }

        let name = parts.first().unwrap_or("");
        let name_prefix = if name.len() > 2 { &name[..2] } else { name };

        if max_width == min_width {
            self.truncated = format!("{} {}", indicator, name_prefix);
            self.unicode_width = min_width;
            return;
        }

        // Add more of the name if space available
        let remaining = max_width - 3; // indicator + space + name prefix (2)
        let name_fit = if name.len() > remaining {
            format!("{}..", &name[..remaining.saturating_sub(2).min(name.len())])
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
    use agent_core::agent_runtime::{AgentId, AgentCodename, ProviderType};
    use agent_core::agent_role::AgentRole;

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
        let row = OverviewAgentRow::from_snapshot(&snapshot, false);
        assert!(row.full.contains("○"));
        assert!(row.full.contains("alpha"));
        assert!(row.full.contains("idle"));
    }

    #[test]
    fn row_format_blocked_agent() {
        let snapshot = make_snapshot(AgentSlotStatus::blocked("API design not confirmed"));
        let row = OverviewAgentRow::from_snapshot(&snapshot, false);
        assert!(row.full.contains("🔶"));
        assert!(row.full.contains("blk"));
        assert!(row.full.contains("API design not confirmed"));
    }

    #[test]
    fn row_truncate_preserves_indicator() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false);
        row.truncate_to(5);
        assert!(row.truncated.starts_with("○"));
    }

    #[test]
    fn row_truncate_preserves_name_prefix() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false);
        row.truncate_to(4);
        assert!(row.truncated.contains("al")); // "alpha" prefix
    }

    #[test]
    fn row_truncate_minimum_width() {
        let snapshot = make_snapshot(AgentSlotStatus::Idle);
        let mut row = OverviewAgentRow::from_snapshot(&snapshot, false);
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
```

- [ ] **Step 2: Run test to verify it fails initially (no unicode-width crate yet)**

Run: `cargo test --package agent-tui overview_row`
Expected: May need unicode-width crate or pass with simple impl

- [ ] **Step 3: Add unicode-width dependency (if needed)**

```toml
# In tui/Cargo.toml, add:

[dependencies]
unicode-width = "0.1"
```

Then update the unicode_width_str function:

```rust
fn unicode_width_str(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package agent-tui overview_row`
Expected: PASS

- [ ] **Step 5: Export module**

```rust
// In tui/src/lib.rs

pub mod overview_row;
```

- [ ] **Step 6: Commit**

```bash
git add tui/src/overview_row.rs tui/src/lib.rs tui/Cargo.toml
git commit -m "feat(tui): add agent row formatter with truncation"
```

---

## Task 5: Add Overview Input Handling

**Files:**
- Modify: `tui/src/input.rs`
- Test: `tui/src/input.rs` (inline tests)

**Context:** Add Overview-specific key handling: filtering (f/r/a), search (/), page navigation ([/]), agent focus (Tab/1-9).

- [ ] **Step 1: Write the failing test**

```rust
// In tui/src/input.rs tests module

#[test]
fn overview_f_filters_blocked() {
    let mut state = make_tui_state();
    state.view_state.switch_by_number(6); // Overview mode
    let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::OverviewFilterBlocked));
}

#[test]
fn overview_r_filters_running() {
    let mut state = make_tui_state();
    state.view_state.switch_by_number(6);
    let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::OverviewFilterRunning));
}

#[test]
fn overview_a_shows_all() {
    let mut state = make_tui_state();
    state.view_state.switch_by_number(6);
    let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::OverviewFilterAll));
}

#[test]
fn overview_left_bracket_page_up() {
    let mut state = make_tui_state();
    state.view_state.switch_by_number(6);
    let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::OverviewPageUp));
}

#[test]
fn overview_right_bracket_page_down() {
    let mut state = make_tui_state();
    state.view_state.switch_by_number(6);
    let outcome = handle_key_event(&mut state, KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert!(matches!(outcome, InputOutcome::OverviewPageDown));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package agent-tui overview_f_filter`
Expected: FAIL with "no variant OverviewFilterBlocked"

- [ ] **Step 3: Add new InputOutcome variants**

```rust
// In tui/src/input.rs InputOutcome enum

pub enum InputOutcome {
    // ... existing variants ...
    /// Overview: filter to blocked agents
    OverviewFilterBlocked,
    /// Overview: filter to running agents
    OverviewFilterRunning,
    /// Overview: show all agents
    OverviewFilterAll,
    /// Overview: page up in agent list
    OverviewPageUp,
    /// Overview: page down in agent list
    OverviewPageDown,
    /// Overview: start search
    OverviewSearchStart,
    /// Overview: cancel search
    OverviewSearchCancel,
    /// Overview: focus by number (when search active)
    OverviewSearchSelect,
}
```

- [ ] **Step 4: Add key handling for Overview mode**

```rust
// In handle_key_event match block, add after Mail view handling:

// Overview view: filter keys
KeyEvent {
    code: KeyCode::Char('f'),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
    && state.view_state.mode == crate::view_mode::ViewMode::Overview => InputOutcome::OverviewFilterBlocked,

KeyEvent {
    code: KeyCode::Char('r'),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
    && state.view_state.mode == crate::view_mode::ViewMode::Overview => InputOutcome::OverviewFilterRunning,

KeyEvent {
    code: KeyCode::Char('a'),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
    && state.view_state.mode == crate::view_mode::ViewMode::Overview => InputOutcome::OverviewFilterAll,

// Overview: page navigation
KeyEvent {
    code: KeyCode::Char '['),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
    && state.view_state.mode == crate::view_mode::ViewMode::Overview => InputOutcome::OverviewPageUp,

KeyEvent {
    code: KeyCode::Char(']'),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::NONE) && state.composer.is_empty()
    && state.view_state.mode == crate::view_mode::ViewMode::Overview => InputOutcome::OverviewPageDown,
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --package agent-tui overview_f_filter`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add tui/src/input.rs
git commit -m "feat(tui): add Overview mode input handling"
```

---

## Task 6: Implement Overview Rendering

**Files:**
- Modify: `tui/src/render.rs`
- Test: `tui/src/render.rs` (snapshot tests)

**Context:** Implement `render_overview_view` function with layout: agent status list (fixed N rows) + scroll log area + composer.

- [ ] **Step 1: Add render_overview_view skeleton**

```rust
// In tui/src/render.rs, after render_task_matrix_view

/// Render Overview view - multi-agent coordination
fn render_overview_view(frame: &mut Frame<'_>, state: &mut TuiState) {
    let agent_list_height = state.view_state.overview.agent_list_rows as u16;
    let composer_height = state.composer.desired_height(frame.area().width, 8);

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(agent_list_height), // Agent status list
            Constraint::Min(1),                    // Scroll log area
            Constraint::Length(composer_height),   // Composer
            Constraint::Length(1),                 // Footer
        ])
        .split(frame.area());

    render_overview_agent_list(frame, state, areas[0]);
    render_overview_scroll_log(frame, state, areas[1]);
    render_composer(frame, state, areas[2]);
    render_overview_footer(frame, state, areas[3]);
}
```

- [ ] **Step 2: Add render_overview_agent_list**

```rust
fn render_overview_agent_list(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());

    let statuses = state.agent_statuses();
    let focused_index = state.view_state.overview.focused_agent_index;
    let filter = state.view_state.overview.filter;

    // Apply filter
    let filtered: Vec<(usize, &AgentStatusSnapshot)> = statuses
        .iter()
        .enumerate()
        .filter(|(_, s)| match filter {
            crate::overview_state::OverviewFilter::All => true,
            crate::overview_state::OverviewFilter::BlockedOnly => s.status.is_blocked(),
            crate::overview_state::OverviewFilter::RunningOnly => s.status.is_active(),
        })
        .collect();

    // Build lines for each agent row
    let mut lines = Vec::new();
    let max_width = area.width as usize;

    for (row_idx, (original_idx, snapshot)) in filtered.iter().enumerate() {
        let is_focused = row_idx == focused_index;
        let mut row = crate::overview_row::OverviewAgentRow::from_snapshot(snapshot, is_focused);
        row.truncate_to(max_width);

        let style = if is_focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(Span::styled(row.truncated, style)));

        // Stop when we've filled the list area
        if lines.len() >= area.height as usize {
            break;
        }
    }

    // Fill remaining rows with empty lines (empty state handling)
    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }

    // If no agents, show hint
    if filtered.is_empty() {
        lines[0] = Line::from(Span::styled(
            "◎ OVERVIEW idle Coordinating Agent work",
            Style::default().fg(Color::White),
        ));
        if lines.len() > 1 {
            lines[area.height as usize - 1] = Line::from(Span::styled(
                "Hint: Press Ctrl+N to create a new Agent",
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 3: Add render_overview_scroll_log**

```rust
fn render_overview_scroll_log(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default());

    let log_buffer = &state.view_state.overview.log_buffer;
    let scroll_offset = state.view_state.overview.log_scroll_offset;

    // Build log lines
    let mut lines = Vec::new();
    for msg in log_buffer.iter().skip(scroll_offset) {
        let timestamp_str = format_time_from_u32(msg.timestamp);
        let indicator = msg.message_type.indicator();

        let color = match msg.message_type {
            crate::overview_state::OverviewMessageType::Blocked => Color::Yellow,
            crate::overview_state::OverviewMessageType::Complete => Color::Green,
            crate::overview_state::OverviewMessageType::Quick => Color::Cyan,
            _ => Color::Gray,
        };

        lines.push(Line::from(vec![
            Span::styled(format!("[{}]", timestamp_str), Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(indicator, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(&msg.agent, Style::default().fg(Color::White)),
            Span::raw(": "),
            Span::styled(&msg.content, Style::default().fg(Color::Gray)),
        ]));

        if lines.len() >= area.height as usize {
            break;
        }
    }

    // If no messages, show placeholder
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No activity yet. Agents will report progress here.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    state.transcript_viewport_height = area.height;
    let max_scroll = log_buffer.len().saturating_sub(area.height as usize);
    // Update scroll offset tracking for follow-tail behavior

    let paragraph = Paragraph::new(lines).scroll((
        scroll_offset.min(u16::MAX as usize) as u16,
        0,
    ));
    frame.render_widget(paragraph, area);
}

fn format_time_from_u32(time: u32) -> String {
    let hours = time / 10000;
    let mins = (time % 10000) / 100;
    let secs = time % 100;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}
```

- [ ] **Step 4: Add render_overview_footer**

```rust
fn render_overview_footer(frame: &mut Frame<'_>, state: &TuiState, area: Rect) {
    if area.height == 0 {
        return;
    }
    fill_background(frame, area, Style::default().bg(Color::Rgb(28, 31, 38)));

    let filter_label = match state.view_state.overview.filter {
        crate::overview_state::OverviewFilter::All => "all",
        crate::overview_state::OverviewFilter::BlockedOnly => "blocked",
        crate::overview_state::OverviewFilter::RunningOnly => "running",
    };

    let hint = format!(
        "Overview | filter:{} | Tab:focus | f/r/a:filter | [/]:page | Ctrl+N:spawn | Ctrl+X:stop",
        filter_label
    );

    let line = Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 5: Update render_app to call render_overview_view**

```rust
// In render_app match block, add:

match state.view_state.mode {
    ViewMode::Focused => render_focused_view(frame, state),
    ViewMode::Split => render_split_view(frame, state),
    ViewMode::Dashboard => render_dashboard_view(frame, state),
    ViewMode::Mail => render_mail_view(frame, state),
    ViewMode::TaskMatrix => render_task_matrix_view(frame, state),
    ViewMode::Overview => render_overview_view(frame, state),
}
```

- [ ] **Step 6: Run compilation test**

Run: `cargo build --package agent-tui`
Expected: PASS (no compilation errors)

- [ ] **Step 7: Commit**

```bash
git add tui/src/render.rs
git commit -m "feat(tui): implement Overview view rendering"
```

---

## Task 7: Handle InputOutcome in app_loop

**Files:**
- Modify: `tui/src/app_loop.rs`
- Test: `tui/src/app_loop.rs` (integration tests)

**Context:** Handle Overview-specific InputOutcome variants in the main loop.

- [ ] **Step 1: Add handling for Overview outcomes**

```rust
// In app_loop.rs run() function, after handle_key_event call

match handle_key_event(&mut state, key_event) {
    // ... existing outcomes ...
    InputOutcome::OverviewFilterBlocked => {
        state.view_state.overview.filter = crate::overview_state::OverviewFilter::BlockedOnly;
        state.view_state.overview.clamp_focus(filtered_agent_count(&state));
    }
    InputOutcome::OverviewFilterRunning => {
        state.view_state.overview.filter = crate::overview_state::OverviewFilter::RunningOnly;
        state.view_state.overview.clamp_focus(filtered_agent_count(&state));
    }
    InputOutcome::OverviewFilterAll => {
        state.view_state.overview.filter = crate::overview_state::OverviewFilter::All;
        state.view_state.overview.clamp_focus(filtered_agent_count(&state));
    }
    InputOutcome::OverviewPageUp => {
        let total_pages = calculate_total_pages(&state);
        state.view_state.overview.page_up(total_pages);
    }
    InputOutcome::OverviewPageDown => {
        let total_pages = calculate_total_pages(&state);
        state.view_state.overview.page_down(total_pages);
    }
    // ... rest
}
```

- [ ] **Step 2: Add helper functions**

```rust
// In app_loop.rs

fn filtered_agent_count(state: &TuiState) -> usize {
    let statuses = state.agent_statuses();
    let filter = state.view_state.overview.filter;
    statuses.iter().filter(|s| match filter {
        crate::overview_state::OverviewFilter::All => true,
        crate::overview_state::OverviewFilter::BlockedOnly => s.status.is_blocked(),
        crate::overview_state::OverviewFilter::RunningOnly => s.status.is_active(),
    }).count()
}

fn calculate_total_pages(state: &TuiState) -> usize {
    let total_agents = filtered_agent_count(state);
    let rows_per_page = state.view_state.overview.agent_list_rows;
    if rows_per_page == 0 { return 0; }
    (total_agents + rows_per_page - 1) / rows_per_page
}
```

- [ ] **Step 3: Run compilation test**

Run: `cargo build --package agent-tui`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tui/src/app_loop.rs
git commit -m "feat(tui): handle Overview input outcomes in app loop"
```

---

## Task 8: Implement Scroll Log Message Aggregation

**Files:**
- Modify: `tui/src/ui_state.rs`
- Modify: `tui/src/app_loop.rs`
- Test: Integration test

**Context:** Aggregate events from all agents into the scroll log buffer. This happens in the event polling phase.

- [ ] **Step 1: Add log message generation from provider events**

```rust
// In tui/src/app_loop.rs, in the event polling section

// When polling events from EventAggregator, generate log messages
if let Some(event) = state.event_aggregator.poll() {
    // ... existing event handling ...

    // Generate Overview log message for significant events
    if state.view_state.mode == ViewMode::Overview {
        let log_msg = generate_overview_log_message(&event, &state);
        if let Some(msg) = log_msg {
            state.view_state.overview.push_log_message(msg);
        }
    }
}
```

- [ ] **Step 2: Add generate_overview_log_message helper**

```rust
// In tui/src/app_loop.rs

use crate::overview_state::{OverviewLogMessage, OverviewMessageType};

fn generate_overview_log_message(event: &ProviderEvent, state: &TuiState) -> Option<OverviewLogMessage> {
    let timestamp = current_time_as_u32();

    match event {
        ProviderEvent::StatusChange { agent_id, new_status } => {
            let codename = state.agent_pool.as_ref()
                .and_then(|p| p.get_slot_by_id(agent_id))
                .map(|s| s.codename().as_str().to_string())
                .unwrap_or_else(|| agent_id.as_str().to_string());

            let (msg_type, content) = match new_status {
                AgentSlotStatus::Responding { .. } => {
                    (OverviewMessageType::Progress, "Started working".to_string())
                }
                AgentSlotStatus::Idle => {
                    (OverviewMessageType::Complete, "Task complete".to_string())
                }
                AgentSlotStatus::Blocked { reason } => {
                    (OverviewMessageType::Blocked, format!("BLOCKED - {}", reason))
                }
                AgentSlotStatus::Error { message } => {
                    (OverviewMessageType::Blocked, format!("ERROR - {}", message))
                }
                _ => return None,
            };

            Some(OverviewLogMessage {
                timestamp,
                agent: codename,
                message_type: msg_type,
                content,
            })
        }
        ProviderEvent::ToolCall { agent_id, tool_name, .. } => {
            let codename = state.agent_pool.as_ref()
                .and_then(|p| p.get_slot_by_id(agent_id))
                .map(|s| s.codename().as_str().to_string())
                .unwrap_or_else(|| agent_id.as_str().to_string());

            Some(OverviewLogMessage {
                timestamp,
                agent: codename,
                message_type: OverviewMessageType::Progress,
                content: format!("Using tool: {}", tool_name),
            })
        }
        _ => None,
    }
}

fn current_time_as_u32() -> u32 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let secs_part = secs % 60;
    hours * 10000 + mins * 100 + secs_part
}
```

- [ ] **Step 3: Run compilation test**

Run: `cargo build --package agent-tui`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tui/src/app_loop.rs
git commit -m "feat(tui): aggregate agent events into Overview scroll log"
```

---

## Task 9: Add @ Command Routing

**Files:**
- Modify: `tui/src/app_loop.rs`
- Modify: `tui/src/ui_state.rs`
- Test: `tui/src/app_loop.rs`

**Context:** Parse @ commands before routing: `@alpha hello` → send to alpha; `@alpha,bravo hi` → broadcast to multiple.

- [ ] **Step 1: Add @ command parsing function**

```rust
// In tui/src/ui_state.rs or new tui/src/at_command.rs

/// Parsed @ command result
#[derive(Debug, Clone)]
pub enum AtCommandResult {
    /// Send to single agent
    Single { agent: String, message: String },
    /// Broadcast to multiple agents
    Broadcast { agents: Vec<String>, message: String },
    /// No @ command, normal input
    Normal(String),
    /// Malformed @ command
    Invalid { error: String },
}

/// Parse input for @ command syntax
pub fn parse_at_command(input: &str) -> AtCommandResult {
    if !input.starts_with('@') {
        return AtCommandResult::Normal(input.to_string());
    }

    // Find the message part after agents
    let parts: Vec<&str> = input[1..].splitn(2, ' ').collect();
    if parts.len() < 2 {
        return AtCommandResult::Invalid {
            error: "Missing message after agent name".to_string(),
        };
    }

    let agent_spec = parts[0];
    let message = parts[1].trim().to_string();

    // Support comma-separated: @alpha,bravo
    // Support space-separated: @alpha @bravo (handled recursively)
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

    if agents.len() == 1 {
        AtCommandResult::Single {
            agent: agents[0],
            message,
        }
    } else {
        AtCommandResult::Broadcast {
            agents,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_agent() {
        let result = parse_at_command("@alpha hello world");
        assert!(matches!(result, AtCommandResult::Single { agent, message }
            if agent == "alpha" && message == "hello world"));
    }

    #[test]
    fn parse_comma_separated() {
        let result = parse_at_command("@alpha,bravo hello");
        assert!(matches!(result, AtCommandResult::Broadcast { agents, message }
            if agents == vec!["alpha", "bravo"] && message == "hello"));
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
}
```

- [ ] **Step 2: Use @ command parser in input submission**

```rust
// In tui/src/app_loop.rs, in Submit handling

InputOutcome::Submit(text) => {
    let parsed = parse_at_command(&text);

    match parsed {
        AtCommandResult::Normal(msg) => {
            // Existing behavior: send to focused agent
            start_provider_request(&mut state, msg, &mut provider_rx);
        }
        AtCommandResult::Single { agent, message } => {
            // Find agent by codename and send
            if let Some(agent_id) = find_agent_by_codename(&state, &agent) {
                state.focus_agent_by_id(&agent_id);
                start_provider_request(&mut state, message, &mut provider_rx);
                state.app_mut().push_status_message(format!("Sent to {}", agent));
            } else {
                state.app_mut().push_error_message(format!("Agent '{}' not found", agent));
            }
        }
        AtCommandResult::Broadcast { agents, message } => {
            // Send to multiple agents (via mailbox)
            for agent_name in &agents {
                if let Some(agent_id) = find_agent_by_codename(&state, agent_name) {
                    state.mailbox.send_mail(
                        state.focused_agent_id().unwrap_or_default(),
                        agent_id.clone(),
                        "Broadcast".to_string(),
                        message.clone(),
                    );
                }
            }
            state.app_mut().push_status_message(format!("Broadcasted to {} agents", agents.len()));
        }
        AtCommandResult::Invalid { error } => {
            state.app_mut().push_error_message(format!("Invalid @ command: {}", error));
        }
    }
}
```

- [ ] **Step 3: Add find_agent_by_codename helper**

```rust
// In tui/src/app_loop.rs

fn find_agent_by_codename(state: &TuiState, codename: &str) -> Option<AgentId> {
    state.agent_pool.as_ref()
        .and_then(|p| {
            p.agent_statuses()
                .iter()
                .find(|s| s.codename.as_str() == codename)
                .map(|s| s.agent_id.clone())
        })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package agent-tui parse_at_command`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tui/src/ui_state.rs tui/src/app_loop.rs
git commit -m "feat(tui): add @ command routing for Overview mode"
```

---

## Task 10: Add Alt+6 Key Binding for Overview Mode

**Files:**
- Modify: `tui/src/input.rs`
- Test: `tui/src/input.rs`

**Context:** Add Alt+6 key binding to switch to Overview mode directly.

- [ ] **Step 1: Write the failing test**

```rust
// In tui/src/input.rs tests module

#[test]
fn alt_6_switches_to_overview_view() {
    let app = AppState::new(ProviderKind::Mock);
    let mut state = state_from_app(app);

    let outcome = handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('6'), KeyModifiers::ALT),
    );

    assert!(matches!(outcome, InputOutcome::SwitchViewMode(6)));
}
```

- [ ] **Step 2: Add Alt+6 key handling**

```rust
// In tui/src/input.rs handle_key_event, add after Alt+5:

KeyEvent {
    code: KeyCode::Char('6'),
    modifiers,
    ..
} if modifiers.contains(KeyModifiers::ALT) => InputOutcome::SwitchViewMode(6),
```

- [ ] **Step 3: Run test**

Run: `cargo test --package agent-tui alt_6_switches`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tui/src/input.rs
git commit -m "feat(tui): add Alt+6 key for Overview mode"
```

---

## Task 11: Add Focus Indicator for OVERVIEW Agent

**Files:**
- Modify: `tui/src/overview_row.rs`
- Test: `tui/src/overview_row.rs`

**Context:** The Overview Agent (OVERVIEW codename) has a special ◎ indicator and is always at the top. Add handling for this special agent.

- [ ] **Step 1: Add Overview agent indicator handling**

```rust
// In tui/src/overview_row.rs OverviewAgentRow::from_snapshot

pub fn from_snapshot(snapshot: &AgentStatusSnapshot, focused: bool, is_overview_agent: bool) -> Self {
    let indicator = if is_overview_agent {
        "◎" // Overview Agent always uses ◎
    } else {
        Self::status_indicator(&snapshot.status)
    };

    // ... rest of formatting
}
```

- [ ] **Step 2: Update render_overview_agent_list**

```rust
// In tui/src/render.rs render_overview_agent_list

// Check if we have an OVERVIEW agent (agent_id == "OVERVIEW" or special role)
let has_overview = statuses.iter().any(|s| s.role == AgentRole::ProductOwner);

// If Overview agent exists, render it first
if has_overview {
    let overview_snapshot = statuses.iter().find(|s| s.role == AgentRole::ProductOwner).unwrap();
    let mut row = OverviewAgentRow::from_snapshot(overview_snapshot, false, true);
    row.truncate_to(max_width);
    lines.push(Line::from(Span::styled(row.truncated, Style::default().fg(Color::White))));
}

// Then render worker agents
for (row_idx, (original_idx, snapshot)) in filtered.iter().enumerate() {
    if snapshot.role == AgentRole::ProductOwner {
        continue; // Already rendered above
    }
    // ... existing logic
}
```

- [ ] **Step 3: Run compilation test**

Run: `cargo build --package agent-tui`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tui/src/overview_row.rs tui/src/render.rs
git commit -m "feat(tui): add ◎ indicator for Overview Agent"
```

---

## Task 12: Integration Test for Overview Mode

**Files:**
- Create: `tui/src/overview_integration_test.rs`
- Test: `tui/src/overview_integration_test.rs`

**Context:** Comprehensive integration test for Overview mode functionality.

- [ ] **Step 1: Write integration test**

```rust
// Create tui/src/overview_integration_test.rs (or add to existing tests)

#[cfg(test)]
mod overview_integration_tests {
    use crate::ui_state::TuiState;
    use crate::view_mode::ViewMode;
    use crate::overview_state::{OverviewFilter, OverviewMessageType};
    use agent_core::app::AppState;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use tempfile::TempDir;

    fn make_multi_agent_state() -> TuiState {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        // Spawn multiple agents
        state.spawn_agent(ProviderKind::Mock);
        state.spawn_agent(ProviderKind::Mock);
        state.spawn_agent(ProviderKind::Mock);

        state
    }

    #[test]
    fn overview_mode_displays_all_agents() {
        let mut state = make_multi_agent_state();
        state.view_state.switch_by_number(6); // Overview mode

        let statuses = state.agent_statuses();
        assert!(statuses.len() >= 3);
    }

    #[test]
    fn overview_filter_blocked_shows_only_blocked() {
        let mut state = make_multi_agent_state();
        state.view_state.switch_by_number(6);
        state.view_state.overview.filter = OverviewFilter::BlockedOnly;

        // Manually block one agent
        if let Some(pool) = &mut state.agent_pool {
            if let Some(slot) = pool.get_slot_mut(1) {
                slot.transition_to(agent_core::agent_slot::AgentSlotStatus::blocked("test")).unwrap();
            }
        }

        let filtered = state.agent_statuses()
            .iter()
            .filter(|s| s.status.is_blocked())
            .count();

        assert_eq!(filtered, 1);
    }

    #[test]
    fn overview_log_buffer_accepts_messages() {
        let mut state = make_multi_agent_state();
        state.view_state.switch_by_number(6);

        state.view_state.overview.push_log_message(
            crate::overview_state::OverviewLogMessage {
                timestamp: 143215,
                agent: "alpha".to_string(),
                message_type: OverviewMessageType::Progress,
                content: "Started task".to_string(),
            }
        );

        assert_eq!(state.view_state.overview.log_buffer.len(), 1);
    }

    #[test]
    fn overview_focus_navigation_cycles() {
        let mut state = make_multi_agent_state();
        state.view_state.switch_by_number(6);

        let count = state.agent_statuses().len();
        state.view_state.overview.focused_agent_index = 0;
        state.view_state.overview.focus_next(count);
        assert_eq!(state.view_state.overview.focused_agent_index, 1);
        state.view_state.overview.focus_prev(count);
        assert_eq!(state.view_state.overview.focused_agent_index, 0);
    }
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test --package agent-tui overview_integration`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tui/tests/overview_integration_test.rs
git commit -m "test(tui): add Overview mode integration tests"
```

---

## Final Verification

- [ ] **Run all tests**

```bash
cargo test --workspace
```

Expected: All tests pass

- [ ] **Run clippy**

```bash
cargo clippy --workspace
```

Expected: No warnings

- [ ] **Run format**

```bash
cargo fmt -- --check
```

Expected: All files formatted

- [ ] **Manual testing checklist**
  - [ ] Start app with multiple agents
  - [ ] Press Alt+6 to enter Overview mode
  - [ ] Verify agent list displays correctly
  - [ ] Test Tab/Shift+Tab focus switching
  - [ ] Test f/r/a filter keys
  - [ ] Test [/] page navigation
  - [ ] Test Ctrl+N spawn agent
  - [ ] Test Ctrl+X stop agent
  - [ ] Test @ command routing
  - [ ] Verify scroll log updates with agent activity
  - [ ] Test on narrow terminal (truncation)
  - [ ] Test empty state (no agents)

---

## Summary

This plan implements the Overview display mode for multi-agent TUI, covering:
1. Blocked status addition to AgentSlotStatus
2. Overview ViewMode variant and state structure
3. Agent row formatting with truncation logic
4. Overview-specific input handling (filtering, search, pagination)
5. Rendering implementation (agent list, scroll log, footer)
6. Event aggregation for scroll log
7. @ command routing for directed/broadcast messages
8. Key bindings and focus management
9. Integration tests

The implementation follows TDD principles with tests before implementation, uses the existing infrastructure (AgentPool, AgentSlot, EventAggregator), and addresses the identified challenges (width truncation, memory bounds, focus consistency, @ command parsing).