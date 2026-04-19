//! Model layer - business model definitions
//!
//! This layer provides:
//! - Situation (DecisionSituation trait, builtins, registry)
//! - Action (DecisionAction trait, builtins, registry)
//! - Task (Task entity, status, metrics, preparation, completion)
//! - Workflow (DecisionProcess, DecisionStage, Condition)

pub mod situation;
pub mod action;
pub mod task;
pub mod workflow;

// Re-export key types
pub use situation::*;
pub use action::*;
pub use task::*;
pub use workflow::*;
