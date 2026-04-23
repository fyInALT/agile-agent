//! Unified domain event type for the agile-agent ecosystem.
//!
//! `DomainEvent` is the single source of truth for all events emitted by
//! LLM providers and consumed by the runtime, decision layer, and UI.
//!
//! This enum replaces the dual `ProviderEvent` definitions that previously
//! existed in `agent-provider` and `agent-decision`.

use crate::{
    ExecCommandStatus, McpInvocation, McpToolCallStatus, PatchApplyStatus, PatchChange,
    SessionHandle, WebSearchAction,
};
use serde::{Deserialize, Serialize};

/// Unified domain event — single source of truth for all provider events.
///
/// Each variant represents a distinct event in the lifecycle of an agent
/// interacting with an LLM provider. Events are categorized as:
/// - **Lifecycle**: WorkerStarted, WorkerFinished, WorkerFailed, SessionAcquired
/// - **Streaming**: AssistantChunk, ThinkingChunk, StatusUpdate
/// - **Tool execution**: ExecCommandStarted/Finished, ExecCommandOutputDelta
/// - **Generic tool calls**: GenericToolCallStarted/Finished
/// - **Web search**: WebSearchStarted/Finished
/// - **Images**: ViewImage, ImageGenerationFinished
/// - **MCP**: McpToolCallStarted/Finished
/// - **Patch apply**: PatchApplyStarted/Finished, PatchApplyOutputDelta
/// - **System**: Error, Finished
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainEvent {
    // ── Lifecycle ──────────────────────────────────────────────
    /// Session handle acquired for multi-turn continuity
    SessionHandle(SessionHandle),

    // ── Streaming ──────────────────────────────────────────────
    /// Assistant response text chunk
    AssistantChunk(String),

    /// Thinking/reasoning text chunk
    ThinkingChunk(String),

    /// Status update message
    Status(String),

    // ── Tool execution ─────────────────────────────────────────
    /// External command execution started
    ExecCommandStarted {
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    },

    /// External command execution finished
    ExecCommandFinished {
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    },

    /// Incremental output from external command
    ExecCommandOutputDelta {
        call_id: Option<String>,
        delta: String,
    },

    // ── Generic tool calls ─────────────────────────────────────
    /// Generic tool call started
    GenericToolCallStarted {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    },

    /// Generic tool call finished
    GenericToolCallFinished {
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },

    // ── Web search ─────────────────────────────────────────────
    /// Web search started
    WebSearchStarted {
        call_id: Option<String>,
        query: String,
    },

    /// Web search finished
    WebSearchFinished {
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    },

    // ── Images ─────────────────────────────────────────────────
    /// Image view request
    ViewImage {
        call_id: Option<String>,
        path: String,
    },

    /// Image generation finished
    ImageGenerationFinished {
        call_id: Option<String>,
        revised_prompt: Option<String>,
        result: Option<String>,
        saved_path: Option<String>,
    },

    // ── MCP ────────────────────────────────────────────────────
    /// MCP tool call started
    McpToolCallStarted {
        call_id: Option<String>,
        invocation: McpInvocation,
    },

    /// MCP tool call finished
    McpToolCallFinished {
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    },

    // ── Patch apply ────────────────────────────────────────────
    /// Patch apply started
    PatchApplyStarted {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    },

    /// Patch apply finished
    PatchApplyFinished {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
    },

    /// Incremental patch apply output
    PatchApplyOutputDelta {
        call_id: Option<String>,
        delta: String,
    },

    // ── System ─────────────────────────────────────────────────
    /// Provider subprocess PID for lifecycle tracking
    ProviderPid(u32),

    /// Generic error message
    Error(String),

    /// Legacy finished signal (provider-specific, prefer WorkerFinished)
    Finished,
}

