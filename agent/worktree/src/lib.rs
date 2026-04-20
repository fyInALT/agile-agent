//! Git worktree management for multi-agent isolation
//!
//! Provides git worktree operations and workplace directory management.
//!
//! Note: workplace_store.rs remains in agent-core due to ShutdownSnapshot dependency.

pub mod logging;
pub mod worktree_manager;
pub mod worktree_state;
pub mod worktree_state_store;
pub mod git_flow_executor;
pub mod git_flow_config;

pub use worktree_manager::*;
pub use worktree_state::*;
pub use worktree_state_store::*;
pub use git_flow_executor::*;
pub use git_flow_config::*;