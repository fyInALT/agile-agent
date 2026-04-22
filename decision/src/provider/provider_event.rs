//! Provider event types for classification
//!
//! Re-exported from `agent-events::DecisionEvent` for backward compatibility.
//! All decision-layer event types now live in the shared kernel.

pub use agent_events::DecisionEvent as ProviderEvent;

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
