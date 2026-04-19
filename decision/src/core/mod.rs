//! Core types layer - foundational types for decision layer
//!
//! This layer provides:
//! - Basic types (SituationType, ActionType, UrgencyLevel, etc.)
//! - Error handling (DecisionError)
//! - Decision context (DecisionContext, RunningContextCache)
//! - Decision output (DecisionOutput, DecisionRecord)

pub mod types;
pub mod error;
pub mod context;
pub mod output;

// Re-export all public types
pub use types::*;
pub use error::*;
pub use context::*;
pub use output::*;
