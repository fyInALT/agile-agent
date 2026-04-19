//! Commit Boundary Detection
//!
//! Detects good points to suggest commits based on agent activity patterns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Minimum interval between commit suggestions (5 minutes)
const DEFAULT_SUGGESTION_INTERVAL_SECS: u64 = 300;

/// Default minimum changes threshold
const DEFAULT_MIN_CHANGES_THRESHOLD: usize = 2;

/// Default batch window for activity
const DEFAULT_BATCH_WINDOW_SECS: u64 = 60;

/// Commit boundary signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitBoundarySignal {
    /// Files that have changed
    pub files: Vec<CommitCandidateFile>,
    /// Reason for the boundary
    pub reason: BoundaryReason,
    /// Suggested commit message
    pub suggested_message: String,
}

/// File ready for commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitCandidateFile {
    /// File path
    pub path: String,
    /// Change type
    pub change_type: String,
}

/// Reason for boundary detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryReason {
    /// Agent is at a natural pause
    NaturalPause,
    /// Tests just passed
    TestPassed,
    /// Feature implementation detected
    FeatureComplete,
    /// Multiple files changed
    MultipleFilesChanged,
    /// User explicitly requested
    UserRequested,
}

impl std::fmt::Display for BoundaryReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoundaryReason::NaturalPause => write!(f, "NaturalPause"),
            BoundaryReason::TestPassed => write!(f, "TestPassed"),
            BoundaryReason::FeatureComplete => write!(f, "FeatureComplete"),
            BoundaryReason::MultipleFilesChanged => write!(f, "MultipleFilesChanged"),
            BoundaryReason::UserRequested => write!(f, "UserRequested"),
        }
    }
}

/// Agent activity snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActivitySnapshot {
    /// Number of uncommitted files
    pub uncommitted_file_count: usize,
    /// Whether agent just finished an action
    pub is_paused: bool,
    /// Last action performed
    pub last_action: String,
    /// Whether tests just passed
    pub tests_just_passed: bool,
    /// Whether a feature seems complete
    pub feature_seems_complete: bool,
    /// Timestamp of last activity
    pub last_activity: DateTime<Utc>,
}

impl AgentActivitySnapshot {
    /// Check if there's enough to commit
    pub fn has_meaningful_changes(&self) -> bool {
        self.uncommitted_file_count >= 2
    }

    /// Check if at a good commit boundary
    pub fn is_at_boundary(&self) -> bool {
        self.is_paused
            || self.tests_just_passed
            || self.feature_seems_complete
            || self.uncommitted_file_count >= 5
    }
}

/// Commit boundary detector
#[derive(Debug, Clone)]
pub struct CommitBoundaryDetector {
    /// Minimum changes threshold for suggesting commit
    min_changes_threshold: usize,
    /// Time window to batch changes (avoid noisy prompts)
    #[allow(dead_code)]
    batch_window_secs: u64,
    /// Last suggestion timestamp (to avoid spam)
    last_suggestion: Option<DateTime<Utc>>,
    /// Minimum interval between suggestions
    min_suggestion_interval_secs: u64,
}

impl CommitBoundaryDetector {
    /// Create a new detector with custom settings
    pub fn new(
        min_changes_threshold: usize,
        min_suggestion_interval_secs: u64,
    ) -> Self {
        Self {
            min_changes_threshold,
            batch_window_secs: DEFAULT_BATCH_WINDOW_SECS,
            last_suggestion: None,
            min_suggestion_interval_secs,
        }
    }

    /// Check if current state is a good commit boundary
    pub fn check_boundary(&mut self, activity: &AgentActivitySnapshot) -> Option<CommitBoundarySignal> {
        // Skip if too soon since last suggestion
        if self.too_soon_for_suggestion() {
            return None;
        }

        // Check for commit-worthy changes
        if activity.uncommitted_file_count >= self.min_changes_threshold {
            // Check if agent appears to be at a natural pause
            if activity.is_at_boundary() {
                return Some(self.create_signal(activity));
            }
        }

        None
    }