impl DomainEvent {
    /// Returns true for streaming/delta events that do not require
    /// decision layer intervention.
    ///
    /// These events are "fire and forget" — they update the UI or
    /// transcript but do not change agent state in a meaningful way.
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            DomainEvent::AssistantChunk(_)
                | DomainEvent::ThinkingChunk(_)
                | DomainEvent::Status(_)
                | DomainEvent::ExecCommandOutputDelta { .. }
                | DomainEvent::PatchApplyOutputDelta { .. }
                | DomainEvent::ExecCommandStarted { .. }
                | DomainEvent::GenericToolCallStarted { .. }
                | DomainEvent::WebSearchStarted { .. }
                | DomainEvent::ViewImage { .. }
                | DomainEvent::ImageGenerationFinished { .. }
                | DomainEvent::McpToolCallStarted { .. }
                | DomainEvent::PatchApplyStarted { .. }
                | DomainEvent::SessionHandle(_)
                | DomainEvent::ProviderPid(_)
        )
    }

    /// Returns true if this event may require decision layer attention.
    ///
    /// This includes terminal events, failures, and completion signals.
    /// Note: not all events that return true here will trigger a decision —
    /// the decision layer may classify them as no-ops.
    pub fn may_need_decision(&self) -> bool {
        matches!(
            self,
            DomainEvent::Error(_)
                | DomainEvent::Finished
                | DomainEvent::ExecCommandFinished { .. }
                | DomainEvent::GenericToolCallFinished { .. }
                | DomainEvent::WebSearchFinished { .. }
                | DomainEvent::McpToolCallFinished { .. }
                | DomainEvent::PatchApplyFinished { .. }
        )
    }

    /// Returns true if this event should be broadcast to UI observers.
    ///
    /// Most events are broadcast, but some internal-only events
    /// (like `WorkerStarted`) may be suppressed.
    pub fn should_broadcast(&self) -> bool {
        // All events are broadcast by default; specific filtering
        // happens in the event aggregator layer.
        true
    }

    /// Returns true if this event represents a failure state.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            DomainEvent::Error(_)
                | DomainEvent::ExecCommandFinished {
                    status: ExecCommandStatus::Failed | ExecCommandStatus::Declined,
                    ..
                }
                | DomainEvent::GenericToolCallFinished { success: false, .. }
                | DomainEvent::McpToolCallFinished { is_error: true, .. }
                | DomainEvent::PatchApplyFinished {
                    status: PatchApplyStatus::Failed | PatchApplyStatus::Declined,
                    ..
                }
        )
    }

    /// Returns true if this event represents a completed/success state.
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            DomainEvent::Finished
                | DomainEvent::ExecCommandFinished {
                    status: ExecCommandStatus::Completed,
                    ..
                }
                | DomainEvent::GenericToolCallFinished { success: true, .. }
                | DomainEvent::McpToolCallFinished {
                    status: McpToolCallStatus::Completed,
                    is_error: false,
                    ..
                }
                | DomainEvent::PatchApplyFinished {
                    status: PatchApplyStatus::Completed,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExecCommandStatus, McpInvocation, McpToolCallStatus, PatchApplyStatus, SessionHandle,
        WebSearchAction,
    };

    #[test]
    fn domain_event_comprehensive() {
        // Construct every variant (compilation already guarantees this, but we
        // need instances to exercise the helper methods and equality).
        let session_handle = DomainEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id: "s".to_string(),
        });
        let assistant_chunk = DomainEvent::AssistantChunk("a".to_string());
        let thinking_chunk = DomainEvent::ThinkingChunk("t".to_string());
        let status = DomainEvent::Status("st".to_string());
        let exec_started = DomainEvent::ExecCommandStarted {
            call_id: Some("c".to_string()),
            input_preview: Some("i".to_string()),
            source: Some("src".to_string()),
        };
        let exec_finished_ok = DomainEvent::ExecCommandFinished {
            call_id: Some("c".to_string()),
            output_preview: Some("o".to_string()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(1),
            source: None,
        };
        let exec_finished_fail = DomainEvent::ExecCommandFinished {
            call_id: Some("c".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: None,
            source: None,
        };
        let exec_finished_declined = DomainEvent::ExecCommandFinished {
            call_id: Some("c".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Declined,
            exit_code: None,
            duration_ms: None,
            source: None,
        };
        let exec_delta = DomainEvent::ExecCommandOutputDelta {
            call_id: Some("c".to_string()),
            delta: "d".to_string(),
        };
        let generic_started = DomainEvent::GenericToolCallStarted {
            name: "n".to_string(),
            call_id: Some("c".to_string()),
            input_preview: None,
        };
        let generic_finished_ok = DomainEvent::GenericToolCallFinished {
            name: "n".to_string(),
            call_id: Some("c".to_string()),
            output_preview: Some("o".to_string()),
            success: true,
            exit_code: None,
            duration_ms: Some(1),
        };
        let generic_finished_fail = DomainEvent::GenericToolCallFinished {
            name: "n".to_string(),
            call_id: Some("c".to_string()),
            output_preview: None,
            success: false,
            exit_code: Some(1),
            duration_ms: None,
        };
        let websearch_started = DomainEvent::WebSearchStarted {
            call_id: Some("c".to_string()),
            query: "q".to_string(),
        };
        let websearch_finished = DomainEvent::WebSearchFinished {
            call_id: Some("c".to_string()),
            query: "q".to_string(),
            action: Some(WebSearchAction::Search {
                query: Some("q".to_string()),
                queries: None,
            }),
        };
        let view_image = DomainEvent::ViewImage {
            call_id: Some("c".to_string()),
            path: "p".to_string(),
        };
        let image_gen = DomainEvent::ImageGenerationFinished {
            call_id: Some("c".to_string()),
            revised_prompt: None,
            result: None,
            saved_path: Some("p".to_string()),
        };
        let mcp_started = DomainEvent::McpToolCallStarted {
            call_id: Some("c".to_string()),
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
        };
        let mcp_finished_ok = DomainEvent::McpToolCallFinished {
            call_id: Some("c".to_string()),
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: None,
            status: McpToolCallStatus::Completed,
            is_error: false,
        };
        let mcp_finished_fail = DomainEvent::McpToolCallFinished {
            call_id: Some("c".to_string()),
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: Some("e".to_string()),
            status: McpToolCallStatus::Failed,
            is_error: true,
        };
        let patch_started = DomainEvent::PatchApplyStarted {
            call_id: Some("c".to_string()),
            changes: vec![],
        };
        let patch_finished_ok = DomainEvent::PatchApplyFinished {
            call_id: Some("c".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Completed,
        };
        let patch_finished_fail = DomainEvent::PatchApplyFinished {
            call_id: Some("c".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Failed,
        };
        let patch_finished_declined = DomainEvent::PatchApplyFinished {
            call_id: Some("c".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Declined,
        };
        let patch_delta = DomainEvent::PatchApplyOutputDelta {
            call_id: Some("c".to_string()),
            delta: "d".to_string(),
        };
        let error = DomainEvent::Error("e".to_string());
        let finished = DomainEvent::Finished;

        // ---- is_running ----
        assert!(session_handle.is_running());
        assert!(assistant_chunk.is_running());
        assert!(thinking_chunk.is_running());
        assert!(status.is_running());
        assert!(exec_started.is_running());
        assert!(exec_delta.is_running());
        assert!(generic_started.is_running());
        assert!(websearch_started.is_running());
        assert!(view_image.is_running());
        assert!(image_gen.is_running());
        assert!(mcp_started.is_running());
        assert!(patch_started.is_running());
        assert!(patch_delta.is_running());
        assert!(!exec_finished_ok.is_running());
        assert!(!exec_finished_fail.is_running());
        assert!(!generic_finished_ok.is_running());
        assert!(!websearch_finished.is_running());
        assert!(!mcp_finished_ok.is_running());
        assert!(!patch_finished_ok.is_running());
        assert!(!error.is_running());
        assert!(!finished.is_running());

        // ---- may_need_decision ----
        assert!(error.may_need_decision());
        assert!(finished.may_need_decision());
        assert!(exec_finished_ok.may_need_decision());
        assert!(exec_finished_fail.may_need_decision());
        assert!(generic_finished_ok.may_need_decision());
        assert!(websearch_finished.may_need_decision());
        assert!(mcp_finished_ok.may_need_decision());
        assert!(patch_finished_ok.may_need_decision());
        assert!(!session_handle.may_need_decision());
        assert!(!assistant_chunk.may_need_decision());
        assert!(!exec_started.may_need_decision());
        assert!(!exec_delta.may_need_decision());
        assert!(!patch_delta.may_need_decision());

        // ---- is_failure ----
        assert!(error.is_failure());
        assert!(exec_finished_fail.is_failure());
        assert!(exec_finished_declined.is_failure());
        assert!(generic_finished_fail.is_failure());
        assert!(mcp_finished_fail.is_failure());
        assert!(patch_finished_fail.is_failure());
        assert!(patch_finished_declined.is_failure());
        assert!(!session_handle.is_failure());
        assert!(!assistant_chunk.is_failure());
        assert!(!finished.is_failure());
        assert!(!exec_finished_ok.is_failure());
        assert!(!generic_finished_ok.is_failure());
        assert!(!mcp_finished_ok.is_failure());
        assert!(!patch_finished_ok.is_failure());

        // ---- is_success ----
        assert!(finished.is_success());
        assert!(exec_finished_ok.is_success());
        assert!(generic_finished_ok.is_success());
        assert!(mcp_finished_ok.is_success());
        assert!(patch_finished_ok.is_success());
        assert!(!error.is_success());
        assert!(!exec_finished_fail.is_success());
        assert!(!exec_finished_declined.is_success());
        assert!(!generic_finished_fail.is_success());
        assert!(!mcp_finished_fail.is_success());
        assert!(!patch_finished_fail.is_success());
        assert!(!patch_finished_declined.is_success());

        // ---- should_broadcast ----
        assert!(session_handle.should_broadcast());
        assert!(error.should_broadcast());
        assert!(finished.should_broadcast());
        assert!(assistant_chunk.should_broadcast());
        assert!(exec_started.should_broadcast());

        // ---- equality ----
        // Same variant, same fields
        assert_eq!(
            DomainEvent::Status("x".to_string()),
            DomainEvent::Status("x".to_string())
        );
        // Same variant, different fields
        assert_ne!(
            DomainEvent::Status("x".to_string()),
            DomainEvent::Status("y".to_string())
        );
        // Cross-variant inequality
        assert_ne!(assistant_chunk, thinking_chunk);
        assert_ne!(status, error);
        assert_ne!(exec_started, exec_finished_ok);
        assert_ne!(finished, error);
        // Same variant with struct field differences
        let exec_a = DomainEvent::ExecCommandFinished {
            call_id: Some("1".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Completed,
            exit_code: None,
            duration_ms: None,
            source: None,
        };
        let exec_b = DomainEvent::ExecCommandFinished {
            call_id: Some("2".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Completed,
            exit_code: None,
            duration_ms: None,
            source: None,
        };
        assert_ne!(exec_a, exec_b);
    }
}
