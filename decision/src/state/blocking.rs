//! Blocking reason trait and blocked state

use crate::model::action::DecisionAction;
use crate::model::situation::{ChoiceOption, DecisionSituation};
use crate::core::types::UrgencyLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

/// Blocking reason trait - extensible
pub trait BlockingReason: Send + Sync + 'static {
    /// Reason type identifier
    fn reason_type(&self) -> &str;

    /// Urgency level
    fn urgency(&self) -> UrgencyLevel;

    /// Expiration time (if applicable)
    fn expires_at(&self) -> Option<DateTime<Utc>>;

    /// Whether can auto-resolve
    fn can_auto_resolve(&self) -> bool;

    /// Auto-resolve action (if can_auto_resolve)
    fn auto_resolve_action(&self) -> Option<AutoAction>;

    /// Blocking description for display
    fn description(&self) -> String;

    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn BlockingReason>;

    /// Try to downcast to RateLimitBlockedReason (immutable).
    /// Returns None if this is not a RateLimitBlockedReason.
    fn as_rate_limit_reason(&self) -> Option<&RateLimitBlockedReason> {
        None
    }

    /// Try to downcast to RateLimitBlockedReason (mutable).
    /// Returns None if this is not a RateLimitBlockedReason.
    fn as_rate_limit_reason_mut(&mut self) -> Option<&mut RateLimitBlockedReason> {
        None
    }
}

/// Auto-resolve action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoAction {
    FollowRecommendation,
    SelectDefault,
    Cancel,
    MarkTaskFailed,
    ReleaseResource,
}

/// Blocked state - generic wrapper
pub struct BlockedState {
    /// Blocking reason (trait reference)
    reason: Box<dyn BlockingReason>,

    /// Blocked start time
    blocked_at: Instant,

    /// Blocking context
    context: BlockingContext,
}

impl BlockedState {
    pub fn new(reason: Box<dyn BlockingReason>) -> Self {
        Self {
            reason,
            blocked_at: Instant::now(),
            context: BlockingContext::default(),
        }
    }

    pub fn with_context(self, context: BlockingContext) -> Self {
        Self { context, ..self }
    }

    pub fn reason(&self) -> &dyn BlockingReason {
        self.reason.as_ref()
    }

    /// Get mutable reference to the blocking reason.
    /// Needed for updating rate limit retry state.
    pub fn reason_mut(&mut self) -> &mut dyn BlockingReason {
        self.reason.as_mut()
    }

    /// Check if blocked due to rate limit (HTTP 429).
    pub fn is_rate_limit(&self) -> bool {
        self.reason.reason_type() == "rate_limit"
    }

    pub fn elapsed(&self) -> Duration {
        self.blocked_at.elapsed()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.reason.expires_at() {
            Utc::now() > expires
        } else {
            false
        }
    }

    pub fn context(&self) -> &BlockingContext {
        &self.context
    }
}

impl fmt::Debug for BlockedState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockedState")
            .field("reason_type", &self.reason.reason_type())
            .field("elapsed", &self.elapsed())
            .field("context", &self.context)
            .finish()
    }
}

impl Clone for BlockedState {
    fn clone(&self) -> Self {
        Self {
            reason: self.reason.clone_boxed(),
            blocked_at: self.blocked_at,
            context: self.context.clone(),
        }
    }
}

/// Blocking context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockingContext {
    pub task_id: Option<String>,
    pub story_id: Option<String>,
    pub additional_info: HashMap<String, String>,
}

impl BlockingContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_task(self, task_id: impl Into<String>) -> Self {
        Self {
            task_id: Some(task_id.into()),
            ..self
        }
    }

    pub fn with_story(self, story_id: impl Into<String>) -> Self {
        Self {
            story_id: Some(story_id.into()),
            ..self
        }
    }

    pub fn with_info(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut additional_info = self.additional_info;
        additional_info.insert(key.into(), value.into());
        Self {
            additional_info,
            ..self
        }
    }
}

/// Human decision blocking - one implementation
pub struct HumanDecisionBlocking {
    pub decision_request_id: String,
    pub situation: Box<dyn DecisionSituation>,
    pub options: Vec<ChoiceOption>,
    pub recommendation: Option<Box<dyn DecisionAction>>,
    pub expires_at: DateTime<Utc>,
}

impl HumanDecisionBlocking {
    pub fn new(
        decision_request_id: impl Into<String>,
        situation: Box<dyn DecisionSituation>,
        options: Vec<ChoiceOption>,
    ) -> Self {
        Self {
            decision_request_id: decision_request_id.into(),
            situation,
            options,
            recommendation: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
        }
    }

