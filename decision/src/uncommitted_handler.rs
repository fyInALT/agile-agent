//! Uncommitted Changes Analysis and Handling
//!
//! Analyzes uncommitted changes to determine appropriate action:
//! commit, stash, discard, or request human decision.

use crate::git_state::{FileChangeType, GitState};
use serde::{Deserialize, Serialize};

/// Analysis result for uncommitted changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncommittedAnalysis {
    /// Context classification of the changes
    pub changes_context: ChangesContext,
    /// Whether changes appear valuable
    pub is_valuable: bool,
    /// Suggested action to take
    pub suggested_action: UncommittedAction,
    /// Reason for the suggestion
    pub reason: String,
}

impl UncommittedAnalysis {
    /// Create a new analysis result
    pub fn new(
        changes_context: ChangesContext,
        is_valuable: bool,
        suggested_action: UncommittedAction,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            changes_context,
            is_valuable,
            suggested_action,
            reason: reason.into(),
        }
    }

    /// Get a summary for display
    pub fn summary(&self) -> String {
        format!(
            "{} changes - {} - {}",
            self.changes_context,
            if self.is_valuable { "valuable" } else { "low value" },
            self.suggested_action
        )
    }
}

/// Context classification for changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangesContext {
    /// Related to current assigned task
    CurrentTask,
    /// Related to previous completed/aborted task
    PreviousTask,
    /// Unknown or experimental changes
    Unknown,
    /// Temporary/debugging changes
    Temporary,
}

impl std::fmt::Display for ChangesContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangesContext::CurrentTask => write!(f, "CurrentTask"),
            ChangesContext::PreviousTask => write!(f, "PreviousTask"),
            ChangesContext::Unknown => write!(f, "Unknown"),
            ChangesContext::Temporary => write!(f, "Temporary"),
        }
    }
}

/// Action to take for uncommitted changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UncommittedAction {
    /// Commit with task-related message
    Commit,
    /// Stash with description
    Stash,
    /// Discard changes
    Discard,
    /// Request human decision
    RequestHuman,
}

impl std::fmt::Display for UncommittedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UncommittedAction::Commit => write!(f, "Commit"),
            UncommittedAction::Stash => write!(f, "Stash"),
            UncommittedAction::Discard => write!(f, "Discard"),
            UncommittedAction::RequestHuman => write!(f, "RequestHuman"),
        }
    }
}

/// Patterns that indicate temporary or low-value changes
const TEMPORARY_PATTERNS: &[&str] = &[
    ".tmp",
    ".temp",
    ".bak",
    ".backup",
    "~",
    "DEBUG:",
    "XXX:",
    "FIXME:",
    "TODO:debug",
    "console.log",
    "print!(",
    "dbg!(",
];

/// Uncommitted changes analyzer
pub struct UncommittedAnalyzer;

impl UncommittedAnalyzer {
    /// Analyze uncommitted changes and determine appropriate action
    pub fn analyze(git_state: &GitState, current_task_id: Option<&str>) -> UncommittedAnalysis {
        if git_state.uncommitted_files.is_empty() {
            return UncommittedAnalysis::new(
                ChangesContext::Unknown,
                false,
                UncommittedAction::Commit,
                "No uncommitted changes",
            );
        }

        let changes_context = Self::classify_context(git_state, current_task_id);
        let is_valuable = Self::assess_value(git_state);
        let suggested_action = Self::determine_action(changes_context, is_valuable);
        let reason = Self::explain_reason(changes_context, is_valuable, &git_state.uncommitted_files);

        UncommittedAnalysis {
            changes_context,
            is_valuable,
            suggested_action,
            reason,
        }
    }

    /// Classify the context of changes
    fn classify_context(git_state: &GitState, _task_id: Option<&str>) -> ChangesContext {
        let files = &git_state.uncommitted_files;

        // Check for temporary patterns
        if files.iter().any(|f| Self::is_temporary_file(&f.path)) {
            return ChangesContext::Temporary;
        }

        // Check if all files are new/untracked (potentially experimental)
        if files.iter().all(|f| f.change_type == FileChangeType::Untracked) {
            return ChangesContext::Unknown;
        }

        // Check for significant file types that indicate real work
        let has_meaningful_files = files.iter().any(|f| {
            let path = f.path.to_lowercase();
            path.ends_with(".rs")
                || path.ends_with(".ts")
                || path.ends_with(".js")
                || path.ends_with(".py")
                || path.ends_with(".go")
        });

        if has_meaningful_files {
            // Assume current task if files look meaningful
            ChangesContext::CurrentTask
        } else {
            ChangesContext::Unknown
        }
    }