    /// Check if suggestion is too soon
    fn too_soon_for_suggestion(&self) -> bool {
        if let Some(last) = self.last_suggestion {
            let elapsed = Utc::now().signed_duration_since(last);
            elapsed.num_seconds() < self.min_suggestion_interval_secs as i64
        } else {
            false
        }
    }

    /// Record that a suggestion was made
    pub fn record_suggestion(&mut self) {
        self.last_suggestion = Some(Utc::now());
    }

    /// Create a boundary signal
    fn create_signal(&self, activity: &AgentActivitySnapshot) -> CommitBoundarySignal {
        let reason = self.determine_reason(activity);
        let message = self.generate_suggestion(activity, &reason);

        CommitBoundarySignal {
            files: Vec::new(), // Would be populated from actual git state
            reason,
            suggested_message: message,
        }
    }

    /// Determine the boundary reason
    fn determine_reason(&self, activity: &AgentActivitySnapshot) -> BoundaryReason {
        if activity.tests_just_passed {
            BoundaryReason::TestPassed
        } else if activity.feature_seems_complete {
            BoundaryReason::FeatureComplete
        } else if activity.uncommitted_file_count >= 5 {
            BoundaryReason::MultipleFilesChanged
        } else if activity.is_paused {
            BoundaryReason::NaturalPause
        } else {
            BoundaryReason::MultipleFilesChanged
        }
    }

    /// Generate suggestion message
    fn generate_suggestion(&self, activity: &AgentActivitySnapshot, reason: &BoundaryReason) -> String {
        match reason {
            BoundaryReason::TestPassed => {
                "Tests passed - good point to commit".to_string()
            }
            BoundaryReason::FeatureComplete => {
                "Feature implementation detected - consider committing".to_string()
            }
            BoundaryReason::NaturalPause => {
                "Good commit boundary - consider committing".to_string()
            }
            BoundaryReason::MultipleFilesChanged => {
                format!(
                    "{} files changed - consider committing",
                    activity.uncommitted_file_count
                )
            }
            BoundaryReason::UserRequested => {
                "User requested commit reminder".to_string()
            }
        }
    }
}

impl Default for CommitBoundaryDetector {
    fn default() -> Self {
        Self::new(DEFAULT_MIN_CHANGES_THRESHOLD, DEFAULT_SUGGESTION_INTERVAL_SECS)
    }
}

/// Pre-commit validation
#[derive(Debug, Clone)]
pub struct PreCommitValidator {
    /// Sensitive file patterns
    sensitive_patterns: Vec<String>,
    /// Maximum recommended commit size (files)
    max_commit_size: usize,
}

