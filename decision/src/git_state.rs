//! Git State Analysis for Task Preparation
//!
//! Provides git state detection and analysis for decision-making
//! before task execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Default timeout for git commands (30 seconds)
const DEFAULT_GIT_TIMEOUT_MS: u64 = 30000;

/// Git state analysis errors
#[derive(Debug, thiserror::Error)]
pub enum GitStateError {
    #[error("not a git repository: {0}")]
    NotAGitRepository(String),

    #[error("worktree not found: {0}")]
    WorktreeNotFound(String),

    #[error("git command failed: {0}")]
    CommandFailed(String),

    #[error("git command timeout after {0}ms")]
    Timeout(u64),

    #[error("failed to parse output: {0}")]
    ParseError(String),

    #[error("worktree path is invalid")]
    InvalidPath,
}

/// File change type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeType {
    /// New file added
    Added,
    /// Existing file modified
    Modified,
    /// File deleted
    Deleted,
    /// File renamed
    Renamed,
    /// File copied
    Copied,
    /// Untracked file
    Untracked,
    /// Ignored file
    Ignored,
    /// Unmerged file (conflict)
    Unmerged,
}

impl FileChangeType {
    /// Parse from porcelain status code
    pub fn from_porcelain(code: &str) -> Option<Self> {
        match code {
            "M" | "MM" => Some(FileChangeType::Modified),
            "A" | "AM" | "AD" => Some(FileChangeType::Added),
            "D" | "DM" => Some(FileChangeType::Deleted),
            "R" | "RM" => Some(FileChangeType::Renamed),
            "C" | "CM" => Some(FileChangeType::Copied),
            "??" => Some(FileChangeType::Untracked),
            "!!" => Some(FileChangeType::Ignored),
            "UU" | "AA" | "DD" => Some(FileChangeType::Unmerged),
            _ => None,
        }
    }
}

/// Status of a single file in the working tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    /// Path to the file
    pub path: String,
    /// Type of change
    pub change_type: FileChangeType,
    /// Whether the file is staged
    pub is_staged: bool,
    /// Whether the file is untracked (new)
    pub is_new: bool,
}

impl FileStatus {
    /// Create from porcelain status line
    pub fn from_porcelain_line(line: &str) -> Option<Self> {
        if line.len() < 3 {
            return None;
        }

        let index_status = &line[..1];
        let worktree_status = &line[1..2];
        let path = line[3..].trim().to_string();

        // Determine the primary change type from worktree status
        let change_type = if worktree_status != " " && worktree_status != "?" {
            FileChangeType::from_porcelain(worktree_status)
                .unwrap_or_else(|| FileChangeType::from_porcelain(&line[0..2]).unwrap_or(FileChangeType::Modified))
        } else if index_status != " " && index_status != "?" {
            FileChangeType::from_porcelain(index_status)
                .unwrap_or_else(|| FileChangeType::from_porcelain(&line[0..2]).unwrap_or(FileChangeType::Modified))
        } else {
            FileChangeType::from_porcelain(&line[0..2])
                .unwrap_or(FileChangeType::Modified)
        };

        let is_staged = index_status != " " && index_status != "?" && index_status != "!";
        let is_new = change_type == FileChangeType::Added || change_type == FileChangeType::Untracked;

        Some(FileStatus {
            path,
            change_type,
            is_staged,
            is_new,
        })
    }
}

/// Git state analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitState {
    /// Current branch name
    pub current_branch: String,

    /// Whether there are uncommitted changes
    pub has_uncommitted: bool,

    /// List of uncommitted files with their status
    pub uncommitted_files: Vec<FileStatus>,

    /// Number of commits ahead of base branch
    pub commits_ahead: usize,

    /// Number of commits behind base branch
    pub commits_behind: usize,

    /// Whether there are merge/rebase conflicts
    pub has_conflicts: bool,

    /// Last commit SHA (short form)
    pub last_commit_sha: Option<String>,

    /// Last commit message (first line)
    pub last_commit_message: Option<String>,

    /// Worktree is healthy (exists, no lock issues)
    pub is_healthy: bool,

    /// Timestamp of last change (approximate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
}

impl Default for GitState {
    fn default() -> Self {
        Self {
            current_branch: String::new(),
            has_uncommitted: false,
            uncommitted_files: Vec::new(),
            commits_ahead: 0,
            commits_behind: 0,
            has_conflicts: false,
            last_commit_sha: None,
            last_commit_message: None,
            is_healthy: true,
            last_activity: None,
        }
    }
}