    pub fn with_recommendation(self, action: Box<dyn DecisionAction>) -> Self {
        Self {
            recommendation: Some(action),
            ..self
        }
    }

    pub fn with_expires_at(self, expires_at: DateTime<Utc>) -> Self {
        Self { expires_at, ..self }
    }

    /// Clone this blocking
    pub fn clone_blocking(&self) -> Self {
        Self {
            decision_request_id: self.decision_request_id.clone(),
            situation: self.situation.clone_boxed(),
            options: self.options.clone(),
            recommendation: self.recommendation.as_ref().map(|r| r.clone_boxed()),
            expires_at: self.expires_at,
        }
    }
}

impl BlockingReason for HumanDecisionBlocking {
    fn reason_type(&self) -> &str {
        "human_decision"
    }

    fn urgency(&self) -> UrgencyLevel {
        self.situation.human_urgency()
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        Some(self.expires_at)
    }

    fn can_auto_resolve(&self) -> bool {
        // Bug fix: Consistent logic - can auto resolve if we have a recommendation
        // OR if there are options available (can select default)
        self.recommendation.is_some() || !self.options.is_empty()
    }

    fn auto_resolve_action(&self) -> Option<AutoAction> {
        // Bug fix: Consistent with can_auto_resolve
        if self.recommendation.is_some() {
            Some(AutoAction::FollowRecommendation)
        } else if !self.options.is_empty() {
            // Only return SelectDefault if there are actual options
            Some(AutoAction::SelectDefault)
        } else {
            // No recommendation and no options - cannot auto resolve
            None
        }
    }

    fn description(&self) -> String {
        format!(
            "Waiting for human decision: {}",
            self.situation.situation_type()
        )
    }

    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone_blocking())
    }
}

impl fmt::Debug for HumanDecisionBlocking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HumanDecisionBlocking")
            .field("decision_request_id", &self.decision_request_id)
            .field("situation_type", &self.situation.situation_type())
            .field("options", &self.options)
            .field("has_recommendation", &self.recommendation.is_some())
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Resource blocking - another implementation (example)
#[derive(Debug, Clone)]
pub struct ResourceBlocking {
    pub resource_type: String,
    pub resource_id: String,
    pub wait_reason: String,
}

impl ResourceBlocking {
    pub fn new(
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        wait_reason: impl Into<String>,
    ) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            wait_reason: wait_reason.into(),
        }
    }
}

impl BlockingReason for ResourceBlocking {
    fn reason_type(&self) -> &str {
        "resource_waiting"
    }

    fn urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        None
    }

    fn can_auto_resolve(&self) -> bool {
        true
    }

    fn auto_resolve_action(&self) -> Option<AutoAction> {
        None
    }

    fn description(&self) -> String {
        format!("Waiting for {}: {}", self.resource_type, self.resource_id)
    }

    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }
}

/// Rate limit blocked reason - when agent hits HTTP 429
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitBlockedReason {
    started_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    retry_count: u32,
    interval_secs: u64,
}

impl RateLimitBlockedReason {
    pub fn new(started_at: DateTime<Utc>) -> Self {
        Self {
            started_at,
            last_retry_at: None,
            retry_count: 0,
            interval_secs: 1800, // 30 minutes default
        }
    }

    pub fn with_interval_secs(self, interval_secs: u64) -> Self {
        Self { interval_secs, ..self }
    }

    pub fn with_last_retry_at(self, last_retry_at: DateTime<Utc>) -> Self {
        Self { last_retry_at: Some(last_retry_at), ..self }
    }

    pub fn with_retry_count(self, retry_count: u32) -> Self {
        Self { retry_count, ..self }
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn last_retry_at(&self) -> Option<DateTime<Utc>> {
        self.last_retry_at
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    /// Minutes since first 429
    pub fn elapsed_minutes(&self) -> i64 {
        (Utc::now() - self.started_at).num_minutes()
    }

    /// Whether enough time has passed to retry
    pub fn can_retry_now(&self) -> bool {
        if let Some(last) = self.last_retry_at {
            let elapsed = (Utc::now() - last).num_seconds() as u64;
            elapsed >= self.interval_secs
        } else {
            true // Never tried, can retry now
        }
    }

    /// Record a retry attempt
    pub fn record_retry(&mut self) {
        self.last_retry_at = Some(Utc::now());
        self.retry_count += 1;
    }
}

impl BlockingReason for RateLimitBlockedReason {
    fn reason_type(&self) -> &'static str {
        "rate_limit"
    }

    fn urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        None // No expiration, wait indefinitely
    }

