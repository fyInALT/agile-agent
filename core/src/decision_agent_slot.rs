//! Decision Agent Slot
//!
//! Represents a decision agent's runtime slot in the agent pool.
//! Each decision agent is paired with a work agent and handles
//! decision-making requests from the classifier.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Decision Agent Slot                      │
//! │                                                               │
//! │  ┌─────────────────┐     ┌──────────────────────────────┐   │
//! │  │ Mail Receiver   │     │ Decision Engine              │   │
//! │  │                 │     │                              │   │
//! │  │ - request_rx    │────▶│ - TieredDecisionEngine       │   │
//! │  │                 │     │ - ClassifierRegistry         │   │
//! │  │                 │     │ - ActionRegistry             │   │
//! │  └─────────────────┘     └──────────────────────────────┘   │
//! │                                       │                       │
//! │                                       ▼                       │
//! │  ┌─────────────────┐     ┌──────────────────────────────┐   │
//! │  │ Mail Sender     │     │ Provider Thread              │   │
//! │  │                 │     │                              │   │
//! │  │ - response_tx   │◀────│ - Owns provider process      │   │
//! │  │                 │     │ - Sends/receives to LLM      │   │
//! │  └─────────────────┘     └──────────────────────────────┘   │
//! │                                                               │
//! │  Status: Idle | Thinking | Responding | Error | Stopped       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Thread Safety
//!
//! DecisionAgentSlot is owned by the main thread (TUI loop).
//! The decision provider thread sends events through the channel.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use std::time::Instant;

use crate::decision_mail::{DecisionMailReceiver, DecisionRequest, DecisionResponse};
use crate::logging;
use crate::provider::ProviderKind;

use agent_decision::action_registry::ActionRegistry;
use agent_decision::builtin_actions::register_action_builtins;
use agent_decision::engine::DecisionEngine;
use agent_decision::initializer::DecisionLayerComponents;
use agent_decision::tiered_engine::{TieredDecisionEngine, TieredEngineConfig};
use agent_decision::llm_engine::LLMEngineConfig;
use agent_decision::provider_kind::ProviderKind as DecisionProviderKind;
use agent_decision::provider_event::ProviderEvent;

/// Status of a decision agent slot
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionAgentStatus {
    /// Decision agent is idle, waiting for requests
    Idle,
    /// Decision agent is thinking (processing request)
    Thinking { started_at: Instant },
    /// Decision agent is responding (sending result)
    Responding,
    /// Decision agent encountered an error
    Error { message: String },
    /// Decision agent has been stopped
    Stopped { reason: String },
}

impl DecisionAgentStatus {
    /// Create idle status
    pub fn idle() -> Self {
        Self::Idle
    }

    /// Create thinking status with current timestamp
    pub fn thinking_now() -> Self {
        Self::Thinking {
            started_at: Instant::now(),
        }
    }

    /// Create responding status
    pub fn responding() -> Self {
        Self::Responding
    }

    /// Create error status
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Create stopped status
    pub fn stopped(reason: impl Into<String>) -> Self {
        Self::Stopped {
            reason: reason.into(),
        }
    }

    /// Check if agent is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Check if agent is thinking
    pub fn is_thinking(&self) -> bool {
        matches!(self, Self::Thinking { .. })
    }

    /// Check if agent is responding
    pub fn is_responding(&self) -> bool {
        matches!(self, Self::Responding)
    }

    /// Check if agent is active (thinking or responding)
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Thinking { .. } | Self::Responding)
    }

    /// Check if agent is stopped
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped { .. })
    }

    /// Check if agent has error
    pub fn has_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Get elapsed time since thinking started
    pub fn thinking_elapsed(&self) -> Option<std::time::Duration> {
        match self {
            Self::Thinking { started_at } => Some(started_at.elapsed()),
            _ => None,
        }
    }

    /// Get a human-readable label for the status
    pub fn label(&self) -> String {
        match self {
            Self::Idle => "idle".to_string(),
            Self::Thinking { .. } => "thinking".to_string(),
            Self::Responding => "responding".to_string(),
            Self::Error { message } => format!("error:{}", message),
            Self::Stopped { reason } => format!("stopped:{}", reason),
        }
    }
}

