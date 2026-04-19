//! Decision agent lifecycle management

use crate::core::output::DecisionRecord;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Decision agent creation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DecisionAgentCreationPolicy {
    /// Create immediately when Main Agent spawns
    Eager,

    /// Create on first blocked event (recommended)
    #[default]
    Lazy,

    /// Follow configuration setting
    Configured,
}

impl DecisionAgentCreationPolicy {
    pub fn is_eager(&self) -> bool {
        matches!(self, DecisionAgentCreationPolicy::Eager)
    }

    pub fn is_lazy(&self) -> bool {
        matches!(self, DecisionAgentCreationPolicy::Lazy)
    }
}

/// Destruction trigger
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestructionTrigger {
    /// Story completed successfully
    StoryComplete,

    /// Main Agent stopped
    MainAgentStopped,

    /// Idle timeout exceeded
    IdleTimeout,

    /// Manual stop command
    ManualStop,

    /// Error requiring full reset
    FatalError,
}

impl DestructionTrigger {
    pub fn is_story_complete(&self) -> bool {
        matches!(self, DestructionTrigger::StoryComplete)
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, DestructionTrigger::FatalError)
    }
}

/// Task ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        TaskId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Story ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoryId(pub String);

impl StoryId {
    pub fn new(id: impl Into<String>) -> Self {
        StoryId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Agent ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        AgentId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task decision context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecisionContext {
    /// Task ID
    pub task_id: TaskId,

    /// Decisions made for this task
    pub decisions: Vec<DecisionRecord>,

    /// Reflection rounds for this task
    pub reflection_rounds: u8,

    /// Retry count for this task
    pub retry_count: u8,

    /// Timeout count for this task
    pub timeout_count: u8,

    /// Task start time
    pub started_at: DateTime<Utc>,

    /// Task completion time (if completed)
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskDecisionContext {
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            decisions: Vec::new(),
            reflection_rounds: 0,
            retry_count: 0,
            timeout_count: 0,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    pub fn add_decision(&mut self, record: DecisionRecord) {
        self.decisions.push(record);
    }

    pub fn increment_reflection(&mut self) {
        self.reflection_rounds += 1;
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn increment_timeout(&mut self) {
        self.timeout_count += 1;
    }

    pub fn mark_complete(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    pub fn decision_count(&self) -> usize {
        self.decisions.len()
    }

    pub fn elapsed_seconds(&self) -> i64 {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        (end - self.started_at).num_seconds()
    }
}

/// Decision agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAgentConfig {
    /// Creation policy
    pub creation_policy: DecisionAgentCreationPolicy,

    /// Idle timeout in milliseconds (default: 30 minutes)
    pub idle_timeout_ms: u64,

    /// Keep transcript after destruction
    pub keep_transcript: bool,

    /// Maximum reflection rounds
    pub max_reflection_rounds: u8,

    /// Maximum retry count
    pub max_retry_count: u8,

    /// Context cache max bytes
    pub context_cache_max_bytes: usize,
}

impl Default for DecisionAgentConfig {
    fn default() -> Self {
        Self {
            creation_policy: DecisionAgentCreationPolicy::Lazy,
            idle_timeout_ms: 1800000, // 30 minutes
            keep_transcript: true,
            max_reflection_rounds: 3,
            max_retry_count: 3,
            context_cache_max_bytes: 10240, // 10KB
        }
    }
}

/// Decision agent state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAgentState {
    /// Decision agent ID
    pub agent_id: AgentId,

    /// Parent main agent ID
    pub parent_agent_id: AgentId,

    /// Current task ID
    pub current_task_id: Option<TaskId>,

    /// Current story ID
    pub current_story_id: Option<StoryId>,

    /// Task contexts (active)
    pub task_contexts: HashMap<TaskId, TaskDecisionContext>,

    /// Configuration
    pub config: DecisionAgentConfig,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Reflection rounds total
    pub reflection_rounds: u8,

    /// Retry count total
    pub retry_count: u8,
}

impl DecisionAgentState {
    pub fn new(agent_id: AgentId, parent_agent_id: AgentId, config: DecisionAgentConfig) -> Self {
        Self {
            agent_id,
            parent_agent_id,
            current_task_id: None,
            current_story_id: None,
            task_contexts: HashMap::new(),
            config,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            reflection_rounds: 0,
            retry_count: 0,
        }
    }

    pub fn switch_task(&mut self, new_task: TaskId) {
        // Archive current task
        if let Some(current_id) = &self.current_task_id {
            if let Some(ctx) = self.task_contexts.get_mut(current_id) {
                ctx.mark_complete();
            }
        }

        // Create or restore new task context
        if !self.task_contexts.contains_key(&new_task) {
            self.task_contexts
                .insert(new_task.clone(), TaskDecisionContext::new(new_task.clone()));
        }

        self.current_task_id = Some(new_task);
        self.last_activity = Utc::now();
    }

    pub fn switch_story(&mut self, new_story: StoryId) {
        self.current_story_id = Some(new_story);
        self.last_activity = Utc::now();

        // Reset reflection rounds for new story
        self.reflection_rounds = 0;
    }

    pub fn record_decision(&mut self, record: DecisionRecord) {
        if let Some(task_id) = &self.current_task_id {
            if let Some(ctx) = self.task_contexts.get_mut(task_id) {
                ctx.add_decision(record);
            }
        }
        self.last_activity = Utc::now();
    }

    pub fn increment_reflection(&mut self) {
        self.reflection_rounds += 1;
        if let Some(task_id) = &self.current_task_id {
            if let Some(ctx) = self.task_contexts.get_mut(task_id) {
                ctx.increment_reflection();
            }
        }
        self.last_activity = Utc::now();
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        if let Some(task_id) = &self.current_task_id {
            if let Some(ctx) = self.task_contexts.get_mut(task_id) {
                ctx.increment_retry();
            }
        }
        self.last_activity = Utc::now();
    }

    pub fn can_reflect(&self) -> bool {
        self.reflection_rounds < self.config.max_reflection_rounds
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.config.max_retry_count
    }

    pub fn is_idle_expired(&self) -> bool {
        let elapsed = (Utc::now() - self.last_activity).num_milliseconds();
        elapsed > self.config.idle_timeout_ms as i64
    }

    pub fn persist(&self, path: &Path) -> crate::error::Result<()> {
        let json = serde_json::to_string(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn restore(path: &Path) -> crate::error::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&json)?;
        Ok(state)
    }

    pub fn clear_for_new_story(&mut self) {
        self.reflection_rounds = 0;
        self.retry_count = 0;
        self.last_activity = Utc::now();
    }

    pub fn destroy(&mut self, _trigger: DestructionTrigger) {
        // Archive all task contexts
        for ctx in self.task_contexts.values_mut() {
            if !ctx.is_complete() {
                ctx.mark_complete();
            }
        }

        // Clear current references
        self.current_task_id = None;
        self.current_story_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_policy_default() {
        let policy = DecisionAgentCreationPolicy::default();
        assert!(policy.is_lazy());
        assert!(!policy.is_eager());
    }

    #[test]
    fn test_creation_policy_eager() {
        let policy = DecisionAgentCreationPolicy::Eager;
        assert!(policy.is_eager());
        assert!(!policy.is_lazy());
    }

    #[test]
    fn test_destruction_trigger_story_complete() {
        let trigger = DestructionTrigger::StoryComplete;
        assert!(trigger.is_story_complete());
        assert!(!trigger.is_fatal());
    }

    #[test]
    fn test_destruction_trigger_fatal() {
        let trigger = DestructionTrigger::FatalError;
        assert!(trigger.is_fatal());
        assert!(!trigger.is_story_complete());
    }

    #[test]
    fn test_task_id() {
        let id = TaskId::new("task-1");
        assert_eq!(id.as_str(), "task-1");
        assert_eq!(format!("{}", id), "task-1");
    }

    #[test]
    fn test_task_decision_context_new() {
        let ctx = TaskDecisionContext::new(TaskId::new("task-1"));
        assert_eq!(ctx.task_id.as_str(), "task-1");
        assert_eq!(ctx.decisions.len(), 0);
        assert_eq!(ctx.reflection_rounds, 0);
        assert!(!ctx.is_complete());
    }

    #[test]
    fn test_task_decision_context_increment() {
        let mut ctx = TaskDecisionContext::new(TaskId::new("task-1"));
        ctx.increment_reflection();
        ctx.increment_retry();
        assert_eq!(ctx.reflection_rounds, 1);
        assert_eq!(ctx.retry_count, 1);
    }

    #[test]
    fn test_task_decision_context_complete() {
        let mut ctx = TaskDecisionContext::new(TaskId::new("task-1"));
        ctx.mark_complete();
        assert!(ctx.is_complete());
        assert!(ctx.completed_at.is_some());
    }

    #[test]
    fn test_decision_agent_config_default() {
        let config = DecisionAgentConfig::default();
        assert!(config.creation_policy.is_lazy());
        assert_eq!(config.idle_timeout_ms, 1800000);
        assert!(config.keep_transcript);
        assert_eq!(config.max_reflection_rounds, 3);
    }

    #[test]
    fn test_decision_agent_state_new() {
        let state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );
        assert_eq!(state.agent_id.as_str(), "dec-1");
        assert_eq!(state.parent_agent_id.as_str(), "main-1");
        assert!(state.current_task_id.is_none());
    }

    #[test]
    fn test_decision_agent_state_switch_task() {
        let mut state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        state.switch_task(TaskId::new("task-1"));
        assert!(state.current_task_id.is_some());
        assert!(state.task_contexts.contains_key(&TaskId::new("task-1")));
    }

    #[test]
    fn test_decision_agent_state_switch_story() {
        let mut state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        state.reflection_rounds = 2;
        state.switch_story(StoryId::new("story-2"));
        assert_eq!(state.reflection_rounds, 0); // Reset for new story
        assert!(state.current_story_id.is_some());
    }

    #[test]
    fn test_decision_agent_state_can_reflect() {
        let state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        assert!(state.can_reflect()); // 0 < 3
    }

    #[test]
    fn test_decision_agent_state_max_reflection() {
        let mut state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        for _ in 0..3 {
            state.increment_reflection();
        }
        assert!(!state.can_reflect()); // 3 >= 3
    }

    #[test]
    fn test_decision_agent_state_can_retry() {
        let state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        assert!(state.can_retry()); // 0 < 3
    }

    #[test]
    fn test_decision_agent_state_serde() {
        let state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        let json = serde_json::to_string(&state).unwrap();
        let parsed: DecisionAgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.agent_id, parsed.agent_id);
    }

    #[test]
    fn test_decision_agent_state_persist() {
        let state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("state.json");

        state.persist(&path).unwrap();
        let restored = DecisionAgentState::restore(&path).unwrap();

        assert_eq!(state.agent_id, restored.agent_id);
    }

    #[test]
    fn test_task_decision_context_elapsed() {
        let ctx = TaskDecisionContext::new(TaskId::new("task-1"));
        // Should be close to 0 seconds since just created
        assert!(ctx.elapsed_seconds() < 5);
    }

    #[test]
    fn test_decision_agent_state_clear_for_new_story() {
        let mut state = DecisionAgentState::new(
            AgentId::new("dec-1"),
            AgentId::new("main-1"),
            DecisionAgentConfig::default(),
        );

        state.reflection_rounds = 2;
        state.retry_count = 1;
        state.clear_for_new_story();

        assert_eq!(state.reflection_rounds, 0);
        assert_eq!(state.retry_count, 0);
    }
}
