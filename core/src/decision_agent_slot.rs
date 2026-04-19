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
use std::sync::Arc;
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
use agent_decision::LLMCaller;
use agent_decision::LLMEngineConfig;
use agent_decision::provider::ProviderEvent;
use agent_decision::provider::ProviderKind as DecisionProviderKind;
use agent_decision::TieredDecisionEngine;
use agent_decision::TieredEngineConfig;

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
    /// Provider event receiver (optional, for future use)
    #[allow(dead_code)]
    event_rx: Option<Receiver<ProviderEvent>>,
    /// Working directory for provider execution
    cwd: PathBuf,
    /// Last activity timestamp
    last_activity: Instant,
    /// Decision count for statistics
    decision_count: u64,
    /// Error count for monitoring
    error_count: u64,
    /// Profile ID for decision layer (optional)
    profile_id: Option<String>,
    /// Current reflection round for claims_completion tracking
    /// This persists across decision cycles to enforce max reflection limit
    reflection_round: u8,
    /// Shared pending reflection round from async processing
    /// The async thread writes the updated reflection_round here after decision
    pending_reflection_round: std::sync::Arc<std::sync::Mutex<Option<u8>>>,
    /// Fallback response storage for when async channel send fails
    /// This allows the main thread to retrieve the response on next poll
    pending_fallback_response: std::sync::Arc<std::sync::Mutex<Option<DecisionResponse>>>,
    /// Timestamp when the last decision started for this agent
    /// Used by UI to show "Analyzing" for a minimum duration even for fast decisions
    last_decision_started_at: Option<std::time::Instant>,
}

