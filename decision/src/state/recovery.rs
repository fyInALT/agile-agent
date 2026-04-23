//! Error recovery and escalation handling
//!
//! Sprint 7: Implements tiered recovery escalation, timeout handling,
//! and health monitoring for the decision agent.

use crate::core::error::DecisionError;
use crate::state::lifecycle::DecisionAgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Recovery level for tiered escalation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum RecoveryLevel {
    /// Level 0: Automatic retry (same prompt)
    #[default]
    AutoRetry,

    /// Level 1: Adjusted retry (different prompt, more context)
    AdjustedRetry,

    /// Level 2: Switch decision engine (LLM -> RuleBased)
    SwitchEngine,

    /// Level 3: Human intervention (pause main agent)
    HumanIntervention,

    /// Level 4: Task failed (select next task)
    TaskFailed,
}

impl RecoveryLevel {
    /// Get level number (0-4)
    pub fn level(&self) -> u8 {
        match self {
            RecoveryLevel::AutoRetry => 0,
            RecoveryLevel::AdjustedRetry => 1,
            RecoveryLevel::SwitchEngine => 2,
            RecoveryLevel::HumanIntervention => 3,
            RecoveryLevel::TaskFailed => 4,
        }
    }

    /// Check if can retry
    pub fn can_retry(&self) -> bool {
        matches!(
            self,
            RecoveryLevel::AutoRetry | RecoveryLevel::AdjustedRetry
        )
    }

    /// Check if needs human
    pub fn needs_human(&self) -> bool {
        matches!(self, RecoveryLevel::HumanIntervention)
    }

    /// Check if task failed
    pub fn is_failed(&self) -> bool {
        matches!(self, RecoveryLevel::TaskFailed)
    }

    /// Determine recovery level from retry count and state
    pub fn determine(
        retry_count: u8,
        max_retries: u8,
        engine_switch_count: u8,
        human_requested: bool,
    ) -> Self {
        // Bug fix: Handle edge case where max_retries is 0
        // If max_retries is 0, we should still have at least one AutoRetry attempt
        // before escalating
        let effective_max = if max_retries == 0 { 1 } else { max_retries };

        if retry_count < effective_max {
            if retry_count < 2 {
                RecoveryLevel::AutoRetry
            } else {
                RecoveryLevel::AdjustedRetry
            }
        } else if engine_switch_count < 1 {
            RecoveryLevel::SwitchEngine
        } else if !human_requested {
            RecoveryLevel::HumanIntervention
        } else {
            RecoveryLevel::TaskFailed
        }
    }

    /// Escalate to next level
    pub fn escalate(&self) -> Self {
        match self {
            RecoveryLevel::AutoRetry => RecoveryLevel::AdjustedRetry,
            RecoveryLevel::AdjustedRetry => RecoveryLevel::SwitchEngine,
            RecoveryLevel::SwitchEngine => RecoveryLevel::HumanIntervention,
            RecoveryLevel::HumanIntervention => RecoveryLevel::TaskFailed,
            RecoveryLevel::TaskFailed => RecoveryLevel::TaskFailed,
        }
    }
}


impl std::fmt::Display for RecoveryLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoveryLevel::AutoRetry => write!(f, "AutoRetry (Level 0)"),
            RecoveryLevel::AdjustedRetry => write!(f, "AdjustedRetry (Level 1)"),
            RecoveryLevel::SwitchEngine => write!(f, "SwitchEngine (Level 2)"),
            RecoveryLevel::HumanIntervention => write!(f, "HumanIntervention (Level 3)"),
            RecoveryLevel::TaskFailed => write!(f, "TaskFailed (Level 4)"),
        }
    }
}

/// Timeout fallback strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimeoutFallback {
    /// Use RuleBased engine for fallback
    #[default]
    UseRuleBased,

    /// Return default decision (first option)
    DefaultDecision,

    /// Request human intervention
    HumanIntervention,
}

impl TimeoutFallback {
    pub fn is_rule_based(&self) -> bool {
        matches!(self, TimeoutFallback::UseRuleBased)
    }

