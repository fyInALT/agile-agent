//! Resume session overlay for shutdown snapshot restore
//!
//! Provides UI for choosing how to restore a previous session.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_core::shutdown_snapshot::ShutdownReason;
use agent_core::shutdown_snapshot::ShutdownSnapshot;

/// Resume session overlay state
#[derive(Debug, Clone)]
pub struct ResumeOverlay {
    /// Snapshot being offered for restore
    snapshot: ShutdownSnapshot,
    /// Currently selected option index
    pub selected_index: usize,
    /// Available options
    pub options: Vec<ResumeOption>,
}

/// Resume option choice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeOption {
    /// Resume all agents from snapshot
    Resume,
    /// Start fresh but keep transcripts
    Fresh,
    /// Cancel restore and start completely clean
    Clean,
}

impl ResumeOption {
    /// Get the display label for this option
    pub fn label(&self) -> &'static str {
        match self {
            Self::Resume => "Resume all active agents",
            Self::Fresh => "Start fresh (keep transcripts)",
            Self::Clean => "Cancel restore, start clean",
        }
    }

    /// Get the key hint for this option
    pub fn key_hint(&self) -> &'static str {
        match self {
            Self::Resume => "[R]",
            Self::Fresh => "[S]",
            Self::Clean => "[C]",
        }
    }
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeCommand {
    /// Resume the session from snapshot
    Resume,
    /// Start fresh session
    StartFresh,
    /// Cancel restore and start clean
    CancelRestore,
}

impl ResumeOverlay {
    /// Create a new overlay from shutdown snapshot
    pub fn new(snapshot: ShutdownSnapshot) -> Self {
        Self {
            snapshot,
            selected_index: 0,
            options: vec![
                ResumeOption::Resume,
                ResumeOption::Fresh,
                ResumeOption::Clean,
            ],
        }
    }

    /// Get the shutdown snapshot
    pub fn snapshot(&self) -> &ShutdownSnapshot {
        &self.snapshot
    }

    /// Get the shutdown reason
    #[allow(dead_code)]
    pub fn shutdown_reason(&self) -> &ShutdownReason {
        &self.snapshot.shutdown_reason
    }

    /// Get agents that were active at shutdown
    pub fn agents_count(&self) -> usize {
        self.snapshot.agents.len()
    }

    /// Get the currently selected option
    pub fn selected_option(&self) -> ResumeOption {
        self.options
            .get(self.selected_index)
            .copied()
            .unwrap_or(ResumeOption::Clean)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index < self.options.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ResumeCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Char('r') | KeyCode::Char('R') => Some(ResumeCommand::Resume),
            KeyCode::Char('s') | KeyCode::Char('S') => Some(ResumeCommand::StartFresh),
            KeyCode::Char('c') | KeyCode::Char('C') => Some(ResumeCommand::CancelRestore),
            KeyCode::Up => {
                self.move_up();
                None
            }
            KeyCode::Down => {
                self.move_down();
                None
            }
            KeyCode::Enter => Some(self.selected_option().into_command()),
            KeyCode::Esc => Some(ResumeCommand::CancelRestore),
            _ => None,
        }
    }
}

impl ResumeOption {
    /// Convert option to command
    pub fn into_command(self) -> ResumeCommand {
        match self {
            Self::Resume => ResumeCommand::Resume,
            Self::Fresh => ResumeCommand::StartFresh,
            Self::Clean => ResumeCommand::CancelRestore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::agent_runtime::{
        AgentCodename, AgentId, AgentMeta, AgentStatus, ProviderType, WorkplaceId,
    };
    use agent_core::backlog::BacklogState;
    use agent_core::shutdown_snapshot::AgentShutdownSnapshot;

    fn make_meta() -> AgentMeta {
        AgentMeta {
            agent_id: AgentId::new("agent_001"),
            codename: AgentCodename::new("alpha"),
            workplace_id: WorkplaceId::new("test"),
            provider_type: ProviderType::Mock,
            provider_session_id: None,
            created_at: "2026-04-14T09:00:00Z".to_string(),
            updated_at: "2026-04-14T10:00:00Z".to_string(),
            status: AgentStatus::Running,
        }
    }

    fn make_snapshot() -> ShutdownSnapshot {
        ShutdownSnapshot::new(
            "test".to_string(),
            vec![AgentShutdownSnapshot {
                meta: make_meta(),
                assigned_task_id: Some("task-1".to_string()),
                was_active: true,
                had_error: false,
                provider_thread_state: None,
                captured_at: "2026-04-14T10:00:00Z".to_string(),
            }],
            BacklogState::default(),
            vec![], // no pending mail
            ShutdownReason::UserQuit,
        )
    }

    #[test]
    fn overlay_new_from_snapshot() {
        let snapshot = make_snapshot();
        let overlay = ResumeOverlay::new(snapshot);
        assert_eq!(overlay.agents_count(), 1);
        assert_eq!(overlay.selected_option(), ResumeOption::Resume);
    }

    #[test]
    fn r_key_returns_resume() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        let cmd = overlay.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(cmd, Some(ResumeCommand::Resume));
    }

    #[test]
    fn s_key_returns_start_fresh() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        let cmd = overlay.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(cmd, Some(ResumeCommand::StartFresh));
    }

    #[test]
    fn c_key_returns_cancel() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        let cmd = overlay.handle_key_event(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(cmd, Some(ResumeCommand::CancelRestore));
    }

    #[test]
    fn esc_returns_cancel() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        let cmd = overlay.handle_key_event(KeyEvent::from(KeyCode::Esc));
        assert_eq!(cmd, Some(ResumeCommand::CancelRestore));
    }

    #[test]
    fn enter_returns_selected_option_command() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        overlay.selected_index = 1; // Fresh
        let cmd = overlay.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(cmd, Some(ResumeCommand::StartFresh));
    }

    #[test]
    fn arrow_keys_change_selection() {
        let snapshot = make_snapshot();
        let mut overlay = ResumeOverlay::new(snapshot);
        assert_eq!(overlay.selected_option(), ResumeOption::Resume);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(overlay.selected_option(), ResumeOption::Fresh);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(overlay.selected_option(), ResumeOption::Clean);

        overlay.handle_key_event(KeyEvent::from(KeyCode::Up));
        assert_eq!(overlay.selected_option(), ResumeOption::Fresh);
    }
}
