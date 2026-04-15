//! Blocking reason trait and blocked state

use crate::action::DecisionAction;
use crate::situation::{ChoiceOption, DecisionSituation};
use crate::types::UrgencyLevel;
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
        Self { additional_info, ..self }
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
        self.recommendation.is_some()
    }

    fn auto_resolve_action(&self) -> Option<AutoAction> {
        if self.recommendation.is_some() {
            Some(AutoAction::FollowRecommendation)
        } else {
            Some(AutoAction::SelectDefault)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_situations::WaitingForChoiceSituation;

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
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        // Without recommendation, can_auto_resolve is false
        assert!(!blocking.can_auto_resolve());
        // But auto_resolve_action still returns SelectDefault as fallback
        assert_eq!(blocking.auto_resolve_action(), Some(AutoAction::SelectDefault));
    }

    #[test]
    fn test_human_decision_blocking_auto_resolve() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        )
        .with_recommendation(Box::new(
            crate::builtin_actions::SelectOptionAction::new("A", "test"),
        ));
        assert!(blocking.can_auto_resolve());
        assert_eq!(blocking.auto_resolve_action(), Some(AutoAction::FollowRecommendation));
    }

    #[test]
    fn test_human_decision_blocking_with_recommendation() {
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        )
        .with_recommendation(Box::new(
            crate::builtin_actions::SelectOptionAction::new("A", "test"),
        ));

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
}