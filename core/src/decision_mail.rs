//! Decision Mail Types
//!
//! Message types for communication between work agents and decision agents.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐     ┌─────────────────────────────┐
//! │   Work Agent Slot    │     │   Decision Agent Slot       │
//! │                      │     │                              │
//! │  - Provider Thread   │◄───►│  - Decision Provider Thread  │
//! │  - event_rx          │     │  - TieredDecisionEngine      │
//! │                      │     │  - ClassifierRegistry        │
//! │                      │     │                              │
//! │  Sends:              │     │  Receives:                   │
//! │  DecisionRequest     │────▶│ DecisionRequest              │
//! │                      │     │                              │
//! │  Receives:           │     │  Sends:                      │
//! │  DecisionResponse    │◀────│ DecisionResponse             │
//! └─────────────────────┘     └─────────────────────────────┘
//! ```
//!
//! # Thread Safety
//!
//! Uses std::sync::mpsc channels for thread-safe message passing.
//! Main thread owns both mailboxes and processes them during event loop.

use std::sync::mpsc::{Receiver, Sender, channel};

use crate::agent_runtime::AgentId;

use agent_decision::context::DecisionContext;
use agent_decision::output::DecisionOutput;
use agent_decision::types::SituationType;

/// Request from work agent to decision agent
///
/// Sent when classifier detects a situation requiring decision.
pub struct DecisionRequest {
    /// The work agent ID that needs a decision
    pub work_agent_id: AgentId,
    /// The detected situation type
    pub situation_type: SituationType,
    /// Decision context (current state, rules, etc.)
    pub context: DecisionContext,
}

impl std::fmt::Debug for DecisionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionRequest")
            .field("work_agent_id", &self.work_agent_id)
            .field("situation_type", &self.situation_type)
            .field("context", &"<DecisionContext>")
            .finish()
    }
}

impl DecisionRequest {
    /// Create a new decision request
    pub fn new(
        work_agent_id: AgentId,
        situation_type: SituationType,
        context: DecisionContext,
    ) -> Self {
        Self {
            work_agent_id,
            situation_type,
            context,
        }
    }
}

/// Response from decision agent to work agent
///
/// Contains the decision output (actions to execute) or error.
pub struct DecisionResponse {
    /// The work agent ID that requested the decision
    pub work_agent_id: AgentId,
    /// The decision output (if successful)
    pub output: Option<DecisionOutput>,
    /// Error message (if decision failed)
    pub error: Option<String>,
}

impl std::fmt::Debug for DecisionResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionResponse")
            .field("work_agent_id", &self.work_agent_id)
            .field("output", &self.output.as_ref().map(|_| "<DecisionOutput>"))
            .field("error", &self.error)
            .finish()
    }
}

impl Clone for DecisionResponse {
    fn clone(&self) -> Self {
        // Note: DecisionOutput can't be cloned, so we only clone error case
        // For success case, we need to reconstruct from output data
        match &self.output {
            Some(output) => {
                // Clone what we can from DecisionOutput
                DecisionResponse {
                    work_agent_id: self.work_agent_id.clone(),
                    output: None, // Can't clone Box<dyn DecisionAction>
                    error: Some(format!(
                        "cloned response (original had {} actions)",
                        output.actions.len()
                    )),
                }
            }
            None => DecisionResponse {
                work_agent_id: self.work_agent_id.clone(),
                output: None,
                error: self.error.clone(),
            },
        }
    }
}

impl DecisionResponse {
    /// Create a successful decision response
    pub fn success(work_agent_id: AgentId, output: DecisionOutput) -> Self {
        Self {
            work_agent_id,
            output: Some(output),
            error: None,
        }
    }

    /// Create an error decision response
    pub fn error(work_agent_id: AgentId, error_message: String) -> Self {
        Self {
            work_agent_id,
            output: None,
            error: Some(error_message),
        }
    }

    /// Check if this response has a successful output
    pub fn is_success(&self) -> bool {
        self.output.is_some()
    }

    /// Check if this response has an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the output if available
    pub fn output(&self) -> Option<&DecisionOutput> {
        self.output.as_ref()
    }

