//! Task Completion Git Workflow
//!
//! Handles git workflow when task completes:
//! - Final commit verification
//! - PR preparation
//! - Branch cleanup

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Git state at task completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletionGitState {
    /// Current branch name
    pub current_branch: String,
    /// Whether there are uncommitted changes
    pub has_uncommitted: bool,
    /// Files with uncommitted changes
    pub uncommitted_files: Vec<CompletionCandidateFile>,
    /// Number of commits ahead of base
    pub commits_ahead: usize,
    /// Number of commits behind base
    pub commits_behind: usize,
    /// Whether branch is ready for merge
    pub ready_for_merge: bool,
    /// Suggested PR title
    pub suggested_pr_title: Option<String>,
    /// Suggested PR description
    pub suggested_pr_description: Option<String>,
}

/// File at task completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCandidateFile {
    /// File path
    pub path: String,
    /// Change type
    pub change_type: String,
}

/// Commit made during task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCommitInfo {
    /// Commit SHA (short)
    pub sha: String,
    /// Commit message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Files changed count
    pub files_changed: usize,
}

/// Final commit verification result
#[derive(Debug, Clone)]
pub enum FinalCommitResult {
    /// All changes verified and committed
    Verified {
        commit_count: usize,
    },
    /// Needs final commit
    NeedsCommit {
        files: Vec<CompletionCandidateFile>,
        suggested_message: String,
    },
}

/// Final commit verifier
pub struct FinalCommitVerifier;

impl FinalCommitVerifier {
    /// Verify task completion state
    pub fn verify(
        &self,
        has_uncommitted: bool,
        uncommitted_files: &[CompletionCandidateFile],
    ) -> FinalCommitResult {
        if has_uncommitted {
            let suggested = self.generate_final_commit_message(uncommitted_files);
            FinalCommitResult::NeedsCommit {
                files: uncommitted_files.to_vec(),
                suggested_message: suggested,
            }
        } else {
            FinalCommitResult::Verified { commit_count: 0 }
        }
    }

    /// Generate final commit message
    fn generate_final_commit_message(&self, files: &[CompletionCandidateFile]) -> String {
        if files.is_empty() {
            "chore: final cleanup for task completion".to_string()
        } else {
            format!(
                "chore: finalize changes ({} files)",
                files.len()
            )
        }
    }
}

impl Default for FinalCommitVerifier {
    fn default() -> Self {
        Self
    }
}

/// PR title generator
pub struct PrTitleGenerator;

impl PrTitleGenerator {
    /// Generate PR title from commits and task info
    pub fn generate(
        commits: &[TaskCommitInfo],
        task_id: &str,
        task_summary: &str,
        task_type: &str,
    ) -> String {
        // If single commit, use its message
        if commits.len() == 1 {
            let msg = &commits[0].message;
            // Extract meaningful part
            if let Some(colon_pos) = msg.find(':') {
                let prefix = &msg[..colon_pos];
                let body = msg[colon_pos + 1..].trim();
                if !body.is_empty() {
                    return format!("{}({}): {}", prefix, task_id, body);
                }
            }
            return format!("{}({}): {}", Self::to_prefix(task_type), task_id, msg);
        }

        // Multiple commits - use task info
        let prefix = Self::to_prefix(task_type);
        let summary = if task_summary.len() > 50 {
            format!("{}...", &task_summary[..47])
        } else {
            task_summary.to_string()
        };

        format!("{}({}): {}", prefix, task_id, summary)
    }

    /// Convert task type to commit prefix
    fn to_prefix(task_type: &str) -> &'static str {
        match task_type.to_lowercase().as_str() {
            "feature" => "feat",
            "bugfix" => "fix",
            "refactor" => "refactor",
            "docs" => "docs",
            "test" => "test",
            "chore" => "chore",
            "hotfix" => "fix",
            _ => "feat",
        }
    }

    /// Detect dominant commit type
    #[allow(dead_code)]
    fn detect_dominant_type(&self, commits: &[TaskCommitInfo]) -> &'static str {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

        for commit in commits {
            let msg_lower = commit.message.to_lowercase();
            if msg_lower.starts_with("feat") {
                *counts.entry("feat").or_insert(0) += 1;
            } else if msg_lower.starts_with("fix") {
                *counts.entry("fix").or_insert(0) += 1;
            } else if msg_lower.starts_with("refactor") {
                *counts.entry("refactor").or_insert(0) += 1;
            } else if msg_lower.starts_with("test") {
                *counts.entry("test").or_insert(0) += 1;
            } else if msg_lower.starts_with("docs") {
                *counts.entry("docs").or_insert(0) += 1;
            } else {
                *counts.entry("chore").or_insert(0) += 1;
            }
        }

        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(t, _)| t)
            .unwrap_or("feat")
    }
}

