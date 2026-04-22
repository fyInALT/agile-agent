//! Foundation types for agile-agent ecosystem
//!
//! Pure data types with no implementation dependencies.

pub mod agent_id;
pub mod worker_status;
pub mod role;
pub mod runtime_mode;
pub mod task_status;
pub mod provider_type;
pub mod task_types;

pub use agent_id::*;
pub use worker_status::*;

// Backward compatibility: AgentStatus was renamed to WorkerStatus
pub use worker_status::WorkerStatus as AgentStatus;
pub use role::*;
pub use runtime_mode::*;
pub use task_status::*;
pub use provider_type::*;
pub use task_types::*;