    /// Get the error message if available
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Mail channel pair for decision communication
///
/// Contains channels for both request and response directions.
pub struct DecisionMail {
    /// Sender for decision requests (work agent -> decision agent)
    request_tx: Sender<DecisionRequest>,
    /// Receiver for decision requests (decision agent receives)
    request_rx: Receiver<DecisionRequest>,
    /// Sender for decision responses (decision agent -> work agent)
    response_tx: Sender<DecisionResponse>,
    /// Receiver for decision responses (work agent receives)
    response_rx: Receiver<DecisionResponse>,
}

impl DecisionMail {
    /// Create a new decision mail channel pair
    pub fn new() -> Self {
        let (request_tx, request_rx) = channel();
        let (response_tx, response_rx) = channel();
        Self {
            request_tx,
            request_rx,
            response_tx,
            response_rx,
        }
    }

    /// Get request sender (for work agent to send requests)
    pub fn request_sender(&self) -> Sender<DecisionRequest> {
        self.request_tx.clone()
    }

    /// Get request receiver (for decision agent to receive requests)
    pub fn request_receiver(&self) -> &Receiver<DecisionRequest> {
        &self.request_rx
    }

    /// Get response sender (for decision agent to send responses)
    pub fn response_sender(&self) -> Sender<DecisionResponse> {
        self.response_tx.clone()
    }

    /// Get response receiver (for work agent to receive responses)
    pub fn response_receiver(&self) -> &Receiver<DecisionResponse> {
        &self.response_rx
    }

    /// Split into sender and receiver halves
    ///
    /// Useful when one agent needs only send, another needs only receive.
    pub fn split(self) -> (DecisionMailSender, DecisionMailReceiver) {
        (
            DecisionMailSender {
                request_tx: self.request_tx,
                response_rx: self.response_rx,
            },
            DecisionMailReceiver {
                request_rx: self.request_rx,
                response_tx: self.response_tx,
            },
        )
    }
}

impl Default for DecisionMail {
    fn default() -> Self {
        Self::new()
    }
}

/// Sender half of decision mail
///
/// Used by work agent to send requests and receive responses.
pub struct DecisionMailSender {
    /// Sender for decision requests
    request_tx: Sender<DecisionRequest>,
    /// Receiver for decision responses
    response_rx: Receiver<DecisionResponse>,
}

impl DecisionMailSender {
    /// Send a decision request to the decision agent
    pub fn send_request(&self, request: DecisionRequest) -> Result<(), String> {
        self.request_tx
            .send(request)
            .map_err(|e| format!("Failed to send decision request: {}", e))
    }

    /// Try to receive a decision response (non-blocking)
    pub fn try_receive_response(&self) -> Option<DecisionResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Receive a decision response (blocking)
    pub fn receive_response(&self) -> Result<DecisionResponse, String> {
        self.response_rx
            .recv()
            .map_err(|e| format!("Failed to receive decision response: {}", e))
    }

    /// Receive response with timeout
    pub fn receive_response_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Option<DecisionResponse>, String> {
        match self.response_rx.recv_timeout(timeout) {
            Ok(response) => Ok(Some(response)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("Decision agent disconnected".to_string())
            }
        }
    }
}

/// Receiver half of decision mail
///
/// Used by decision agent to receive requests and send responses.
pub struct DecisionMailReceiver {
    /// Receiver for decision requests
    request_rx: Receiver<DecisionRequest>,
    /// Sender for decision responses
    response_tx: Sender<DecisionResponse>,
}

impl DecisionMailReceiver {
    /// Try to receive a decision request (non-blocking)
    pub fn try_receive_request(&self) -> Option<DecisionRequest> {
        self.request_rx.try_recv().ok()
    }

    /// Receive a decision request (blocking)
    pub fn receive_request(&self) -> Result<DecisionRequest, String> {
        self.request_rx
            .recv()
            .map_err(|e| format!("Failed to receive decision request: {}", e))
    }

    /// Receive request with timeout
    pub fn receive_request_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Option<DecisionRequest>, String> {
        match self.request_rx.recv_timeout(timeout) {
            Ok(request) => Ok(Some(request)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("Work agent disconnected".to_string())
            }
        }
    }