    /// Assess whether changes appear valuable
    fn assess_value(git_state: &GitState) -> bool {
        let files = &git_state.uncommitted_files;

        // Empty changes are not valuable
        if files.is_empty() {
            return false;
        }

        // Count files with valuable patterns
        let mut valuable_count = 0;
        let total_count = files.len();

        for file in files {
            // If file has been modified (not just added), it's likely valuable
            if file.change_type == FileChangeType::Modified || file.change_type == FileChangeType::Renamed {
                valuable_count += 1;
            } else if file.change_type == FileChangeType::Added {
                // New files need content check - assume valuable if in src directory
                if file.path.starts_with("src/") || file.path.starts_with("lib/") {
                    valuable_count += 1;
                }
            } else if file.change_type == FileChangeType::Deleted {
                valuable_count += 1;
            }
        }

        // Consider valuable if more than half the files seem meaningful
        valuable_count * 2 >= total_count
    }

    /// Determine the appropriate action based on context and value
    fn determine_action(context: ChangesContext, valuable: bool) -> UncommittedAction {
        match (context, valuable) {
            (ChangesContext::CurrentTask, true) => UncommittedAction::Commit,
            (ChangesContext::CurrentTask, false) => UncommittedAction::Stash,
            (ChangesContext::PreviousTask, true) => UncommittedAction::Commit,
            (ChangesContext::PreviousTask, false) => UncommittedAction::Discard,
            (ChangesContext::Unknown, true) => UncommittedAction::Stash,
            (ChangesContext::Unknown, false) => UncommittedAction::RequestHuman,
            (ChangesContext::Temporary, _) => UncommittedAction::Discard,
        }
    }

    /// Generate explanation for the suggested action
    fn explain_reason(
        context: ChangesContext,
        valuable: bool,
        files: &[crate::git_state::FileStatus],
    ) -> String {
        let file_count = files.len();
        let file_summary = if file_count == 1 {
            format!("1 file ({})", files[0].path)
        } else {
            format!("{} files", file_count)
        };

        match (context, valuable) {
            (ChangesContext::CurrentTask, true) => {
                format!("Changes appear related to current task with {} - suggesting commit", file_summary)
            }
            (ChangesContext::CurrentTask, false) => {
                format!("Changes appear related to current task but {} - stashing for safety", file_summary)
            }
            (ChangesContext::PreviousTask, true) => {
                format!("Changes appear from previous task with {} - suggesting commit", file_summary)
            }
            (ChangesContext::PreviousTask, false) => {
                format!("Changes appear from previous task but {} - discarding", file_summary)
            }
            (ChangesContext::Unknown, true) => {
                format!("Uncertain context with {} - stashing for review", file_summary)
            }
            (ChangesContext::Unknown, false) => {
                format!("Uncertain context with {} - human decision needed", file_summary)
            }
            (ChangesContext::Temporary, _) => {
                format!("Temporary/debug changes detected - discarding")
            }
        }
    }

    /// Check if a file path indicates temporary content
    fn is_temporary_file(path: &str) -> bool {
        let path_lower = path.to_lowercase();

        // Check file name patterns
        for pattern in TEMPORARY_PATTERNS {
            if path_lower.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        // Check for common temp directories
        if path_lower.contains("/tmp/") || path_lower.contains("/temp/") {
            return true;
        }

        false
    }

    /// Check if file content appears valuable
    #[allow(dead_code)]
    fn looks_valuable_content(_path: &str, _content: &str) -> bool {
        // This would be used if we had file content access
        // For now, we rely on file path patterns
        false
    }
}

/// Commit message generator
pub struct CommitMessageGenerator;

impl CommitMessageGenerator {
    /// Generate a commit message from task metadata
    pub fn generate(task_id: &str, task_summary: &str, task_type: &str) -> String {
        let type_prefix = Self::to_commit_prefix(task_type);
        format!("{}({}): {}", type_prefix, task_id, task_summary)
    }

    /// Generate a WIP commit message
    pub fn generate_wip(task_id: &str, task_summary: &str, task_type: &str) -> String {
        let type_prefix = Self::to_commit_prefix(task_type);
        format!("{}({}): {} [wip]", type_prefix, task_id, task_summary)
    }

    /// Convert task type to commit type prefix
    fn to_commit_prefix(task_type: &str) -> &'static str {
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

