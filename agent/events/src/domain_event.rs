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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    use crate::{ExecCommandStatus, McpToolCallStatus, PatchApplyStatus};

    // ── Construction tests for all variants ──────────────────────

    #[test]
    fn construct_session_handle() {
        let _ = DomainEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id: "sess-1".to_string(),
        });
    }

    #[test]
    fn construct_assistant_chunk() {
        let _ = DomainEvent::AssistantChunk("hello".to_string());
    }

    #[test]
    fn construct_thinking_chunk() {
        let _ = DomainEvent::ThinkingChunk("thinking...".to_string());
    }

    #[test]
    fn construct_status() {
        let _ = DomainEvent::Status("working".to_string());
    }

    #[test]
    fn construct_exec_command_started() {
        let _ = DomainEvent::ExecCommandStarted {
            call_id: Some("cmd-1".to_string()),
            input_preview: Some("ls -la".to_string()),
            source: None,
        };
    }

    #[test]
    fn construct_exec_command_finished() {
        let _ = DomainEvent::ExecCommandFinished {
            call_id: Some("cmd-1".to_string()),
            output_preview: Some("file1 file2".to_string()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(100),
            source: None,
        };
    }

    #[test]
    fn construct_exec_command_output_delta() {
        let _ = DomainEvent::ExecCommandOutputDelta {
            call_id: Some("cmd-1".to_string()),
            delta: "output".to_string(),
        };
    }

    #[test]
    fn construct_generic_tool_call_started() {
        let _ = DomainEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            input_preview: Some("/path".to_string()),
        };
    }

    #[test]
    fn construct_generic_tool_call_finished() {
        let _ = DomainEvent::GenericToolCallFinished {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            output_preview: Some("content".to_string()),
            success: true,
            exit_code: None,
            duration_ms: Some(50),
        };
    }

    #[test]
    fn construct_web_search_started() {
        let _ = DomainEvent::WebSearchStarted {
            call_id: Some("ws-1".to_string()),
            query: "rust".to_string(),
        };
    }

    #[test]
    fn construct_web_search_finished() {
        let _ = DomainEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "rust".to_string(),
            action: None,
        };
    }

    #[test]
    fn construct_view_image() {
        let _ = DomainEvent::ViewImage {
            call_id: Some("img-1".to_string()),
            path: "/tmp/img.png".to_string(),
        };
    }

    #[test]
    fn construct_image_generation_finished() {
        let _ = DomainEvent::ImageGenerationFinished {
            call_id: Some("gen-1".to_string()),
            revised_prompt: None,
            result: None,
            saved_path: Some("/tmp/out.png".to_string()),
        };
    }

    #[test]
    fn construct_mcp_tool_call_started() {
        let _ = DomainEvent::McpToolCallStarted {
            call_id: Some("mcp-1".to_string()),
            invocation: McpInvocation {
                server: "srv".to_string(),
                tool: "tool".to_string(),
                arguments: None,
            },
        };
    }

    #[test]
    fn construct_mcp_tool_call_finished() {
        let _ = DomainEvent::McpToolCallFinished {
            call_id: Some("mcp-1".to_string()),
            invocation: McpInvocation {
                server: "srv".to_string(),
                tool: "tool".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: None,
            status: McpToolCallStatus::Completed,
            is_error: false,
        };
    }

    #[test]
    fn construct_patch_apply_started() {
        let _ = DomainEvent::PatchApplyStarted {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
        };
    }

    #[test]
    fn construct_patch_apply_finished() {
        let _ = DomainEvent::PatchApplyFinished {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Completed,
        };
    }

    #[test]
    fn construct_patch_apply_output_delta() {
        let _ = DomainEvent::PatchApplyOutputDelta {
            call_id: Some("patch-1".to_string()),
            delta: "diff".to_string(),
        };
    }

    #[test]
    fn construct_error() {
        let _ = DomainEvent::Error("something went wrong".to_string());
    }

    #[test]
    fn construct_finished() {
        let _ = DomainEvent::Finished;
    }

    // ── Helper method tests ──────────────────────────────────────

    #[test]
    fn assistant_chunk_is_running() {
        assert!(DomainEvent::AssistantChunk("hi".to_string()).is_running());
        assert!(!DomainEvent::AssistantChunk("hi".to_string()).may_need_decision());
    }

    #[test]
    fn exec_command_failed_is_failure() {
        let e = DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: None,
            status: ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: None,
            source: None,
        };
        assert!(e.is_failure());
        assert!(e.may_need_decision());
    }

    #[test]
    fn exec_command_completed_is_success() {
        let e = DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: None,
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: None,
            source: None,
        };
        assert!(e.is_success());
        assert!(!e.is_failure());
    }

    #[test]
    fn generic_tool_call_failed_is_failure() {
        let e = DomainEvent::GenericToolCallFinished {
            name: "test".to_string(),
            call_id: None,
            output_preview: None,
            success: false,
            exit_code: None,
            duration_ms: None,
        };
        assert!(e.is_failure());
    }

    #[test]
    fn mcp_tool_call_error_is_failure() {
        let e = DomainEvent::McpToolCallFinished {
            call_id: None,
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: Some("err".to_string()),
            status: McpToolCallStatus::Failed,
            is_error: true,
        };
        assert!(e.is_failure());
    }

    #[test]
    fn patch_apply_declined_is_failure() {
        let e = DomainEvent::PatchApplyFinished {
            call_id: None,
            changes: vec![],
            status: PatchApplyStatus::Declined,
        };
        assert!(e.is_failure());
    }

    #[test]
    fn all_events_broadcast_by_default() {
        assert!(DomainEvent::Status("ok".to_string()).should_broadcast());
        assert!(DomainEvent::Error("e".to_string()).should_broadcast());
        assert!(DomainEvent::Finished.should_broadcast());
    }

    #[test]
    fn equality_works() {
        let a = DomainEvent::Status("x".to_string());
        let b = DomainEvent::Status("x".to_string());
        let c = DomainEvent::Status("y".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
