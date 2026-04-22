//! Decision-layer event type with automatic conversion from DomainEvent.
//!
//! `DecisionEvent` is the subset of domain events that the decision layer
//! cares about. The conversion from `DomainEvent` is explicit and
//! compile-time verified — adding a new `DomainEvent` variant without
//! updating the `From` implementation will produce a compiler warning
//! (or error if deny(warnings) is enabled).

use serde::{Deserialize, Serialize};

use crate::domain_event::DomainEvent;
use crate::{ExecCommandStatus, PatchApplyStatus, SessionHandle};

/// Decision-layer event — subset of DomainEvent relevant to decision making.
///
/// This enum mirrors the structure previously used by `agent-decision`'s
/// simplified `ProviderEvent`, but lives in the shared kernel for
/// single-source-of-truth consistency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecisionEvent {
    /// Provider/agent finished execution
    Finished { summary: Option<String> },

    /// Provider/agent error
    Error { message: String, error_type: Option<String> },

    // ── Claude-specific streaming & tool events ─────────────────
    /// Assistant response chunk
    ClaudeAssistantChunk { text: String },

    /// Thinking/reasoning chunk
    ClaudeThinkingChunk { text: String },

    /// Tool call started
    ClaudeToolCallStarted { name: String, input: Option<String> },

    /// Tool call finished
    ClaudeToolCallFinished { name: String, output: Option<String>, success: bool },

    // ── Codex-specific events ───────────────────────────────────
    /// Approval request from Codex
    CodexApprovalRequest {
        method: String,
        params: serde_json::Value,
        request_id: Option<String>,
    },

    /// Patch apply started
    CodexPatchApplyStarted { path: String },

    /// Codex-specific error
    CodexError { kind: String, message: String },

    // ── ACP-specific events (OpenCode/Kimi) ─────────────────────
    /// ACP notification
    ACPNotification { method: String, params: serde_json::Value },

    /// ACP error
    ACPError { code: String, message: String },

    // ── Generic events ──────────────────────────────────────────
    /// Status update
    StatusUpdate { status: String },

    /// Session handle acquired
    SessionHandle { session_id: String, info: Option<String> },
}

impl DecisionEvent {
    /// Check if this is a running event (no decision needed)
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            DecisionEvent::ClaudeAssistantChunk { .. }
                | DecisionEvent::ClaudeThinkingChunk { .. }
                | DecisionEvent::ClaudeToolCallStarted { .. }
                | DecisionEvent::ClaudeToolCallFinished { .. }
                | DecisionEvent::CodexPatchApplyStarted { .. }
                | DecisionEvent::StatusUpdate { .. }
                | DecisionEvent::SessionHandle { .. }
        )
    }

    /// Check if this is a decision-triggering event
    pub fn needs_decision(&self) -> bool {
        matches!(
            self,
            DecisionEvent::Finished { .. }
                | DecisionEvent::Error { .. }
                | DecisionEvent::CodexApprovalRequest { .. }
                | DecisionEvent::CodexError { .. }
                | DecisionEvent::ACPNotification { .. }
                | DecisionEvent::ACPError { .. }
        )
    }
}