impl GitState {
    /// Check if there are any staged changes
    pub fn has_staged_changes(&self) -> bool {
        self.uncommitted_files.iter().any(|f| f.is_staged)
    }

    /// Check if there are untracked files
    pub fn has_untracked_files(&self) -> bool {
        self.uncommitted_files
            .iter()
            .any(|f| f.change_type == FileChangeType::Untracked)
    }

    /// Check if changes need attention before task switch
    pub fn needs_commit_before_switch(&self) -> bool {
        self.has_uncommitted && !self.has_staged_changes()
    }

    /// Get summary of changes for display
    pub fn change_summary(&self) -> String {
        if self.uncommitted_files.is_empty() {
            return "No uncommitted changes".to_string();
        }

        let mut counts: HashMap<FileChangeType, usize> = HashMap::new();
        for file in &self.uncommitted_files {
            *counts.entry(file.change_type).or_insert(0) += 1;
        }

        counts
            .iter()
            .map(|(t, c)| format!("{:?}:{}", t, c))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Git command executor with timeout protection
#[derive(Debug, Clone)]
pub struct GitCommandExecutor {
    #[allow(dead_code)]
    timeout_ms: u64,
}

impl Default for GitCommandExecutor {
    fn default() -> Self {
        Self::new(DEFAULT_GIT_TIMEOUT_MS)
    }
}

impl GitCommandExecutor {
    /// Create with custom timeout
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    /// Execute git command and return stdout
    pub fn execute(
        &self,
        cwd: &Path,
        args: &[&str],
    ) -> Result<String, GitStateError> {
        // Validate path
        if !cwd.exists() {
            return Err(GitStateError::WorktreeNotFound(
                cwd.display().to_string(),
            ));
        }

        // Build command
        let mut cmd = Command::new("git");
        cmd.current_dir(cwd).args(args);

        // Execute with timeout using wait_timeout approach
        let output = cmd
            .output()
            .map_err(|e| GitStateError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitStateError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Execute git command with longer timeout (for fetch, rebase, etc.)
    pub fn execute_with_timeout(
        &self,
        cwd: &Path,
        args: &[&str],
        _timeout_ms: u64,
    ) -> Result<String, GitStateError> {
        if !cwd.exists() {
            return Err(GitStateError::WorktreeNotFound(
                cwd.display().to_string(),
            ));
        }

        let mut cmd = Command::new("git");
        cmd.current_dir(cwd).args(args);

        let output = cmd
            .output()
            .map_err(|e| GitStateError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitStateError::CommandFailed(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Parser for git command outputs
pub struct GitStateParser;

impl GitStateParser {
    /// Parse `git status --porcelain` output
    pub fn parse_status(output: &str) -> Vec<FileStatus> {
        output
            .lines()
            .filter_map(GitStateParser::parse_status_line)
            .collect()
    }

    /// Parse a single status line
    fn parse_status_line(line: &str) -> Option<FileStatus> {
        FileStatus::from_porcelain_line(line)
    }

    /// Parse `git rev-list --left-right --count HEAD...base` output
    ///
    /// Returns (ahead, behind) counts
    pub fn parse_ahead_behind(output: &str) -> (usize, usize) {
        let parts: Vec<&str> = output.trim().split('\t').collect();
        if parts.len() != 2 {
            return (0, 0);
        }

        let ahead = parts[0].parse().unwrap_or(0);
        let behind = parts[1].parse().unwrap_or(0);
        (ahead, behind)
    }

    /// Parse `git log -1 --format="%h %s"` output
    ///
    /// Returns (sha, message) tuple
    pub fn parse_log(output: &str) -> Option<(String, String)> {
        let output = output.trim();
        if output.is_empty() {
            return None;
        }

        let first_space = output.find(' ')?;
        let sha = output[..first_space].to_string();
        let message = output[first_space + 1..].to_string();
        Some((sha, message))
    }

    /// Parse current branch name from `git branch --show-current`
    pub fn parse_current_branch(output: &str) -> Option<String> {
        let branch = output.trim();
        if branch.is_empty() {
            None
        } else {
            Some(branch.to_string())
        }
    }

    /// Check if output indicates conflicts
    pub fn has_conflicts(output: &str) -> bool {
        // Conflict markers in status or unmerged files
        output.contains("UU")
            || output.contains("AA")
            || output.contains("DD")
            || output.contains("unmerged")
    }
}

/// Git state analyzer
#[derive(Debug, Clone)]
pub struct GitStateAnalyzer {
    executor: GitCommandExecutor,
    base_branch: String,
}

impl Default for GitStateAnalyzer {
    fn default() -> Self {
        Self::new("main")
    }
}

impl GitStateAnalyzer {
    /// Create with base branch name
    pub fn new(base_branch: &str) -> Self {
        Self {
            executor: GitCommandExecutor::default(),
            base_branch: base_branch.to_string(),
        }
    }

    /// Analyze git state for a worktree
    pub fn analyze(&self, worktree_path: &Path) -> Result<GitState, GitStateError> {
        // Validate worktree
        if !worktree_path.exists() {
            return Err(GitStateError::WorktreeNotFound(
                worktree_path.display().to_string(),
            ));
        }

        // Check if it's a git repo
        let is_repo = self
            .executor
            .execute(worktree_path, &["rev-parse", "--git-dir"])
            .is_ok();

        if !is_repo {
            return Err(GitStateError::NotAGitRepository(
                worktree_path.display().to_string(),
            ));
        }

        // Get current branch
        let current_branch = self
            .executor
            .execute(worktree_path, &["branch", "--show-current"])
            .ok()
            .and_then(|o| GitStateParser::parse_current_branch(&o))
            .unwrap_or_else(|| "detached".to_string());

        // Get status
        let status_output = self
            .executor
            .execute(worktree_path, &["status", "--porcelain"])?;
        let uncommitted_files = GitStateParser::parse_status(&status_output);
        let has_conflicts = GitStateParser::has_conflicts(&status_output);

        // Get ahead/behind
        let (commits_ahead, commits_behind) = self
            .get_ahead_behind(worktree_path)
            .unwrap_or((0, 0));

        // Get last commit info
        let (last_commit_sha, last_commit_message) = self
            .executor
            .execute(worktree_path, &["log", "-1", "--format=%h %s"])
            .ok()
            .and_then(|o| GitStateParser::parse_log(&o))
            .unwrap_or((String::new(), String::new()));

        Ok(GitState {
            current_branch,
            has_uncommitted: !uncommitted_files.is_empty(),
            uncommitted_files,
            commits_ahead,
            commits_behind,
            has_conflicts,
            last_commit_sha: Some(last_commit_sha),
            last_commit_message: Some(last_commit_message),
            is_healthy: true,
            last_activity: None,
        })
    }

    /// Get commits ahead/behind base branch
    fn get_ahead_behind(&self, worktree_path: &Path) -> Result<(usize, usize), GitStateError> {
        // Get the base branch commit
        let base_rev = self.get_base_rev(worktree_path)?;

        // Use rev-list to count
        let output = self.executor.execute(
            worktree_path,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{}...HEAD", base_rev),
            ],
        )?;

        Ok(GitStateParser::parse_ahead_behind(&output))
    }

    /// Get the revision to compare against (base branch or its remote tracking)
    fn get_base_rev(&self, worktree_path: &Path) -> Result<String, GitStateError> {
        // Try origin/base_branch first
        let origin_branch = format!("origin/{}", self.base_branch);
        if self
            .executor
            .execute(worktree_path, &["rev-parse", "--verify", &origin_branch])
            .is_ok()
        {
            return Ok(origin_branch);
        }

        // Fall back to local base_branch
        if self
            .executor
            .execute(worktree_path, &["rev-parse", "--verify", &self.base_branch])
            .is_ok()
        {
            return Ok(self.base_branch.clone());
        }

        // Last resort: use HEAD
        Ok("HEAD".to_string())
    }

    /// Quick health check - just verify git repo exists
    pub fn is_healthy(&self, worktree_path: &Path) -> bool {
        self.executor
            .execute(worktree_path, &["rev-parse", "--git-dir"])
            .is_ok()
    }

    /// Get diff summary (files changed, insertions, deletions)
    pub fn get_diff_summary(&self, worktree_path: &Path) -> Result<String, GitStateError> {
        self.executor.execute(worktree_path, &["diff", "--stat"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_modified() {
        let output = " M src/main.rs\n M src/lib.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].change_type, FileChangeType::Modified);
        assert!(!files[0].is_staged);
    }

    #[test]
    fn test_parse_status_staged() {
        let output = "M  src/main.rs\nM  src/lib.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 2);
        assert!(files[0].is_staged);
    }

    #[test]
    fn test_parse_status_untracked() {
        let output = "?? test.txt\n?? new_file.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].change_type, FileChangeType::Untracked);
    }

    #[test]
    fn test_parse_status_added() {
        let output = "A  new_file.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].change_type, FileChangeType::Added);
        assert!(files[0].is_staged);
    }

    #[test]
    fn test_parse_status_deleted() {
        let output = "D  deleted_file.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].change_type, FileChangeType::Deleted);
    }

    #[test]
    fn test_parse_status_conflict() {
        let output = "UU src/conflict.rs";
        let files = GitStateParser::parse_status(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].change_type, FileChangeType::Unmerged);
    }

    #[test]
    fn test_parse_status_empty() {
        let output = "";
        let files = GitStateParser::parse_status(output);
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_ahead_behind() {
        let output = "3\t5";
        let (ahead, behind) = GitStateParser::parse_ahead_behind(output);
        assert_eq!(ahead, 3);
        assert_eq!(behind, 5);
    }

    #[test]
    fn test_parse_ahead_behind_empty() {
        let output = "";
        let (ahead, behind) = GitStateParser::parse_ahead_behind(output);
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_log() {
        let output = "abc123 Add user authentication feature";
        let (sha, msg) = GitStateParser::parse_log(output).unwrap();
        assert_eq!(sha, "abc123");
        assert_eq!(msg, "Add user authentication feature");
    }

    #[test]
    fn test_parse_log_empty() {
        let output = "";
        assert!(GitStateParser::parse_log(output).is_none());
    }

    #[test]
    fn test_parse_current_branch() {
        assert_eq!(
            GitStateParser::parse_current_branch("feature/add-auth\n"),
            Some("feature/add-auth".to_string())
        );
    }

    #[test]
    fn test_parse_current_branch_empty() {
        assert_eq!(GitStateParser::parse_current_branch(""), None);
    }

    #[test]
    fn test_has_conflicts() {
        assert!(GitStateParser::has_conflicts("UU src/file.rs"));
        assert!(GitStateParser::has_conflicts("AA src/file.rs"));
        assert!(GitStateParser::has_conflicts("DD src/file.rs"));
    }

    #[test]
    fn test_has_no_conflicts() {
        assert!(!GitStateParser::has_conflicts(" M src/main.rs"));
        assert!(!GitStateParser::has_conflicts("?? new_file.rs"));
    }

    #[test]
    fn test_git_state_change_summary() {
        let mut state = GitState::default();
        state.uncommitted_files = vec![
            FileStatus {
                path: "file1.rs".to_string(),
                change_type: FileChangeType::Modified,
                is_staged: false,
                is_new: false,
            },
            FileStatus {
                path: "file2.rs".to_string(),
                change_type: FileChangeType::Added,
                is_staged: true,
                is_new: true,
            },
        ];
        state.has_uncommitted = true;

        let summary = state.change_summary();
        assert!(summary.contains("Modified"));
        assert!(summary.contains("Added"));
    }

    #[test]
    fn test_git_state_no_changes() {
        let state = GitState::default();
        assert_eq!(state.change_summary(), "No uncommitted changes");
    }

    #[test]
    fn test_git_state_has_staged_changes() {
        let mut state = GitState::default();
        state.uncommitted_files = vec![FileStatus {
            path: "file.rs".to_string(),
            change_type: FileChangeType::Modified,
            is_staged: true,
            is_new: false,
        }];
        state.has_uncommitted = true;

        assert!(state.has_staged_changes());
    }

    #[test]
    fn test_file_status_from_porcelain_line() {
        let file = FileStatus::from_porcelain_line("M  src/main.rs").unwrap();
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.change_type, FileChangeType::Modified);
        assert!(file.is_staged);
    }

    #[test]
    fn test_file_status_from_porcelain_untracked() {
        let file = FileStatus::from_porcelain_line("?? new_file.rs").unwrap();
        assert_eq!(file.path, "new_file.rs");
        assert_eq!(file.change_type, FileChangeType::Untracked);
        assert!(!file.is_staged);
        assert!(file.is_new);
    }
}
