//! Provider event types for classification

use serde::{Deserialize, Serialize};

/// Provider event - simplified for decision layer
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProviderEvent {
    // Common events
    /// Provider finished execution
    Finished { summary: Option<String> },

    /// Provider error
    Error {
        message: String,
        error_type: Option<String>,
    },

    // Claude-specific events
    /// Claude assistant chunk
    ClaudeAssistantChunk { text: String },

    /// Claude thinking chunk
    ClaudeThinkingChunk { text: String },

    /// Claude tool call started
    ClaudeToolCallStarted { name: String, input: Option<String> },

    /// Claude tool call finished
    ClaudeToolCallFinished {
        name: String,
        output: Option<String>,
        success: bool,
    },

    // Codex-specific events
    /// Codex approval request
    CodexApprovalRequest {
        method: String,
        params: serde_json::Value,
        request_id: Option<String>,
    },

    /// Codex patch apply started
    CodexPatchApplyStarted { path: String },

    /// Codex error
    CodexError { kind: String, message: String },

    // ACP-specific events (OpenCode/Kimi)
    /// ACP notification
    ACPNotification {
        method: String,
        params: serde_json::Value,
    },

    /// ACP error
    ACPError { code: String, message: String },

    // Generic running events
    /// Status update
    StatusUpdate { status: String },

    /// Session handle
    SessionHandle {
        session_id: String,
        info: Option<String>,
    },
}

impl ProviderEvent {
    /// Check if this is a running event (no decision needed)
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            ProviderEvent::ClaudeAssistantChunk { .. }
                | ProviderEvent::ClaudeThinkingChunk { .. }
                | ProviderEvent::ClaudeToolCallStarted { .. }
                | ProviderEvent::ClaudeToolCallFinished { .. }
                | ProviderEvent::CodexPatchApplyStarted { .. }
                | ProviderEvent::StatusUpdate { .. }
                | ProviderEvent::SessionHandle { .. }
        )
    }

    /// Check if this is a decision-triggering event
    pub fn needs_decision(&self) -> bool {
        matches!(
            self,
            ProviderEvent::Finished { .. }
                | ProviderEvent::Error { .. }
                | ProviderEvent::CodexApprovalRequest { .. }
                | ProviderEvent::CodexError { .. }
                | ProviderEvent::ACPNotification { .. }
                | ProviderEvent::ACPError { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_event_is_running() {
        let event = ProviderEvent::ClaudeAssistantChunk {
            text: "hello".to_string(),
        };
        assert!(event.is_running());
        assert!(!event.needs_decision());
    }

    #[test]
    fn test_provider_event_needs_decision() {
        let event = ProviderEvent::Finished { summary: None };
        assert!(event.needs_decision());
        assert!(!event.is_running());
    }

    #[test]
    fn test_provider_event_error() {
        let event = ProviderEvent::Error {
            message: "timeout".to_string(),
            error_type: Some("timeout".to_string()),
        };
        assert!(event.needs_decision());
    }

    #[test]
    fn test_provider_event_approval_request() {
        let event = ProviderEvent::CodexApprovalRequest {
            method: "execCommandApproval".to_string(),
            params: serde_json::json!({}),
            request_id: None,
        };
        assert!(event.needs_decision());
    }

    #[test]
    fn test_provider_event_serde() {
        let event = ProviderEvent::Finished {
            summary: Some("done".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: ProviderEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