impl Default for PrTitleGenerator {
    fn default() -> Self {
        Self
    }
}

/// PR description generator
pub struct PrDescriptionGenerator;

impl PrDescriptionGenerator {
    /// Generate PR description from commits and task info
    pub fn generate(
        commits: &[TaskCommitInfo],
        task_id: &str,
        task_summary: &str,
        task_type: &str,
    ) -> String {
        let mut description = String::new();

        // Summary section
        description.push_str("## Summary\n\n");
        description.push_str(task_summary);
        description.push_str(&format!("\n\nTask ID: {}\n", task_id));
        description.push_str(&format!("Type: {}\n\n", task_type));

        // Changes section
        description.push_str("## Changes\n\n");
        if commits.is_empty() {
            description.push_str("- No commits yet\n");
        } else {
            for commit in commits {
                description.push_str(&format!("- {} ({})\n", commit.message, commit.sha));
            }
        }
        description.push('\n');

        // Test plan section
        description.push_str("## Test Plan\n\n");
        description.push_str("- [ ] Tests added/updated\n");
        description.push_str("- [ ] Manual testing completed\n");
        description.push_str("- [ ] Code reviewed\n\n");

        // AI attribution
        description.push_str("---\n");
        description.push_str("*Generated with Claude Code*\n");

        description
    }
}

impl Default for PrDescriptionGenerator {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_final_commit_verifier_clean() {
        let verifier = FinalCommitVerifier::default();
        let result = verifier.verify(false, &[]);

        assert!(matches!(result, FinalCommitResult::Verified { .. }));
    }

    #[test]
    fn test_final_commit_verifier_dirty() {
        let verifier = FinalCommitVerifier::default();
        let files = vec![CompletionCandidateFile {
            path: "src/main.rs".to_string(),
            change_type: "Modified".to_string(),
        }];
        let result = verifier.verify(true, &files);

        assert!(matches!(result, FinalCommitResult::NeedsCommit { .. }));
        if let FinalCommitResult::NeedsCommit { suggested_message, .. } = result {
            assert!(suggested_message.contains("finalize"));
        }
    }

    #[test]
    fn test_pr_title_generator_single_commit() {
        let commits = vec![TaskCommitInfo {
            sha: "abc123".to_string(),
            message: "feat(auth): add login".to_string(),
            timestamp: Utc::now(),
            files_changed: 3,
        }];

        let title = PrTitleGenerator::generate(&commits, "PROJ-123", "Add login feature", "feature");
        assert!(title.contains("PROJ-123"));
        assert!(title.contains("login") || title.contains("feat"));
    }

    #[test]
    fn test_pr_title_generator_multiple_commits() {
        let commits = vec![
            TaskCommitInfo {
                sha: "abc123".to_string(),
                message: "feat: add auth".to_string(),
                timestamp: Utc::now(),
                files_changed: 3,
            },
            TaskCommitInfo {
                sha: "def456".to_string(),
                message: "test: add auth tests".to_string(),
                timestamp: Utc::now(),
                files_changed: 2,
            },
        ];

        let title = PrTitleGenerator::generate(&commits, "PROJ-123", "Add authentication", "feature");
        assert!(title.contains("PROJ-123"));
        assert!(title.contains("Add authentication"));
    }

    #[test]
    fn test_pr_description_generator() {
        let commits = vec![TaskCommitInfo {
            sha: "abc123".to_string(),
            message: "feat: add login".to_string(),
            timestamp: Utc::now(),
            files_changed: 3,
        }];

        let desc = PrDescriptionGenerator::generate(&commits, "PROJ-123", "Add login feature", "feature");
        assert!(desc.contains("## Summary"));
        assert!(desc.contains("PROJ-123"));
        assert!(desc.contains("## Changes"));
        assert!(desc.contains("## Test Plan"));
        assert!(desc.contains("add login"));
    }

    #[test]
    fn test_pr_description_generator_empty_commits() {
        let desc = PrDescriptionGenerator::generate(&[], "PROJ-123", "Add feature", "feature");
        assert!(desc.contains("No commits yet"));
    }

    #[test]
    fn test_task_commit_info() {
        let commit = TaskCommitInfo {
            sha: "abc123".to_string(),
            message: "feat: test".to_string(),
            timestamp: Utc::now(),
            files_changed: 5,
        };
        assert_eq!(commit.sha, "abc123");
        assert_eq!(commit.files_changed, 5);
    }
}