/// Convert DomainEvent to DecisionEvent.
///
/// Returns `None` for events that have no decision-layer equivalent
/// (e.g., pure streaming deltas that are only relevant for UI).
impl From<&DomainEvent> for Option<DecisionEvent> {
    fn from(event: &DomainEvent) -> Self {
        match event {
            // ── Lifecycle ─────────────────────────────────────────
            DomainEvent::SessionHandle(handle) => Some(DecisionEvent::SessionHandle {
                session_id: match handle {
                    SessionHandle::ClaudeSession { session_id } => session_id.clone(),
                    SessionHandle::CodexThread { thread_id } => thread_id.clone(),
                },
                info: None,
            }),

            // ── Streaming ─────────────────────────────────────────
            DomainEvent::AssistantChunk(text) => Some(DecisionEvent::ClaudeAssistantChunk {
                text: text.clone(),
            }),
            DomainEvent::ThinkingChunk(text) => Some(DecisionEvent::ClaudeThinkingChunk {
                text: text.clone(),
            }),
            DomainEvent::Status(text) => Some(DecisionEvent::StatusUpdate {
                status: text.clone(),
            }),

            // ── Tool execution ────────────────────────────────────
            DomainEvent::ExecCommandStarted { input_preview, .. } => {
                Some(DecisionEvent::ClaudeToolCallStarted {
                    name: "exec".to_string(),
                    input: input_preview.clone(),
                })
            }
            DomainEvent::ExecCommandFinished {
                output_preview,
                status,
                ..
            } => Some(DecisionEvent::ClaudeToolCallFinished {
                name: "exec".to_string(),
                output: output_preview.clone(),
                success: matches!(status, ExecCommandStatus::Completed),
            }),
            DomainEvent::ExecCommandOutputDelta { .. } => Some(DecisionEvent::StatusUpdate {
                status: "running".to_string(),
            }),

            // ── Generic tool calls ────────────────────────────────
            DomainEvent::GenericToolCallStarted {
                name,
                input_preview,
                ..
            } => Some(DecisionEvent::ClaudeToolCallStarted {
                name: name.clone(),
                input: input_preview.clone(),
            }),
            DomainEvent::GenericToolCallFinished {
                name,
                output_preview,
                success,
                ..
            } => Some(DecisionEvent::ClaudeToolCallFinished {
                name: name.clone(),
                output: output_preview.clone(),
                success: *success,
            }),

            // ── Web search ────────────────────────────────────────
            DomainEvent::WebSearchStarted { .. } => Some(DecisionEvent::StatusUpdate {
                status: "websearch started".to_string(),
            }),
            DomainEvent::WebSearchFinished { .. } => Some(DecisionEvent::StatusUpdate {
                status: "websearch completed".to_string(),
            }),

            // ── Images ────────────────────────────────────────────
            DomainEvent::ViewImage { .. } | DomainEvent::ImageGenerationFinished { .. } => {
                Some(DecisionEvent::StatusUpdate {
                    status: "running".to_string(),
                })
            }

            // ── MCP ───────────────────────────────────────────────
            DomainEvent::McpToolCallStarted { invocation, .. } => {
                Some(DecisionEvent::ClaudeToolCallStarted {
                    name: "mcp".to_string(),
                    input: Some(format!("{}:{}", invocation.server, invocation.tool)),
                })
            }
            DomainEvent::McpToolCallFinished { error, .. } => {
                Some(DecisionEvent::ClaudeToolCallFinished {
                    name: "mcp".to_string(),
                    output: error.clone(),
                    success: error.is_none(),
                })
            }

            // ── Patch apply ───────────────────────────────────────
            DomainEvent::PatchApplyStarted { changes, .. } => {
                let path = changes.first().map(|c| c.path.clone()).unwrap_or_default();
                Some(DecisionEvent::CodexPatchApplyStarted { path })
            }
            DomainEvent::PatchApplyFinished { status, .. } => Some(DecisionEvent::StatusUpdate {
                status: match status {
                    PatchApplyStatus::Completed => "patch completed".to_string(),
                    PatchApplyStatus::Failed => "patch failed".to_string(),
                    PatchApplyStatus::Declined => "patch declined".to_string(),
                    PatchApplyStatus::InProgress => "patch in progress".to_string(),
                },
            }),
            DomainEvent::PatchApplyOutputDelta { .. } => Some(DecisionEvent::StatusUpdate {
                status: "running".to_string(),
            }),

            // ── System ────────────────────────────────────────────
            DomainEvent::Error(msg) => Some(DecisionEvent::Error {
                message: msg.clone(),
                error_type: None,
            }),
            DomainEvent::Finished => Some(DecisionEvent::Finished { summary: None }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_event::DomainEvent;
    use crate::{
        ExecCommandStatus, McpInvocation, McpToolCallStatus, PatchApplyStatus, PatchChange,
        PatchChangeKind, SessionHandle, WebSearchAction,
    };

    // ── Conversion tests matching event_converter.rs coverage ───

    #[test]
    fn convert_finished() {
        let event = DomainEvent::Finished;
        let result: Option<DecisionEvent> = (&event).into();
        assert!(matches!(result, Some(DecisionEvent::Finished { summary: None })));
    }

    #[test]
    fn convert_error() {
        let event = DomainEvent::Error("test error".to_string());
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::Error { message, .. }) => assert_eq!(message, "test error"),
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn convert_status_update() {
        let event = DomainEvent::Status("working".to_string());
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => assert_eq!(status, "working"),
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_assistant_chunk() {
        let event = DomainEvent::AssistantChunk("hello".to_string());
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeAssistantChunk { text }) => assert_eq!(text, "hello"),
            _ => panic!("Expected ClaudeAssistantChunk variant"),
        }
    }

    #[test]
    fn convert_thinking_chunk() {
        let event = DomainEvent::ThinkingChunk("thinking...".to_string());
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeThinkingChunk { text }) => assert_eq!(text, "thinking..."),
            _ => panic!("Expected ClaudeThinkingChunk variant"),
        }
    }

