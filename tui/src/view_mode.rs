//! TUI View Mode System
//!
//! Provides different view modes for multi-agent workflow:
//! - Focused: Single agent transcript (default)
//! - Split: Two agents side by side
//! - Dashboard: All agents in compact cards
//! - Mail: Cross-agent communication focus
//! - TaskMatrix: Task assignment grid

/// View mode for TUI display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Single agent transcript view (default)
    #[default]
    Focused,
    /// Two agents side by side
    Split,
    /// All agents in compact cards
    Dashboard,
    /// Mail/communication focus
    Mail,
    /// Task assignment grid
    TaskMatrix,
}

impl ViewMode {
    /// Get display label for this mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::Focused => "Focused",
            Self::Split => "Split",
            Self::Dashboard => "Dashboard",
            Self::Mail => "Mail",
            Self::TaskMatrix => "Tasks",
        }
    }

    /// Get key hint for this mode (Ctrl+V number)
    pub fn key_hint(&self) -> &'static str {
        match self {
            Self::Focused => "Ctrl+V 1",
            Self::Split => "Ctrl+V 2",
            Self::Dashboard => "Ctrl+V 3",
            Self::Mail => "Ctrl+V 4",
            Self::TaskMatrix => "Ctrl+V 5",
        }
    }

    /// Get mode from number key (1-5)
    pub fn from_number(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Focused),
            2 => Some(Self::Split),
            3 => Some(Self::Dashboard),
            4 => Some(Self::Mail),
            5 => Some(Self::TaskMatrix),
            _ => None,
        }
    }

    /// Cycle to next mode
    pub fn next(self) -> Self {
        match self {
            Self::Focused => Self::Split,
            Self::Split => Self::Dashboard,
            Self::Dashboard => Self::Mail,
            Self::Mail => Self::TaskMatrix,
            Self::TaskMatrix => Self::Focused,
        }
    }

    /// Cycle to previous mode
    pub fn prev(self) -> Self {
        match self {
            Self::Focused => Self::TaskMatrix,
            Self::Split => Self::Focused,
            Self::Dashboard => Self::Split,
            Self::Mail => Self::Dashboard,
            Self::TaskMatrix => Self::Mail,
        }
    }

    /// Check if this mode shows multiple agents
    pub fn shows_multiple_agents(self) -> bool {
        matches!(self, Self::Split | Self::Dashboard | Self::TaskMatrix)
    }

    /// Check if this mode focuses on mail
    pub fn focuses_on_mail(self) -> bool {
        matches!(self, Self::Mail)
    }
}

/// State specific to split view mode
#[derive(Debug, Clone)]
pub struct SplitViewState {
    /// Index of left agent in pool
    pub left_agent_index: usize,
    /// Index of right agent in pool
    pub right_agent_index: usize,
    /// Which side is currently focused (0=left, 1=right)
    pub focused_side: usize,
    /// Split ratio (0.0-1.0, where 0.5 is equal)
    pub split_ratio: f32,
}

impl Default for SplitViewState {
    fn default() -> Self {
        Self {
            left_agent_index: 0,
            right_agent_index: 1,
            focused_side: 0,
            split_ratio: 0.5,
        }
    }
}

impl SplitViewState {
    /// Create new split view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Swap left and right agents
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.left_agent_index, &mut self.right_agent_index);
    }

    /// Focus on left side
    pub fn focus_left(&mut self) {
        self.focused_side = 0;
    }

    /// Focus on right side
    pub fn focus_right(&mut self) {
        self.focused_side = 1;
    }

    /// Toggle focused side
    pub fn toggle_focus(&mut self) {
        self.focused_side = if self.focused_side == 0 { 1 } else { 0 };
    }

    /// Set equal split (50/50)
    pub fn equal_split(&mut self) {
        self.split_ratio = 0.5;
    }

    /// Get focused agent index
    pub fn focused_agent_index(&self) -> usize {
        if self.focused_side == 0 {
            self.left_agent_index
        } else {
            self.right_agent_index
        }
    }

    /// Adjust split ratio for width
    pub fn adjust_for_width(&mut self, width: u16) {
        // On narrow screens, give more space to focused side
        if width < 100 {
            if self.focused_side == 0 {
                self.split_ratio = 0.6;
            } else {
                self.split_ratio = 0.4;
            }
        }
    }
}

