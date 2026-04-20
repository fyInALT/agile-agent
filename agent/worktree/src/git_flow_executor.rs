//! Git Flow Executor
//!
//! Unified executor for Git Flow task preparation operations.
//! Orchestrates the complete workflow: metadata extraction, baseline sync,
//! health check, uncommitted handling, and branch setup.

use std::path::PathBuf;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::git_flow_config::{GitFlowConfig, TaskType, TaskPriority};
use crate::worktree_manager::{WorktreeManager, WorktreeError};

/// Git Flow executor errors
#[derive(Debug, thiserror::Error)]
pub enum GitFlowError {
    #[error("git operation failed: {0}")]
    GitOperationFailed(#[from] WorktreeError),
    
    #[error("workspace health check failed: {0}")]
    HealthCheckFailed(String),
    
    #[error("uncommitted changes require handling: {0}")]
    UncommittedChangesNeedHandling(String),
    
    #[error("branch collision detected: existing={0}")]
    BranchCollision(String),
    
    #[error("rebase conflicts detected: {0}")]
    RebaseConflicts(String),
    
    #[error("preparation timeout")]
    PreparationTimeout,
    
    #[error("invalid worktree path: {0}")]
    InvalidWorktreePath(PathBuf),
    
    #[error("configuration error: {0}")]
    ConfigurationError(String),
}

/// Result of task preparation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationResult {
    /// Whether preparation succeeded
    pub success: bool,
    /// Task metadata
    pub task_meta: TaskPreparationMeta,
    /// Branch name created/used
    pub branch_name: String,
    /// Base commit SHA
    pub base_commit: String,
    /// Worktree path
    pub worktree_path: PathBuf,
    /// Whether branch was created fresh or reused
    pub branch_action: BranchAction,
    /// Warnings generated during preparation
    pub warnings: Vec<String>,
    /// Operations log
    pub operations_log: Vec<GitOperationLog>,
    /// Timestamp
    pub prepared_at: DateTime<Utc>,
}

impl PreparationResult {
    /// Create a successful preparation result
    pub fn success(
        task_meta: TaskPreparationMeta,
        branch_name: String,
        base_commit: String,
        worktree_path: PathBuf,
        branch_action: BranchAction,
    ) -> Self {
        Self {
            success: true,
            task_meta,
            branch_name,
            base_commit,
            worktree_path,
            branch_action,
            warnings: Vec::new(),
            operations_log: Vec::new(),
            prepared_at: Utc::now(),
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add an operation log entry
    pub fn with_operation(mut self, operation: GitOperationLog) -> Self {
        self.operations_log.push(operation);
        self
    }

    /// Generate context message for agent
    pub fn to_context_message(&self) -> String {
        let mut msg = String::new();
        msg.push_str("=== Git Flow Task Preparation ===\n\n");
        msg.push_str(&format!("Task: {} ({})\n", self.task_meta.task_id, self.task_meta.summary));
        msg.push_str(&format!("Branch: {}\n", self.branch_name));
        msg.push_str(&format!("Base Commit: {} (origin/{} as of {})\n", 
            &self.base_commit[..8.min(self.base_commit.len())],
            self.task_meta.base_branch,
            self.prepared_at.format("%Y-%m-%d %H:%M")
        ));
        msg.push_str(&format!("Action: {}\n\n", self.branch_action));
        
        if !self.warnings.is_empty() {
            msg.push_str("Warnings:\n");
            for w in &self.warnings {
                msg.push_str(&format!("- {}\n", w));
            }
            msg.push('\n');
        }
        
        msg.push_str("Ready to begin development.\n");
        msg
    }
}

/// Task preparation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPreparationMeta {
    /// Task ID
    pub task_id: String,
    /// Generated branch name
    pub branch_name: String,
    /// Short summary
    pub summary: String,
    /// Original description
    pub original_description: String,
    /// Classified task type
    pub task_type: TaskType,
    /// Task priority
    pub priority: TaskPriority,
    /// Base branch name
    pub base_branch: String,
    /// Classification confidence
    pub confidence: f64,
}

impl TaskPreparationMeta {
    /// Create from task description
    pub fn new(task_id: impl Into<String>, description: impl Into<String>, config: &GitFlowConfig) -> Self {
        let task_id = task_id.into();
        let original_description = description.into();
        
        let (task_type, confidence) = classify_task_type(&original_description);
        let summary = generate_summary(&original_description, config.max_desc_length);
        let branch_name = generate_branch_name(&task_id, task_type, &summary, config);
        
        Self {
            task_id,
            branch_name,
            summary,
            original_description,
            task_type,
            priority: TaskPriority::default(),
            base_branch: config.base_branch.clone(),
            confidence,
        }
    }
}

/// Action taken for branch setup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchAction {
    CreatedNew,
    ReusedExisting,
    RebasedExisting,
    CheckedOutExisting,
}