    pub fn is_default(&self) -> bool {
        matches!(self, TimeoutFallback::DefaultDecision)
    }

    pub fn needs_human(&self) -> bool {
        matches!(self, TimeoutFallback::HumanIntervention)
    }
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Decision timeout in milliseconds
    pub decision_timeout_ms: u64,

    /// Number of timeout retries before fallback
    pub timeout_retries: u8,

    /// Fallback strategy on timeout exhaustion
    pub fallback: TimeoutFallback,

    /// Cooldown between retries in milliseconds
    pub retry_cooldown_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            decision_timeout_ms: 60000, // 60 seconds
            timeout_retries: 2,
            fallback: TimeoutFallback::UseRuleBased,
            retry_cooldown_ms: 5000, // 5 seconds
        }
    }
}

/// Timeout handling result
#[derive(Debug, Clone)]
pub enum TimeoutResult {
    /// Decision completed within timeout
    Completed,

    /// Timeout occurred, retry scheduled
    RetryScheduled { retry_count: u8 },

    /// Timeout retries exhausted, fallback applied
    FallbackApplied { strategy: TimeoutFallback },

    /// Timeout exhausted, waiting for human
    WaitingForHuman,
}

/// Decision agent error types for self-error recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionAgentError {
    /// Engine call failed
    EngineError { message: String },

    /// Provider session lost
    SessionLost,

    /// Context parsing error
    ContextParseError,

    /// Internal state corruption
    InternalError,

    /// Communication error with main agent
    CommunicationError { message: String },

    /// Timeout exceeded
    TimeoutExceeded { timeout_ms: u64 },
}

impl DecisionAgentError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            DecisionAgentError::EngineError { .. }
                | DecisionAgentError::SessionLost
                | DecisionAgentError::ContextParseError
        )
    }

    pub fn needs_reset(&self) -> bool {
        matches!(self, DecisionAgentError::InternalError)
    }

    pub fn needs_session_recreate(&self) -> bool {
        matches!(self, DecisionAgentError::SessionLost)
    }

    pub fn needs_context_rebuild(&self) -> bool {
        matches!(self, DecisionAgentError::ContextParseError)
    }

    pub fn engine_message(&self) -> Option<&str> {
        match self {
            DecisionAgentError::EngineError { message } => Some(message),
            DecisionAgentError::CommunicationError { message } => Some(message),
            _ => None,
        }
    }
}

impl std::fmt::Display for DecisionAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionAgentError::EngineError { message } => {
                write!(f, "Engine error: {}", message)
            }
            DecisionAgentError::SessionLost => write!(f, "Session lost"),
            DecisionAgentError::ContextParseError => write!(f, "Context parse error"),
            DecisionAgentError::InternalError => write!(f, "Internal error"),
            DecisionAgentError::CommunicationError { message } => {
                write!(f, "Communication error: {}", message)
            }
            DecisionAgentError::TimeoutExceeded { timeout_ms } => {
                write!(f, "Timeout exceeded: {}ms", timeout_ms)
            }
        }
    }
}

impl From<DecisionAgentError> for DecisionError {
    fn from(err: DecisionAgentError) -> DecisionError {
        DecisionError::EngineError(err.to_string())
    }
}

/// Session health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum SessionHealthStatus {
    /// Session is active and healthy
    #[default]
    Active,

    /// Session is stale but usable
    Stale,

    /// Session is lost/unavailable
    Lost,
}

impl SessionHealthStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, SessionHealthStatus::Active)
    }

    pub fn is_lost(&self) -> bool {
        matches!(self, SessionHealthStatus::Lost)
    }

    pub fn is_usable(&self) -> bool {
        matches!(
            self,
            SessionHealthStatus::Active | SessionHealthStatus::Stale
        )
    }
}


/// Engine health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum EngineHealthStatus {
    /// Engine is healthy
    #[default]
    Healthy,

    /// Engine is degraded but functional
    Degraded,

    /// Engine has failed
    Failed,
}

