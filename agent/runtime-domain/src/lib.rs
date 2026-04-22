//! agent-runtime-domain — pure domain model for the agent runtime.
//!
//! This crate contains zero I/O dependencies. All types are pure data
//! structures representing the domain model of the runtime:
//! - Worker: aggregate root for per-agent state
//! - WorkerState: explicit state machine
//! - TranscriptJournal: structured transcript storage
//!
//! Dependency direction: domain → types/events/toolkit (no cycles)

pub mod transcript_journal;
pub mod worker_state;

// Re-export key types for ergonomic use
pub use transcript_journal::{JournalEntry, TranscriptJournal};
pub use worker_state::{InvalidTransition, RespondingSubState, WorkerState};