    fn can_auto_resolve(&self) -> bool {
        false // Must actually try LLM call to verify recovery
    }

    fn auto_resolve_action(&self) -> Option<AutoAction> {
        None
    }

    fn description(&self) -> String {
        let mins = self.elapsed_minutes();
        format!("💤 Rate limited ({} min)", mins)
    }

    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }

    fn as_rate_limit_reason(&self) -> Option<&RateLimitBlockedReason> {
        Some(self)
    }

    fn as_rate_limit_reason_mut(&mut self) -> Option<&mut RateLimitBlockedReason> {
        Some(self)
    }
}

/// Agent slot status - generic blocked
#[derive(Debug)]
pub enum AgentSlotStatus {
    /// Agent is running
    Running,

    /// Agent is blocked (generic)
    Blocked(BlockedState),

    /// Agent is idle (no task)
    Idle,

    /// Agent is stopped
    Stopped,
}

impl AgentSlotStatus {
    pub fn running() -> Self {
        AgentSlotStatus::Running
    }

    pub fn blocked(reason: Box<dyn BlockingReason>) -> Self {
        AgentSlotStatus::Blocked(BlockedState::new(reason))
    }

    pub fn idle() -> Self {
        AgentSlotStatus::Idle
    }

    pub fn stopped() -> Self {
        AgentSlotStatus::Stopped
    }

    pub fn is_running(&self) -> bool {
        matches!(self, AgentSlotStatus::Running)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, AgentSlotStatus::Blocked(_))
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, AgentSlotStatus::Idle)
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self, AgentSlotStatus::Stopped)
    }

    pub fn blocked_state(&self) -> Option<&BlockedState> {
        match self {
            AgentSlotStatus::Blocked(state) => Some(state),
            _ => None,
        }
    }
}

/// Recommendation from Decision Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommended action type
    pub action_type: String,

    /// Action parameters (JSON)
    pub action_params: String,

    /// Reasoning
    pub reasoning: String,

    /// Confidence (0.0-1.0)
    pub confidence: f64,
}

impl Recommendation {
    pub fn new(
        action_type: impl Into<String>,
        reasoning: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            action_type: action_type.into(),
            action_params: "{}".to_string(),
            reasoning: reasoning.into(),
            confidence,
        }
    }

    pub fn with_params(self, params: impl Into<String>) -> Self {
        Self {
            action_params: params.into(),
            ..self
        }
    }
}

/// Human decision request (for queue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionRequest {
    /// Request ID
    pub id: String,

    /// Agent ID
    pub agent_id: String,

    /// Situation type
    pub situation_type: crate::types::SituationType,

    /// Situation description
    pub situation_description: String,

    /// Available options
    pub options: Vec<ChoiceOption>,

    /// Recommendation from Decision Agent
    pub recommendation: Option<Recommendation>,

    /// Urgency level
    pub urgency: UrgencyLevel,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,

    /// Blocking context
    pub context: BlockingContext,
}

impl HumanDecisionRequest {
    pub fn new(
        id: impl Into<String>,
        agent_id: impl Into<String>,
        situation_type: crate::types::SituationType,
        options: Vec<ChoiceOption>,
        urgency: UrgencyLevel,
        timeout_ms: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            situation_type,
            situation_description: String::new(),
            options,
            recommendation: None,
            urgency,
            created_at: now,
            expires_at: now + chrono::Duration::milliseconds(timeout_ms as i64),
            context: BlockingContext::default(),
        }
    }

    pub fn with_description(self, description: impl Into<String>) -> Self {
        Self {
            situation_description: description.into(),
            ..self
        }
    }

    pub fn with_recommendation(self, recommendation: Recommendation) -> Self {
        Self {
            recommendation: Some(recommendation),
            ..self
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn remaining_seconds(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }
}

/// Human selection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HumanSelection {
    /// Selected specific option
    Selected { option_id: String },

    /// Accepted recommendation
    AcceptedRecommendation,

    /// Custom instruction provided
    Custom { instruction: String },

    /// Task skipped
    Skipped,

    /// Operation cancelled
    Cancelled,
}

impl HumanSelection {
    pub fn selected(option_id: impl Into<String>) -> Self {
        HumanSelection::Selected {
            option_id: option_id.into(),
        }
    }

    pub fn accept_recommendation() -> Self {
        HumanSelection::AcceptedRecommendation
    }

    pub fn custom(instruction: impl Into<String>) -> Self {
        HumanSelection::Custom {
            instruction: instruction.into(),
        }
    }

    pub fn skip() -> Self {
        HumanSelection::Skipped
    }

    pub fn cancel() -> Self {
        HumanSelection::Cancelled
    }
}