/// State specific to dashboard view mode
#[derive(Debug, Clone)]
pub struct DashboardViewState {
    /// Currently selected card index
    pub selected_card_index: usize,
    /// Number of cards per row (responsive)
    pub cards_per_row: usize,
    /// Scroll offset (row offset for cards that overflow)
    pub scroll_offset: usize,
}

impl Default for DashboardViewState {
    fn default() -> Self {
        Self {
            selected_card_index: 0,
            cards_per_row: 3,
            scroll_offset: 0,
        }
    }
}

impl DashboardViewState {
    /// Create new dashboard view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up one row
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down one row
    pub fn scroll_down(&mut self, total_rows: usize) {
        if self.scroll_offset < total_rows.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Ensure selected card is visible (adjust scroll if needed)
    pub fn ensure_selected_visible(&mut self, cards_per_row: usize, visible_rows: usize) {
        let selected_row = self.selected_card_index / cards_per_row;
        // Scroll up if selected is above visible area
        if selected_row < self.scroll_offset {
            self.scroll_offset = selected_row;
        }
        // Scroll down if selected is below visible area
        if selected_row >= self.scroll_offset + visible_rows {
            self.scroll_offset = selected_row - visible_rows.saturating_sub(1);
        }
    }

    /// Select next card
    pub fn select_next(&mut self, total_cards: usize) {
        if total_cards > 0 && self.selected_card_index < total_cards - 1 {
            self.selected_card_index += 1;
        }
    }

    /// Select previous card
    pub fn select_prev(&mut self) {
        if self.selected_card_index > 0 {
            self.selected_card_index -= 1;
        }
    }

    /// Select card by number key (1-9)
    pub fn select_by_number(&mut self, n: u8, total_cards: usize) {
        let index = (n as usize).saturating_sub(1);
        if index < total_cards {
            self.selected_card_index = index;
        }
    }

    /// Adjust cards per row based on width
    pub fn adjust_for_width(&mut self, width: u16) {
        self.cards_per_row = if width < 80 {
            1
        } else if width < 120 {
            2
        } else if width < 160 {
            3
        } else {
            4
        };
    }
}

/// State specific to mail view mode
#[derive(Debug, Clone)]
pub struct MailViewState {
    /// Currently selected mail index
    pub selected_mail_index: usize,
    /// Currently viewing agent's inbox
    pub viewing_agent_index: usize,
    /// Compose mode active
    pub composing: bool,
    /// Compose text buffer
    pub compose_buffer: String,
}

impl Default for MailViewState {
    fn default() -> Self {
        Self {
            selected_mail_index: 0,
            viewing_agent_index: 0,
            composing: false,
            compose_buffer: String::new(),
        }
    }
}

impl MailViewState {
    /// Create new mail view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Clamp selected_mail_index to valid range
    pub fn clamp_selection(&mut self, total_mails: usize) {
        if total_mails == 0 {
            self.selected_mail_index = 0;
        } else if self.selected_mail_index >= total_mails {
            self.selected_mail_index = total_mails - 1;
        }
    }

    /// Select next mail
    pub fn select_next(&mut self, total_mails: usize) {
        if total_mails > 0 && self.selected_mail_index < total_mails - 1 {
            self.selected_mail_index += 1;
        }
    }

    /// Select previous mail
    pub fn select_prev(&mut self) {
        if self.selected_mail_index > 0 {
            self.selected_mail_index -= 1;
        }
    }

    /// Start composing new mail
    pub fn start_compose(&mut self) {
        self.composing = true;
        self.compose_buffer.clear();
    }

    /// Cancel composing
    pub fn cancel_compose(&mut self) {
        self.composing = false;
        self.compose_buffer.clear();
    }

    /// Append character to compose buffer
    pub fn append_char(&mut self, c: char) {
        self.compose_buffer.push(c);
    }

    /// Remove last character from compose buffer
    pub fn remove_char(&mut self) {
        self.compose_buffer.pop();
    }
}

/// State specific to task matrix view mode
#[derive(Debug, Clone)]
pub struct TaskMatrixViewState {
    /// Currently selected row (task)
    pub selected_row: usize,
    /// Currently selected column (agent)
    pub selected_column: usize,
}

impl Default for TaskMatrixViewState {
    fn default() -> Self {
        Self {
            selected_row: 0,
            selected_column: 0,
        }
    }
}

impl TaskMatrixViewState {
    /// Create new task matrix view state
    pub fn new() -> Self {
        Self::default()
    }