/// A decision agent's runtime slot
///
/// Contains all state for managing one decision agent's execution,
/// including decision engine, mail channels, and provider thread.
pub struct DecisionAgentSlot {
    /// Work agent ID this decision agent is paired with
    work_agent_id: String,
    /// Decision agent's own ID (derived from work agent)
    agent_id: String,
    /// Provider kind (same as work agent)
    provider_kind: ProviderKind,
    /// Current status
    status: DecisionAgentStatus,
    /// Decision engine for making decisions
    engine: TieredDecisionEngine,
    /// Action registry for decision actions
    action_registry: ActionRegistry,
    /// Mail receiver for communication with work agent
    mail_receiver: DecisionMailReceiver,
    /// Provider thread handle for LLM calls (optional, created when needed)
    provider_thread: Option<JoinHandle<()>>,
    /// Provider event receiver (optional)
    event_rx: Option<Receiver<ProviderEvent>>,
    /// Working directory for provider execution
    cwd: PathBuf,
    /// Last activity timestamp
    last_activity: Instant,
    /// Decision count for statistics
    decision_count: u64,
    /// Error count for monitoring
    error_count: u64,
}

impl std::fmt::Debug for DecisionAgentSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionAgentSlot")
            .field("work_agent_id", &self.work_agent_id)
            .field("agent_id", &self.agent_id)
            .field("provider_kind", &self.provider_kind)
            .field("status", &self.status)
            .field("has_provider_thread", &self.provider_thread.is_some())
            .field("cwd", &self.cwd)
            .field("decision_count", &self.decision_count)
            .field("error_count", &self.error_count)
            .finish()
    }
}

impl DecisionAgentSlot {
    /// Create a new decision agent slot
    ///
    /// # Arguments
    ///
    /// * `work_agent_id` - The work agent ID this decision agent is paired with
    /// * `provider_kind` - Provider type (same as work agent)
    /// * `mail_receiver` - Mail receiver for requests from work agent
    /// * `cwd` - Working directory for provider execution
    /// * `components` - Decision layer components (for initialization)
    pub fn new(
        work_agent_id: String,
        provider_kind: ProviderKind,
        mail_receiver: DecisionMailReceiver,
        cwd: PathBuf,
        _components: &DecisionLayerComponents,
    ) -> Self {
        let agent_id = format!("decision-{}", work_agent_id);

        // Convert core ProviderKind to decision ProviderKind
        let decision_provider = match provider_kind {
            ProviderKind::Claude => DecisionProviderKind::Claude,
            ProviderKind::Codex => DecisionProviderKind::Codex,
            ProviderKind::Mock => DecisionProviderKind::Unknown, // Mock doesn't need decisions
        };

        // Create tiered engine with same provider as work agent
        let engine_config = TieredEngineConfig {
            llm_provider: decision_provider,
            llm_config: LLMEngineConfig::default(),
            use_cli_for_critical: false, // Decision agents don't use CLI
            fallback_tier: agent_decision::tiered_engine::DecisionTier::Medium,
        };
        let engine = TieredDecisionEngine::new(engine_config);

        // Create action registry with builtins
        let action_registry = ActionRegistry::new();
        register_action_builtins(&action_registry);

        Self {
            work_agent_id,
            agent_id,
            provider_kind,
            status: DecisionAgentStatus::idle(),
            engine,
            action_registry,
            mail_receiver,
            provider_thread: None,
            event_rx: None,
            cwd,
            last_activity: Instant::now(),
            decision_count: 0,
            error_count: 0,
        }
    }

    /// Get the work agent ID
    pub fn work_agent_id(&self) -> &str {
        &self.work_agent_id
    }

    /// Get the decision agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Get the provider kind
    pub fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    /// Get the current status
    pub fn status(&self) -> &DecisionAgentStatus {
        &self.status
    }

    /// Get decision count
    pub fn decision_count(&self) -> u64 {
        self.decision_count
    }

    /// Get error count
    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    /// Check if agent is active (thinking or responding)
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Check if agent is idle
    pub fn is_idle(&self) -> bool {
        self.status.is_idle()
    }

    /// Try to receive a decision request (non-blocking)
    ///
    /// Returns None if no request is pending.
    pub fn try_receive_request(&self) -> Option<DecisionRequest> {
        self.mail_receiver.try_receive_request()
    }