impl EngineHealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, EngineHealthStatus::Healthy)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, EngineHealthStatus::Failed)
    }

    pub fn is_usable(&self) -> bool {
        matches!(
            self,
            EngineHealthStatus::Healthy | EngineHealthStatus::Degraded
        )
    }
}


/// Decision agent health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAgentHealth {
    /// Recent decision success rate (0.0-1.0)
    pub success_rate: f64,

    /// Average decision duration in milliseconds
    pub avg_decision_time_ms: u64,

    /// Consecutive failures count
    pub consecutive_failures: u8,

    /// Session status
    pub session_status: SessionHealthStatus,

    /// Engine status
    pub engine_status: EngineHealthStatus,

    /// Last health check time
    pub last_check: DateTime<Utc>,
}

impl DecisionAgentHealth {
    /// Create new health metrics
    pub fn new() -> Self {
        Self {
            success_rate: 1.0,
            avg_decision_time_ms: 0,
            consecutive_failures: 0,
            session_status: SessionHealthStatus::Active,
            engine_status: EngineHealthStatus::Healthy,
            last_check: Utc::now(),
        }
    }

    /// Check if agent is healthy based on criteria:
    /// - Success rate > 70%
    /// - Avg decision time < 30 seconds
    /// - No more than 3 consecutive failures
    /// - Session active
    /// - Engine healthy
    pub fn is_healthy(&self) -> bool {
        self.success_rate > 0.7
            && self.avg_decision_time_ms < 30000
            && self.consecutive_failures < 3
            && self.session_status.is_active()
            && self.engine_status.is_healthy()
    }

    /// Check if agent needs recovery
    pub fn needs_recovery(&self) -> bool {
        !self.is_healthy()
    }

    /// Update success rate from history
    pub fn update_success_rate(&mut self, successes: usize, total: usize) {
        if total > 0 {
            self.success_rate = successes as f64 / total as f64;
        } else {
            self.success_rate = 1.0;
        }
        self.last_check = Utc::now();
    }

    /// Record a failure
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_check = Utc::now();
    }

    /// Record a success (reset consecutive failures)
    pub fn record_success(&mut self, duration_ms: u64) {
        self.consecutive_failures = 0;
        self.avg_decision_time_ms = duration_ms;
        self.last_check = Utc::now();
    }

    /// Mark session as lost
    pub fn mark_session_lost(&mut self) {
        self.session_status = SessionHealthStatus::Lost;
        self.last_check = Utc::now();
    }

    /// Mark engine as failed
    pub fn mark_engine_failed(&mut self) {
        self.engine_status = EngineHealthStatus::Failed;
        self.last_check = Utc::now();
    }

    /// Reset health to healthy state
    pub fn reset(&mut self) {
        self.success_rate = 1.0;
        self.avg_decision_time_ms = 0;
        self.consecutive_failures = 0;
        self.session_status = SessionHealthStatus::Active;
        self.engine_status = EngineHealthStatus::Healthy;
        self.last_check = Utc::now();
    }
}

impl Default for DecisionAgentHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// Recovery action to execute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Retry with cooldown
    RetryWithCooldown { cooldown_ms: u64 },

    /// Retry with adjusted prompt
    RetryWithAdjustedPrompt { additional_context: String },

    /// Switch engine type
    SwitchEngine,

    /// Request human intervention
    RequestHuman { reason: String },

    /// Mark task as failed
    MarkTaskFailed { reason: String },

    /// Recreate session
    RecreateSession,

    /// Rebuild context
    RebuildContext,

    /// Full reset
    FullReset,
}

impl RecoveryAction {
    pub fn needs_cooldown(&self) -> Option<u64> {
        match self {
            RecoveryAction::RetryWithCooldown { cooldown_ms } => Some(*cooldown_ms),
            _ => None,
        }
    }

    pub fn needs_human(&self) -> bool {
        matches!(self, RecoveryAction::RequestHuman { .. })
    }
}

/// Recovery context for handling errors
#[derive(Debug, Clone)]
pub struct RecoveryContext {
    /// Agent ID
    pub agent_id: DecisionAgentId,

    /// Current retry count
    pub retry_count: u8,

