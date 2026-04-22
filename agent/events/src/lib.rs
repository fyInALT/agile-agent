//! Shared event kernel for the agile-agent ecosystem
//!
//! This crate provides the single source of truth for all event types
//! used across the system. It depends on `agent-toolkit` for execution
//! status types and `agent-types` (transitively) for provider kind.
//!
//! Design principles:
//! - Pure data types only — no I/O, no threading, no business logic
//! - All event variants are exhaustively matchable (no `_ =>` catches)
//! - Conversion between event types is compile-time verified

pub mod decision_event;
pub mod domain_event;

pub use domain_event::DomainEvent;
pub use decision_event::DecisionEvent;

// Re-export toolkit types that are part of the event vocabulary
use serde::{Deserialize, Serialize};

pub use agent_toolkit::{
    ExecCommandStatus, McpInvocation, McpToolCallStatus,
    PatchApplyStatus, PatchChange, WebSearchAction, PatchChangeKind,
};

/// Session handle for multi-turn conversation continuity.
///
/// This type is part of the event kernel because it appears in
/// `DomainEvent::SessionHandle` and is used across multiple crates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_handle_claude_display() {
        let handle = SessionHandle::ClaudeSession {
            session_id: "sess-123".to_string(),
        };
        let debug = format!("{:?}", handle);
        assert!(debug.contains("sess-123"));
    }

    #[test]
    fn session_handle_codex_display() {
        let handle = SessionHandle::CodexThread {
            thread_id: "thread-456".to_string(),
        };
        let debug = format!("{:?}", handle);
        assert!(debug.contains("thread-456"));
    }

    #[test]
    fn session_handle_equality() {
        let a = SessionHandle::ClaudeSession {
            session_id: "same".to_string(),
        };
        let b = SessionHandle::ClaudeSession {
            session_id: "same".to_string(),
        };
        let c = SessionHandle::CodexThread {
            thread_id: "same".to_string(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