/// Human decision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionResponse {
    /// Request ID
    pub request_id: String,

    /// Human selection
    pub selection: HumanSelection,

    /// Response timestamp
    pub responded_at: DateTime<Utc>,

    /// Response time in milliseconds
    pub response_time_ms: u64,
}

impl HumanDecisionResponse {
    pub fn new(request_id: impl Into<String>, selection: HumanSelection) -> Self {
        Self {
            request_id: request_id.into(),
            selection,
            responded_at: Utc::now(),
            response_time_ms: 0,
        }
    }

    pub fn with_response_time(self, created_at: DateTime<Utc>) -> Self {
        let response_time_ms = (self.responded_at - created_at).num_milliseconds() as u64;
        Self {
            response_time_ms,
            ..self
        }
    }
}

/// Human decision timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionTimeoutConfig {
    /// Default timeout (1 hour)
    pub default_timeout_ms: u64,

    /// High urgency timeout (30 minutes)
    pub high_timeout_ms: u64,

    /// Critical urgency timeout (15 minutes)
    pub critical_timeout_ms: u64,

    /// Low urgency timeout (2 hours)
    pub low_timeout_ms: u64,

    /// Warning before timeout (1 minute)
    pub warning_before_ms: u64,

    /// Default action on timeout
    pub timeout_default: AutoAction,
}

impl Default for HumanDecisionTimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 3600000, // 1 hour
            high_timeout_ms: 1800000,    // 30 min
            critical_timeout_ms: 900000, // 15 min
            low_timeout_ms: 7200000,     // 2 hours
            warning_before_ms: 60000,    // 1 min
            timeout_default: AutoAction::FollowRecommendation,
        }
    }
}

impl HumanDecisionTimeoutConfig {
    pub fn timeout_for_urgency(&self, urgency: UrgencyLevel) -> u64 {
        match urgency {
            UrgencyLevel::Critical => self.critical_timeout_ms,
            UrgencyLevel::High => self.high_timeout_ms,
            UrgencyLevel::Medium => self.default_timeout_ms,
            UrgencyLevel::Low => self.low_timeout_ms,
        }
    }
}

/// Human decision queue with priority ordering
#[derive(Debug)]
pub struct HumanDecisionQueue {
    /// Priority queues
    critical: Vec<HumanDecisionRequest>,
    high: Vec<HumanDecisionRequest>,
    medium: Vec<HumanDecisionRequest>,
    low: Vec<HumanDecisionRequest>,

    /// Index for O(1) lookup: request_id -> urgency level
    id_index: HashMap<String, UrgencyLevel>,

    /// Completed requests (history)
    history: Vec<HumanDecisionResponse>,

    /// Timeout configuration
    timeout_config: HumanDecisionTimeoutConfig,
}

impl HumanDecisionQueue {
    pub fn new(timeout_config: HumanDecisionTimeoutConfig) -> Self {
        Self {
            critical: Vec::new(),
            high: Vec::new(),
            medium: Vec::new(),
            low: Vec::new(),
            id_index: HashMap::new(),
            history: Vec::new(),
            timeout_config,
        }
    }

    /// Push request to appropriate priority queue
    pub fn push(&mut self, request: HumanDecisionRequest) {
        let urgency = request.urgency;
        let request_id = request.id.clone();
        match urgency {
            UrgencyLevel::Critical => self.critical.push(request),
            UrgencyLevel::High => self.high.push(request),
            UrgencyLevel::Medium => self.medium.push(request),
            UrgencyLevel::Low => self.low.push(request),
        }
        self.id_index.insert(request_id, urgency);
    }

