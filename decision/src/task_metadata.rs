//! Task Metadata for Git Flow Task Preparation
//!
//! Provides task metadata extraction and branch name generation
//! following Git Flow conventions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task metadata extracted from task description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Task identifier (e.g., PROJ-123, agent_001_task)
    pub task_id: String,
    
    /// Generated branch name following Git Flow convention
    pub branch_name: String,
    
    /// Short summary of the task (for branch description)
    pub summary: String,
    
    /// Original task description
    pub original_description: String,
    
    /// Classified task type
    pub task_type: TaskType,
    
    /// Task priority
    pub priority: TaskPriority,
    
    /// Confidence score for classification (0.0-1.0)
    pub classification_confidence: f64,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl TaskMetadata {
    /// Create new task metadata from task ID and description
    pub fn new(task_id: impl Into<String>, description: impl Into<String>) -> Self {
        let task_id = task_id.into();
        let original_description = description.into();
        
        // Classify task type from description
        let (task_type, confidence) = classify_task_type(&original_description);
        
        // Generate summary from description
        let summary = generate_summary(&original_description);
        
        // Generate branch name
        let branch_name = generate_branch_name(&task_id, task_type, &summary);
        
        Self {
            task_id,
            branch_name,
            summary,
            original_description,
            task_type,
            priority: TaskPriority::default(),
            classification_confidence: confidence,
            created_at: Utc::now(),
        }
    }
    
    /// Create with explicit task type
    pub fn with_type(mut self, task_type: TaskType) -> Self {
        self.task_type = task_type;
        self.branch_name = generate_branch_name(&self.task_id, task_type, &self.summary);
        self
    }
    
    /// Create with explicit priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
    
    /// Create with custom branch name (override generated)
    pub fn with_branch_name(mut self, branch_name: impl Into<String>) -> Self {
        self.branch_name = branch_name.into();
        self
    }
}

/// Task type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Feature,
    Bugfix,
    Refactor,
    Docs,
    Test,
    Chore,
    Hotfix,
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::Feature
    }
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
    Critical,
    High,
    Medium,
    Low,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Medium
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Critical => f.write_str("Critical"),
            TaskPriority::High => f.write_str("High"),
            TaskPriority::Medium => f.write_str("Medium"),
            TaskPriority::Low => f.write_str("Low"),
        }
    }
}

/// Classification keywords for task type inference
const FEATURE_KEYWORDS: &[&str] = &["add", "implement", "create", "new", "introduce", "develop"];
const BUGFIX_KEYWORDS: &[&str] = &["fix", "bug", "issue", "error", "resolve", "patch", "solve"];
const REFACTOR_KEYWORDS: &[&str] = &["refactor", "simplify", "optimize", "clean", "improve", "restructure"];
const DOCS_KEYWORDS: &[&str] = &["document", "readme", "doc", "update docs", "documentation", "write"];
const TEST_KEYWORDS: &[&str] = &["test", "testing", "spec", "coverage", "verify", "unit test"];
const CHORE_KEYWORDS: &[&str] = &["chore", "maintenance", "cleanup", "update", "bump", "upgrade"];
const HOTFIX_KEYWORDS: &[&str] = &["hotfix", "urgent", "critical", "emergency", "production"];

/// Classify task type from description keywords
///
/// Returns the classified type and confidence score (0.0-1.0)
pub fn classify_task_type(description: &str) -> (TaskType, f64) {
    let desc_lower = description.to_lowercase();
    
    // Count keyword matches for each type
    let mut matches: HashMap<TaskType, usize> = HashMap::new();
    
    for keyword in FEATURE_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Feature).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in BUGFIX_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Bugfix).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in REFACTOR_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Refactor).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in DOCS_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Docs).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in TEST_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Test).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in CHORE_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Chore).unwrap_or(&mut 0) += 1;
        }
    }
    
    for keyword in HOTFIX_KEYWORDS {
        if desc_lower.contains(keyword) {
            *matches.get_mut(&TaskType::Hotfix).unwrap_or(&mut 0) += 1;
        }
    }
    
    // Find the type with most matches
    let (best_type, best_count) = matches
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(t, c)| (*t, *c))
        .unwrap_or((TaskType::Feature, 0));
    
    // Calculate confidence: 0.5 for single match, increases with more matches
    let confidence = if best_count == 0 {
        0.5 // Default confidence when no keywords found
    } else {
        0.5 + (best_count as f64 * 0.15).min(0.45)
    };
    
    (best_type, confidence)
}

/// Generate short summary from description
///
/// Limits to approximately 30 characters for branch naming
pub fn generate_summary(description: &str) -> String {
    // Take first sentence or first N words
    let words = description
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>();
    
    let summary = words.join(" ");
    
    // Truncate to max length
    if summary.len() > 30 {
        summary.chars().take(30).collect()
    } else {
        summary
    }
}

/// Generate branch name following Git Flow convention
///
/// Format: `<type>/<task-id>-<short-description>`
pub fn generate_branch_name(task_id: &str, task_type: TaskType, summary: &str) -> String {
    let prefix = task_type.branch_prefix();
    
    // Sanitize summary for branch name
    let sanitized_desc = sanitize_branch_description(summary);
    
    // Build branch name
    format!("{}/{}/{}", prefix, task_id, sanitized_desc)
}

