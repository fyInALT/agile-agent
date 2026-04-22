//! Built-in situation implementations

use chrono::{DateTime, Utc};
use crate::model::situation::{ChoiceOption, CompletionProgress, DecisionSituation, ErrorInfo};
use crate::model::situation::situation_registry::SituationRegistry;
use crate::core::types::{ActionType, SituationType, UrgencyLevel};
use serde::{Deserialize, Serialize};

// Built-in situation type getters (functions instead of const)
pub fn waiting_for_choice() -> SituationType {
    SituationType::new("waiting_for_choice")
}

pub fn claims_completion() -> SituationType {
    SituationType::new("claims_completion")
}

pub fn partial_completion() -> SituationType {
    SituationType::new("partial_completion")
}

pub fn error() -> SituationType {
    SituationType::new("error")
}

// Provider-specific subtypes
pub fn claude_finished() -> SituationType {
    SituationType::with_subtype("finished", "claude")
}

pub fn codex_approval() -> SituationType {
    SituationType::with_subtype("waiting_for_choice", "codex")
}

pub fn acp_permission() -> SituationType {
    SituationType::with_subtype("waiting_for_choice", "acp")
}

/// Agent idle situation - triggered when agent enters idle state
pub fn agent_idle() -> SituationType {
    SituationType::new("agent_idle")
}

/// Task starting situation - triggered when new task is about to begin
pub fn task_starting() -> SituationType {
    SituationType::new("task_starting")
}

/// Uncommitted changes detected situation
pub fn uncommitted_changes_detected() -> SituationType {
    SituationType::new("uncommitted_changes_detected")
}

/// Situation: Task starting - needs git preparation before work begins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStartingSituation {
    /// Task description
    pub task_description: String,

    /// Task ID from backlog (if available)
    pub task_id: Option<String>,

    /// Extracted task metadata (branch name, type, etc.)
    pub task_meta: Option<crate::model::task::task_metadata::TaskMetadata>,

    /// Current git state (if analyzed)
    pub git_state: Option<crate::state::git_state::GitState>,

    /// Whether git state has been analyzed
    pub git_state_analyzed: bool,

    /// Current worktree path
    pub worktree_path: Option<String>,
}

impl TaskStartingSituation {
    pub fn new(task_description: impl Into<String>) -> Self {
        Self {
            task_description: task_description.into(),
            task_id: None,
            task_meta: None,
            git_state: None,
            git_state_analyzed: false,
            worktree_path: None,
        }
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_task_meta(mut self, meta: crate::model::task::task_metadata::TaskMetadata) -> Self {
        self.task_meta = Some(meta);
        self
    }

    pub fn with_git_state(mut self, state: crate::state::git_state::GitState) -> Self {
        self.git_state = Some(state);
        self.git_state_analyzed = true;
        self
    }

    pub fn with_worktree_path(mut self, path: impl Into<String>) -> Self {
        self.worktree_path = Some(path.into());
        self
    }
}

impl Default for TaskStartingSituation {
    fn default() -> Self {
        Self::new("Untitled task")
    }
}

impl DecisionSituation for TaskStartingSituation {
    fn situation_type(&self) -> SituationType {
        task_starting()
    }

    fn implementation_type(&self) -> &'static str {
        "TaskStartingSituation"
    }

    fn requires_human(&self) -> bool {
        // Only if there are git conflicts or critical decisions
        self.git_state
            .as_ref()
            .map(|g| g.has_conflicts)
            .unwrap_or(false)
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.git_state
            .as_ref()
            .map(|g| g.has_conflicts)
            .unwrap_or(false)
        {
            UrgencyLevel::High
        } else {
            UrgencyLevel::Low
        }
    }

    fn available_actions(&self) -> Vec<ActionType> {
        let mut actions = vec![
            crate::builtin_actions::prepare_task_start(),
            crate::builtin_actions::create_task_branch(),
            crate::builtin_actions::rebase_to_main(),
        ];

        if self.git_state
            .as_ref()
            .map(|g| g.has_uncommitted)
            .unwrap_or(false)
        {
            actions.push(crate::builtin_actions::custom_instruction());
        }

        actions.push(crate::builtin_actions::request_human());

        actions
    }

