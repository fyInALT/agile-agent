//! agent-behavior-infra — behavior infrastructure and effect system.
//!
//! This crate contains the effect handler trait and concrete implementations
//! that bridge pure domain logic to runtime side effects:
//! - EffectHandler: trait for executing side effects
//! - NoopEffectHandler, RecordingEffectHandler: test doubles
//! - Per-variant handler traits (SpawnProviderHandler, etc.)
//! - CompositeEffectHandler: combines per-variant handlers into EffectHandler
//!
//! The command types (`RuntimeCommand`, `RuntimeCommandQueue`, `EffectError`)
//! live in `agent-runtime-domain` and are re-exported here for convenience.
//!
//! Dependency direction: behavior-infra → runtime-domain → types/events

pub mod handlers;
pub mod runtime_command;

pub use handlers::{
    CompositeEffectHandler, NotifyUserHandler, RequestDecisionHandler,
    SendToProviderHandler, SpawnProviderHandler, TerminateHandler, TransitionStateHandler,
    UpdateWorktreeHandler,
};
pub use runtime_command::{
    EffectError, EffectHandler, NoopEffectHandler, RecordingEffectHandler,
    RuntimeCommand, RuntimeCommandQueue,
};