    /// Current engine switch count
    pub engine_switch_count: u8,

    /// Whether human has been requested
    pub human_requested: bool,

    /// Maximum retries allowed
    pub max_retries: u8,

    /// Current health
    pub health: DecisionAgentHealth,

    /// Error that triggered recovery
    pub trigger_error: Option<DecisionAgentError>,
}

impl RecoveryContext {
    pub fn new(agent_id: DecisionAgentId, max_retries: u8) -> Self {
        Self {
            agent_id,
            retry_count: 0,
            engine_switch_count: 0,
            human_requested: false,
            max_retries,
            health: DecisionAgentHealth::new(),
            trigger_error: None,
        }
    }

    /// Determine recovery level
    pub fn determine_level(&self) -> RecoveryLevel {
        RecoveryLevel::determine(
            self.retry_count,
            self.max_retries,
            self.engine_switch_count,
            self.human_requested,
        )
    }

    /// Determine recovery action for error
    pub fn determine_action(&self, error: &DecisionAgentError) -> RecoveryAction {
        if error.needs_reset() {
            return RecoveryAction::FullReset;
        }

        if error.needs_session_recreate() {
            return RecoveryAction::RecreateSession;
        }

        if error.needs_context_rebuild() {
            return RecoveryAction::RebuildContext;
        }

        // Use escalation level
        let level = self.determine_level();
        match level {
            RecoveryLevel::AutoRetry => RecoveryAction::RetryWithCooldown { cooldown_ms: 5000 },
            RecoveryLevel::AdjustedRetry => RecoveryAction::RetryWithAdjustedPrompt {
                additional_context: "Previous attempt failed. Consider alternative approach."
                    .to_string(),
            },
            RecoveryLevel::SwitchEngine => RecoveryAction::SwitchEngine,
            RecoveryLevel::HumanIntervention => RecoveryAction::RequestHuman {
                reason: error.to_string(),
            },
            RecoveryLevel::TaskFailed => RecoveryAction::MarkTaskFailed {
                reason: error.to_string(),
            },
        }
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.health.record_failure();
    }

    /// Increment engine switch count
    pub fn increment_engine_switch(&mut self) {
        self.engine_switch_count += 1;
    }

    /// Mark human requested
    pub fn mark_human_requested(&mut self) {
        self.human_requested = true;
    }

