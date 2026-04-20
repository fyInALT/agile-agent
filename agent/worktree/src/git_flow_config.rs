//! Git Flow Configuration for Task Preparation
//!
//! Provides configuration options for Git Flow workflow management,
//! including branch naming conventions, auto-sync settings, and policy options.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Default base branch name
const DEFAULT_BASE_BRANCH: &str = "main";

/// Default branch name pattern
const DEFAULT_BRANCH_PATTERN: &str = "<type>/<task-id>-<desc>";

/// Default stale branch threshold in days
const DEFAULT_STALE_BRANCH_DAYS: u64 = 30;

/// Git Flow configuration for task preparation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFlowConfig {
    /// Default base branch (main or master)
    pub base_branch: String,
    
    /// Branch naming pattern template
    /// Supported placeholders: <type>, <task-id>, <desc>
    pub branch_pattern: String,
    
    /// Automatically sync baseline before task start
    pub auto_sync_baseline: bool,
    
    /// Automatically stash uncommitted changes
    pub auto_stash_changes: bool,
    
    /// Automatically cleanup merged branches
    pub auto_cleanup_merged: bool,
    
    /// Days threshold for stale branch warning
    pub stale_branch_days: u64,
    
    /// Enforce conventional commits format
    pub enforce_conventional_commits: bool,
    
    /// Task type to branch prefix mapping
    pub task_type_prefixes: HashMap<String, String>,
    
    /// Maximum branch description length
    pub max_desc_length: usize,
    
    /// Characters to strip from branch names
    pub invalid_chars: Vec<char>,
}

impl Default for GitFlowConfig {
    fn default() -> Self {
        let mut task_type_prefixes = HashMap::new();
        task_type_prefixes.insert("feature".to_string(), "feature".to_string());
        task_type_prefixes.insert("bugfix".to_string(), "bugfix".to_string());
        task_type_prefixes.insert("refactor".to_string(), "refactor".to_string());
        task_type_prefixes.insert("docs".to_string(), "docs".to_string());
        task_type_prefixes.insert("test".to_string(), "test".to_string());
        task_type_prefixes.insert("chore".to_string(), "chore".to_string());
        task_type_prefixes.insert("hotfix".to_string(), "hotfix".to_string());
        
        Self {
            base_branch: DEFAULT_BASE_BRANCH.to_string(),
            branch_pattern: DEFAULT_BRANCH_PATTERN.to_string(),
            auto_sync_baseline: true,
            auto_stash_changes: true,
            auto_cleanup_merged: false,
            stale_branch_days: DEFAULT_STALE_BRANCH_DAYS,
            enforce_conventional_commits: true,
            task_type_prefixes,
            max_desc_length: 30,
            invalid_chars: vec![' ', '_', '.', '/', '\\', ':', '*', '?', '[', ']', '{', '}'],
        }
    }
}

impl GitFlowConfig {
    /// Create a new GitFlowConfig with defaults
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create config with custom base branch
    pub fn with_base_branch(mut self, branch: impl Into<String>) -> Self {
        self.base_branch = branch.into();
        self
    }
    
    /// Create config with custom branch pattern
    pub fn with_branch_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.branch_pattern = pattern.into();
        self
    }
    
    /// Get the branch prefix for a task type
    pub fn get_prefix_for_type(&self, task_type: &str) -> &str {
        self.task_type_prefixes
            .get(task_type)
            .map(|s| s.as_str())
            .unwrap_or("feature")
    }
    
    /// Load configuration from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), GitFlowConfigError> {
        if self.base_branch.is_empty() {
            return Err(GitFlowConfigError::InvalidBaseBranch(
                "base_branch cannot be empty".to_string()
            ));
        }
        
        if !self.branch_pattern.contains("<type>") {
            return Err(GitFlowConfigError::InvalidBranchPattern(
                "branch_pattern must contain <type> placeholder".to_string()
            ));
        }
        
        if !self.branch_pattern.contains("<task-id>") && !self.branch_pattern.contains("<desc>") {
            return Err(GitFlowConfigError::InvalidBranchPattern(
                "branch_pattern must contain at least <task-id> or <desc> placeholder".to_string()
            ));
        }
        
        if self.max_desc_length < 5 {
            return Err(GitFlowConfigError::InvalidDescLength(
                "max_desc_length must be at least 5".to_string()
            ));
        }
        
        if self.stale_branch_days < 1 {
            return Err(GitFlowConfigError::InvalidStaleDays(
                "stale_branch_days must be at least 1".to_string()
            ));
        }
        
        Ok(())
    }
}

/// Git Flow configuration errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum GitFlowConfigError {
    #[error("invalid base branch: {0}")]
    InvalidBaseBranch(String),
    
    #[error("invalid branch pattern: {0}")]
    InvalidBranchPattern(String),
    
    #[error("invalid description length setting: {0}")]
    InvalidDescLength(String),
    
    #[error("invalid stale days setting: {0}")]
    InvalidStaleDays(String),

    #[error("JSON parse error: {0}")]
    JsonError(String),
}