    fn to_prompt_text(&self) -> String {
        let mut text = format!("Task starting: {}", self.task_description);

        if let Some(ref meta) = self.task_meta {
            text.push_str(&format!("\nBranch: {}", meta.branch_name));
            text.push_str(&format!("\nType: {}", meta.task_type));
        }

        if let Some(ref git_state) = self.git_state {
            text.push_str(&format!("\nCurrent branch: {}", git_state.current_branch));
            if git_state.has_uncommitted {
                text.push_str(&format!(
                    "\nUncommitted changes: {} files",
                    git_state.uncommitted_files.len()
                ));
            }
            if git_state.has_conflicts {
                text.push_str("\n⚠️ Conflicts detected!");
            }
            if git_state.commits_behind > 0 {
                text.push_str(&format!(
                    "\n⚠️ Branch is {} commits behind base",
                    git_state.commits_behind
                ));
            }
        }

        text
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation: Uncommitted changes detected - needs handling before task switch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncommittedChangesSituation {
    /// Git state with uncommitted files
    pub git_state: crate::state::git_state::GitState,

    /// Analysis of the changes
    pub analysis: crate::uncommitted_handler::UncommittedAnalysis,

    /// Current task ID (if switching tasks)
    pub pending_task_id: Option<String>,

    /// Worktree path for operations
    pub worktree_path: Option<String>,
}

impl UncommittedChangesSituation {
    /// Create a new uncommitted changes situation
    pub fn new(
        git_state: crate::state::git_state::GitState,
        analysis: crate::uncommitted_handler::UncommittedAnalysis,
    ) -> Self {
        Self {
            git_state,
            analysis,
            pending_task_id: None,
            worktree_path: None,
        }
    }

    /// Set pending task ID
    pub fn with_pending_task(mut self, task_id: impl Into<String>) -> Self {
        self.pending_task_id = Some(task_id.into());
        self
    }

    /// Set worktree path
    pub fn with_worktree_path(mut self, path: impl Into<String>) -> Self {
        self.worktree_path = Some(path.into());
        self
    }
}

impl DecisionSituation for UncommittedChangesSituation {
    fn situation_type(&self) -> SituationType {
        uncommitted_changes_detected()
    }

    fn implementation_type(&self) -> &'static str {
        "UncommittedChangesSituation"
    }

    fn requires_human(&self) -> bool {
        self.analysis.suggested_action == crate::uncommitted_handler::UncommittedAction::RequestHuman
    }

    fn human_urgency(&self) -> UrgencyLevel {
        match self.analysis.changes_context {
            crate::uncommitted_handler::ChangesContext::Temporary => UrgencyLevel::Low,
            _ => UrgencyLevel::Medium,
        }
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            crate::builtin_actions::commit_changes(),
            crate::builtin_actions::stash_changes(),
            crate::builtin_actions::discard_changes(),
            crate::builtin_actions::request_human(),
        ]
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Uncommitted changes detected:\n\
             Files: {}\n\
             Context: {}\n\
             Value: {}\n\
             Suggested: {}",
            self.git_state.uncommitted_files.len(),
            self.analysis.changes_context,
            if self.analysis.is_valuable {
                "valuable"
            } else {
                "low value"
            },
            self.analysis.suggested_action
        )
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 1: Waiting for choice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct WaitingForChoiceSituation {
    /// Available options
    pub options: Vec<ChoiceOption>,

    /// Permission type (for security check)
    pub permission_type: Option<String>,

    /// Whether this is a critical choice
    pub critical: bool,
}

impl WaitingForChoiceSituation {
    pub fn new(options: Vec<ChoiceOption>) -> Self {
        Self {
            options,
            permission_type: None,
            critical: false,
        }
    }

    pub fn with_permission_type(self, permission_type: impl Into<String>) -> Self {
        Self {
            permission_type: Some(permission_type.into()),
            ..self
        }
    }

    pub fn critical(self) -> Self {
        Self {
            critical: true,
            ..self
        }
    }
}


impl DecisionSituation for WaitingForChoiceSituation {
    fn situation_type(&self) -> SituationType {
        waiting_for_choice()
    }

    fn implementation_type(&self) -> &'static str {
        "WaitingForChoiceSituation"
    }

    fn requires_human(&self) -> bool {
        self.critical
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.critical {
            UrgencyLevel::High
        } else {
            UrgencyLevel::Low
        }
    }

