//! SharedWorkplaceState for multi-agent runtime
//!
//! Contains state shared across all agents in a workplace.
//!
//! # Thread Safety Pattern
//!
//! This state is designed to be owned by the main thread (TUI loop):
//!
//! ```text
//! Main Thread (TUI Loop)
//! ┌─────────────────────────────────────────────────────────┐
//! │  SharedWorkplaceState (Arc-wrapped for potential sharing)│
//! │  - Backlog: todos, active_tasks                          │
//! │  - Skills: registry                                       │
//! │  - LoopControl: should_quit, iteration count             │
//! │  - Kanban: tasks, stories, sprints                        │
//! │                                                           │
//! │  All mutations happen HERE after receiving events        │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Access Pattern
//!
//! - Main thread: Direct mutable access via `Arc::get_mut()` or clone
//! - Provider threads: NEVER access this state directly
//! - Cross-thread: Provider sends events → Main thread updates state
//!
//! ## Future Interior Mutability
//!
//! For multi-agent task pickup scenarios, BacklogState may need `Mutex`:
//! - Agents check for available tasks
//! - Claim atomically without race conditions
//! - Currently handled by main thread assignment (no Mutex needed)

use std::sync::Arc;

use crate::backlog::BacklogState;
use crate::agent_runtime::WorkplaceId;
use crate::skills::SkillRegistry;
use agent_kanban::service::KanbanService;
use agent_kanban::file_repository::FileKanbanRepository;
use agent_kanban::events::KanbanEventBus;

/// Loop control flags shared across agents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopControlFlags {
    /// Whether any agent should quit
    pub should_quit: bool,
    /// Whether loop execution is paused
    pub loop_paused: bool,
    /// Whether the autonomous loop is actively running
    pub loop_run_active: bool,
    /// Maximum iterations allowed across all agents
    pub max_iterations: usize,
    /// Current iteration count (shared across agents)
    pub current_iteration: usize,
}

impl Default for LoopControlFlags {
    fn default() -> Self {
        Self {
            should_quit: false,
            loop_paused: false,
            loop_run_active: false,
            max_iterations: 100,
            current_iteration: 0,
        }
    }
}

impl LoopControlFlags {
    /// Create new control flags with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create new control flags with custom max iterations
    pub fn with_max_iterations(max: usize) -> Self {
        Self {
            max_iterations: max,
            ..Self::default()
        }
    }

    /// Check if more iterations are allowed
    pub fn can_iterate(&self) -> bool {
        !self.should_quit && !self.loop_paused && self.current_iteration < self.max_iterations
    }

    /// Check if autonomous loop is running
    pub fn is_loop_running(&self) -> bool {
        self.loop_run_active && !self.should_quit
    }

    /// Get remaining iterations (max - current)
    pub fn remaining_iterations(&self) -> usize {
        self.max_iterations.saturating_sub(self.current_iteration)
    }

    /// Start autonomous loop with max iterations
    pub fn start_loop(&mut self, max_iterations: usize) {
        self.loop_run_active = true;
        self.max_iterations = max_iterations;
        self.current_iteration = 0;
    }

    /// Stop autonomous loop
    pub fn stop_loop(&mut self) {
        self.loop_run_active = false;
    }

    /// Increment iteration count
    pub fn increment_iteration(&mut self) {
        self.current_iteration += 1;
    }

    /// Signal quit to all agents
    pub fn signal_quit(&mut self) {
        self.should_quit = true;
    }

    /// Pause loop execution
    pub fn pause(&mut self) {
        self.loop_paused = true;
    }

    /// Resume loop execution
    pub fn resume(&mut self) {
        self.loop_paused = false;
    }

    /// Reset iteration count
    pub fn reset_iterations(&mut self) {
        self.current_iteration = 0;
    }
}

/// State shared across all agents in a workplace
///
/// Contains workplace-wide data that all agents need access to,
/// including backlog, skills registry, kanban, and loop control.
#[derive(Debug)]
pub struct SharedWorkplaceState {
    /// Unique workplace identifier
    pub workplace_id: WorkplaceId,
    /// Shared backlog (todos and tasks)
    pub backlog: BacklogState,
    /// Skills registry (shared across agents)
    pub skills: SkillRegistry,
    /// Kanban service for task management (optional, requires workplace path)
    pub kanban: Option<Arc<KanbanService<FileKanbanRepository>>>,
    /// Loop control flags
    pub loop_control: LoopControlFlags,
}

impl Clone for SharedWorkplaceState {
    fn clone(&self) -> Self {
        Self {
            workplace_id: self.workplace_id.clone(),
            backlog: self.backlog.clone(),
            skills: self.skills.clone(),
            kanban: self.kanban.clone(),
            loop_control: self.loop_control.clone(),
        }
    }
}

