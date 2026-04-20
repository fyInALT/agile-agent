//! Event conversion utilities for decision layer
//!
//! Provides conversion from core ProviderEvent types to decision layer
//! ProviderEvent types. This module bridges the provider event system
//! with the decision layer's event classification.

use crate::{ExecCommandStatus, PatchApplyStatus, ProviderEvent, SessionHandle};
use agent_decision::provider_event::ProviderEvent as DecisionProviderEvent;

/// Convert core ProviderEvent to decision layer ProviderEvent
pub fn convert_provider_event_to_decision(event: &ProviderEvent) -> DecisionProviderEvent {
    match event {
        ProviderEvent::Finished => DecisionProviderEvent::Finished { summary: None },
        ProviderEvent::Error(msg) => DecisionProviderEvent::Error {
            message: msg.clone(),
            error_type: None,
        },
        ProviderEvent::Status(text) => DecisionProviderEvent::StatusUpdate {
            status: text.clone(),
        },
        ProviderEvent::AssistantChunk(text) => DecisionProviderEvent::ClaudeAssistantChunk {
            text: text.clone(),
        },
        ProviderEvent::ThinkingChunk(text) => DecisionProviderEvent::ClaudeThinkingChunk {
            text: text.clone(),
        },
        ProviderEvent::SessionHandle(handle) => DecisionProviderEvent::SessionHandle {
            session_id: match handle {
                SessionHandle::ClaudeSession { session_id } => session_id.clone(),
                SessionHandle::CodexThread { thread_id } => thread_id.clone(),
            },
            info: None,
        },
        ProviderEvent::ExecCommandStarted { input_preview, .. } => {
            DecisionProviderEvent::ClaudeToolCallStarted {
                name: "exec".to_string(),
                input: input_preview.clone(),
            }
        }
        ProviderEvent::ExecCommandFinished {
            output_preview,
            status,
            ..
        } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: "exec".to_string(),
            output: output_preview.clone(),
            success: matches!(status, ExecCommandStatus::Completed),
        },
        ProviderEvent::GenericToolCallStarted {
            name,
            input_preview,
            ..
        } => DecisionProviderEvent::ClaudeToolCallStarted {
            name: name.clone(),
            input: input_preview.clone(),
        },
        ProviderEvent::GenericToolCallFinished {
            name,
            output_preview,
            success,
            ..
        } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: name.clone(),
            output: output_preview.clone(),
            success: *success,
        },
        ProviderEvent::PatchApplyStarted { changes, .. } => {
            // Extract first change path if available
            let path = changes.first()
                .map(|c| c.path.clone())
                .unwrap_or_default();
            DecisionProviderEvent::CodexPatchApplyStarted { path }
        }
        ProviderEvent::PatchApplyFinished { status, .. } => DecisionProviderEvent::StatusUpdate {
            status: match status {
                PatchApplyStatus::Completed => "patch completed".to_string(),
                PatchApplyStatus::Failed => "patch failed".to_string(),
                PatchApplyStatus::Declined => "patch declined".to_string(),
                PatchApplyStatus::InProgress => "patch in progress".to_string(),
            },
        },
        ProviderEvent::McpToolCallStarted { invocation, .. } => {
            DecisionProviderEvent::ClaudeToolCallStarted {
                name: "mcp".to_string(),
                input: Some(format!("{}:{}", invocation.server, invocation.tool)),
            }
        }
        ProviderEvent::McpToolCallFinished { error, .. } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: "mcp".to_string(),
            output: error.clone(),
            success: error.is_none(),
        },
        ProviderEvent::WebSearchStarted { .. } => DecisionProviderEvent::StatusUpdate {
            status: "websearch started".to_string(),
        },
        ProviderEvent::WebSearchFinished { .. } => DecisionProviderEvent::StatusUpdate {
            status: "websearch completed".to_string(),
        },
        ProviderEvent::ViewImage { .. }
        | ProviderEvent::ImageGenerationFinished { .. }
        | ProviderEvent::ExecCommandOutputDelta { .. }
        | ProviderEvent::PatchApplyOutputDelta { .. } => DecisionProviderEvent::StatusUpdate {
            status: "running".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::McpInvocation;
    use agent_toolkit::{McpToolCallStatus, PatchChange, PatchChangeKind};

    #[test]
    fn convert_finished_event() {
        let event = ProviderEvent::Finished;
        let result = convert_provider_event_to_decision(&event);
        assert!(matches!(result, DecisionProviderEvent::Finished { summary: None }));
    }

    #[test]
    fn convert_error_event() {
        let event = ProviderEvent::Error("test error".to_string());
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::Error { message, .. } => {
                assert_eq!(message, "test error");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn convert_status_event() {
        let event = ProviderEvent::Status("working".to_string());
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "working");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_assistant_chunk_event() {
        let event = ProviderEvent::AssistantChunk("hello".to_string());
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeAssistantChunk { text } => {
                assert_eq!(text, "hello");
            }
            _ => panic!("Expected ClaudeAssistantChunk variant"),
        }
    }

    #[test]
    fn convert_thinking_chunk_event() {
        let event = ProviderEvent::ThinkingChunk("thinking...".to_string());
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeThinkingChunk { text } => {
                assert_eq!(text, "thinking...");
            }
            _ => panic!("Expected ClaudeThinkingChunk variant"),
        }
    }

    #[test]
    fn convert_session_handle_claude() {
        let event = ProviderEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id: "sess-123".to_string(),
        });
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::SessionHandle { session_id, .. } => {
                assert_eq!(session_id, "sess-123");
            }
            _ => panic!("Expected SessionHandle variant"),
        }
    }

    #[test]
    fn convert_session_handle_codex() {
        let event = ProviderEvent::SessionHandle(SessionHandle::CodexThread {
            thread_id: "thread-456".to_string(),
        });
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::SessionHandle { session_id, .. } => {
                assert_eq!(session_id, "thread-456");
            }
            _ => panic!("Expected SessionHandle variant"),
        }
    }

    #[test]
    fn convert_exec_command_started() {
        let event = ProviderEvent::ExecCommandStarted {
            call_id: Some("cmd-1".to_string()),
            input_preview: Some("ls -la".to_string()),
            source: None,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallStarted { name, input } => {
                assert_eq!(name, "exec");
                assert_eq!(input, Some("ls -la".to_string()));
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_exec_command_finished_success() {
        let event = ProviderEvent::ExecCommandFinished {
            call_id: Some("cmd-1".to_string()),
            output_preview: Some("file1 file2".to_string()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(100),
            source: None,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallFinished { name, output, success } => {
                assert_eq!(name, "exec");
                assert_eq!(output, Some("file1 file2".to_string()));
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_exec_command_finished_failed() {
        let event = ProviderEvent::ExecCommandFinished {
            call_id: Some("cmd-2".to_string()),
            output_preview: None,
            status: ExecCommandStatus::Failed,
            exit_code: Some(1),
            duration_ms: Some(50),
            source: None,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallFinished { name, output: _, success } => {
                assert_eq!(name, "exec");
                assert!(!success, "Failed status should map to success=false");
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_generic_tool_call_started() {
        let event = ProviderEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            input_preview: Some("/path/to/file".to_string()),
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallStarted { name, input } => {
                assert_eq!(name, "read_file");
                assert_eq!(input, Some("/path/to/file".to_string()));
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_generic_tool_call_finished() {
        let event = ProviderEvent::GenericToolCallFinished {
            name: "read_file".to_string(),
            call_id: Some("tool-1".to_string()),
            output_preview: Some("file content".to_string()),
            success: true,
            exit_code: None,
            duration_ms: Some(50),
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallFinished { name, output, success } => {
                assert_eq!(name, "read_file");
                assert_eq!(output, Some("file content".to_string()));
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_patch_apply_started_with_path() {
        let event = ProviderEvent::PatchApplyStarted {
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
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::CodexPatchApplyStarted { path } => {
                assert_eq!(path, "/file.rs", "Should extract first change path");
            }
            _ => panic!("Expected CodexPatchApplyStarted variant"),
        }
    }

    #[test]
    fn convert_patch_apply_started_empty_changes() {
        let event = ProviderEvent::PatchApplyStarted {
            call_id: Some("patch-2".to_string()),
            changes: vec![],
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::CodexPatchApplyStarted { path } => {
                assert_eq!(path, "", "Empty changes should produce empty path");
            }
            _ => panic!("Expected CodexPatchApplyStarted variant"),
        }
    }

    #[test]
    fn convert_patch_apply_finished_completed() {
        let event = ProviderEvent::PatchApplyFinished {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Completed,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "patch completed");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_patch_apply_finished_failed() {
        let event = ProviderEvent::PatchApplyFinished {
            call_id: Some("patch-1".to_string()),
            changes: vec![],
            status: PatchApplyStatus::Failed,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "patch failed");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_started() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: None,
        };
        let event = ProviderEvent::McpToolCallStarted {
            call_id: Some("mcp-1".to_string()),
            invocation,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallStarted { name, input } => {
                assert_eq!(name, "mcp");
                assert_eq!(input, Some("test-server:test-tool".to_string()), "Should include server:tool");
            }
            _ => panic!("Expected ClaudeToolCallStarted variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_finished_with_error() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: None,
        };
        let event = ProviderEvent::McpToolCallFinished {
            call_id: Some("mcp-1".to_string()),
            invocation,
            result_blocks: vec![],
            error: Some("tool failed".to_string()),
            status: McpToolCallStatus::Failed,
            is_error: true,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallFinished { name, output, success } => {
                assert_eq!(name, "mcp");
                assert_eq!(output, Some("tool failed".to_string()));
                assert!(!success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_mcp_tool_call_finished_no_error() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: None,
        };
        let event = ProviderEvent::McpToolCallFinished {
            call_id: Some("mcp-1".to_string()),
            invocation,
            result_blocks: vec![],
            error: None,
            status: McpToolCallStatus::Completed,
            is_error: false,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::ClaudeToolCallFinished { name, output, success } => {
                assert_eq!(name, "mcp");
                assert!(output.is_none());
                assert!(success);
            }
            _ => panic!("Expected ClaudeToolCallFinished variant"),
        }
    }

    #[test]
    fn convert_websearch_started() {
        let event = ProviderEvent::WebSearchStarted {
            call_id: Some("ws-1".to_string()),
            query: "test query".to_string(),
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "websearch started");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_websearch_finished() {
        let event = ProviderEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "test query".to_string(),
            action: None,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "websearch completed", "Finished should be 'completed', not 'running'");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_exec_command_output_delta() {
        let event = ProviderEvent::ExecCommandOutputDelta {
            call_id: Some("cmd-1".to_string()),
            delta: "some output".to_string(),
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_view_image() {
        let event = ProviderEvent::ViewImage {
            call_id: Some("img-1".to_string()),
            path: "/image.png".to_string(),
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }

    #[test]
    fn convert_image_generation_finished() {
        let event = ProviderEvent::ImageGenerationFinished {
            call_id: Some("gen-1".to_string()),
            revised_prompt: None,
            result: None,
            saved_path: None,
        };
        let result = convert_provider_event_to_decision(&event);
        match result {
            DecisionProviderEvent::StatusUpdate { status } => {
                assert_eq!(status, "running");
            }
            _ => panic!("Expected StatusUpdate variant"),
        }
    }
}