    fn to_prompt_text(&self) -> String {
        let options_text = self
            .options
            .iter()
            .map(|o| format!("[{}] {}", o.id, o.label))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Waiting for choice:\nOptions:\n{}\nPermission type: {}",
            options_text,
            self.permission_type.as_deref().unwrap_or("unknown")
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("select_option"),
            ActionType::new("select_first"),
            ActionType::new("reject_all"),
            ActionType::new("custom_instruction"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 2: Claims completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimsCompletionSituation {
    /// Completion summary
    pub summary: String,

    /// Reflection rounds so far
    pub reflection_rounds: u8,

    /// Maximum reflection rounds
    pub max_reflection_rounds: u8,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

impl ClaimsCompletionSituation {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            reflection_rounds: 0,
            max_reflection_rounds: 2,
            confidence: 0.8,
        }
    }

    pub fn with_reflection_rounds(self, rounds: u8, max: u8) -> Self {
        Self {
            reflection_rounds: rounds,
            max_reflection_rounds: max,
            ..self
        }
    }

    pub fn with_confidence(self, confidence: f64) -> Self {
        Self { confidence, ..self }
    }
}

impl Default for ClaimsCompletionSituation {
    fn default() -> Self {
        Self {
            summary: String::new(),
            reflection_rounds: 0,
            max_reflection_rounds: 2,
            confidence: 0.8,
        }
    }
}

impl DecisionSituation for ClaimsCompletionSituation {
    fn situation_type(&self) -> SituationType {
        claims_completion()
    }

    fn implementation_type(&self) -> &'static str {
        "ClaimsCompletionSituation"
    }

    fn requires_human(&self) -> bool {
        // Requires human if reflection exhausted and low confidence
        self.reflection_rounds >= self.max_reflection_rounds && self.confidence < 0.7
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.confidence < 0.5 {
            UrgencyLevel::Critical
        } else {
            UrgencyLevel::Medium
        }
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Claims completion (round {}):\nSummary: {}\nConfidence: {:.0}%",
            self.reflection_rounds,
            self.summary,
            self.confidence * 100.0
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        if self.reflection_rounds < self.max_reflection_rounds {
            vec![
                ActionType::new("reflect"),
                ActionType::new("confirm_completion"),
            ]
        } else {
            vec![
                ActionType::new("confirm_completion"),
                ActionType::new("request_human"),
            ]
        }
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 3: Partial completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct PartialCompletionSituation {
    pub progress: CompletionProgress,
    pub blocker: Option<String>,
}

impl PartialCompletionSituation {
    pub fn new(progress: CompletionProgress) -> Self {
        Self {
            progress,
            blocker: None,
        }
    }

    pub fn with_blocker(self, blocker: impl Into<String>) -> Self {
        Self {
            blocker: Some(blocker.into()),
            ..self
        }
    }
}


impl DecisionSituation for PartialCompletionSituation {
    fn situation_type(&self) -> SituationType {
        partial_completion()
    }

    fn implementation_type(&self) -> &'static str {
        "PartialCompletionSituation"
    }

    fn requires_human(&self) -> bool {
        self.blocker.is_some()
    }

    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Medium
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Partial completion:\nCompleted: {}\nRemaining: {}\nBlocker: {}",
            self.progress.completed_items.join(", "),
            self.progress.remaining_items.join(", "),
            self.blocker.as_deref().unwrap_or("none")
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("continue"),
            ActionType::new("skip_remaining"),
            ActionType::new("request_context"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 4: Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSituation {
    pub error: ErrorInfo,
}

impl ErrorSituation {
    pub fn new(error: ErrorInfo) -> Self {
        Self { error }
    }
}

impl Default for ErrorSituation {
    fn default() -> Self {
        Self {
            error: ErrorInfo::new("unknown", "Unknown error"),
        }
    }
}

impl DecisionSituation for ErrorSituation {
    fn situation_type(&self) -> SituationType {
        error()
    }

    fn implementation_type(&self) -> &'static str {
        "ErrorSituation"
    }

    fn requires_human(&self) -> bool {
        !self.error.recoverable || self.error.retry_count >= 3
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.error.recoverable {
            UrgencyLevel::Medium
        } else {
            UrgencyLevel::High
        }
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Error (retry {}):\nType: {}\nMessage: {}\nRecoverable: {}",
            self.error.retry_count,
            self.error.error_type,
            self.error.message,
            self.error.recoverable
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        if self.error.recoverable && self.error.retry_count < 3 {
            vec![
                ActionType::new("retry"),
                ActionType::new("retry_adjusted"),
                ActionType::new("restart"),
            ]
        } else {
            vec![ActionType::new("request_human"), ActionType::new("abort")]
        }
    }

    fn error_info(&self) -> Option<&crate::model::situation::ErrorInfo> {
        Some(&self.error)
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Initialize registry with built-in situations
pub fn register_situation_builtins(registry: &SituationRegistry) {
    registry.register_default(Box::new(WaitingForChoiceSituation::default()));
    registry.register_default(Box::new(ClaimsCompletionSituation::default()));
    registry.register_default(Box::new(PartialCompletionSituation::default()));
    registry.register_default(Box::new(ErrorSituation::default()));
    registry.register_default(Box::new(AgentIdleSituation::default()));
    registry.register_default(Box::new(TaskStartingSituation::default()));
}

/// Situation: Agent Idle
///
/// Triggered when an agent enters idle state. The decision layer needs to
/// determine whether the agent should continue working on pending tasks
/// or stop if all tasks are complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdleSituation {
    /// Trigger reason (idle_timeout, idle_check, finished)
    pub trigger_reason: String,
    /// Whether agent has an assigned task
    pub has_assigned_task: bool,
    /// Idle duration in seconds
    pub idle_duration_secs: u64,
}

impl AgentIdleSituation {
    pub fn new(trigger_reason: impl Into<String>) -> Self {
        Self {
            trigger_reason: trigger_reason.into(),
            has_assigned_task: false,
            idle_duration_secs: 0,
        }
    }

    pub fn with_assigned_task(self, has_task: bool) -> Self {
        Self {
            has_assigned_task: has_task,
            ..self
        }
    }

    pub fn with_idle_duration(self, secs: u64) -> Self {
        Self {
            idle_duration_secs: secs,
            ..self
        }
    }
}

impl Default for AgentIdleSituation {
    fn default() -> Self {
        Self::new("unknown")
    }
}

impl DecisionSituation for AgentIdleSituation {
    fn situation_type(&self) -> SituationType {
        agent_idle()
    }

    fn implementation_type(&self) -> &'static str {
        "AgentIdleSituation"
    }

    fn requires_human(&self) -> bool {
        // Not critical - decision layer can handle automatically
        false
    }

    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Agent Idle State:\nTrigger: {}\nHas assigned task: {}\nIdle duration: {}s\n\n\
            Determine whether agent should continue working or stop.\n\
            Check Kanban/Backlog for pending tasks.",
            self.trigger_reason, self.has_assigned_task, self.idle_duration_secs
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("continue_all_tasks"),
            ActionType::new("stop_if_complete"),
            ActionType::new("request_human"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation: Rate Limit Recovery
///
/// Triggered when an agent is in resting state due to HTTP 429 and needs to
/// decide whether to attempt recovery (retry) or continue waiting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRecoverySituation {
    started_at: DateTime<Utc>,
    retry_count: u32,
    last_error: Option<String>,
}

impl RateLimitRecoverySituation {
    pub fn new(started_at: DateTime<Utc>, retry_count: u32) -> Self {
        Self {
            started_at,
            retry_count,
            last_error: None,
        }
    }

    pub fn with_last_error(self, error: impl Into<String>) -> Self {
        Self {
            last_error: Some(error.into()),
            ..self
        }
    }

    /// Minutes since first 429
    pub fn elapsed_minutes(&self) -> i64 {
        (Utc::now() - self.started_at).num_minutes()
    }
}

impl Default for RateLimitRecoverySituation {
    fn default() -> Self {
        Self::new(Utc::now(), 0)
    }
}

impl DecisionSituation for RateLimitRecoverySituation {
    fn situation_type(&self) -> SituationType {
        SituationType::new("rate_limit_recovery")
    }

    fn implementation_type(&self) -> &'static str {
        "RateLimitRecoverySituation"
    }

    fn requires_human(&self) -> bool {
        false
    }

    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn to_prompt_text(&self) -> String {
        let mins = self.elapsed_minutes();
        let error_text = self.last_error.as_deref().unwrap_or("None");
        format!(
            "Rate Limit Recovery:\nFirst 429 hit: {} min ago\nRetry attempts: {}\nLast error: {}\n\n\
            Option: retry (try LLM call to check if rate limit cleared)\n\
            Option: request_human (ask user for manual intervention)",
            mins, self.retry_count, error_text
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("retry"),
            ActionType::new("request_human"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waiting_for_choice_situation_type() {
        let situation = WaitingForChoiceSituation::default();
        assert_eq!(situation.situation_type(), waiting_for_choice());
    }

    #[test]
    fn test_waiting_for_choice_options() {
        let situation = WaitingForChoiceSituation::new(vec![
            ChoiceOption::new("A", "Option A"),
            ChoiceOption::new("B", "Option B"),
        ]);
        assert_eq!(situation.options.len(), 2);
        assert_eq!(situation.options[0].id, "A");
    }

    #[test]
    fn test_waiting_for_choice_critical() {
        let situation = WaitingForChoiceSituation::new(vec![]).critical();
        assert!(situation.requires_human());
        assert_eq!(situation.human_urgency(), UrgencyLevel::High);
    }

    #[test]
    fn test_waiting_for_choice_available_actions() {
        let situation = WaitingForChoiceSituation::default();
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("select_option")));
        assert!(actions.contains(&ActionType::new("reject_all")));
    }

    #[test]
    fn test_claims_completion_situation_type() {
        let situation = ClaimsCompletionSituation::default();
        assert_eq!(situation.situation_type(), claims_completion());
    }

    #[test]
    fn test_claims_completion_reflection_rounds() {
        let situation = ClaimsCompletionSituation::new("Done")
            .with_reflection_rounds(1, 2)
            .with_confidence(0.9);
        assert_eq!(situation.reflection_rounds, 1);
        assert_eq!(situation.max_reflection_rounds, 2);
        assert!(!situation.requires_human()); // High confidence, not exhausted
    }

    #[test]
    fn test_claims_completion_requires_human_when_exhausted() {
        let situation = ClaimsCompletionSituation::new("Done")
            .with_reflection_rounds(2, 2)
            .with_confidence(0.5); // Low confidence
        assert!(situation.requires_human());
    }

    #[test]
    fn test_claims_completion_available_actions_reflect() {
        let situation = ClaimsCompletionSituation::new("Done").with_reflection_rounds(0, 2);
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("reflect")));
    }

    #[test]
    fn test_claims_completion_available_actions_no_reflect() {
        let situation = ClaimsCompletionSituation::new("Done").with_reflection_rounds(2, 2);
        let actions = situation.available_actions();
        assert!(!actions.contains(&ActionType::new("reflect")));
    }

    #[test]
    fn test_partial_completion_situation_type() {
        let situation = PartialCompletionSituation::default();
        assert_eq!(situation.situation_type(), partial_completion());
    }

    #[test]
    fn test_partial_completion_progress() {
        let progress = CompletionProgress {
            completed_items: vec!["item1".to_string()],
            remaining_items: vec!["item2".to_string()],
            estimated_remaining_minutes: Some(30),
        };
        let situation = PartialCompletionSituation::new(progress);
        assert_eq!(situation.progress.completed_items.len(), 1);
        assert_eq!(situation.progress.remaining_items.len(), 1);
    }

    #[test]
    fn test_partial_completion_blocker() {
        let situation = PartialCompletionSituation::default().with_blocker("Missing dependency");
        assert!(situation.requires_human());
    }

    #[test]
    fn test_error_situation_type() {
        let situation = ErrorSituation::default();
        assert_eq!(situation.situation_type(), error());
    }

    #[test]
    fn test_error_situation_recoverable() {
        let error = ErrorInfo::new("timeout", "Connection timeout").with_retry_count(1);
        let situation = ErrorSituation::new(error);
        assert!(situation.error.recoverable);
        assert!(!situation.requires_human()); // Recoverable and retry count < 3
    }

    #[test]
    fn test_error_situation_unrecoverable() {
        let error = ErrorInfo::new("fatal", "Critical error").unrecoverable();
        let situation = ErrorSituation::new(error);
        assert!(situation.requires_human());
    }

    #[test]
    fn test_error_situation_retry_count_exhausted() {
        let error = ErrorInfo::new("timeout", "Timeout").with_retry_count(3);
        let situation = ErrorSituation::new(error);
        assert!(situation.requires_human());
    }

    #[test]
    fn test_to_prompt_text_format() {
        let situation = WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]);
        let text = situation.to_prompt_text();
        assert!(text.contains("Waiting for choice"));
        assert!(text.contains("[A] Option A"));
    }