    /// Move selection up
    pub fn move_up(&mut self, total_rows: usize) {
        if total_rows > 0 && self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self, total_rows: usize) {
        if total_rows > 0 && self.selected_row < total_rows - 1 {
            self.selected_row += 1;
        }
    }

    /// Move selection left
    pub fn move_left(&mut self) {
        if self.selected_column > 0 {
            self.selected_column -= 1;
        }
    }

    /// Move selection right
    pub fn move_right(&mut self, total_columns: usize) {
        if total_columns > 0 && self.selected_column < total_columns - 1 {
            self.selected_column += 1;
        }
    }
}

/// Combined view state for all modes
#[derive(Debug, Clone, Default)]
pub struct TuiViewState {
    /// Current view mode
    pub mode: ViewMode,
    /// Split view specific state
    pub split: SplitViewState,
    /// Dashboard view specific state
    pub dashboard: DashboardViewState,
    /// Mail view specific state
    pub mail: MailViewState,
    /// Task matrix view specific state
    pub task_matrix: TaskMatrixViewState,
}

impl TuiViewState {
    /// Create new view state with default mode
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to a specific mode
    pub fn switch_to(&mut self, mode: ViewMode) {
        self.mode = mode;
    }

    /// Switch to next mode
    pub fn next_mode(&mut self) {
        self.mode = self.mode.next();
    }

    /// Switch to previous mode
    pub fn prev_mode(&mut self) {
        self.mode = self.mode.prev();
    }

    /// Switch to mode by number
    pub fn switch_by_number(&mut self, n: u8) {
        if let Some(mode) = ViewMode::from_number(n) {
            self.mode = mode;
        }
    }