    /// Record success
    pub fn record_success(&mut self, duration_ms: u64) {
        self.retry_count = 0;
        self.engine_switch_count = 0;
        self.human_requested = false;
        self.health.record_success(duration_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_level_level_number() {
        assert_eq!(RecoveryLevel::AutoRetry.level(), 0);
        assert_eq!(RecoveryLevel::AdjustedRetry.level(), 1);
        assert_eq!(RecoveryLevel::SwitchEngine.level(), 2);
        assert_eq!(RecoveryLevel::HumanIntervention.level(), 3);
        assert_eq!(RecoveryLevel::TaskFailed.level(), 4);
    }

    #[test]
    fn test_recovery_level_can_retry() {
        assert!(RecoveryLevel::AutoRetry.can_retry());
        assert!(RecoveryLevel::AdjustedRetry.can_retry());
        assert!(!RecoveryLevel::SwitchEngine.can_retry());
        assert!(!RecoveryLevel::HumanIntervention.can_retry());
        assert!(!RecoveryLevel::TaskFailed.can_retry());
    }

    #[test]
    fn test_recovery_level_needs_human() {
        assert!(!RecoveryLevel::AutoRetry.needs_human());
        assert!(!RecoveryLevel::AdjustedRetry.needs_human());
        assert!(!RecoveryLevel::SwitchEngine.needs_human());
        assert!(RecoveryLevel::HumanIntervention.needs_human());
        assert!(!RecoveryLevel::TaskFailed.needs_human());
    }

    #[test]
    fn test_recovery_level_is_failed() {
        assert!(!RecoveryLevel::AutoRetry.is_failed());
        assert!(!RecoveryLevel::AdjustedRetry.is_failed());
        assert!(!RecoveryLevel::SwitchEngine.is_failed());
        assert!(!RecoveryLevel::HumanIntervention.is_failed());
        assert!(RecoveryLevel::TaskFailed.is_failed());
    }

    #[test]
    fn test_recovery_level_determine_auto_retry() {
        let level = RecoveryLevel::determine(0, 3, 0, false);
        assert_eq!(level, RecoveryLevel::AutoRetry);
    }

    #[test]
    fn test_recovery_level_determine_adjusted_retry() {
        let level = RecoveryLevel::determine(2, 3, 0, false);
        assert_eq!(level, RecoveryLevel::AdjustedRetry);
    }

    #[test]
    fn test_recovery_level_determine_switch_engine() {
        let level = RecoveryLevel::determine(3, 3, 0, false);
        assert_eq!(level, RecoveryLevel::SwitchEngine);
    }

    #[test]
    fn test_recovery_level_determine_human_intervention() {
        let level = RecoveryLevel::determine(3, 3, 1, false);
        assert_eq!(level, RecoveryLevel::HumanIntervention);
    }

    #[test]
    fn test_recovery_level_determine_task_failed() {
        let level = RecoveryLevel::determine(3, 3, 1, true);
        assert_eq!(level, RecoveryLevel::TaskFailed);
    }

    #[test]
    fn test_recovery_level_escalate() {
        assert_eq!(
            RecoveryLevel::AutoRetry.escalate(),
            RecoveryLevel::AdjustedRetry
        );
        assert_eq!(
            RecoveryLevel::AdjustedRetry.escalate(),
            RecoveryLevel::SwitchEngine
        );
        assert_eq!(
            RecoveryLevel::SwitchEngine.escalate(),
            RecoveryLevel::HumanIntervention
        );
        assert_eq!(
            RecoveryLevel::HumanIntervention.escalate(),
            RecoveryLevel::TaskFailed
        );
        assert_eq!(
            RecoveryLevel::TaskFailed.escalate(),
            RecoveryLevel::TaskFailed
        );
    }

    #[test]
    fn test_recovery_level_display() {
        assert_eq!(
            format!("{}", RecoveryLevel::AutoRetry),
            "AutoRetry (Level 0)"
        );
        assert_eq!(
            format!("{}", RecoveryLevel::TaskFailed),
            "TaskFailed (Level 4)"
        );
    }

    #[test]
    fn test_recovery_level_serde() {
        let level = RecoveryLevel::AdjustedRetry;
        let json = serde_json::to_string(&level).unwrap();
        let parsed: RecoveryLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, parsed);
    }

    #[test]
    fn test_timeout_fallback_default() {
        let fallback = TimeoutFallback::default();
        assert_eq!(fallback, TimeoutFallback::UseRuleBased);
        assert!(fallback.is_rule_based());
    }

    #[test]
    fn test_timeout_fallback_variants() {
        assert!(TimeoutFallback::UseRuleBased.is_rule_based());
        assert!(TimeoutFallback::DefaultDecision.is_default());
        assert!(TimeoutFallback::HumanIntervention.needs_human());
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.decision_timeout_ms, 60000);
        assert_eq!(config.timeout_retries, 2);
        assert_eq!(config.fallback, TimeoutFallback::UseRuleBased);
        assert_eq!(config.retry_cooldown_ms, 5000);
    }