    /// Process a decision request
    ///
    /// This is the main decision-making logic:
    /// 1. Set status to Thinking
    /// 2. Call decision engine
    /// 3. Send response to work agent
    /// 4. Update statistics
    pub fn process_request(&mut self, request: DecisionRequest) -> DecisionResponse {
        // Set status to thinking
        self.status = DecisionAgentStatus::thinking_now();
        self.last_activity = Instant::now();

        logging::debug_event(
            "decision_agent.processing",
            "processing decision request",
            serde_json::json!({
                "agent_id": self.agent_id,
                "work_agent_id": request.work_agent_id.as_str(),
                "situation_type": request.situation_type.name,
            }),
        );

        // Make decision using engine - pass context directly (not cloned)
        let result = self.engine.decide(request.context, &self.action_registry);

        match result {
            Ok(output) => {
                self.decision_count += 1;
                self.status = DecisionAgentStatus::responding();

                // Send success response
                let response = DecisionResponse::success(request.work_agent_id.clone(), output);

                if let Err(e) = self.mail_receiver.send_response(response.clone()) {
                    self.error_count += 1;
                    self.status = DecisionAgentStatus::error(e.clone());
                    DecisionResponse::error(request.work_agent_id.clone(), e)
                } else {
                    // Return to idle after successful send
                    self.status = DecisionAgentStatus::idle();
                    response
                }
            }
            Err(e) => {
                self.error_count += 1;
                let error_msg = e.to_string();
                self.status = DecisionAgentStatus::error(error_msg.clone());

                logging::warn_event(
                    "decision_agent.error",
                    "decision engine error",
                    serde_json::json!({
                        "agent_id": self.agent_id,
                        "error": error_msg,
                    }),
                );

                // Send error response
                let response = DecisionResponse::error(request.work_agent_id.clone(), error_msg);
                if let Err(send_err) = self.mail_receiver.send_response(response.clone()) {
                    DecisionResponse::error(request.work_agent_id.clone(), send_err)
                } else {
                    response
                }
            }
        }
    }

    /// Poll for pending requests and process them
    ///
    /// Returns the number of requests processed.
    pub fn poll_and_process(&mut self) -> usize {
        let mut processed = 0;

        // Only process if idle
        if !self.status.is_idle() {
            return 0;
        }

        // Try to receive and process one request
        if let Some(request) = self.try_receive_request() {
            self.process_request(request);
            processed += 1;
        }

        processed
    }

    /// Stop the decision agent
    ///
    /// Gracefully shuts down any provider thread and marks as stopped.
    pub fn stop(&mut self, reason: impl Into<String>) {
        // Stop provider thread if running
        if let Some(thread) = self.provider_thread.take() {
            // Drop the thread handle - thread will clean up
            // In production, we'd want to join with timeout
            drop(thread);
        }

        self.status = DecisionAgentStatus::stopped(reason);

        logging::debug_event(
            "decision_agent.stopped",
            "decision agent stopped",
            serde_json::json!({
                "agent_id": self.agent_id,
                "decision_count": self.decision_count,
                "error_count": self.error_count,
            }),
        );
    }

    /// Reset error status and return to idle
    ///
    /// Called when the work agent wants to recover from errors.
    pub fn reset_error(&mut self) {
        if self.status.has_error() {
            self.status = DecisionAgentStatus::idle();
            logging::debug_event(
                "decision_agent.reset",
                "decision agent reset from error",
                serde_json::json!({
                    "agent_id": self.agent_id,
                }),
            );
        }
    }

    /// Get last activity timestamp
    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    /// Get elapsed time since last activity
    pub fn elapsed_since_activity(&self) -> std::time::Duration {
        self.last_activity.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision_mail::DecisionMail;
    use agent_decision::builtin_situations::register_situation_builtins;
    use agent_decision::context::DecisionContext;
    use agent_decision::initializer::{DecisionLayerComponents, initialize_decision_layer};
    use agent_decision::situation_registry::SituationRegistry;
    use agent_decision::types::SituationType;
    use tempfile::TempDir;

    fn make_test_components() -> DecisionLayerComponents {
        initialize_decision_layer()
    }

    fn make_test_context() -> DecisionContext {
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);
        let situation = registry.build(SituationType::new("waiting_for_choice"));
        DecisionContext::new(situation, "test-agent")
    }