    #[test]
    fn test_register_builtins() {
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);

        assert!(registry.is_registered(&waiting_for_choice()));
        assert!(registry.is_registered(&claims_completion()));
        assert!(registry.is_registered(&partial_completion()));
        assert!(registry.is_registered(&error()));
    }

    #[test]
    fn test_situation_serde() {
        let situation = WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")])
            .with_permission_type("execute");

        let json = serde_json::to_string(&situation).unwrap();
        let parsed: WaitingForChoiceSituation = serde_json::from_str(&json).unwrap();
        assert_eq!(situation.options.len(), parsed.options.len());
        assert_eq!(situation.permission_type, parsed.permission_type);
    }

    #[test]
    fn test_situation_type_getters() {
        assert_eq!(waiting_for_choice().name, "waiting_for_choice");
        assert_eq!(claims_completion().name, "claims_completion");
        assert_eq!(claude_finished().name, "finished");
        assert_eq!(claude_finished().subtype, Some("claude".to_string()));
    }

    #[test]
    fn test_rate_limit_recovery_situation_type() {
        let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
        assert_eq!(situation.situation_type(), SituationType::new("rate_limit_recovery"));
    }

    #[test]
    fn test_rate_limit_recovery_available_actions() {
        let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("retry")));
        assert!(actions.contains(&ActionType::new("request_human")));
    }

    #[test]
    fn test_rate_limit_recovery_requires_human_false() {
        let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
        assert!(!situation.requires_human());
    }

    #[test]
    fn test_rate_limit_recovery_elapsed_minutes() {
        let started = Utc::now() - chrono::Duration::minutes(15);
        let situation = RateLimitRecoverySituation::new(started, 2);
        let elapsed = situation.elapsed_minutes();
        assert!(elapsed >= 14 && elapsed <= 16);
    }

    #[test]
    fn test_rate_limit_recovery_to_prompt_text() {
        let situation = RateLimitRecoverySituation::new(Utc::now(), 3);
        let text = situation.to_prompt_text();
        assert!(text.contains("Rate Limit Recovery"));
        assert!(text.contains("retry"));
        assert!(text.contains("request_human"));
    }

    #[test]
    fn test_task_starting_situation_type() {
        let situation = TaskStartingSituation::default();
        assert_eq!(situation.situation_type(), task_starting());
    }

    #[test]
    fn test_task_starting_with_task_id() {
        let situation = TaskStartingSituation::new("Implement login feature")
            .with_task_id("PROJ-123");
        assert_eq!(situation.task_id, Some("PROJ-123".to_string()));
        assert_eq!(situation.task_description, "Implement login feature");
    }

    #[test]
    fn test_task_starting_does_not_require_human_without_conflicts() {
        let situation = TaskStartingSituation::new("Simple task");
        assert!(!situation.requires_human());
    }

    #[test]
    fn test_task_starting_available_actions() {
        let situation = TaskStartingSituation::new("Test task");
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("prepare_task_start")));
        assert!(actions.contains(&ActionType::new("create_task_branch")));
        assert!(actions.contains(&ActionType::new("rebase_to_main")));
        assert!(actions.contains(&ActionType::new("request_human")));
    }

    #[test]
    fn test_task_starting_to_prompt_text() {
        let situation = TaskStartingSituation::new("Implement feature X")
            .with_task_id("TASK-001");
        let text = situation.to_prompt_text();
        assert!(text.contains("Task starting"));
        assert!(text.contains("Implement feature X"));
        // Note: task_id is stored but not displayed in to_prompt_text (it's in task_meta for that)
    }

    #[test]
    fn test_task_starting_with_worktree_path() {
        let situation = TaskStartingSituation::new("Task with path")
            .with_worktree_path("/path/to/worktree");
        assert_eq!(situation.worktree_path, Some("/path/to/worktree".to_string()));
    }
}
