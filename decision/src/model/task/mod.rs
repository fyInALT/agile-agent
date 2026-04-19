//! Task subsystem - task entity and lifecycle
//!
//! Provides:
//! - Task entity with status transitions
//! - TaskMetrics for performance tracking
//! - TaskMetadata for git-flow preparation
//! - TaskPreparation for starting tasks
//! - TaskCompletion for finishing tasks

pub mod task;
pub mod task_metrics;
pub mod task_metadata;
pub mod task_preparation;
pub mod task_completion;

pub use task::*;
pub use task_metrics::*;
pub use task_metadata::*;
pub use task_preparation::*;
pub use task_completion::*;