impl std::fmt::Debug for DecisionAgentSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_pending_fallback = self.pending_fallback_response
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false);
        f.debug_struct("DecisionAgentSlot")
            .field("work_agent_id", &self.work_agent_id)
            .field("agent_id", &self.agent_id)
            .field("provider_kind", &self.provider_kind)
            .field("status", &self.status)
            .field("has_provider_thread", &self.provider_thread.is_some())
            .field("cwd", &self.cwd)
            .field("decision_count", &self.decision_count)
            .field("error_count", &self.error_count)
            .field("profile_id", &self.profile_id)
            .field("reflection_round", &self.reflection_round)
            .field("has_pending_fallback", &has_pending_fallback)
            .field("has_recent_decision", &self.last_decision_started_at.is_some())
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
            fallback_tier: agent_decision::engine::DecisionTier::Medium,
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
            profile_id: None,
            reflection_round: 0,
            pending_reflection_round: std::sync::Arc::new(std::sync::Mutex::new(None)),
            pending_fallback_response: std::sync::Arc::new(std::sync::Mutex::new(None)),
            last_decision_started_at: None,
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

    /// Get current reflection round
    pub fn reflection_round(&self) -> u8 {
        self.reflection_round
    }

    /// Get profile ID
    pub fn profile_id(&self) -> Option<&String> {
        self.profile_id.as_ref()
    }

    /// Set profile ID
    pub fn set_profile_id(&mut self, id: String) {
        self.profile_id = Some(id);
    }

    /// Check if agent has a profile set
    pub fn has_profile(&self) -> bool {
        self.profile_id.is_some()
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

        // Log: Decision triggered
        let situation_prompt = request.context.trigger_situation.to_prompt_text();
        let available_actions = request
            .context
            .trigger_situation
            .available_actions()
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>();

        logging::debug_event(
            "decision_layer.triggered",
            "decision layer triggered by work agent event",
            serde_json::json!({
                "decision_agent_id": self.agent_id,
                "work_agent_id": request.work_agent_id.as_str(),
                "situation_type": request.situation_type.name,
                "situation_prompt": situation_prompt,
                "available_actions": available_actions,
                "requires_human": request.context.trigger_situation.requires_human(),
            }),
        );

        // Build and log the prompt sent to decision engine
        let decision_prompt = self
            .engine
            .build_prompt(&request.context, &self.action_registry);
        logging::debug_event(
            "decision_layer.prompt_sent",
            "prompt sent to decision engine",
            serde_json::json!({
                "decision_agent_id": self.agent_id,
                "work_agent_id": request.work_agent_id.as_str(),
                "prompt_length": decision_prompt.len(),
                "prompt_preview": if decision_prompt.len() > 500 {
                    format!("{}...[truncated]", &decision_prompt[..500])
                } else {
                    decision_prompt.clone()
                },
            }),
        );

        // Make decision using engine - pass context directly (not cloned)
        // First, inject current reflection_round into context for proper tracking
        let context_with_reflection = request.context.with_reflection_round(self.reflection_round);
        let result = self.engine.decide(context_with_reflection, &self.action_registry);

        match result {
            Ok(output) => {
                self.decision_count += 1;
                // Sync reflection_round from engine after decision (engine increments on reflect)
                self.reflection_round = self.engine.reflection_round();
                self.status = DecisionAgentStatus::responding();

                // Log: Decision engine response
                let action_types = output
                    .actions
                    .iter()
                    .map(|a| a.action_type().name.clone())
                    .collect::<Vec<_>>();
                let action_params = output
                    .actions
                    .iter()
                    .map(|a| a.serialize_params())
                    .collect::<Vec<_>>();

                logging::debug_event(
                    "decision_layer.engine_response",
                    "decision engine returned response",
                    serde_json::json!({
                        "decision_agent_id": self.agent_id,
                        "work_agent_id": request.work_agent_id.as_str(),
                        "action_types": action_types,
                        "action_params": action_params,
                        "reasoning": output.reasoning,
                        "confidence": output.confidence,
                        "tier": self.engine.tier_stats().total,
                    }),
                );

                // Send success response
                let response = DecisionResponse::success(request.work_agent_id.clone(), output);

                if let Err(e) = self.mail_receiver.send_response(response.clone()) {
                    self.error_count += 1;
                    self.status = DecisionAgentStatus::error(e.clone());

                    logging::warn_event(
                        "decision_layer.response_send_failed",
                        "failed to send decision response to work agent",
                        serde_json::json!({
                            "decision_agent_id": self.agent_id,
                            "work_agent_id": request.work_agent_id.as_str(),
                            "error": e,
                        }),
                    );

                    DecisionResponse::error(request.work_agent_id.clone(), e)
                } else {
                    // Log: Response sent to work agent
                    logging::debug_event(
                        "decision_layer.response_sent",
                        "decision response sent to work agent",
                        serde_json::json!({
                            "decision_agent_id": self.agent_id,
                            "work_agent_id": request.work_agent_id.as_str(),
                            "status": "success",
                        }),
                    );

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
                    "decision_layer.engine_error",
                    "decision engine returned error",
                    serde_json::json!({
                        "decision_agent_id": self.agent_id,
                        "work_agent_id": request.work_agent_id.as_str(),
                        "error": error_msg,
                        "situation_type": request.situation_type.name,
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
    /// Note: This method is non-blocking - it spawns a thread for LLM processing
    /// and returns immediately. The response is sent via the mail channel.
    /// After calling this, the caller should call `clear_thinking_status()`
    /// when responses have been collected.
    pub fn poll_and_process(&mut self) -> usize {
        let mut processed = 0;

        // If not idle, a decision is in progress in another thread - don't process
        if !self.status.is_idle() {
            return 0;
        }

        // Try to receive and process one request (spawns thread, non-blocking)
        if let Some(request) = self.try_receive_request() {
            // Increment decision count immediately when spawning (not after completion)
            // This reflects that a decision request has been initiated
            self.decision_count += 1;
            self.spawn_async_processing(request);
            processed += 1;
        }

        processed
    }

    /// Clear thinking status after async processing is complete
    ///
    /// Called by the owner after it has collected the response from the mail channel.
    pub fn clear_thinking_status(&mut self, had_error: bool) {
        if self.status.is_thinking() {
            // Sync pending reflection round from async thread before clearing status
            // Reset to None after consuming to avoid stale value on next call
            if let Ok(mut guard) = self.pending_reflection_round.lock() {
                if let Some(r) = *guard {
                    self.reflection_round = r;
                }
                *guard = None;
            }

            // If async thread reported an error, increment error count
            if had_error {
                self.error_count += 1;
            }

            self.status = DecisionAgentStatus::idle();
        }
    }

    /// Spawn asynchronous decision processing in a background thread
    ///
    /// This prevents LLM calls from blocking the TUI.
    fn spawn_async_processing(&mut self, request: DecisionRequest) {
        // Clone the response sender for the thread
        let response_tx = self.mail_receiver.clone_response_tx();

        // Take ownership of what we need for the thread
        let engine_config = self.engine.config();
        let work_agent_id = request.work_agent_id.clone();
        let agent_id = self.agent_id.clone();
        let reflection_round = self.reflection_round;
        let pending_reflection = self.pending_reflection_round.clone();
        let pending_fallback = self.pending_fallback_response.clone();

        // Set status to thinking
        self.status = DecisionAgentStatus::thinking_now();
        self.last_activity = Instant::now();
        // Record when decision started for UI display (ensures "Analyzing" shows even for fast decisions)
        self.last_decision_started_at = Some(Instant::now());

        // Log: Decision triggered
        logging::debug_event(
            "decision_layer.triggered",
            "decision layer triggered by work agent event (async)",
            serde_json::json!({
                "decision_agent_id": agent_id,
                "work_agent_id": work_agent_id.as_str(),
                "situation_type": request.situation_type.name,
            }),
        );

        // Spawn background thread for LLM processing
        std::thread::spawn(move || {
            // Create engine in thread (can't move self.engine due to borrow)
            let mut engine = TieredDecisionEngine::new(engine_config);
            let action_registry = ActionRegistry::new();
            register_action_builtins(&action_registry);

            // Build prompt
            let decision_prompt = engine
                .build_prompt(&request.context, &action_registry);

            logging::debug_event(
                "decision_layer.prompt_sent",
                "prompt sent to decision engine (async)",
                serde_json::json!({
                    "decision_agent_id": agent_id,
                    "work_agent_id": work_agent_id.as_str(),
                    "prompt_length": decision_prompt.len(),
                }),
            );

            // Make decision - inject reflection_round
            let context_with_reflection = request.context.with_reflection_round(reflection_round);
            let result = engine.decide(context_with_reflection, &action_registry);

            // Sync reflection_round from engine after decision
            let updated_reflection = engine.reflection_round();
            if let Ok(mut guard) = pending_reflection.lock() {
                *guard = Some(updated_reflection);
            } else {
                // Log error instead of silently dropping - this indicates mutex poisoning
                logging::error_event(
                    "decision_layer.reflection_round_sync_failed",
                    "failed to lock pending_reflection_round mutex",
                    serde_json::json!({
                        "decision_agent_id": agent_id,
                        "work_agent_id": work_agent_id.as_str(),
                        "reflection_round_lost": updated_reflection,
                    }),
                );
            }

            let response = match result {
                Ok(output) => {
                    logging::debug_event(
                        "decision_layer.engine_response",
                        "decision engine returned response (async)",
                        serde_json::json!({
                            "decision_agent_id": agent_id,
                            "work_agent_id": work_agent_id.as_str(),
                            "confidence": output.confidence,
                            "reflection_round": updated_reflection,
                        }),
                    );
                    DecisionResponse::success(work_agent_id.clone(), output)
                }
                Err(e) => {
                    logging::warn_event(
                        "decision_layer.engine_error",
                        "decision engine returned error (async)",
                        serde_json::json!({
                            "decision_agent_id": agent_id,
                            "work_agent_id": work_agent_id.as_str(),
                            "error": e.to_string(),
                        }),
                    );
                    DecisionResponse::error(work_agent_id.clone(), e.to_string())
                }
            };

            // Send response via channel (non-blocking for TUI)
            // Clone response first so we can use it for fallback if send fails
            let response_clone = response.clone();
            if let Err(e) = response_tx.send(response) {
                // Channel send failed - store in fallback for main thread to retrieve
                // This ensures the decision is not lost even if the channel is full
                if let Ok(mut guard) = pending_fallback.lock() {
                    // Overwrite any previous fallback (shouldn't happen, but be safe)
                    *guard = Some(response_clone);
                    logging::warn_event(
                        "decision_layer.async_response_fallback_stored",
                        "response stored in fallback due to channel send failure",
                        serde_json::json!({
                            "decision_agent_id": agent_id,
                            "work_agent_id": work_agent_id.as_str(),
                        }),
                    );
                } else {
                    // Even fallback lock failed - this is a critical error
                    logging::error_event(
                        "decision_layer.async_response_lost",
                        "failed to store response in fallback - decision lost",
                        serde_json::json!({
                            "decision_agent_id": agent_id,
                            "work_agent_id": work_agent_id.as_str(),
                            "error": e.to_string(),
                        }),
                    );
                }
            }
        });
    }

    /// Check if async processing completed and clear status if so
    ///
    /// This should be called by the owner after poll_decision_agents collects responses.
    /// If we were thinking and responses were collected, we can return to idle.
    pub fn mark_response_collected(&mut self) {
        if self.status.is_thinking() {
            // Also sync reflection_round like clear_thinking_status does
            if let Ok(mut guard) = self.pending_reflection_round.lock() {
                if let Some(r) = *guard {
                    self.reflection_round = r;
                }
                *guard = None;
            }
            self.status = DecisionAgentStatus::idle();
        }
    }

    /// Take and return any pending fallback response
    ///
    /// This is called by the owner when the channel-based response wasn't received
    /// but a fallback was stored (due to channel send failure).
    /// Returns Some(response) if a fallback exists, None otherwise.
    pub fn take_fallback_response(&mut self) -> Option<DecisionResponse> {
        if let Ok(mut guard) = self.pending_fallback_response.lock() {
            guard.take()
        } else {
            None
        }
    }

    /// Check if a fallback response is available
    pub fn has_fallback_response(&self) -> bool {
        self.pending_fallback_response
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }

    /// Get timestamp when the last decision started
    pub fn last_decision_started_at(&self) -> Option<std::time::Instant> {
        self.last_decision_started_at
    }

    /// Check if there's a recent decision that should still show "Analyzing"
    /// Returns Some(started_at) if decision started within the display window
    pub fn has_recent_decision(&self) -> bool {
        const MIN_DECISION_DISPLAY_MS: u64 = 1500;
        if let Some(started_at) = self.last_decision_started_at {
            let elapsed = std::time::Instant::now().duration_since(started_at);
            elapsed.as_millis() < MIN_DECISION_DISPLAY_MS as u128
        } else {
            false
        }
    }

    /// Clear the recent decision timestamp
    /// Called when the decision display window has passed
    pub fn clear_recent_decision(&mut self) {
        self.last_decision_started_at = None;
    }

    /// Stop the decision agent
    ///
    /// Gracefully shuts down any provider thread and marks as stopped.
    pub fn stop(&mut self, reason: impl Into<String>) {
        let reason_str = reason.into();

        // Stop provider thread if running
        if let Some(thread) = self.provider_thread.take() {
            // Drop the thread handle - thread will clean up
            // In production, we'd want to join with timeout
            drop(thread);
        }

        self.status = DecisionAgentStatus::stopped(reason_str.clone());

        logging::debug_event(
            "decision_layer.terminated",
            "decision agent terminated",
            serde_json::json!({
                "decision_agent_id": self.agent_id,
                "reason": reason_str,
                "total_decisions": self.decision_count,
                "total_errors": self.error_count,
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
                "decision_layer.reset",
                "decision agent reset from error state",
                serde_json::json!({
                    "decision_agent_id": self.agent_id,
                }),
            );
        }
    }

    /// Set LLM caller for real provider calls
    ///
    /// This injects a real provider caller into the decision engine,
    /// replacing the mock caller used in tests.
    pub fn set_llm_caller(&mut self, caller: Arc<dyn LLMCaller>) {
        self.engine.set_llm_caller(caller);
        logging::debug_event(
            "decision_agent.llm_caller_set",
            "LLM caller injected into decision engine",
            serde_json::json!({
                "agent_id": self.agent_id,
            }),
        );
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
    fn test_poll_and_process_spawns_async_and_status_transitions() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Initially idle
        assert!(slot.status().is_idle());

        // Poll and process - should spawn thread and set status to thinking
        let processed = slot.poll_and_process();
        assert_eq!(processed, 1);
        assert!(slot.status().is_thinking(), "Status should be Thinking after spawning async");

        // Wait for response to be received
        let timeout = std::time::Duration::from_secs(5);
        let result = sender.receive_response_timeout(timeout);
        assert!(result.is_ok());
        let response = result.unwrap();
        let had_error = response.map(|r| r.is_error()).unwrap_or(false);

        // After clear_thinking_status with error status, should return to idle
        slot.clear_thinking_status(had_error);
        assert!(slot.status().is_idle(), "Status should be Idle after clearing");
    }

    #[test]
    fn test_poll_and_process_response_reaches_sender() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Poll and process - spawns async thread
        slot.poll_and_process();

        // The response should eventually be received by sender
        // Give the thread time to complete
        let timeout = std::time::Duration::from_secs(5);
        let result = sender.receive_response_timeout(timeout);

        // Should receive a response (either success or error, depending on engine)
        assert!(result.is_ok(), "Should be able to receive response");
        let maybe_response = result.unwrap();
        assert!(maybe_response.is_some(), "Should have received a response");

        // After receiving, clear thinking status with error status
        let had_error = maybe_response.as_ref().map(|r| r.is_error()).unwrap_or(false);
        slot.clear_thinking_status(had_error);
        assert!(slot.status().is_idle());
    }

    #[test]
    fn test_sync_process_request_updates_reflection_round() {
        // Test that sync path properly accesses reflection_round after decision
        let (mut slot, sender) = make_test_slot();

        // Initial reflection_round should be 0 (or some default)
        // Note: reflection_round may not actually change in mock engine
        // but we verify the field is accessible and consistent

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Receive and process
        let received = slot.try_receive_request().unwrap();
        let response = slot.process_request(received);

        // Response should be successful
        assert!(response.is_success());

        // Verify reflection_round is accessible
        // The actual value depends on engine internals
        let _reflection = slot.reflection_round();
    }

    #[test]
    fn test_pending_reflection_round_set_after_async_completion() {
        // Verify that pending_reflection_round is set by the async thread
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // Before poll: pending should be None
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_none(), "Pending should be None before poll");
        }

        // Poll and process - spawns async thread
        slot.poll_and_process();

        // Wait for response
        let timeout = std::time::Duration::from_secs(5);
        let result = sender.receive_response_timeout(timeout);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // After response is received, pending should be set
        // (Thread writes reflection_round before sending response)
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_some(), "Pending should be set after async completion");
        }

        // After clear_thinking_status, pending should be reset to None
        slot.clear_thinking_status(false);
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_none(), "Pending should be reset to None after clear_thinking_status");
        }
    }

    #[test]
    fn test_clear_thinking_status_with_error_increments_error_count() {
        // Test that clear_thinking_status(true) increments error_count
        let (mut slot, _) = make_test_slot();

        // Initially no errors
        assert_eq!(slot.error_count(), 0);

        // Simulate error condition by calling clear_thinking_status with true
        // But we need to be in thinking state first
        slot.status = DecisionAgentStatus::thinking_now();
        slot.clear_thinking_status(true);

        // Error count should be incremented
        assert_eq!(slot.error_count(), 1);
        assert!(slot.status().is_idle());
    }

    #[test]
    fn test_clear_thinking_status_without_error_does_not_increment_error_count() {
        // Test that clear_thinking_status(false) does NOT increment error_count
        let (mut slot, _) = make_test_slot();

        // Initially no errors
        assert_eq!(slot.error_count(), 0);

        // Simulate success condition
        slot.status = DecisionAgentStatus::thinking_now();
        slot.clear_thinking_status(false);

        // Error count should still be 0
        assert_eq!(slot.error_count(), 0);
        assert!(slot.status().is_idle());
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

    #[test]
    fn test_poll_and_process_returns_zero_when_not_idle() {
        let (mut slot, sender) = make_test_slot();

        // First, set status to thinking (simulating an in-flight async decision)
        slot.status = DecisionAgentStatus::thinking_now();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // poll_and_process should return 0 because status is not Idle
        let processed = slot.poll_and_process();
        assert_eq!(processed, 0, "Should return 0 when not idle");

        // Request should still be in the channel (not consumed)
        assert!(slot.try_receive_request().is_some(), "Request should still be pending");
    }

    #[test]
    fn test_process_request_does_not_set_pending_reflection_round() {
        // Test that the sync process_request path does NOT set pending_reflection_round
        // This is important because only the async path uses pending_reflection_round
        let (mut slot, sender) = make_test_slot();

        // Before process_request, pending should be None
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_none(), "Pending should be None before request");
        }

        // Send and process request via sync path
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        let received = slot.try_receive_request().unwrap();
        let _response = slot.process_request(received);

        // After process_request, pending should STILL be None
        // (sync path doesn't use the pending_reflection_round mechanism)
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_none(), "Pending should remain None after sync process_request");
        }
    }

    #[test]
    fn test_clear_thinking_status_is_noop_when_not_thinking() {
        let (mut slot, _) = make_test_slot();

        // Initially idle
        assert!(slot.status().is_idle());

        // Set pending_reflection_round to Some manually (simulating async state)
        {
            let mut pending = slot.pending_reflection_round.lock().unwrap();
            *pending = Some(2);
        }

        // Call clear_thinking_status when NOT in thinking state
        slot.clear_thinking_status(false);

        // pending should still be Some(2) since we didn't enter the if block
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_some(), "Pending should still be Some because status was not Thinking");
            assert_eq!(*pending, Some(2));
        }

        // Status should still be idle
        assert!(slot.status().is_idle());

        // Now simulate the correct flow: set to Thinking first, then clear
        slot.status = DecisionAgentStatus::thinking_now();
        slot.clear_thinking_status(false);

        // Now pending should be None and status should be idle
        {
            let pending = slot.pending_reflection_round.lock().unwrap();
            assert!(pending.is_none(), "Pending should be None after proper clear");
        }
        assert!(slot.status().is_idle());
    }

    #[test]
    fn test_multiple_async_decisions_sequence() {
        // Test that multiple sequential async decisions work correctly
        let (mut slot, sender) = make_test_slot();

        // First decision
        let request1 = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request1).unwrap();

        slot.poll_and_process();
        assert!(slot.status().is_thinking());

        // Receive first response
        let timeout = std::time::Duration::from_secs(5);
        let result1 = sender.receive_response_timeout(timeout);
        assert!(result1.is_ok() && result1.unwrap().is_some());

        slot.clear_thinking_status(false);
        assert!(slot.status().is_idle());

        // Second decision - send new request
        let request2 = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request2).unwrap();

        slot.poll_and_process();
        assert!(slot.status().is_thinking());

        // Receive second response
        let result2 = sender.receive_response_timeout(timeout);
        assert!(result2.is_ok() && result2.unwrap().is_some());

        slot.clear_thinking_status(false);
        assert!(slot.status().is_idle());

        // Decision count should be 2
        assert_eq!(slot.decision_count(), 2);
    }

    #[test]
    fn test_has_recent_decision_initially_false() {
        let (slot, _) = make_test_slot();

        // Initially no recent decision
        assert!(!slot.has_recent_decision());
        assert!(slot.last_decision_started_at().is_none());
    }

    #[test]
    fn test_has_recent_decision_set_after_poll_and_process() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // poll_and_process should set last_decision_started_at
        slot.poll_and_process();
        assert!(slot.has_recent_decision());
        assert!(slot.last_decision_started_at().is_some());

        // Status should be thinking
        assert!(slot.status().is_thinking());
    }

    #[test]
    fn test_has_recent_decision_still_true_after_response_received() {
        let (mut slot, sender) = make_test_slot();

        // Send a decision request
        let request = DecisionRequest::new(
            crate::agent_runtime::AgentId::new("agent_001"),
            SituationType::new("waiting_for_choice"),
            make_test_context(),
        );
        sender.send_request(request).unwrap();

        // poll_and_process - spawns thread
        slot.poll_and_process();

        // Receive response
        let timeout = std::time::Duration::from_secs(5);
        let result = sender.receive_response_timeout(timeout);
        assert!(result.is_ok() && result.unwrap().is_some());

        // After receiving response but BEFORE clearing status
        // has_recent_decision should still be true
        assert!(slot.has_recent_decision());

        // Clear thinking status
        slot.clear_thinking_status(false);

        // After clearing, has_recent_decision should still be true
        // (because within the 1.5s window)
        assert!(slot.has_recent_decision());
    }

    #[test]
    fn test_has_fallback_response_initially_false() {
        let (slot, _) = make_test_slot();

        // Initially no fallback
        assert!(!slot.has_fallback_response());
    }

    #[test]
    fn test_take_fallback_response_returns_none_when_empty() {
        let (mut slot, _) = make_test_slot();

        // Should return None when no fallback
        let result = slot.take_fallback_response();
        assert!(result.is_none());
    }

    #[test]
    fn test_has_fallback_response_after_storing() {
        let (slot, _) = make_test_slot();

        // Manually store a fallback response
        let response = DecisionResponse::success(
            crate::agent_runtime::AgentId::new("agent_001"),
            agent_decision::output::DecisionOutput::new(Vec::new(), "test"),
        );
        {
            let mut guard = slot.pending_fallback_response.lock().unwrap();
            *guard = Some(response);
        }

        // Now should have fallback
        assert!(slot.has_fallback_response());
    }

    #[test]
    fn test_take_fallback_response_returns_stored_response() {
        let (mut slot, _) = make_test_slot();

        // Store a fallback response
        let response = DecisionResponse::success(
            crate::agent_runtime::AgentId::new("agent_001"),
            agent_decision::output::DecisionOutput::new(Vec::new(), "test"),
        );
        {
            let mut guard = slot.pending_fallback_response.lock().unwrap();
            *guard = Some(response.clone());
        }

        // Should return the stored response
        let result = slot.take_fallback_response();
        assert!(result.is_some());
        assert_eq!(result.unwrap().output().unwrap().reasoning, "test");

        // After taking, should be empty
        assert!(!slot.has_fallback_response());
    }

    #[test]
    fn test_take_fallback_response_can_only_be_called_once() {
        let (mut slot, _) = make_test_slot();

        // Store a fallback response
        let response = DecisionResponse::success(
            crate::agent_runtime::AgentId::new("agent_001"),
            agent_decision::output::DecisionOutput::new(Vec::new(), "test"),
        );
        {
            let mut guard = slot.pending_fallback_response.lock().unwrap();
            *guard = Some(response);
        }

        // First call returns response
        let result1 = slot.take_fallback_response();
        assert!(result1.is_some());

        // Second call returns None
        let result2 = slot.take_fallback_response();
        assert!(result2.is_none());
    }
}