    #[test]
    fn test_timeout_config_serde() {
        let config = TimeoutConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TimeoutConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.decision_timeout_ms, parsed.decision_timeout_ms);
    }

    #[test]
    fn test_decision_agent_error_engine_error() {
        let err = DecisionAgentError::EngineError {
            message: "test error".to_string(),
        };
        assert!(err.is_recoverable());
        assert!(!err.needs_reset());
        assert!(!err.needs_session_recreate());
        assert!(!err.needs_context_rebuild());
        assert_eq!(err.engine_message(), Some("test error"));
    }

    #[test]
    fn test_decision_agent_error_session_lost() {
        let err = DecisionAgentError::SessionLost;
        assert!(err.is_recoverable());
        assert!(err.needs_session_recreate());
    }

    #[test]
    fn test_decision_agent_error_context_parse() {
        let err = DecisionAgentError::ContextParseError;
        assert!(err.is_recoverable());
        assert!(err.needs_context_rebuild());
    }

    #[test]
    fn test_decision_agent_error_internal() {
        let err = DecisionAgentError::InternalError;
        assert!(!err.is_recoverable());
        assert!(err.needs_reset());
    }

    #[test]
    fn test_decision_agent_error_display() {
        let err = DecisionAgentError::EngineError {
            message: "test".to_string(),
        };
        assert_eq!(format!("{}", err), "Engine error: test");

        let err = DecisionAgentError::SessionLost;
        assert_eq!(format!("{}", err), "Session lost");
    }

    #[test]
    fn test_session_health_status_active() {
        let status = SessionHealthStatus::Active;
        assert!(status.is_active());
        assert!(!status.is_lost());
        assert!(status.is_usable());
    }

    #[test]
    fn test_session_health_status_stale() {
        let status = SessionHealthStatus::Stale;
        assert!(!status.is_active());
        assert!(!status.is_lost());
        assert!(status.is_usable());
    }

    #[test]
    fn test_session_health_status_lost() {
        let status = SessionHealthStatus::Lost;
        assert!(!status.is_active());
        assert!(status.is_lost());
        assert!(!status.is_usable());
    }

    #[test]
    fn test_engine_health_status_healthy() {
        let status = EngineHealthStatus::Healthy;
        assert!(status.is_healthy());
        assert!(!status.is_failed());
        assert!(status.is_usable());
    }

    #[test]
    fn test_engine_health_status_degraded() {
        let status = EngineHealthStatus::Degraded;
        assert!(!status.is_healthy());
        assert!(!status.is_failed());
        assert!(status.is_usable());
    }

    #[test]
    fn test_engine_health_status_failed() {
        let status = EngineHealthStatus::Failed;
        assert!(!status.is_healthy());
        assert!(status.is_failed());
        assert!(!status.is_usable());
    }

    #[test]
    fn test_decision_agent_health_new() {
        let health = DecisionAgentHealth::new();
        assert_eq!(health.success_rate, 1.0);
        assert_eq!(health.avg_decision_time_ms, 0);
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.session_status.is_active());
        assert!(health.engine_status.is_healthy());
    }

    #[test]
    fn test_decision_agent_health_is_healthy() {
        let health = DecisionAgentHealth::new();
        assert!(health.is_healthy());

        // Success rate too low
        let mut health = DecisionAgentHealth::new();
        health.success_rate = 0.5;
        assert!(!health.is_healthy());

        // Too many consecutive failures
        let mut health = DecisionAgentHealth::new();
        health.consecutive_failures = 5;
        assert!(!health.is_healthy());

        // Session lost
        let mut health = DecisionAgentHealth::new();
        health.session_status = SessionHealthStatus::Lost;
        assert!(!health.is_healthy());

        // Engine failed
        let mut health = DecisionAgentHealth::new();
        health.engine_status = EngineHealthStatus::Failed;
        assert!(!health.is_healthy());
    }

    #[test]
    fn test_decision_agent_health_needs_recovery() {
        let health = DecisionAgentHealth::new();
        assert!(!health.needs_recovery());

        let mut health = DecisionAgentHealth::new();
        health.success_rate = 0.5;
        assert!(health.needs_recovery());
    }

    #[test]
    fn test_decision_agent_health_update_success_rate() {
        let mut health = DecisionAgentHealth::new();
        health.update_success_rate(7, 10);
        assert_eq!(health.success_rate, 0.7);

        health.update_success_rate(0, 0);
        assert_eq!(health.success_rate, 1.0); // Default when no data
    }

    #[test]
    fn test_decision_agent_health_record_failure() {
        let mut health = DecisionAgentHealth::new();
        health.record_failure();
        assert_eq!(health.consecutive_failures, 1);

        health.record_failure();
        assert_eq!(health.consecutive_failures, 2);
    }

    #[test]
    fn test_decision_agent_health_record_success() {
        let mut health = DecisionAgentHealth::new();
        health.consecutive_failures = 3;
        health.record_success(5000);
        assert_eq!(health.consecutive_failures, 0);
        assert_eq!(health.avg_decision_time_ms, 5000);
    }

    #[test]
    fn test_decision_agent_health_reset() {
        let mut health = DecisionAgentHealth::new();
        health.success_rate = 0.5;
        health.consecutive_failures = 5;
        health.session_status = SessionHealthStatus::Lost;
        health.engine_status = EngineHealthStatus::Failed;
        health.reset();

        assert_eq!(health.success_rate, 1.0);
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.session_status.is_active());
        assert!(health.engine_status.is_healthy());
    }

    #[test]
    fn test_decision_agent_health_serde() {
        let health = DecisionAgentHealth::new();
        let json = serde_json::to_string(&health).unwrap();
        let parsed: DecisionAgentHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(health.success_rate, parsed.success_rate);
    }

    #[test]
    fn test_recovery_action_needs_cooldown() {
        let action = RecoveryAction::RetryWithCooldown { cooldown_ms: 5000 };
        assert_eq!(action.needs_cooldown(), Some(5000));

        let action = RecoveryAction::SwitchEngine;
        assert_eq!(action.needs_cooldown(), None);
    }

    #[test]
    fn test_recovery_action_needs_human() {
        let action = RecoveryAction::RequestHuman {
            reason: "test".to_string(),
        };
        assert!(action.needs_human());

        let action = RecoveryAction::SwitchEngine;
        assert!(!action.needs_human());
    }

    #[test]
    fn test_recovery_context_new() {
        let ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        assert_eq!(ctx.retry_count, 0);
        assert_eq!(ctx.engine_switch_count, 0);
        assert!(!ctx.human_requested);
        assert!(ctx.health.is_healthy());
    }

    #[test]
    fn test_recovery_context_determine_level() {
        let ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        assert_eq!(ctx.determine_level(), RecoveryLevel::AutoRetry);

        let mut ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        ctx.retry_count = 2;
        assert_eq!(ctx.determine_level(), RecoveryLevel::AdjustedRetry);

        let mut ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        ctx.retry_count = 3;
        assert_eq!(ctx.determine_level(), RecoveryLevel::SwitchEngine);
    }

    #[test]
    fn test_recovery_context_determine_action_internal_error() {
        let ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        let err = DecisionAgentError::InternalError;
        let action = ctx.determine_action(&err);
        assert!(matches!(action, RecoveryAction::FullReset));
    }

    #[test]
    fn test_recovery_context_determine_action_session_lost() {
        let ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        let err = DecisionAgentError::SessionLost;
        let action = ctx.determine_action(&err);
        assert!(matches!(action, RecoveryAction::RecreateSession));
    }

    #[test]
    fn test_recovery_context_determine_action_context_parse() {
        let ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        let err = DecisionAgentError::ContextParseError;
        let action = ctx.determine_action(&err);
        assert!(matches!(action, RecoveryAction::RebuildContext));
    }

    #[test]
    fn test_recovery_context_increment() {
        let mut ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        ctx.increment_retry();
        assert_eq!(ctx.retry_count, 1);
        assert_eq!(ctx.health.consecutive_failures, 1);

        ctx.increment_engine_switch();
        assert_eq!(ctx.engine_switch_count, 1);

        ctx.mark_human_requested();
        assert!(ctx.human_requested);
    }

    #[test]
    fn test_recovery_context_record_success() {
        let mut ctx = RecoveryContext::new(DecisionAgentId::new("agent-1"), 3);
        ctx.retry_count = 2;
        ctx.engine_switch_count = 1;
        ctx.human_requested = true;
        ctx.health.consecutive_failures = 2;

        ctx.record_success(1000);

        assert_eq!(ctx.retry_count, 0);
        assert_eq!(ctx.engine_switch_count, 0);
        assert!(!ctx.human_requested);
        assert_eq!(ctx.health.consecutive_failures, 0);
        assert_eq!(ctx.health.avg_decision_time_ms, 1000);
    }
}