    fn make_test_slot() -> (DecisionAgentSlot, crate::decision_mail::DecisionMailSender) {
        let mail = DecisionMail::new();
        let (sender, receiver) = mail.split();

        let temp_dir = TempDir::new().unwrap();
        let components = make_test_components();

        let slot = DecisionAgentSlot::new(
            "agent_001".to_string(),
            ProviderKind::Claude,
            receiver,
            temp_dir.path().to_path_buf(),
            &components,
        );

        (slot, sender)
    }

    #[test]
    fn test_decision_agent_status_idle() {
        let status = DecisionAgentStatus::idle();
        assert!(status.is_idle());
        assert!(!status.is_thinking());
        assert!(!status.is_responding());
        assert!(!status.is_active());
        assert!(!status.is_stopped());
        assert!(!status.has_error());
        assert_eq!(status.label(), "idle");
    }

    #[test]
    fn test_decision_agent_status_thinking() {
        let status = DecisionAgentStatus::thinking_now();
        assert!(status.is_thinking());
        assert!(status.is_active());
        assert!(!status.is_idle());
        assert!(status.thinking_elapsed().is_some());
        assert_eq!(status.label(), "thinking");
    }

    #[test]
    fn test_decision_agent_status_responding() {
        let status = DecisionAgentStatus::responding();
        assert!(status.is_responding());
        assert!(status.is_active());
        assert!(!status.is_idle());
        assert_eq!(status.label(), "responding");
    }

    #[test]
    fn test_decision_agent_status_error() {
        let status = DecisionAgentStatus::error("test error");
        assert!(status.has_error());
        assert!(!status.is_idle());
        assert_eq!(status.label(), "error:test error");
    }

    #[test]
    fn test_decision_agent_status_stopped() {
        let status = DecisionAgentStatus::stopped("graceful shutdown");
        assert!(status.is_stopped());
        assert!(!status.is_idle());
        assert_eq!(status.label(), "stopped:graceful shutdown");
    }

    #[test]
    fn test_decision_agent_slot_new() {
        let (slot, _) = make_test_slot();

        assert_eq!(slot.work_agent_id(), "agent_001");
        assert_eq!(slot.agent_id(), "decision-agent_001");
        assert_eq!(slot.provider_kind(), ProviderKind::Claude);
        assert!(slot.status().is_idle());
        assert_eq!(slot.decision_count(), 0);
        assert_eq!(slot.error_count(), 0);
    }

    #[test]
    fn test_decision_agent_slot_try_receive_request_empty() {
        let (slot, _) = make_test_slot();

        // No request pending
        assert!(slot.try_receive_request().is_none());
    }

    #[test]
    fn test_decision_agent_slot_process_request() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Slot should receive the request
        let received = slot.try_receive_request();
        assert!(received.is_some());

        // Process the request
        let response = slot.process_request(received.unwrap());

        // Response should be successful
        assert!(response.is_success());
        assert!(response.output().is_some());

        // Slot should have made one decision
        assert_eq!(slot.decision_count(), 1);
        assert_eq!(slot.error_count(), 0);

        // Status should be back to idle after processing
        assert!(slot.status().is_idle());
    }

    #[test]
    fn test_decision_agent_slot_poll_and_process() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Poll and process
        let processed = slot.poll_and_process();
        assert_eq!(processed, 1);
        assert_eq!(slot.decision_count(), 1);

        // Poll again - should be 0 since no more requests
        let processed_again = slot.poll_and_process();
        assert_eq!(processed_again, 0);
    }

    #[test]
    fn test_decision_agent_slot_stop() {
        let (mut slot, _) = make_test_slot();

        slot.stop("test stop");

        assert!(slot.status().is_stopped());
        assert_eq!(slot.status().label(), "stopped:test stop");
    }

    #[test]
    fn test_decision_agent_slot_reset_error() {
        let (mut slot, _) = make_test_slot();

        // Set to error status
        slot.status = DecisionAgentStatus::error("test error");
        assert!(slot.status().has_error());

        // Reset error
        slot.reset_error();

        assert!(slot.status().is_idle());
    }

    #[test]
    fn test_decision_agent_slot_elapsed_since_activity() {
        let (slot, _) = make_test_slot();

        // Elapsed should be small initially
        let elapsed = slot.elapsed_since_activity();
        assert!(elapsed < std::time::Duration::from_secs(1));
    }
}