//! agent-runtime-domain — pure domain model for the agent runtime.
//!
//! This crate contains zero I/O dependencies. All types are pure data
//! structures representing the domain model of the runtime:
//! - Worker: aggregate root for per-agent state
//! - WorkerState: explicit state machine
//! - TranscriptJournal: structured transcript storage
//! - RuntimeCommand: effect descriptor enum (pure data)
//!
//! Dependency direction: domain → types/events/toolkit (no cycles)

pub mod runtime_command;
pub mod transcript_journal;
pub mod worker_state;
pub mod worker;

// Re-export key types for ergonomic use
pub use runtime_command::{EffectError, RuntimeCommand, RuntimeCommandQueue};
pub use transcript_journal::{JournalEntry, TranscriptJournal};
pub use worker_state::{InvalidTransition, RespondingSubState, WorkerState};
pub use worker::{Worker, WorkerError};