    /// Adjust all mode states for terminal width
    pub fn adjust_for_width(&mut self, width: u16) {
        self.split.adjust_for_width(width);
        self.dashboard.adjust_for_width(width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_mode_labels() {
        assert_eq!(ViewMode::Focused.label(), "Focused");
        assert_eq!(ViewMode::Split.label(), "Split");
        assert_eq!(ViewMode::Dashboard.label(), "Dashboard");
        assert_eq!(ViewMode::Mail.label(), "Mail");
        assert_eq!(ViewMode::TaskMatrix.label(), "Tasks");
    }

    #[test]
    fn view_mode_key_hints() {
        assert_eq!(ViewMode::Focused.key_hint(), "Ctrl+V 1");
        assert_eq!(ViewMode::Split.key_hint(), "Ctrl+V 2");
        assert_eq!(ViewMode::Dashboard.key_hint(), "Ctrl+V 3");
    }

    #[test]
    fn view_mode_from_number() {
        assert_eq!(ViewMode::from_number(1), Some(ViewMode::Focused));
        assert_eq!(ViewMode::from_number(3), Some(ViewMode::Dashboard));
        assert_eq!(ViewMode::from_number(6), None);
    }

    #[test]
    fn view_mode_cycle() {
        assert_eq!(ViewMode::Focused.next(), ViewMode::Split);
        assert_eq!(ViewMode::TaskMatrix.next(), ViewMode::Focused);
        assert_eq!(ViewMode::Focused.prev(), ViewMode::TaskMatrix);
    }

    #[test]
    fn view_mode_multi_agent_check() {
        assert!(!ViewMode::Focused.shows_multiple_agents());
        assert!(ViewMode::Split.shows_multiple_agents());
        assert!(ViewMode::Dashboard.shows_multiple_agents());
    }

    #[test]
    fn split_view_swap() {
        let mut state = SplitViewState::new();
        state.left_agent_index = 0;
        state.right_agent_index = 1;
        state.swap();
        assert_eq!(state.left_agent_index, 1);
        assert_eq!(state.right_agent_index, 0);
    }

    #[test]
    fn split_view_toggle_focus() {
        let mut state = SplitViewState::new();
        assert_eq!(state.focused_side, 0);
        state.toggle_focus();
        assert_eq!(state.focused_side, 1);
        state.toggle_focus();
        assert_eq!(state.focused_side, 0);
    }

    #[test]
    fn split_view_adjust_for_width() {
        let mut state = SplitViewState::new();
        state.focused_side = 0;
        state.adjust_for_width(80);
        assert_eq!(state.split_ratio, 0.6); // More space for focused side

        state.focused_side = 1;
        state.adjust_for_width(80);
        assert_eq!(state.split_ratio, 0.4);
    }

    #[test]
    fn dashboard_select_next() {
        let mut state = DashboardViewState::new();
        state.select_next(5);
        assert_eq!(state.selected_card_index, 1);
        state.select_next(5);
        state.select_next(5);
        state.select_next(5);
        state.select_next(5); // Should stop at 4 (last)
        assert_eq!(state.selected_card_index, 4);
    }

    #[test]
    fn dashboard_select_by_number() {
        let mut state = DashboardViewState::new();
        state.select_by_number(3, 5);
        assert_eq!(state.selected_card_index, 2);
    }

    #[test]
    fn dashboard_adjust_for_width() {
        let mut state = DashboardViewState::new();
        state.adjust_for_width(60);
        assert_eq!(state.cards_per_row, 1);
        state.adjust_for_width(100);
        assert_eq!(state.cards_per_row, 2);
        state.adjust_for_width(140);
        assert_eq!(state.cards_per_row, 3);
        state.adjust_for_width(180);
        assert_eq!(state.cards_per_row, 4);
    }

    #[test]
    fn dashboard_scroll() {
        let mut state = DashboardViewState::new();
        state.scroll_offset = 0;

        // Scroll down
        state.scroll_down(5);
        assert_eq!(state.scroll_offset, 1);
        state.scroll_down(5);
        state.scroll_down(5);
        state.scroll_down(5); // Should stop at max
        assert_eq!(state.scroll_offset, 4);

        // Scroll up
        state.scroll_up();
        assert_eq!(state.scroll_offset, 3);
    }

    #[test]
    fn dashboard_ensure_selected_visible() {
        let mut state = DashboardViewState::new();
        state.cards_per_row = 3;

        // Select card in row 5, visible rows = 2
        state.selected_card_index = 15; // Row 5 (15 / 3 = 5)
        state.ensure_selected_visible(3, 2);
        assert_eq!(state.scroll_offset, 4); // Scroll so row 5 is visible (4 + 1 visible row)

        // Select card above visible area
        state.selected_card_index = 3; // Row 1
        state.ensure_selected_visible(3, 2);
        assert_eq!(state.scroll_offset, 1);
    }

    #[test]
    fn mail_view_compose() {
        let mut state = MailViewState::new();
        state.start_compose();
        assert!(state.composing);
        state.append_char('H');
        state.append_char('i');
        assert_eq!(state.compose_buffer, "Hi");
        state.remove_char();
        assert_eq!(state.compose_buffer, "H");
        state.cancel_compose();
        assert!(!state.composing);
        assert_eq!(state.compose_buffer, "");
    }

    #[test]
    fn mail_view_clamp_selection() {
        let mut state = MailViewState::new();
        state.selected_mail_index = 5;

        // Clamp to smaller inbox
        state.clamp_selection(3);
        assert_eq!(state.selected_mail_index, 2);

        // Clamp to empty inbox
        state.clamp_selection(0);
        assert_eq!(state.selected_mail_index, 0);

        // Clamp when already in range
        state.selected_mail_index = 1;
        state.clamp_selection(5);
        assert_eq!(state.selected_mail_index, 1);
    }

    #[test]
    fn task_matrix_navigation() {
        let mut state = TaskMatrixViewState::new();
        state.move_down(3);
        state.move_down(3);
        assert_eq!(state.selected_row, 2);
        state.move_right(4);
        state.move_right(4);
        state.move_right(4);
        assert_eq!(state.selected_column, 3);
    }

    #[test]
    fn tui_view_state_switch() {
        let mut state = TuiViewState::new();
        assert_eq!(state.mode, ViewMode::Focused);
        state.switch_to(ViewMode::Split);
        assert_eq!(state.mode, ViewMode::Split);
        state.next_mode();
        assert_eq!(state.mode, ViewMode::Dashboard);
        state.switch_by_number(4);
        assert_eq!(state.mode, ViewMode::Mail);
    }
}