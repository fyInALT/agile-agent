//! State layer - state management
//!
//! Provides:
//! - DecisionAgentState (agent lifecycle)
//! - DecisionAgentConfig (creation policy, timeouts)
//! - BlockedState (blocking reasons)
//! - HumanDecisionQueue (human interaction management)
//! - Recovery mechanism
//! - GitState analysis for task preparation

pub mod lifecycle;
pub mod blocking;
pub mod recovery;
pub mod git_state;
pub mod uncommitted_handler;
pub mod commit_boundary;

pub use lifecycle::*;
pub use blocking::*;
pub use recovery::*;
pub use git_state::*;
pub use uncommitted_handler::*;
pub use commit_boundary::*;