    #[test]
    fn convert_session_handle_claude() {
        let event = DomainEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id: "sess-123".to_string(),
        });
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::SessionHandle { session_id, .. }) => {
                assert_eq!(session_id, "sess-123");
            }
            _ => panic!("Expected SessionHandle variant"),
        }
    }

    #[test]
    fn convert_session_handle_codex() {
        let event = DomainEvent::SessionHandle(SessionHandle::CodexThread {
            thread_id: "thread-456".to_string(),
        });
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::SessionHandle { session_id, .. }) => {
                assert_eq!(session_id, "thread-456");
            }
            _ => panic!("Expected SessionHandle variant"),
        }
    }

    #[test]
    fn convert_exec_command_started() {
        let event = DomainEvent::ExecCommandStarted {
            call_id: Some("cmd-1".to_string()),
            input_preview: Some("ls -la".to_string()),
            source: None,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallStarted { name, input }) => {
                assert_eq!(name, "exec");
                assert_eq!(input, Some("ls -la".to_string()));
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_exec_command_finished_success() {
        let event = DomainEvent::ExecCommandFinished {
            call_id: Some("cmd-1".to_string()),
            output_preview: Some("file1 file2".to_string()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(100),
            source: None,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallFinished { name, output, success }) => {
                assert_eq!(name, "exec");
                assert_eq!(output, Some("file1 file2".to_string()));
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_exec_command_finished_failed() {
        let event = DomainEvent::ExecCommandFinished {
            call_id: Some("cmd-2".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: Some(50),
            source: None,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallFinished { name, success, .. }) => {
                assert_eq!(name, "exec");
                assert!(!success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_generic_tool_call_started() {
        let event = DomainEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            input_preview: Some("/path/to/file".to_string()),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallStarted { name, input }) => {
                assert_eq!(name, "read_file");
                assert_eq!(input, Some("/path/to/file".to_string()));
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_generic_tool_call_finished() {
        let event = DomainEvent::GenericToolCallFinished {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            output_preview: Some("content".to_string()),
            success: true,
            exit_code: None,
            duration_ms: Some(50),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallFinished { name, output, success }) => {
                assert_eq!(name, "read_file");
                assert_eq!(output, Some("content".to_string()));
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_patch_apply_started_with_path() {
        let event = DomainEvent::PatchApplyStarted {
            call_id: Some("patch-1".to_string()),
            changes: vec![PatchChange {
                path: "/file.rs".to_string(),
                move_path: None,
                kind: PatchChangeKind::Add,
                diff: "".to_string(),
                added: 1,
                removed: 0,
            }],
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::CodexPatchApplyStarted { path }) => {
                assert_eq!(path, "/file.rs");
            }
            _ => panic!("Expected CodexPatchApplyStarted variant"),
        }
    }

    #[test]
    fn convert_patch_apply_started_empty_changes() {
        let event = DomainEvent::PatchApplyStarted {
            call_id: Some("patch-2".to_string()),
            changes: vec![],
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::CodexPatchApplyStarted { path }) => {
                assert_eq!(path, "");
            }
            _ => panic!("Expected CodexPatchApplyStarted variant"),
        }
    }

    #[test]
    fn convert_patch_apply_finished_completed() {
        let event = DomainEvent::PatchApplyFinished {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Completed,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "patch completed");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_patch_apply_finished_failed() {
        let event = DomainEvent::PatchApplyFinished {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Failed,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "patch failed");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_started() {
        let event = DomainEvent::McpToolCallStarted {
            call_id: Some("mcp-1".to_string()),
            invocation: McpInvocation {
                server: "test-server".to_string(),
                tool: "test-tool".to_string(),
                arguments: None,
            },
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallStarted { name, input }) => {
                assert_eq!(name, "mcp");
                assert_eq!(input, Some("test-server:test-tool".to_string()));
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_finished_with_error() {
        let event = DomainEvent::McpToolCallFinished {
            call_id: Some("mcp-1".to_string()),
            invocation: McpInvocation {
                server: "test-server".to_string(),
                tool: "test-tool".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: Some("tool failed".to_string()),
            status: McpToolCallStatus::Failed,
            is_error: true,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallFinished { name, output, success }) => {
                assert_eq!(name, "mcp");
                assert_eq!(output, Some("tool failed".to_string()));
                assert!(!success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_finished_no_error() {
        let event = DomainEvent::McpToolCallFinished {
            call_id: Some("mcp-1".to_string()),
            invocation: McpInvocation {
                server: "test-server".to_string(),
                tool: "test-tool".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: None,
            status: McpToolCallStatus::Completed,
            is_error: false,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::ClaudeToolCallFinished { name, output, success }) => {
                assert_eq!(name, "mcp");
                assert!(output.is_none());
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_websearch_started() {
        let event = DomainEvent::WebSearchStarted {
            call_id: Some("ws-1".to_string()),
            query: "test query".to_string(),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "websearch started");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_websearch_finished() {
        let event = DomainEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "test query".to_string(),
            action: Some(WebSearchAction::Search {
                query: Some("q".to_string()),
                queries: None,
            }),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "websearch completed");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_exec_command_output_delta() {
        let event = DomainEvent::ExecCommandOutputDelta {
            call_id: Some("cmd-1".to_string()),
            delta: "some output".to_string(),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_view_image() {
        let event = DomainEvent::ViewImage {
            call_id: Some("img-1".to_string()),
            path: "/image.png".to_string(),
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_image_generation_finished() {
        let event = DomainEvent::ImageGenerationFinished {
            call_id: Some("gen-1".to_string()),
            revised_prompt: None,
            result: None,
            saved_path: None,
        };
        let result: Option<DecisionEvent> = (&event).into();
        match result {
            Some(DecisionEvent::StatusUpdate { status }) => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }



    // ── DecisionEvent helper tests ──────────────────────────────

    #[test]
    fn decision_event_is_running() {
        assert!(DecisionEvent::ClaudeAssistantChunk { text: "hi".to_string() }.is_running());
        assert!(DecisionEvent::StatusUpdate { status: "ok".to_string() }.is_running());
        assert!(!DecisionEvent::Finished { summary: None }.is_running());
    }

    #[test]
    fn decision_event_needs_decision() {
        assert!(DecisionEvent::Finished { summary: None }.needs_decision());
        assert!(DecisionEvent::Error { message: "e".to_string(), error_type: None }.needs_decision());
        assert!(
            DecisionEvent::CodexApprovalRequest {
                method: "m".to_string(),
                params: serde_json::json!({}),
                request_id: None,
            }
            .needs_decision()
        );
        assert!(!DecisionEvent::ClaudeAssistantChunk { text: "hi".to_string() }.needs_decision());
    }

    #[test]
    fn decision_event_serde_roundtrip() {
        let event = DecisionEvent::Finished {
            summary: Some("done".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DecisionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