    /// Parse conventional commit format to extract parts
    pub fn parse(message: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = message.splitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }
        Some((parts[0].to_string(), parts[1].trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_state::FileStatus;

    fn create_git_state(files: Vec<FileStatus>) -> GitState {
        GitState {
            current_branch: "main".to_string(),
            has_uncommitted: !files.is_empty(),
            uncommitted_files: files,
            commits_ahead: 0,
            commits_behind: 0,
            has_conflicts: false,
            last_commit_sha: None,
            last_commit_message: None,
            is_healthy: true,
            last_activity: None,
        }
    }

    #[test]
    fn test_analyze_no_changes() {
        let git_state = create_git_state(vec![]);
        let analysis = UncommittedAnalyzer::analyze(&git_state, Some("task-1"));

        assert_eq!(analysis.suggested_action, UncommittedAction::Commit);
        assert!(!analysis.is_valuable);
    }

    #[test]
    fn test_analyze_temporary_files() {
        let git_state = create_git_state(vec![FileStatus {
            path: "debug.tmp".to_string(),
            change_type: FileChangeType::Modified,
            is_staged: false,
            is_new: false,
        }]);

        let analysis = UncommittedAnalyzer::analyze(&git_state, Some("task-1"));
        assert_eq!(analysis.changes_context, ChangesContext::Temporary);
        assert_eq!(analysis.suggested_action, UncommittedAction::Discard);
    }

    #[test]
    fn test_analyze_meaningful_changes() {
        let git_state = create_git_state(vec![FileStatus {
            path: "src/main.rs".to_string(),
            change_type: FileChangeType::Modified,
            is_staged: false,
            is_new: false,
        }]);

        let analysis = UncommittedAnalyzer::analyze(&git_state, Some("task-1"));
        assert_eq!(analysis.changes_context, ChangesContext::CurrentTask);
        assert!(analysis.is_valuable);
        assert_eq!(analysis.suggested_action, UncommittedAction::Commit);
    }

    #[test]
    fn test_analyze_untracked_experimental() {
        let git_state = create_git_state(vec![FileStatus {
            path: "new_idea.rs".to_string(),
            change_type: FileChangeType::Untracked,
            is_staged: false,
            is_new: true,
        }]);

        let analysis = UncommittedAnalyzer::analyze(&git_state, Some("task-1"));
        assert_eq!(analysis.changes_context, ChangesContext::Unknown);
    }

    #[test]
    fn test_commit_message_generation() {
        let msg = CommitMessageGenerator::generate("PROJ-123", "add user auth", "feature");
        assert_eq!(msg, "feat(PROJ-123): add user auth");
    }

    #[test]
    fn test_wip_commit_message() {
        let msg = CommitMessageGenerator::generate_wip("PROJ-123", "add user auth", "feature");
        assert_eq!(msg, "feat(PROJ-123): add user auth [wip]");
    }

    #[test]
    fn test_commit_prefix_conversion() {
        assert_eq!(CommitMessageGenerator::generate("1", "test", "feature"), "feat(1): test");
        assert_eq!(CommitMessageGenerator::generate("1", "test", "bugfix"), "fix(1): test");
        assert_eq!(CommitMessageGenerator::generate("1", "test", "refactor"), "refactor(1): test");
    }

    #[test]
    fn test_parse_commit_message() {
        let (prefix, body) = CommitMessageGenerator::parse("feat(PROJ-123): add auth").unwrap();
        assert_eq!(prefix, "feat(PROJ-123)");
        assert_eq!(body, "add auth");
    }

    #[test]
    fn test_parse_commit_message_invalid() {
        assert!(CommitMessageGenerator::parse("invalid message").is_none());
    }

    #[test]
    fn test_changes_context_display() {
        assert_eq!(format!("{}", ChangesContext::CurrentTask), "CurrentTask");
        assert_eq!(format!("{}", ChangesContext::PreviousTask), "PreviousTask");
        assert_eq!(format!("{}", ChangesContext::Unknown), "Unknown");
        assert_eq!(format!("{}", ChangesContext::Temporary), "Temporary");
    }

    #[test]
    fn test_uncommitted_action_display() {
        assert_eq!(format!("{}", UncommittedAction::Commit), "Commit");
        assert_eq!(format!("{}", UncommittedAction::Stash), "Stash");
        assert_eq!(format!("{}", UncommittedAction::Discard), "Discard");
        assert_eq!(format!("{}", UncommittedAction::RequestHuman), "RequestHuman");
    }

    #[test]
    fn test_uncommitted_analysis_summary() {
        let analysis = UncommittedAnalysis::new(
            ChangesContext::CurrentTask,
            true,
            UncommittedAction::Commit,
            "Test reason",
        );
        assert!(analysis.summary().contains("CurrentTask"));
        assert!(analysis.summary().contains("valuable"));
        assert!(analysis.summary().contains("Commit"));
    }
}