/// Sanitize description for valid Git branch name
///
/// Rules:
/// - Lowercase alphanumeric and hyphens only
/// - No consecutive hyphens
/// - No leading/trailing hyphens
/// - Max ~30 characters
pub fn sanitize_branch_description(desc: &str) -> String {
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
    
    // Remove trailing hyphen
    if sanitized.ends_with('-') {
        sanitized.pop();
    }
    
    // Limit length
    if sanitized.len() > 30 {
        // Find a good break point (at a hyphen if possible)
        let truncate_at = sanitized
            .char_indices()
            .take(30)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(30);
        
        sanitized = sanitized.chars().take(truncate_at).collect();
        
        // Remove trailing hyphen after truncation
        if sanitized.ends_with('-') {
            sanitized.pop();
        }
    }
    
    sanitized
}

/// Validate branch name follows Git Flow convention
pub fn validate_branch_name(branch_name: &str) -> Result<(), BranchNameError> {
    // Check format: <type>/<task-id>-<desc>
    let parts = branch_name.split('/').collect::<Vec<_>>();
    
    if parts.len() != 3 {
        return Err(BranchNameError::InvalidFormat(
            "Branch name must be in format: <type>/<task-id>-<desc>".to_string()
        ));
    }
    
    // Validate type prefix
    let valid_types = ["feature", "bugfix", "refactor", "docs", "test", "chore", "hotfix", "agent"];
    if !valid_types.contains(&parts[0]) {
        return Err(BranchNameError::InvalidType(
            format!("Invalid type prefix: {}", parts[0])
        ));
    }
    
    // Validate task-id (should not be empty)
    if parts[1].is_empty() {
        return Err(BranchNameError::EmptyTaskId);
    }
    
    // Validate description (should not be empty)
    if parts[2].is_empty() {
        return Err(BranchNameError::EmptyDescription);
    }
    
    // Check for invalid characters
    for c in branch_name.chars() {
        if !c.is_ascii_alphanumeric() && c != '/' && c != '-' && c != '_' {
            return Err(BranchNameError::InvalidCharacter(c));
        }
    }
    
    Ok(())
}

/// Branch name validation errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum BranchNameError {
    #[error("invalid branch name format: {0}")]
    InvalidFormat(String),
    
    #[error("invalid type prefix: {0}")]
    InvalidType(String),
    
    #[error("task ID is empty")]
    EmptyTaskId,
    
    #[error("description is empty")]
    EmptyDescription,
    
    #[error("invalid character in branch name: '{0}'")]
    InvalidCharacter(char),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn task_metadata_new() {
        let metadata = TaskMetadata::new("PROJ-123", "Add user authentication feature");
        
        assert_eq!(metadata.task_id, "PROJ-123");
        assert_eq!(metadata.task_type, TaskType::Feature);
        assert!(metadata.branch_name.starts_with("feature/"));
        assert!(metadata.classification_confidence > 0.5);
    }
    
    #[test]
    fn classify_task_type_feature() {
        let (type_, conf) = classify_task_type("Add new feature for user login");
        assert_eq!(type_, TaskType::Feature);
        assert!(conf > 0.5);
    }
    
    #[test]
    fn classify_task_type_bugfix() {
        let (type_, conf) = classify_task_type("Fix the login timeout bug");
        assert_eq!(type_, TaskType::Bugfix);
        assert!(conf > 0.5);
    }
    
    #[test]
    fn classify_task_type_default() {
        let (type_, conf) = classify_task_type("Some random task");
        assert_eq!(type_, TaskType::Feature); // default
        assert_eq!(conf, 0.5); // default confidence
    }
    
    #[test]
    fn generate_summary_short() {
        let summary = generate_summary("Add user authentication");
        assert_eq!(summary, "Add user authentication");
    }
    
    #[test]
    fn generate_summary_long() {
        let summary = generate_summary("This is a very long description that should be truncated");
        assert!(summary.len() <= 30);
    }
    
    #[test]
    fn sanitize_branch_description() {
        let sanitized = sanitize_branch_description("Add User Authentication");
        assert_eq!(sanitized, "add-user-authentication");
    }
    
    #[test]
    fn sanitize_branch_description_special_chars() {
        let sanitized = sanitize_branch_description("Fix: login timeout issue!");
        assert_eq!(sanitized, "fix-login-timeout-issue");
    }
    
    #[test]
    fn sanitize_branch_description_consecutive_hyphens() {
        let sanitized = sanitize_branch_description("Add   multiple   spaces");
        assert!(!sanitized.contains("--"));
    }
    
    #[test]
    fn generate_branch_name() {
        let branch = generate_branch_name("PROJ-123", TaskType::Feature, "add auth");
        assert_eq!(branch, "feature/PROJ-123/add-auth");
    }
    
    #[test]
    fn validate_branch_name_valid() {
        assert!(validate_branch_name("feature/PROJ-123/add-auth").is_ok());
        assert!(validate_branch_name("bugfix/issue-456/fix-timeout").is_ok());
        assert!(validate_branch_name("agent/agent_001").is_ok());
    }
    
    #[test]
    fn validate_branch_name_invalid_format() {
        assert!(validate_branch_name("invalid-branch").is_err());
        assert!(validate_branch_name("feature/only-two-parts").is_err());
    }
    
    #[test]
    fn validate_branch_name_invalid_type() {
        assert!(validate_branch_name("unknown/PROJ-123/test").is_err());
    }
    
    #[test]
    fn task_metadata_with_type() {
        let metadata = TaskMetadata::new("PROJ-123", "Add user authentication")
            .with_type(TaskType::Bugfix);
        
        assert_eq!(metadata.task_type, TaskType::Bugfix);
        assert!(metadata.branch_name.starts_with("bugfix/"));
    }
}