    /// Pop next request (priority order)
    pub fn pop(&mut self) -> Option<HumanDecisionRequest> {
        // Priority: Critical > High > Medium > Low
        if !self.critical.is_empty() {
            let request = self.critical.remove(0);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if !self.high.is_empty() {
            let request = self.high.remove(0);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if !self.medium.is_empty() {
            let request = self.medium.remove(0);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if !self.low.is_empty() {
            let request = self.low.remove(0);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        None
    }

    /// Peek next request without removing
    pub fn peek(&self) -> Option<&HumanDecisionRequest> {
        // Priority: Critical > High > Medium > Low
        self.critical
            .first()
            .or_else(|| self.high.first())
            .or_else(|| self.medium.first())
            .or_else(|| self.low.first())
    }

    /// Find request by agent ID (for decision action execution)
    ///
    /// Returns a reference to the request belonging to the specified agent.
    /// This is used when executing decision actions to ensure the action
    /// is applied to the correct agent's request.
    pub fn find_by_agent_id(&self, agent_id: &str) -> Option<&HumanDecisionRequest> {
        // Search all queues for the agent's request
        self.critical
            .iter()
            .find(|r| r.agent_id == agent_id)
            .or_else(|| self.high.iter().find(|r| r.agent_id == agent_id))
            .or_else(|| self.medium.iter().find(|r| r.agent_id == agent_id))
            .or_else(|| self.low.iter().find(|r| r.agent_id == agent_id))
    }

    /// Find and remove request by agent ID
    ///
    /// Returns the request belonging to the specified agent and removes it from the queue.
    pub fn find_and_remove_by_agent_id(&mut self, agent_id: &str) -> Option<HumanDecisionRequest> {
        // Try each queue in priority order
        if let Some(pos) = self.critical.iter().position(|r| r.agent_id == agent_id) {
            let request = self.critical.remove(pos);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if let Some(pos) = self.high.iter().position(|r| r.agent_id == agent_id) {
            let request = self.high.remove(pos);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if let Some(pos) = self.medium.iter().position(|r| r.agent_id == agent_id) {
            let request = self.medium.remove(pos);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        if let Some(pos) = self.low.iter().position(|r| r.agent_id == agent_id) {
            let request = self.low.remove(pos);
            self.id_index.remove(&request.id);
            return Some(request);
        }
        None
    }

    /// Complete request
    pub fn complete(&mut self, response: HumanDecisionResponse) -> bool {
        // Find and remove request
        let removed = self.find_and_remove(&response.request_id);
        if removed.is_some() {
            self.history.push(response);
            true
        } else {
            false
        }
    }

    fn find_and_remove(&mut self, id: &str) -> Option<HumanDecisionRequest> {
        // O(1) lookup using index
        let urgency = self.id_index.remove(id)?;

        // Remove from the appropriate queue
        let queue = match urgency {
            UrgencyLevel::Critical => &mut self.critical,
            UrgencyLevel::High => &mut self.high,
            UrgencyLevel::Medium => &mut self.medium,
            UrgencyLevel::Low => &mut self.low,
        };

        queue
            .iter()
            .position(|r| r.id == id)
            .map(|pos| queue.remove(pos))
    }

    /// Check for expired requests
    pub fn check_expired(&mut self) -> Vec<HumanDecisionRequest> {
        let now = Utc::now();
        let expired: Vec<HumanDecisionRequest> = self
            .all_requests()
            .into_iter()
            .filter(|r| now > r.expires_at)
            .cloned()
            .collect();

        // Remove expired
        for req in &expired {
            self.find_and_remove(&req.id);
        }

        expired
    }

    /// Get requests approaching timeout
    pub fn approaching_timeout(&self) -> Vec<&HumanDecisionRequest> {
        let warning_threshold = Utc::now()
            + chrono::Duration::milliseconds(self.timeout_config.warning_before_ms as i64);
        self.all_requests()
            .into_iter()
            .filter(|r| r.expires_at < warning_threshold && !r.is_expired())
            .collect()
    }

    fn all_requests(&self) -> Vec<&HumanDecisionRequest> {
        self.critical
            .iter()
            .chain(self.high.iter())
            .chain(self.medium.iter())
            .chain(self.low.iter())
            .collect()
    }

    pub fn total_pending(&self) -> usize {
        self.critical.len() + self.high.len() + self.medium.len() + self.low.len()
    }

    pub fn critical_count(&self) -> usize {
        self.critical.len()
    }

    pub fn high_count(&self) -> usize {
        self.high.len()
    }

    pub fn medium_count(&self) -> usize {
        self.medium.len()
    }

    pub fn low_count(&self) -> usize {
        self.low.len()
    }

    pub fn history(&self) -> &[HumanDecisionResponse] {
        &self.history
    }

    pub fn timeout_config(&self) -> &HumanDecisionTimeoutConfig {
        &self.timeout_config
    }

    /// Clear all pending requests and history
    pub fn clear(&mut self) {
        self.critical.clear();
        self.high.clear();
        self.medium.clear();
        self.low.clear();
        self.id_index.clear();
        self.history.clear();
    }
}

impl Default for HumanDecisionQueue {
    fn default() -> Self {
        Self::new(HumanDecisionTimeoutConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::situation::builtin_situations::WaitingForChoiceSituation;

    #[test]
    fn test_blocking_reason_trait() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![ChoiceOption::new("A", "Option A")],
        );
        assert_eq!(blocking.reason_type(), "human_decision");
    }

    #[test]
    fn test_blocked_state_new() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let state = BlockedState::new(Box::new(blocking));
        assert!(!state.is_expired());
    }

    #[test]
    fn test_blocked_state_elapsed() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let state = BlockedState::new(Box::new(blocking));
        let elapsed = state.elapsed();
        assert!(elapsed < Duration::from_secs(1)); // Just created
    }

    #[test]
    fn test_human_decision_blocking_urgency() {
        let situation = WaitingForChoiceSituation::new(vec![]).critical();
        let blocking = HumanDecisionBlocking::new("req-1", Box::new(situation), vec![]);
        assert_eq!(blocking.urgency(), UrgencyLevel::High);
    }

    #[test]
    fn test_human_decision_blocking_auto_resolve_without_recommendation() {
        // Bug fix test: Without recommendation and without options,
        // auto_resolve_action should return None (cannot auto resolve)
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![], // Empty options
        );
        // Without recommendation AND without options, can_auto_resolve is false
        assert!(!blocking.can_auto_resolve());
        // auto_resolve_action returns None when there's nothing to select
        assert_eq!(blocking.auto_resolve_action(), None);
    }

    #[test]
    fn test_human_decision_blocking_auto_resolve_without_recommendation_but_with_options() {
        // Bug fix test: Without recommendation but WITH options,
        // auto_resolve_action should return SelectDefault
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![ChoiceOption::new("A", "Option A")], // Has options
        );
        // With options available, can_auto_resolve should be true
        assert!(blocking.can_auto_resolve());
        // auto_resolve_action returns SelectDefault when there are options
        assert_eq!(
            blocking.auto_resolve_action(),
            Some(AutoAction::SelectDefault)
        );
    }

    #[test]
    fn test_human_decision_blocking_auto_resolve() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        )
        .with_recommendation(Box::new(crate::builtin_actions::SelectOptionAction::new(
            "A", "test",
        )));
        assert!(blocking.can_auto_resolve());
        assert_eq!(
            blocking.auto_resolve_action(),
            Some(AutoAction::FollowRecommendation)
        );
    }

    #[test]
    fn test_human_decision_blocking_with_recommendation() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        )
        .with_recommendation(Box::new(crate::builtin_actions::SelectOptionAction::new(
            "A", "test",
        )));

