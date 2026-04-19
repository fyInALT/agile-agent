//! Situation subsystem - decision triggering situations
//!
//! Provides:
//! - DecisionSituation trait
//! - SituationRegistry for dynamic registration
//! - Built-in situations (WaitingForChoice, Error, etc.)

pub mod situation;
pub mod situation_registry;
pub mod builtin_situations;

pub use situation::*;
pub use situation_registry::*;
pub use builtin_situations::*;
