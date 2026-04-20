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
        ProviderEvent::PatchApplyStarted { .. } => DecisionProviderEvent::CodexPatchApplyStarted {
            path: "".to_string(),
        },
        ProviderEvent::PatchApplyFinished { status, .. } => DecisionProviderEvent::StatusUpdate {
            status: match status {
                PatchApplyStatus::Completed => "patch completed".to_string(),
                PatchApplyStatus::Failed => "patch failed".to_string(),
                PatchApplyStatus::Declined => "patch declined".to_string(),
                PatchApplyStatus::InProgress => "patch in progress".to_string(),
            },
        },
        ProviderEvent::McpToolCallStarted { .. } => DecisionProviderEvent::ClaudeToolCallStarted {
            name: "mcp".to_string(),
            input: None,
        },
        ProviderEvent::McpToolCallFinished { error, .. } => DecisionProviderEvent::ClaudeToolCallFinished {
            name: "mcp".to_string(),
            output: error.clone(),
            success: error.is_none(),
        },
        ProviderEvent::WebSearchStarted { .. }
        | ProviderEvent::WebSearchFinished { .. }
        | ProviderEvent::ViewImage { .. }
        | ProviderEvent::ImageGenerationFinished { .. }
        | ProviderEvent::ExecCommandOutputDelta { .. }
        | ProviderEvent::PatchApplyOutputDelta { .. } => DecisionProviderEvent::StatusUpdate {
            status: "running".to_string(),
        },
    }
}