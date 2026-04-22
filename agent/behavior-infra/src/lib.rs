//! agent-behavior-infra — behavior infrastructure and effect system.
//!
//! This crate contains the effect types and handler traits that bridge
//! pure domain logic to runtime side effects:
//! - RuntimeCommand: effect descriptor enum
//! - RuntimeCommandQueue: ordered command buffer
//! - EffectHandler: trait for executing side effects
//! - EffectError: error type for handler failures
//!
//! Dependency direction: behavior-infra → runtime-domain → types/events

pub mod runtime_command;

pub use runtime_command::{
    EffectError, EffectHandler, NoopEffectHandler, RecordingEffectHandler,
    RuntimeCommand, RuntimeCommandQueue,
};