/// Task type classification for branch naming
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// New feature development
    Feature,
    /// Bug fix
    Bugfix,
    /// Code refactoring
    Refactor,
    /// Documentation changes
    Docs,
    /// Test additions/modifications
    Test,
    /// Maintenance tasks
    Chore,
    /// Critical production fixes
    Hotfix,
}

impl TaskType {
    /// Get the branch prefix for this task type
    pub fn branch_prefix(&self) -> &'static str {
        match self {
            TaskType::Feature => "feature",
            TaskType::Bugfix => "bugfix",
            TaskType::Refactor => "refactor",
            TaskType::Docs => "docs",
            TaskType::Test => "test",
            TaskType::Chore => "chore",
            TaskType::Hotfix => "hotfix",
        }
    }
    
    /// Get display name
    pub fn display(&self) -> &'static str {
        match self {
            TaskType::Feature => "Feature",
            TaskType::Bugfix => "Bugfix",
            TaskType::Refactor => "Refactor",
            TaskType::Docs => "Docs",
            TaskType::Test => "Test",
            TaskType::Chore => "Chore",
            TaskType::Hotfix => "Hotfix",
        }
    }
    
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "feature" | "feat" => Some(TaskType::Feature),
            "bugfix" | "fix" | "bug" => Some(TaskType::Bugfix),
            "refactor" | "ref" => Some(TaskType::Refactor),
            "docs" | "doc" | "documentation" => Some(TaskType::Docs),
            "test" | "tests" | "testing" => Some(TaskType::Test),
            "chore" | "maintenance" => Some(TaskType::Chore),
            "hotfix" | "critical" => Some(TaskType::Hotfix),
            _ => None,
        }
    }
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display())
    }
}

/// Task priority classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    /// Critical priority
    Critical,
    /// High priority
    High,
    /// Medium priority (default)
    Medium,
    /// Low priority
    Low,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Medium
    }
}

impl TaskPriority {
    /// Get display name
    pub fn display(&self) -> &'static str {
        match self {
            TaskPriority::Critical => "Critical",
            TaskPriority::High => "High",
            TaskPriority::Medium => "Medium",
            TaskPriority::Low => "Low",
        }
    }
    
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "critical" | "urgent" | "p0" => Some(TaskPriority::Critical),
            "high" | "important" | "p1" => Some(TaskPriority::High),
            "medium" | "normal" | "p2" => Some(TaskPriority::Medium),
            "low" | "optional" | "p3" => Some(TaskPriority::Low),
            _ => None,
        }
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn git_flow_config_default() {
        let config = GitFlowConfig::default();
        assert_eq!(config.base_branch, "main");
        assert_eq!(config.branch_pattern, "<type>/<task-id>-<desc>");
        assert!(config.auto_sync_baseline);
        assert!(config.auto_stash_changes);
        assert!(!config.auto_cleanup_merged);
        assert_eq!(config.stale_branch_days, 30);
        assert!(config.enforce_conventional_commits);
    }
    
    #[test]
    fn git_flow_config_with_base_branch() {
        let config = GitFlowConfig::new().with_base_branch("master");
        assert_eq!(config.base_branch, "master");
    }
    
    #[test]
    fn git_flow_config_validation() {
        let config = GitFlowConfig::default();
        assert!(config.validate().is_ok());
        
        let invalid_config = GitFlowConfig {
            base_branch: "".to_string(),
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn git_flow_config_json_serialization() {
        let config = GitFlowConfig::default();
        let json = config.to_json().unwrap();
        assert!(json.contains("base_branch"));
        
        let parsed = GitFlowConfig::from_json(&json).unwrap();
        assert_eq!(parsed.base_branch, config.base_branch);
    }
    
    #[test]
    fn task_type_branch_prefix() {
        assert_eq!(TaskType::Feature.branch_prefix(), "feature");
        assert_eq!(TaskType::Bugfix.branch_prefix(), "bugfix");
        assert_eq!(TaskType::Hotfix.branch_prefix(), "hotfix");
    }
    
    #[test]
    fn task_type_from_str() {
        assert_eq!(TaskType::from_str("feature"), Some(TaskType::Feature));
        assert_eq!(TaskType::from_str("feat"), Some(TaskType::Feature));
        assert_eq!(TaskType::from_str("fix"), Some(TaskType::Bugfix));
        assert_eq!(TaskType::from_str("unknown"), None);
    }
    
    #[test]
    fn task_priority_from_str() {
        assert_eq!(TaskPriority::from_str("critical"), Some(TaskPriority::Critical));
        assert_eq!(TaskPriority::from_str("p0"), Some(TaskPriority::Critical));
        assert_eq!(TaskPriority::from_str("unknown"), None);
    }
    
    #[test]
    fn get_prefix_for_type() {
        let config = GitFlowConfig::default();
        assert_eq!(config.get_prefix_for_type("feature"), "feature");
        assert_eq!(config.get_prefix_for_type("unknown"), "feature"); // default
    }
}