impl PreCommitValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            sensitive_patterns: vec![
                ".env".to_string(),
                "credentials".to_string(),
                "secrets".to_string(),
                "*.pem".to_string(),
                "*.key".to_string(),
                "password".to_string(),
            ],
            max_commit_size: 20,
        }
    }

    /// Validate files and message
    pub fn validate(
        &self,
        files: &[CommitCandidateFile],
        message: &str,
    ) -> PreCommitValidationResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Check sensitive files
        for file in files {
            if self.is_sensitive(&file.path) {
                warnings.push(format!(
                    "Warning: '{}' may contain sensitive data. Review before committing.",
                    file.path
                ));
            }
        }

        // Check commit size
        if files.len() > self.max_commit_size {
            warnings.push(format!(
                "Large commit ({} files). Consider splitting into multiple commits.",
                files.len()
            ));
        }

        // Validate message format
        if !self.is_valid_message_format(message) {
            errors.push(
                "Commit message should follow conventional format: type(scope): description"
                    .to_string(),
            );
        }

        PreCommitValidationResult {
            valid: errors.is_empty(),
            warnings,
            errors,
        }
    }

    /// Check if file path is sensitive
    fn is_sensitive(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        self.sensitive_patterns
            .iter()
            .any(|pattern| path_lower.contains(&pattern.to_lowercase()))
    }

    /// Check if message follows conventional commit format
    fn is_valid_message_format(&self, message: &str) -> bool {
        // Simple check for conventional commits format
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Check for type prefix
        let valid_prefixes = [
            "feat", "fix", "refactor", "docs", "test", "chore", "hotfix", "perf", "ci", "build",
        ];

        for prefix in valid_prefixes {
            if trimmed.starts_with(prefix) && trimmed.len() > prefix.len() {
                let after_prefix = &trimmed[prefix.len()..];
                if after_prefix.starts_with(':') || after_prefix.starts_with('(') {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for PreCommitValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of pre-commit validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCommitValidationResult {
    /// Whether validation passed
    pub valid: bool,
    /// Warnings (non-blocking)
    pub warnings: Vec<String>,
    /// Errors (blocking)
    pub errors: Vec<String>,
}

impl PreCommitValidationResult {
    /// Check if there are any issues
    pub fn has_issues(&self) -> bool {
        !self.warnings.is_empty() || !self.errors.is_empty()
    }

    /// Get formatted summary
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if !self.errors.is_empty() {
            parts.push(format!("Errors: {}", self.errors.join("; ")));
        }

        if !self.warnings.is_empty() {
            parts.push(format!("Warnings: {}", self.warnings.join("; ")));
        }

        if parts.is_empty() {
            "Validation passed".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_detector_default() {
        let detector = CommitBoundaryDetector::default();
        assert_eq!(detector.min_changes_threshold, DEFAULT_MIN_CHANGES_THRESHOLD);
    }

    #[test]
    fn test_boundary_reason_display() {
        assert_eq!(format!("{}", BoundaryReason::TestPassed), "TestPassed");
        assert_eq!(
            format!("{}", BoundaryReason::FeatureComplete),
            "FeatureComplete"
        );
    }

    #[test]
    fn test_activity_snapshot_has_meaningful_changes() {
        let activity = AgentActivitySnapshot {
            uncommitted_file_count: 5,
            is_paused: false,
            last_action: String::new(),
            tests_just_passed: false,
            feature_seems_complete: false,
            last_activity: Utc::now(),
        };
        assert!(activity.has_meaningful_changes());
    }

    #[test]
    fn test_activity_snapshot_is_at_boundary() {
        let activity = AgentActivitySnapshot {
            uncommitted_file_count: 5,
            is_paused: true,
            last_action: String::new(),
            tests_just_passed: false,
            feature_seems_complete: false,
            last_activity: Utc::now(),
        };
        assert!(activity.is_at_boundary());
    }

    #[test]
    fn test_pre_commit_validator_sensitive() {
        let validator = PreCommitValidator::default();
        assert!(validator.is_sensitive(".env"));
        assert!(validator.is_sensitive("credentials.json"));
        assert!(validator.is_sensitive("secrets.toml"));
        assert!(validator.is_sensitive("passwords.txt"));
        assert!(!validator.is_sensitive("main.rs"));
    }

    #[test]
    fn test_pre_commit_validator_valid_message() {
        let validator = PreCommitValidator::default();
        assert!(validator.is_valid_message_format("feat: add login"));
        assert!(validator.is_valid_message_format("fix(auth): resolve timeout"));
        assert!(validator.is_valid_message_format("refactor: improve performance"));
        assert!(!validator.is_valid_message_format("added some stuff"));
        assert!(!validator.is_valid_message_format(""));
    }

    #[test]
    fn test_pre_commit_validation_result_valid() {
        let result = PreCommitValidationResult {
            valid: true,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        assert!(!result.has_issues());
        assert_eq!(result.summary(), "Validation passed");
    }

    #[test]
    fn test_pre_commit_validation_result_with_warnings() {
        let result = PreCommitValidationResult {
            valid: true,
            warnings: vec!["Large commit".to_string()],
            errors: Vec::new(),
        };
        assert!(result.has_issues());
        assert!(result.summary().contains("Warnings"));
    }

    #[test]
    fn test_pre_commit_validation_result_with_errors() {
        let result = PreCommitValidationResult {
            valid: false,
            warnings: Vec::new(),
            errors: vec!["Invalid format".to_string()],
        };
        assert!(result.has_issues());
        assert!(result.summary().contains("Errors"));
    }
}
