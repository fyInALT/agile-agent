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

/// Loop control flags shared across agents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopControlFlags {
    /// Whether any agent should quit
    pub should_quit: bool,
    /// Whether loop execution is paused
    pub loop_paused: bool,
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
/// including backlog, skills registry, and loop control.
#[derive(Debug, Clone)]
pub struct SharedWorkplaceState {
    /// Unique workplace identifier
    pub workplace_id: WorkplaceId,
    /// Shared backlog (todos and tasks)
    pub backlog: BacklogState,
    /// Skills registry (shared across agents)
    pub skills: SkillRegistry,
    /// Loop control flags
    pub loop_control: LoopControlFlags,
}

impl SharedWorkplaceState {
    /// Create a new shared workplace state
    pub fn new(workplace_id: WorkplaceId) -> Self {
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills: SkillRegistry::default(),
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with custom backlog
    pub fn with_backlog(workplace_id: WorkplaceId, backlog: BacklogState) -> Self {
        Self {
            workplace_id,
            backlog,
            skills: SkillRegistry::default(),
            loop_control: LoopControlFlags::new(),
        }
    }

    /// Create with custom skills
    pub fn with_skills(workplace_id: WorkplaceId, skills: SkillRegistry) -> Self {
        Self {
            workplace_id,
            backlog: BacklogState::default(),
            skills,
            loop_control: LoopControlFlags::new(),
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
    fn shared_workplace_new() {
        let state = SharedWorkplaceState::new(WorkplaceId::new("wp-001"));
        assert_eq!(state.workplace_id().as_str(), "wp-001");
        assert!(state.can_continue());
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
}