    /// Send a decision response to the work agent
    pub fn send_response(&self, response: DecisionResponse) -> Result<(), String> {
        self.response_tx
            .send(response)
            .map_err(|e| format!("Failed to send decision response: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_decision::builtin_situations::register_situation_builtins;
    use agent_decision::context::DecisionContext;
    use agent_decision::output::DecisionOutput;
    use agent_decision::situation_registry::SituationRegistry;
    use agent_decision::types::SituationType;

    fn make_test_agent_id() -> AgentId {
        AgentId::new("agent_001")
    }

    fn make_test_context() -> DecisionContext {
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);
        let situation = registry.build(SituationType::new("waiting_for_choice"));
        DecisionContext::new(situation, "test-agent")
    }

    #[test]
    fn test_decision_request_new() {
        let agent_id = make_test_agent_id();
        let situation_type = SituationType::new("waiting_for_choice");
        let context = make_test_context();

        let request = DecisionRequest::new(agent_id.clone(), situation_type.clone(), context);

        assert_eq!(request.work_agent_id, agent_id);
        assert_eq!(request.situation_type, situation_type);
    }

    #[test]
    fn test_decision_response_success() {
        let agent_id = make_test_agent_id();
        let output = DecisionOutput::new(Vec::new(), "Test reasoning");

        let response = DecisionResponse::success(agent_id.clone(), output);

        assert!(response.is_success());
        assert!(!response.is_error());
        assert!(response.output().is_some());
        assert!(response.error_message().is_none());
    }

    #[test]
    fn test_decision_response_error() {
        let agent_id = make_test_agent_id();

        let response = DecisionResponse::error(agent_id.clone(), "Test error".to_string());

        assert!(!response.is_success());
        assert!(response.is_error());
        assert!(response.output().is_none());
        assert_eq!(response.error_message(), Some("Test error"));
    }

    #[test]
    fn test_decision_mail_new() {
        let mail = DecisionMail::new();

        // Should have valid senders and receivers
        assert!(
            mail.request_sender()
                .send(DecisionRequest::new(
                    make_test_agent_id(),
                    SituationType::new("test"),
                    make_test_context(),
                ))
                .is_ok()
        );

        assert!(mail.request_receiver().try_recv().is_ok());
    }

    #[test]
    fn test_decision_mail_split() {
        let mail = DecisionMail::new();
        let (sender, receiver) = mail.split();

        // Sender can send request
        let request = DecisionRequest::new(
            make_test_agent_id(),
            SituationType::new("test"),
            make_test_context(),
        );
        assert!(sender.send_request(request).is_ok());

        // Receiver can receive request
        let received = receiver.try_receive_request();
        assert!(received.is_some());

        // Receiver can send response
        let response = DecisionResponse::success(
            make_test_agent_id(),
            DecisionOutput::new(Vec::new(), "test"),
        );
        assert!(receiver.send_response(response).is_ok());

        // Sender can receive response
        let response_received = sender.try_receive_response();
        assert!(response_received.is_some());
    }

    #[test]
    fn test_decision_mail_sender_receive_timeout() {
        let mail = DecisionMail::new();
        let (sender, _receiver) = mail.split();

        // No response available, should timeout
        let result = sender.receive_response_timeout(std::time::Duration::from_millis(10));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_decision_mail_receiver_receive_timeout() {
        let mail = DecisionMail::new();
        let (_sender, receiver) = mail.split();

        // No request available, should timeout
        let result = receiver.receive_request_timeout(std::time::Duration::from_millis(10));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_decision_mail_full_roundtrip() {
        let mail = DecisionMail::new();
        let (sender, receiver) = mail.split();

        // Work agent sends request
        let request = DecisionRequest::new(
            make_test_agent_id(),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Decision agent receives request
        let received_request = receiver.try_receive_request().unwrap();
        assert_eq!(
            received_request.situation_type,
            SituationType::new("waiting_for_choice")
        );

        // Decision agent sends response
        let response = DecisionResponse::success(
            received_request.work_agent_id,
            DecisionOutput::new(Vec::new(), "Decision made"),
        );
        receiver.send_response(response).unwrap();

        // Work agent receives response
        let received_response = sender.try_receive_response().unwrap();
        assert!(received_response.is_success());
        assert_eq!(
            received_response.output().unwrap().reasoning,
            "Decision made"
        );
    }
}
