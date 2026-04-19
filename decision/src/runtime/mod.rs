//! Runtime layer - runtime support services
//!
//! Provides:
//! - Session pooling (DecisionSessionPool)
//! - Rate limiting (DecisionRateLimiter)
//! - Human decision arbitration
//! - Auto-checking (AutoChecker)
//! - Decision filtering (DecisionFilter)
//! - Persistence (TaskStore, TaskRegistry)
//! - Metrics (DecisionMetrics)

pub mod concurrent;
pub mod automation;
pub mod persistence;
pub mod metrics;

pub use concurrent::*;
pub use automation::*;
pub use persistence::*;
pub use metrics::*;