impl std::fmt::Display for BranchAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchAction::CreatedNew => write!(f, "Created new branch"),
            BranchAction::ReusedExisting => write!(f, "Reused existing branch"),
            BranchAction::RebasedExisting => write!(f, "Rebased existing branch"),
            BranchAction::CheckedOutExisting => write!(f, "Checked out existing branch"),
        }
    }
}

/// Log entry for a git operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationLog {
    pub operation: String,
    pub duration_ms: u64,
    pub result: String,
    pub timestamp: DateTime<Utc>,
}

impl GitOperationLog {
    pub fn new(operation: impl Into<String>, duration_ms: u64, result: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            duration_ms,
            result: result.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Workspace health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceHealthReport {
    pub score: u8,
    pub is_ready_for_task: bool,
    pub issues: Vec<HealthIssue>,
    pub recommendations: Vec<String>,
}

impl WorkspaceHealthReport {
    pub fn healthy() -> Self {
        Self {
            score: 100,
            is_ready_for_task: true,
            issues: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn with_issues(score: u8, issues: Vec<HealthIssue>) -> Self {
        Self {
            score,
            is_ready_for_task: score >= 80,
            issues,
            recommendations: Vec::new(),
        }
    }
}

/// Health issue detected in workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
    pub suggested_action: String,
}

impl HealthIssue {
    pub fn new(severity: IssueSeverity, category: IssueCategory, description: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            severity,
            category,
            description: description.into(),
            suggested_action: action.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity { Critical, Warning, Info }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueCategory {
    UncommittedChanges,
    BranchStatus,
    WorktreeState,
    ConflictState,
    NetworkIssue,
}

/// Git Flow Executor
pub struct GitFlowExecutor {
    worktree_manager: WorktreeManager,
    config: GitFlowConfig,
}

impl GitFlowExecutor {
    pub fn new(worktree_manager: WorktreeManager, config: GitFlowConfig) -> Self {
        Self { worktree_manager, config }
    }

    pub fn with_defaults(worktree_manager: WorktreeManager) -> Self {
        Self::new(worktree_manager, GitFlowConfig::default())
    }

    /// Prepare workspace for a new task
    pub fn prepare_for_task(
        &self,
        worktree_path: &PathBuf,
        task_id: &str,
        description: &str,
    ) -> Result<PreparationResult, GitFlowError> {
        let start_time = std::time::Instant::now();
        
        let task_meta = TaskPreparationMeta::new(task_id, description, &self.config);
        crate::logging::debug_event(
            "git_flow.preparation.metadata",
            "task metadata extracted",
            serde_json::json!({"task_id": task_id, "branch": task_meta.branch_name}),
        );

        let health = self.check_health(worktree_path)?;
        if health.score < 50 {
            return Err(GitFlowError::HealthCheckFailed(
                format!("Workspace health score too low: {}", health.score)
            ));
        }

        if health.issues.iter().any(|i| i.category == IssueCategory::UncommittedChanges) {
            if self.config.auto_stash_changes {
                self.handle_uncommitted_stash(worktree_path)?;
            } else {
                return Err(GitFlowError::UncommittedChangesNeedHandling(
                    "Uncommitted changes detected but auto_stash is disabled".to_string()
                ));
            }
        }

        let base_commit = if self.config.auto_sync_baseline {
            self.sync_baseline()?
        } else {
            self.worktree_manager.get_remote_head(&self.config.base_branch)?
        };

        let (branch_name, branch_action) = self.setup_branch(&task_meta, worktree_path)?;

        let result = PreparationResult::success(
            task_meta, branch_name, base_commit, worktree_path.clone(), branch_action,
        )
        .with_operation(GitOperationLog::new("prepare_for_task", start_time.elapsed().as_millis() as u64, "success"));

        let result = health.issues.iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .fold(result, |r, i| r.with_warning(&i.description));

        crate::logging::debug_event(
            "git_flow.preparation.completed",
            "task preparation completed",
            serde_json::json!({"task_id": task_id, "branch": result.branch_name}),
        );

        Ok(result)
    }

    pub fn check_health(&self, worktree_path: &PathBuf) -> Result<WorkspaceHealthReport, GitFlowError> {
        let mut issues: Vec<HealthIssue> = Vec::new();
        let mut score: u8 = 100;

        if !worktree_path.exists() {
            return Err(GitFlowError::InvalidWorktreePath(worktree_path.clone()));
        }

        let has_uncommitted = self.worktree_manager.has_uncommitted_changes(worktree_path)?;
        if has_uncommitted {
            score -= 20;
            issues.push(HealthIssue::new(
                IssueSeverity::Warning, IssueCategory::UncommittedChanges,
                "Uncommitted changes detected", "Stash or commit before task switch",
            ));
        }

        let has_conflicts = self.worktree_manager.has_conflicts(worktree_path)?;
        if has_conflicts {
            score = 0;
            issues.push(HealthIssue::new(
                IssueSeverity::Critical, IssueCategory::ConflictState,
                "Merge/rebase conflicts detected", "Resolve conflicts before proceeding",
            ));
        }

        Ok(WorkspaceHealthReport::with_issues(score, issues))
    }

    fn sync_baseline(&self) -> Result<String, GitFlowError> {
        self.worktree_manager.fetch_origin()?;
        let base_commit = self.worktree_manager.get_remote_head(&self.config.base_branch)?;
        Ok(base_commit)
    }

    fn handle_uncommitted_stash(&self, worktree_path: &PathBuf) -> Result<(), GitFlowError> {
        let message = format!("WIP: auto-stash before task preparation at {}", Utc::now().format("%Y-%m-%d %H:%M"));
        self.worktree_manager.stash_changes(worktree_path, &message)?;
        Ok(())
    }

    fn setup_branch(
        &self,
        task_meta: &TaskPreparationMeta,
        worktree_path: &PathBuf,
    ) -> Result<(String, BranchAction), GitFlowError> {
        let branch_name = &task_meta.branch_name;
        let branch_exists = self.worktree_manager.branch_exists(branch_name)?;
        
        if branch_exists {
            let current_branch = self.worktree_manager.get_current_branch(worktree_path)?;
            if current_branch == *branch_name {
                Ok((branch_name.clone(), BranchAction::CheckedOutExisting))
            } else {
                self.worktree_manager.checkout_branch(worktree_path, branch_name)?;
                Ok((branch_name.clone(), BranchAction::CheckedOutExisting))
            }
        } else {
            let _head = self.worktree_manager.create_feature_branch(branch_name, &self.config.base_branch)?;
            self.worktree_manager.checkout_branch(worktree_path, branch_name)?;
            Ok((branch_name.clone(), BranchAction::CreatedNew))
        }
    }
}

/// Result of task finalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFinalizationResult {
    pub success: bool,
    pub branch: String,
    pub commits_ahead: usize,
    pub final_commit_message: Option<String>,
    pub operations_log: Vec<GitOperationLog>,
}

// Helper functions

fn classify_task_type(description: &str) -> (TaskType, f64) {
    let desc_lower = description.to_lowercase();
    let mut matches: HashMap<TaskType, usize> = HashMap::new();

    const KEYWORDS: &[(TaskType, &[&str])] = &[
        (TaskType::Feature, &["add", "implement", "create", "new", "introduce"]),
        (TaskType::Bugfix, &["fix", "bug", "issue", "error", "resolve"]),
        (TaskType::Refactor, &["refactor", "simplify", "optimize", "clean"]),
        (TaskType::Docs, &["document", "readme", "doc", "documentation"]),
        (TaskType::Test, &["test", "testing", "spec", "coverage"]),
        (TaskType::Chore, &["chore", "maintenance", "cleanup", "update"]),
        (TaskType::Hotfix, &["hotfix", "urgent", "critical", "emergency"]),
    ];

    for (type_, kws) in KEYWORDS {
        for kw in *kws {
            if desc_lower.contains(kw) {
                *matches.entry(*type_).or_insert(0) += 1;
            }
        }
    }

    let (best_type, best_count) = matches
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(t, c)| (*t, *c))
        .unwrap_or((TaskType::Feature, 0));

    let confidence = if best_count == 0 { 0.5 } else { 0.5 + (best_count as f64 * 0.15).min(0.45) };
    (best_type, confidence)
}

fn generate_summary(description: &str, max_len: usize) -> String {
    let words = description.split_whitespace().take(5).collect::<Vec<_>>();
    let summary = words.join(" ");
    if summary.len() > max_len {
        summary.chars().take(max_len).collect()
    } else {
        summary
    }
}

fn generate_branch_name(task_id: &str, task_type: TaskType, summary: &str, config: &GitFlowConfig) -> String {
    let prefix = config.get_prefix_for_type(task_type.branch_prefix());
    let sanitized = sanitize_branch_description(summary);
    if sanitized.is_empty() {
        format!("{}/{}", prefix, task_id)
    } else {
        format!("{}/{}/{}", prefix, task_id, sanitized)
    }
}

fn sanitize_branch_description(desc: &str) -> String {
    let mut sanitized = String::new();
    let mut prev_hyphen = false;

    for c in desc.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            sanitized.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen && !sanitized.is_empty() {
            sanitized.push('-');
            prev_hyphen = true;
        }
    }

    if sanitized.ends_with('-') { sanitized.pop(); }
    if sanitized.len() > 30 {
        sanitized = sanitized.chars().take(30).collect();
        if sanitized.ends_with('-') { sanitized.pop(); }
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_preparation_meta() {
        let config = GitFlowConfig::default();
        let meta = TaskPreparationMeta::new("PROJ-123", "Add user authentication", &config);
        assert_eq!(meta.task_id, "PROJ-123");
        assert_eq!(meta.task_type, TaskType::Feature);
    }

    #[test]
    fn test_classify_task_type_feature() {
        let (t, c) = classify_task_type("Add new feature");
        assert_eq!(t, TaskType::Feature);
        assert!(c > 0.5);
    }

    #[test]
    fn test_classify_task_type_bugfix() {
        let (t, c) = classify_task_type("Fix the bug");
        assert_eq!(t, TaskType::Bugfix);
        assert!(c > 0.5);
    }

    #[test]
    fn test_classify_task_type_default() {
        let (t, c) = classify_task_type("Random text");
        assert_eq!(t, TaskType::Feature);
        assert_eq!(c, 0.5);
    }

    #[test]
    fn test_sanitize_branch_description() {
        assert_eq!(sanitize_branch_description("Add User Auth"), "add-user-auth");
        assert_eq!(sanitize_branch_description("Fix: bug!"), "fix-bug");
    }

    #[test]
    fn test_generate_branch_name() {
        let config = GitFlowConfig::default();
        let name = generate_branch_name("PROJ-123", TaskType::Feature, "add-auth", &config);
        assert_eq!(name, "feature/PROJ-123/add-auth");
    }

    #[test]
    fn test_generate_branch_name_empty_summary() {
        let config = GitFlowConfig::default();
        // Empty summary should still produce valid branch name
        let name = generate_branch_name("PROJ-123", TaskType::Feature, "", &config);
        assert_eq!(name, "feature/PROJ-123");
    }

    #[test]
    fn test_preparation_result_context_message() {
        let meta = TaskPreparationMeta::new("PROJ-123", "Add auth", &GitFlowConfig::default());
        let result = PreparationResult::success(
            meta, "feature/PROJ-123/add-auth".to_string(),
            "abc123def456".to_string(), PathBuf::from("/tmp"), BranchAction::CreatedNew,
        );
        let msg = result.to_context_message();
        assert!(msg.contains("Git Flow Task Preparation"));
        assert!(msg.contains("PROJ-123"));
    }

    #[test]
    fn test_health_report_healthy() {
        let report = WorkspaceHealthReport::healthy();
        assert_eq!(report.score, 100);
        assert!(report.is_ready_for_task);
    }

    #[test]
    fn test_branch_action_display() {
        assert_eq!(format!("{}", BranchAction::CreatedNew), "Created new branch");
    }
}