impl SharedWorkplaceState {
    /// Create a new shared workplace state
    pub fn new(workplace_id: WorkplaceId) -> Self {
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills: SkillRegistry::default(),
            kanban: None,
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with custom backlog
    pub fn with_backlog(workplace_id: WorkplaceId, backlog: BacklogState) -> Self {
        Self {
            workplace_id,
            backlog,
            skills: SkillRegistry::default(),
            kanban: None,
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with custom skills
    pub fn with_skills(workplace_id: WorkplaceId, skills: SkillRegistry) -> Self {
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills,
            kanban: None,
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with kanban service from workplace path
    pub fn with_kanban(workplace_id: WorkplaceId, workplace_path: impl Into<std::path::PathBuf>) -> Self {
        let kanban = Self::create_kanban_service(&workplace_path.into());
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills: SkillRegistry::default(),
            kanban,
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with skills and kanban
    pub fn with_skills_and_kanban(
        workplace_id: WorkplaceId,
        skills: SkillRegistry,
        workplace_path: impl Into<std::path::PathBuf>,
    ) -> Self {
        let kanban = Self::create_kanban_service(&workplace_path.into());
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills,
            kanban,
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create KanbanService from workplace path
    fn create_kanban_service(workplace_path: &std::path::Path) -> Option<Arc<KanbanService<FileKanbanRepository>>> {
        let repo = FileKanbanRepository::from_workplace(workplace_path).ok()?;
        let event_bus = Arc::new(KanbanEventBus::new());
        Some(Arc::new(KanbanService::new(Arc::new(repo), event_bus)))
    }

    /// Initialize kanban service (if not already initialized)
    pub fn initialize_kanban(&mut self, workplace_path: impl Into<std::path::PathBuf>) {
        if self.kanban.is_none() {
            self.kanban = Self::create_kanban_service(&workplace_path.into());
        }
    }

    /// Get workplace ID
    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    /// Get backlog reference
    pub fn backlog(&self) -> &BacklogState {
        &self.backlog
    }

    /// Get backlog mutable reference
    pub fn backlog_mut(&mut self) -> &mut BacklogState {
        &mut self.backlog
    }

    /// Get skills registry reference
    pub fn skills(&self) -> &SkillRegistry {
        &self.skills
    }

    /// Get skills registry mutable reference
    pub fn skills_mut(&mut self) -> &mut SkillRegistry {
        &mut self.skills
    }

    /// Get kanban service reference
    pub fn kanban(&self) -> Option<&Arc<KanbanService<FileKanbanRepository>>> {
        self.kanban.as_ref()
    }

    /// Get loop control reference
    pub fn loop_control(&self) -> &LoopControlFlags {
        &self.loop_control
    }

    /// Get loop control mutable reference
    pub fn loop_control_mut(&mut self) -> &mut LoopControlFlags {
        &mut self.loop_control
    }

    /// Check if agents can continue running
    pub fn can_continue(&self) -> bool {
        self.loop_control.can_iterate()
    }

    /// Signal all agents to quit
    pub fn signal_quit(&mut self) {
        self.loop_control.signal_quit();
    }
}

/// Thread-safe wrapper for SharedWorkplaceState
///
/// Use Arc to share state across multiple threads (agents).
pub type SharedWorkplaceStateRef = Arc<SharedWorkplaceState>;

impl SharedWorkplaceState {
    /// Wrap in Arc for thread-safe sharing
    pub fn into_shared(self) -> SharedWorkplaceStateRef {
        Arc::new(self)
    }

    /// Create a new shared reference directly
    pub fn new_shared(workplace_id: WorkplaceId) -> SharedWorkplaceStateRef {
        Self::new(workplace_id).into_shared()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_control_default_allows_iteration() {
        let flags = LoopControlFlags::new();
        assert!(flags.can_iterate());
        assert!(!flags.should_quit);
        assert!(!flags.loop_paused);
    }

    #[test]
    fn loop_control_with_max_iterations() {
        let flags = LoopControlFlags::with_max_iterations(10);
        assert_eq!(flags.max_iterations, 10);
        assert!(flags.can_iterate());
    }

    #[test]
    fn loop_control_signal_quit_prevents_iteration() {
        let mut flags = LoopControlFlags::new();
        flags.signal_quit();
        assert!(flags.should_quit);
        assert!(!flags.can_iterate());
    }

    #[test]
    fn loop_control_pause_prevents_iteration() {
        let mut flags = LoopControlFlags::new();
        flags.pause();
        assert!(flags.loop_paused);
        assert!(!flags.can_iterate());
    }

    #[test]
    fn loop_control_resume_allows_iteration() {
        let mut flags = LoopControlFlags::new();
        flags.pause();
        flags.resume();
        assert!(!flags.loop_paused);
        assert!(flags.can_iterate());
    }

    #[test]
    fn loop_control_iteration_limit() {
        let mut flags = LoopControlFlags::with_max_iterations(2);
        assert!(flags.can_iterate());
        flags.increment_iteration();
        assert!(flags.can_iterate());
        flags.increment_iteration();
        assert!(!flags.can_iterate());
    }

    #[test]
    fn loop_control_reset_iterations() {
        let mut flags = LoopControlFlags::with_max_iterations(2);
        flags.increment_iteration();
        flags.increment_iteration();
        assert!(!flags.can_iterate());
        flags.reset_iterations();
        assert!(flags.can_iterate());
    }

    #[test]
    fn loop_control_start_loop_sets_active_and_max() {
        let mut flags = LoopControlFlags::new();
        flags.start_loop(10);
        assert!(flags.loop_run_active);
        assert_eq!(flags.max_iterations, 10);
        assert_eq!(flags.current_iteration, 0);
    }

    #[test]
    fn loop_control_stop_loop_clears_active() {
        let mut flags = LoopControlFlags::new();
        flags.start_loop(10);
        flags.stop_loop();
        assert!(!flags.loop_run_active);
    }

    #[test]
    fn loop_control_is_loop_running() {
        let mut flags = LoopControlFlags::new();
        assert!(!flags.is_loop_running());
        flags.start_loop(10);
        assert!(flags.is_loop_running());
        flags.signal_quit();
        assert!(!flags.is_loop_running());
    }

    #[test]
    fn loop_control_remaining_iterations() {
        let mut flags = LoopControlFlags::with_max_iterations(10);
        assert_eq!(flags.remaining_iterations(), 10);
        flags.increment_iteration();
        assert_eq!(flags.remaining_iterations(), 9);
        flags.increment_iteration();
        flags.increment_iteration();
        assert_eq!(flags.remaining_iterations(), 7);
    }

    #[test]
    fn loop_control_remaining_iterations_saturates() {
        let mut flags = LoopControlFlags::with_max_iterations(2);
        flags.increment_iteration();
        flags.increment_iteration();
        flags.increment_iteration(); // exceeds max
        assert_eq!(flags.remaining_iterations(), 0);
    }

    #[test]
    fn shared_workplace_new() {
        let state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        assert_eq!(state.workplace_id().as_str(), "wp-001");
        assert!(state.can_continue());
        assert!(state.kanban.is_none());
    }

    #[test]
    fn shared_workplace_signal_quit() {
        let mut state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        state.signal_quit();
        assert!(!state.can_continue());
    }

    #[test]
    fn shared_workplace_into_shared() {
        let state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        let shared = state.into_shared();
        assert_eq!(shared.workplace_id().as_str(), "wp-001");
    }

    #[test]
    fn shared_workplace_new_shared() {
        let shared = SharedWorkplaceState::new_shared(WorkplaceId::new("wp-001"));
        assert_eq!(shared.workplace_id().as_str(), "wp-001");
    }

    #[test]
    fn shared_workplace_backlog_access() {
        let mut state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        let backlog = state.backlog_mut();
        // Backlog is empty by default
        assert!(backlog.todos.is_empty());
    }

    #[test]
    fn shared_workplace_skills_access() {
        let state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        let skills = state.skills();
        // Skills registry is empty by default
        assert!(skills.is_empty());
    }

    #[test]
    fn shared_workplace_with_kanban() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let state = SharedWorkplaceState::with_kanban(
            WorkplaceId::new("wp-001"),
            temp.path(),
        );
        assert!(state.kanban.is_some());
        // Kanban service is initialized
        let kanban = state.kanban().unwrap();
        assert!(kanban.list_elements().unwrap().is_empty());
    }

    #[test]
    fn shared_workplace_initialize_kanban() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let mut state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        assert!(state.kanban.is_none());
        state.initialize_kanban(temp.path());
        assert!(state.kanban.is_some());
    }

    #[test]
    fn shared_workplace_initialize_kanban_once() {
        use tempfile::TempDir;
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();
        let mut state = SharedWorkplaceState::with_kanban(
            WorkplaceId::new("wp-001"),
            temp1.path(),
        );
        // Kanban already initialized, second call should not replace
        state.initialize_kanban(temp2.path());
        // Original kanban path is still used
        assert!(state.kanban.is_some());
    }

    #[test]
    fn shared_workplace_kanban_create_task() {
        use tempfile::TempDir;
        use agent_kanban::domain::{KanbanElement, ElementType};
        let temp = TempDir::new().unwrap();
        let state = SharedWorkplaceState::with_kanban(
            WorkplaceId::new("wp-001"),
            temp.path(),
        );
        let kanban = state.kanban().unwrap();
        let task = KanbanElement::new_task("Test Task");
        let created = kanban.create_element(task).unwrap();
        assert!(created.id().is_some());
        assert_eq!(created.element_type(), ElementType::Task);
    }

    #[test]
    fn shared_workplace_clone_preserves_kanban() {
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let state = SharedWorkplaceState::with_kanban(
            WorkplaceId::new("wp-001"),
            temp.path(),
        );
        let cloned = state.clone();
        assert!(cloned.kanban.is_some());
        // Both share the same Arc
        assert!(state.kanban.as_ref().unwrap().list_elements().unwrap().is_empty());
        assert!(cloned.kanban.as_ref().unwrap().list_elements().unwrap().is_empty());
    }
}