        assert!(blocking.can_auto_resolve());
        assert_eq!(
            blocking.auto_resolve_action(),
            Some(AutoAction::FollowRecommendation)
        );
    }

    #[test]
    fn test_resource_blocking() {
        let blocking = ResourceBlocking::new("file", "/tmp/lock", "waiting for file lock");
        assert_eq!(blocking.reason_type(), "resource_waiting");
        assert_eq!(blocking.urgency(), UrgencyLevel::Low);
        assert!(blocking.can_auto_resolve());
    }

    #[test]
    fn test_rate_limit_blocked_reason_description() {
        let started = Utc::now() - chrono::Duration::minutes(10);
        let reason = RateLimitBlockedReason::new(started);
        let desc = reason.description();
        assert!(desc.contains("Rate limited"));
        assert!(desc.contains("💤"));
    }

    #[test]
    fn test_rate_limit_blocked_reason_properties() {
        let reason = RateLimitBlockedReason::new(Utc::now());
        assert!(!reason.can_auto_resolve());
        assert!(reason.auto_resolve_action().is_none());
        assert_eq!(reason.urgency(), UrgencyLevel::Low);
        assert!(reason.expires_at().is_none());
    }

    #[test]
    fn test_rate_limit_blocked_reason_elapsed() {
        let started = Utc::now() - chrono::Duration::minutes(10);
        let reason = RateLimitBlockedReason::new(started);
        let elapsed = reason.elapsed_minutes();
        assert!((9..=11).contains(&elapsed));
    }

    #[test]
    fn test_rate_limit_blocked_reason_retry_count() {
        let reason = RateLimitBlockedReason::new(Utc::now())
            .with_retry_count(3);
        assert_eq!(reason.retry_count(), 3);
    }

    #[test]
    fn test_rate_limit_blocked_reason_can_retry() {
        let reason = RateLimitBlockedReason::new(Utc::now());
        assert!(reason.can_retry_now()); // Never tried

        let reason_with_retry = reason.with_last_retry_at(Utc::now() - chrono::Duration::seconds(1));
        assert!(!reason_with_retry.can_retry_now()); // Tried recently
    }

    #[test]
    fn test_agent_slot_status_running() {
        let status = AgentSlotStatus::running();
        assert!(status.is_running());
        assert!(!status.is_blocked());
    }

    #[test]
    fn test_agent_slot_status_blocked() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let status = AgentSlotStatus::blocked(Box::new(blocking));
        assert!(status.is_blocked());
        assert!(!status.is_running());
    }

    #[test]
    fn test_agent_slot_status_blocked_state() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let status = AgentSlotStatus::blocked(Box::new(blocking));
        let state = status.blocked_state();
        assert!(state.is_some());
        assert_eq!(state.unwrap().reason().reason_type(), "human_decision");
    }

    #[test]
    fn test_blocking_context() {
        let ctx = BlockingContext::new()
            .with_task("task-1")
            .with_story("story-1")
            .with_info("extra", "value");

        assert_eq!(ctx.task_id, Some("task-1".to_string()));
        assert_eq!(ctx.story_id, Some("story-1".to_string()));
        assert_eq!(ctx.additional_info.get("extra"), Some(&"value".to_string()));
    }

    #[test]
    fn test_blocked_state_expiration() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        )
        .with_expires_at(Utc::now() - chrono::Duration::hours(1)); // Already expired

        let state = BlockedState::new(Box::new(blocking));
        assert!(state.is_expired());
    }

    #[test]
    fn test_auto_action() {
        assert_eq!(
            AutoAction::FollowRecommendation,
            AutoAction::FollowRecommendation
        );
        assert_ne!(AutoAction::FollowRecommendation, AutoAction::Cancel);
    }

    #[test]
    fn test_human_decision_blocking_clone() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![ChoiceOption::new("A", "Option A")],
        );
        let cloned = blocking.clone_blocking();
        assert_eq!(blocking.decision_request_id, cloned.decision_request_id);
        assert_eq!(blocking.options.len(), cloned.options.len());
    }

    #[test]
    fn test_blocked_state_debug() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let state = BlockedState::new(Box::new(blocking));
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("BlockedState"));
        assert!(debug_str.contains("human_decision"));
    }

    #[test]
    fn test_recommendation_new() {
        let rec = Recommendation::new("select_option", "Deny dangerous command", 0.95);
        assert_eq!(rec.action_type, "select_option");
        assert_eq!(rec.confidence, 0.95);
    }

    #[test]
    fn test_recommendation_with_params() {
        let rec = Recommendation::new("select_option", "reason", 0.8)
            .with_params("{\"option_id\": \"A\"}");
        assert_eq!(rec.action_params, "{\"option_id\": \"A\"}");
    }

    #[test]
    fn test_human_decision_request_new() {
        let req = HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("waiting_for_choice"),
            vec![ChoiceOption::new("A", "Option A")],
            UrgencyLevel::High,
            1800000,
        );
        assert_eq!(req.id, "req-1");
        assert_eq!(req.urgency, UrgencyLevel::High);
        assert!(!req.is_expired());
    }

    #[test]
    fn test_human_decision_request_with_description() {
        let req = HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::Medium,
            3600000,
        )
        .with_description("Test description");
        assert_eq!(req.situation_description, "Test description");
    }

    #[test]
    fn test_human_selection_selected() {
        let sel = HumanSelection::selected("A");
        assert!(matches!(sel, HumanSelection::Selected { option_id } if option_id == "A"));
    }

    #[test]
    fn test_human_selection_accept_recommendation() {
        let sel = HumanSelection::accept_recommendation();
        assert!(matches!(sel, HumanSelection::AcceptedRecommendation));
    }

    #[test]
    fn test_human_selection_custom() {
        let sel = HumanSelection::custom("Do something else");
        assert!(
            matches!(sel, HumanSelection::Custom { instruction } if instruction == "Do something else")
        );
    }

    #[test]
    fn test_human_selection_skip() {
        let sel = HumanSelection::skip();
        assert!(matches!(sel, HumanSelection::Skipped));
    }

    #[test]
    fn test_human_selection_cancel() {
        let sel = HumanSelection::cancel();
        assert!(matches!(sel, HumanSelection::Cancelled));
    }

    #[test]
    fn test_human_decision_response_new() {
        let resp = HumanDecisionResponse::new("req-1", HumanSelection::selected("A"));
        assert_eq!(resp.request_id, "req-1");
        assert!(matches!(resp.selection, HumanSelection::Selected { .. }));
    }

    #[test]
    fn test_human_decision_timeout_config_default() {
        let config = HumanDecisionTimeoutConfig::default();
        assert_eq!(config.default_timeout_ms, 3600000);
        assert_eq!(config.critical_timeout_ms, 900000);
    }

    #[test]
    fn test_timeout_config_for_urgency() {
        let config = HumanDecisionTimeoutConfig::default();
        assert_eq!(config.timeout_for_urgency(UrgencyLevel::Critical), 900000);
        assert_eq!(config.timeout_for_urgency(UrgencyLevel::High), 1800000);
        assert_eq!(config.timeout_for_urgency(UrgencyLevel::Medium), 3600000);
        assert_eq!(config.timeout_for_urgency(UrgencyLevel::Low), 7200000);
    }

    #[test]
    fn test_human_decision_queue_new() {
        let queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());
        assert_eq!(queue.total_pending(), 0);
    }

    #[test]
    fn test_human_decision_queue_push_critical() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());
        let req = HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::Critical,
            900000,
        );
        queue.push(req);
        assert_eq!(queue.critical_count(), 1);
        assert_eq!(queue.total_pending(), 1);
    }

    #[test]
    fn test_human_decision_queue_push_high() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());
        let req = HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::High,
            1800000,
        );
        queue.push(req);
        assert_eq!(queue.high_count(), 1);
    }

    #[test]
    fn test_human_decision_queue_pop_priority() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());

        // Push in reverse priority order
        queue.push(HumanDecisionRequest::new(
            "low",
            "agent",
            crate::types::SituationType::new("t"),
            vec![],
            UrgencyLevel::Low,
            7200000,
        ));
        queue.push(HumanDecisionRequest::new(
            "medium",
            "agent",
            crate::types::SituationType::new("t"),
            vec![],
            UrgencyLevel::Medium,
            3600000,
        ));
        queue.push(HumanDecisionRequest::new(
            "critical",
            "agent",
            crate::types::SituationType::new("t"),
            vec![],
            UrgencyLevel::Critical,
            900000,
        ));

        // Pop should return critical first
        let first = queue.pop();
        assert!(first.is_some());
        assert_eq!(first.unwrap().id, "critical");
    }

    #[test]
    fn test_human_decision_queue_complete() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());
        queue.push(HumanDecisionRequest::new(
            "req-1",
            "agent",
            crate::types::SituationType::new("t"),
            vec![],
            UrgencyLevel::Medium,
            3600000,
        ));

        let resp = HumanDecisionResponse::new("req-1", HumanSelection::selected("A"));
        assert!(queue.complete(resp));
        assert_eq!(queue.total_pending(), 0);
        assert_eq!(queue.history().len(), 1);
    }

    #[test]
    fn test_human_decision_queue_peek() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());
        queue.push(HumanDecisionRequest::new(
            "req-1",
            "agent",
            crate::types::SituationType::new("t"),
            vec![],
            UrgencyLevel::High,
            1800000,
        ));

        let peeked = queue.peek();
        assert!(peeked.is_some());
        assert_eq!(peeked.unwrap().id, "req-1");

        // Peek should not remove
        assert_eq!(queue.total_pending(), 1);
    }

    #[test]
    fn test_human_decision_request_serde() {
        let req = HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![ChoiceOption::new("A", "Option A")],
            UrgencyLevel::High,
            1800000,
        );
        let json = serde_json::to_string(&req).unwrap();
        let parsed: HumanDecisionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.id, parsed.id);
    }

    #[test]
    fn test_human_selection_serde() {
        let sel = HumanSelection::selected("A");
        let json = serde_json::to_string(&sel).unwrap();
        let parsed: HumanSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(sel, parsed);
    }

    #[test]
    fn test_human_decision_response_serde() {
        let resp = HumanDecisionResponse::new("req-1", HumanSelection::skip());
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: HumanDecisionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.request_id, parsed.request_id);
    }

    #[test]
    fn test_human_decision_queue_index_lookup() {
        // Test that id_index is properly maintained for O(1) lookup
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());

        // Add requests at different urgency levels
        queue.push(HumanDecisionRequest::new(
            "req-critical",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::Critical,
            900000,
        ));
        queue.push(HumanDecisionRequest::new(
            "req-high",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::High,
            1800000,
        ));

        // Complete should use index for O(1) removal
        let resp = HumanDecisionResponse::new("req-high", HumanSelection::selected("A"));
        assert!(queue.complete(resp));

        // Only critical should remain
        assert_eq!(queue.total_pending(), 1);
        assert_eq!(queue.critical_count(), 1);
        assert_eq!(queue.high_count(), 0);
    }

    #[test]
    fn test_human_decision_queue_pop_removes_from_index() {
        let mut queue = HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default());

        queue.push(HumanDecisionRequest::new(
            "req-1",
            "agent-1",
            crate::types::SituationType::new("test"),
            vec![],
            UrgencyLevel::High,
            1800000,
        ));

        assert_eq!(queue.total_pending(), 1);

        // Pop should remove from index
        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id, "req-1");
        assert_eq!(queue.total_pending(), 0);
    }
}
