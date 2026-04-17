//! Git Worktree Manager for multi-agent isolation
//!
//! Provides worktree creation, management, and cleanup operations
//! to support isolated development environments for multiple agents.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use crate::logging;

/// Worktree status information parsed from git worktree list --porcelain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    /// Absolute path to the worktree directory
    pub path: PathBuf,
    /// Current HEAD commit SHA
    pub head: String,
    /// Branch name (None if detached HEAD)
    pub branch: Option<String>,
    /// Whether this is a detached HEAD worktree
    pub is_detached: bool,
    /// Whether this worktree is locked
    pub is_locked: bool,
    /// Lock reason if locked
    pub lock_reason: Option<String>,
    /// Whether this worktree is prunable
    pub is_prunable: bool,
    /// Prune reason if prunable
    pub prune_reason: Option<String>,
}

impl WorktreeInfo {
    /// Check if this is the main worktree (repository root)
    pub fn is_main(&self) -> bool {
        // Main worktree typically has .git as a directory, not a file
        self.path.join(".git").is_dir()
    }

    /// Get the worktree name (directory name)
    pub fn name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        let branch_str = if self.is_detached {
            "detached HEAD"
        } else {
            self.branch.as_deref().unwrap_or("unknown")
        };
        let head_short = if self.head.len() >= 8 {
            &self.head[..8]
        } else {
            &self.head
        };
        format!("{} [{}] {}", self.path.display(), branch_str, head_short)
    }
}

/// Worktree creation options
#[derive(Debug, Clone)]
pub struct WorktreeCreateOptions {
    /// Worktree path (relative to repo root or absolute)
    pub path: PathBuf,
    /// Branch name (None means detached HEAD)
    pub branch: Option<String>,
    /// Whether to create new branch (if branch doesn't exist)
    pub create_branch: bool,
    /// Base commit/branch to create from (only valid when create_branch=true)
    pub base: Option<String>,
    /// Lock reason (optional, locks the worktree after creation)
    pub lock_reason: Option<String>,
}

impl Default for WorktreeCreateOptions {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            branch: None,
            create_branch: false,
            base: None,
            lock_reason: None,
        }
    }
}

/// Worktree manager errors
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("not a git repository: {0}")]
    NotAGitRepository(PathBuf),

    #[error("worktree not found: {0}")]
    WorktreeNotFound(PathBuf),

    #[error("worktree already exists: {0}")]
    WorktreeAlreadyExists(PathBuf),

    #[error("worktree path is invalid: {0}")]
    InvalidWorktreePath(PathBuf),

    #[error("branch already exists: {0}")]
    BranchAlreadyExists(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("maximum worktrees limit reached: {0}")]
    MaxWorktreesReached(usize),

    #[error("git command failed: {0}")]
    GitCommandFailed(String),

    #[error("git is not available on this system")]
    GitNotAvailable,

    #[error("failed to parse porcelain output: {0}")]
    ParseError(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Configuration for worktree management
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    /// Maximum number of worktrees allowed (excluding main)
    pub max_worktrees: usize,
    /// Worktree directory name prefix for branch naming
    pub prefix: String,
    /// Default base branch for new worktrees
    pub default_base_branch: String,
    /// Whether to auto cleanup completed worktrees
    pub auto_cleanup: bool,
    /// Worktree idle timeout in seconds (0 = no timeout)
    pub idle_timeout_secs: u64,
    /// Root directory for worktrees storage (relative to repo)
    pub worktrees_dir: String,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            max_worktrees: 10,
            prefix: "agent".to_string(),
            default_base_branch: "main".to_string(),
            auto_cleanup: true,
            idle_timeout_secs: 3600, // 1 hour
            worktrees_dir: ".worktrees".to_string(),
        }
    }
}

/// Worktree manager for creating and managing git worktrees
///
/// Provides thread-safe operations for worktree lifecycle management.
/// All git operations are synchronized via an internal mutex.
pub struct WorktreeManager {
    /// Main repository root path
    repo_root: PathBuf,
    /// Worktrees storage directory
    worktrees_dir: PathBuf,
    /// Configuration
    config: WorktreeConfig,
    /// Mutex for synchronizing git operations
    git_lock: Mutex<()>,
}

impl WorktreeManager {
    /// Create a new WorktreeManager
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Path is not a git repository
    /// - Git is not available on the system
    pub fn new(repo_root: PathBuf, config: WorktreeConfig) -> Result<Self, WorktreeError> {
        // Verify git is available
        let git_available = Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !git_available {
            return Err(WorktreeError::GitNotAvailable);
        }

        // Verify it's a git repository (check for .git directory or file)
        let git_path = repo_root.join(".git");
        if !git_path.exists() && !repo_root.join("HEAD").exists() {
            return Err(WorktreeError::NotAGitRepository(repo_root));
        }

        let worktrees_dir = repo_root.join(&config.worktrees_dir);

        // Log initialization before moving values
        logging::debug_event(
            "worktree.manager.created",
            "WorktreeManager initialized",
            serde_json::json!({
                "repo_root": repo_root.display().to_string(),
                "worktrees_dir": worktrees_dir.display().to_string(),
                "max_worktrees": config.max_worktrees
            }),
        );

        let manager = Self {
            repo_root,
            worktrees_dir,
            config,
            git_lock: Mutex::new(()),
        };

        Ok(manager)
    }

    /// Get the repository root path
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Get the worktrees directory path
    pub fn worktrees_dir(&self) -> &Path {
        &self.worktrees_dir
    }

    /// Get the configuration
    pub fn config(&self) -> &WorktreeConfig {
        &self.config
    }

    /// List all worktrees
    ///
    /// Returns a list of WorktreeInfo parsed from git worktree list --porcelain
    pub fn list(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        let output = self.run_git_command(&["worktree", "list", "--porcelain"])?;
        self.parse_porcelain_output(&output)
    }

    /// Create a new worktree
    ///
    /// # Arguments
    ///
    /// * `name` - Name for the worktree (used as directory name)
    /// * `options` - Creation options
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Worktree limit is reached
    /// - Path is invalid
    /// - Git command fails
    pub fn create(&self, name: &str, options: WorktreeCreateOptions) -> Result<WorktreeInfo, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Check worktree limit
        let current_count = self.list_internal()?.len();
        if current_count >= self.config.max_worktrees + 1 {
            // +1 for main worktree
            return Err(WorktreeError::MaxWorktreesReached(self.config.max_worktrees));
        }

        // Determine worktree path
        let path = if options.path.is_absolute() {
            options.path
        } else {
            self.worktrees_dir.join(name)
        };

        // Ensure worktrees directory exists
        if !self.worktrees_dir.exists() {
            std::fs::create_dir_all(&self.worktrees_dir)?;
            logging::debug_event(
                "worktree.dir.created",
                "Worktrees directory created",
                serde_json::json!({"path": self.worktrees_dir.display().to_string()}),
            );
        }

        // Check if path already exists
        if path.exists() {
            return Err(WorktreeError::WorktreeAlreadyExists(path));
        }

        // Build git worktree add command
        let mut args = vec!["worktree", "add"];

        if let Some(branch) = &options.branch {
            if options.create_branch {
                args.push("-b");
                args.push(branch);
            } else {
                // Use existing branch - check if branch exists first
                let branch_exists = self.branch_exists_internal(branch)?;
                if !branch_exists {
                    return Err(WorktreeError::BranchNotFound(branch.clone()));
                }
            }
        } else {
            // detached HEAD
            args.push("--detach");
        }

        args.push(path.to_str().unwrap_or(""));

        if let Some(base) = &options.base {
            args.push(base);
        }

        logging::debug_event(
            "worktree.create.start",
            "Creating worktree",
            serde_json::json!({
                "name": name,
                "path": path.display().to_string(),
                "branch": options.branch,
                "create_branch": options.create_branch,
                "base": options.base
            }),
        );

        self.run_git_command_internal(&args)?;

        // Optionally lock the worktree
        if let Some(reason) = &options.lock_reason {
            self.lock_worktree_internal(&path, reason)?;
        }

        // Get the created worktree info
        let info = self.get_worktree_info_internal(&path)?;

        logging::debug_event(
            "worktree.create.complete",
            "Worktree created successfully",
            serde_json::json!({
                "name": name,
                "path": info.path.display().to_string(),
                "branch": info.branch,
                "head": info.head
            }),
        );

        Ok(info)
    }

    /// Remove a worktree
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the worktree to remove
    /// * `force` - Force removal even with uncommitted changes
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Worktree not found
    /// - Git command fails
    pub fn remove(&self, name: &str, force: bool) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        let path = self.worktrees_dir.join(name);

        // Check if worktree exists
        if !path.exists() {
            return Err(WorktreeError::WorktreeNotFound(path));
        }

        let mut args = vec!["worktree", "remove"];

        if force {
            args.push("--force");
        }

        args.push(path.to_str().unwrap_or(""));

        logging::debug_event(
            "worktree.remove.start",
            "Removing worktree",
            serde_json::json!({
                "name": name,
                "path": path.display().to_string(),
                "force": force
            }),
        );

        self.run_git_command_internal(&args)?;

        logging::debug_event(
            "worktree.remove.complete",
            "Worktree removed successfully",
            serde_json::json!({"name": name}),
        );

        Ok(())
    }

    /// Prune deleted worktree records
    ///
    /// Cleans up administrative files for worktrees that have been
    /// manually deleted from the filesystem.
    pub fn prune(&self) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event("worktree.prune.start", "Pruning worktree records", serde_json::json!({}));

        self.run_git_command_internal(&["worktree", "prune"])?;

        logging::debug_event("worktree.prune.complete", "Worktree records pruned", serde_json::json!({}));

        Ok(())
    }

    /// Lock a worktree to prevent pruning
    pub fn lock_worktree(&self, path: &Path, reason: &str) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        self.lock_worktree_internal(path, reason)
    }

    fn lock_worktree_internal(&self, path: &Path, reason: &str) -> Result<(), WorktreeError> {
        let mut args = vec!["worktree", "lock"];
        if !reason.is_empty() {
            args.push("--reason");
            args.push(reason);
        }
        args.push(path.to_str().unwrap_or(""));

        self.run_git_command_internal(&args)?;
        Ok(())
    }

    /// Unlock a worktree
    pub fn unlock_worktree(&self, path: &Path) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        self.run_git_command_internal(&[
            "worktree", "unlock",
            path.to_str().unwrap_or("")
        ])?;
        Ok(())
    }

    /// Create a worktree specifically for an agent
    ///
    /// This is a convenience method that creates a worktree with
    /// appropriate naming and branch conventions for agent use.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - Agent identifier (used as worktree directory name)
    /// * `task_id` - Task identifier (used for branch naming)
    pub fn create_for_agent(
        &self,
        agent_id: &str,
        task_id: Option<&str>,
    ) -> Result<WorktreeInfo, WorktreeError> {
        let branch_name = match task_id {
            Some(task) => format!("{}/{}", self.config.prefix, task),
            None => format!("{}{}", self.config.prefix, agent_id.replace("agent_", "")),
        };

        let options = WorktreeCreateOptions {
            path: self.worktrees_dir.join(agent_id),
            branch: Some(branch_name),
            create_branch: true,
            base: Some(self.config.default_base_branch.clone()),
            lock_reason: None,
        };

        self.create(agent_id, options)
    }

    /// Get worktree information by path
    pub fn get_worktree_info(&self, path: &Path) -> Result<WorktreeInfo, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        self.get_worktree_info_internal(path)
    }

    fn get_worktree_info_internal(&self, path: &Path) -> Result<WorktreeInfo, WorktreeError> {
        let all = self.list_internal()?;
        all.into_iter()
            .find(|w| w.path == path)
            .ok_or(WorktreeError::WorktreeNotFound(path.to_path_buf()))
    }

    /// Get worktree information by name
    pub fn get_worktree_by_name(&self, name: &str) -> Result<WorktreeInfo, WorktreeError> {
        let path = self.worktrees_dir.join(name);
        self.get_worktree_info(&path)
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, branch: &str) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        self.branch_exists_internal(branch)
    }

    fn branch_exists_internal(&self, branch: &str) -> Result<bool, WorktreeError> {
        let output = self.run_git_command_internal(&["branch", "--list", branch])?;
        Ok(output.trim().contains(branch))
    }

    /// List all agent branches (branches starting with "agent/")
    ///
    /// Returns a list of branch names like "agent/agent_001", "agent/agent_002"
    pub fn list_agent_branches(&self) -> Result<Vec<String>, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        let output = self.run_git_command_internal(&["branch", "--list", "agent/*"])?;

        let branches: Vec<String> = output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                // Remove leading "* " if it's the current branch
                let branch = line.strip_prefix('*').unwrap_or(line).trim();
                if branch.starts_with("agent/") {
                    Some(branch.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(branches)
    }

    /// Get the current HEAD commit SHA
    pub fn get_current_head(&self) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        let output = self.run_git_command_internal(&["rev-parse", "HEAD"])?;
        Ok(output.trim().to_string())
    }

    /// Get the default branch name (detect from remote or config)
    pub fn get_default_branch(&self) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Try to detect from remote
        let output = self.run_git_command_internal(&[
            "symbolic-ref", "refs/remotes/origin/HEAD"
        ]).ok();

        if let Some(refs) = output {
            // refs/remotes/origin/HEAD -> refs/remotes/origin/main
            if let Some(branch) = refs.trim().split('/').nth(2) {
                return Ok(branch.to_string());
            }
        }

        // Fall back to config
        Ok(self.config.default_base_branch.clone())
    }

    /// Check if a branch is currently checked out in any worktree
    ///
    /// Returns the path to the worktree that has this branch checked out,
    /// or None if the branch is not checked out anywhere.
    pub fn branch_checkout_location(&self, branch: &str) -> Result<Option<PathBuf>, WorktreeError> {
        let worktrees = self.list()?;
        for wt in worktrees {
            if wt.branch.as_ref().map(|b| b == branch).unwrap_or(false) {
                return Ok(Some(wt.path));
            }
        }
        Ok(None)
    }

    /// Check if a branch can be used for a new worktree
    ///
    /// A branch can be used if:
    /// - It doesn't exist (can create new branch)
    /// - It exists but is not checked out in any worktree
    pub fn can_checkout_branch(&self, branch: &str) -> Result<bool, WorktreeError> {
        // If branch doesn't exist, we can create it
        if !self.branch_exists(branch)? {
            return Ok(true);
        }
        // If branch exists but not checked out anywhere, we can use it
        let location = self.branch_checkout_location(branch)?;
        Ok(location.is_none())
    }

    /// Get the HEAD commit SHA for a specific branch
    ///
    /// Returns the commit that the branch points to.
    pub fn get_branch_head(&self, branch: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        let output = self.run_git_command_internal(&[
            "rev-parse", &format!("refs/heads/{}", branch)
        ])?;
        Ok(output.trim().to_string())
    }

    /// Count the number of worktrees (excluding main)
    pub fn count_worktrees(&self) -> Result<usize, WorktreeError> {
        let all = self.list()?;
        Ok(all.len().saturating_sub(1)) // Subtract main worktree
    }

    /// Run a git command and return stdout
    fn run_git_command(&self, args: &[&str]) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        self.run_git_command_internal(args)
    }

    fn run_git_command_internal(&self, args: &[&str]) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(WorktreeError::GitCommandFailed(stderr))
        }
    }

    fn list_internal(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let output = self.run_git_command_internal(&["worktree", "list", "--porcelain"])?;
        self.parse_porcelain_output(&output)
    }

    /// Parse porcelain output from git worktree list --porcelain
    ///
    /// Porcelain format:
    /// ```
    /// worktree /path/to/worktree
    /// HEAD abc123...
    /// branch refs/heads/branch-name  # optional
    /// detached                        # optional
    /// locked reason text              # optional
    /// prunable reason text            # optional
    /// ```
    fn parse_porcelain_output(&self, output: &str) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let mut worktrees = Vec::new();
        let mut current_info: Option<WorktreeInfo> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.is_empty() {
                // Empty line marks end of current worktree info
                if let Some(info) = current_info.take() {
                    worktrees.push(info);
                }
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            let key = parts[0];
            let value = parts.get(1).map(|s| s.trim());

            match key {
                "worktree" => {
                    // Start new worktree entry
                    if let Some(info) = current_info.take() {
                        worktrees.push(info);
                    }
                    let path = value
                        .ok_or_else(|| WorktreeError::ParseError("Missing worktree path".into()))?;
                    current_info = Some(WorktreeInfo {
                        path: PathBuf::from(path),
                        head: String::new(),
                        branch: None,
                        is_detached: false,
                        is_locked: false,
                        lock_reason: None,
                        is_prunable: false,
                        prune_reason: None,
                    });
                }
                "HEAD" => {
                    if let Some(ref mut info) = current_info {
                        info.head = value.unwrap_or("").to_string();
                    }
                }
                "branch" => {
                    if let Some(ref mut info) = current_info {
                        // branch refs/heads/branch-name
                        let branch_ref = value.unwrap_or("");
                        // Extract branch name from refs/heads/xxx
                        // For refs/heads/feature/test, we want "feature/test"
                        if branch_ref.starts_with("refs/heads/") {
                            info.branch = Some(branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref).to_string());
                        } else {
                            info.branch = Some(branch_ref.to_string());
                        }
                    }
                }
                "detached" => {
                    if let Some(ref mut info) = current_info {
                        info.is_detached = true;
                    }
                }
                "locked" => {
                    if let Some(ref mut info) = current_info {
                        info.is_locked = true;
                        info.lock_reason = value.map(|s| s.to_string());
                    }
                }
                "prunable" => {
                    if let Some(ref mut info) = current_info {
                        info.is_prunable = true;
                        info.prune_reason = value.map(|s| s.to_string());
                    }
                }
                _ => {
                    // Unknown key, ignore
                }
            }
        }

        // Don't forget the last worktree if output doesn't end with empty line
        if let Some(info) = current_info.take() {
            worktrees.push(info);
        }

        Ok(worktrees)
    }

    /// Check if a worktree has uncommitted changes
    ///
    /// Returns true if there are staged or unstaged changes
    pub fn has_uncommitted_changes(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Run git status --porcelain
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(WorktreeError::GitCommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        // If output is empty, there are no changes
        let status_output = String::from_utf8_lossy(&output.stdout);
        Ok(!status_output.trim().is_empty())
    }

    /// Get the HEAD commit SHA for a worktree
    ///
    /// Returns None if unable to determine HEAD
    pub fn get_head_commit(&self, worktree_path: &Path) -> Option<String> {
        let _lock = self.git_lock.lock().unwrap();
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(worktree_path)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize a git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to init git repo");

        // Create a dummy commit
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to config git");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to config git");

        // Create a file and commit
        std::fs::write(repo_path.join("README.md"), "# Test Repo\n").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to add file");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to commit");

        (temp_dir, repo_path)
    }

    #[test]
    fn worktree_manager_new_succeeds_for_git_repo() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config);
        assert!(manager.is_ok());
    }

    #[test]
    fn worktree_manager_new_fails_for_non_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeConfig::default();
        let result = WorktreeManager::new(temp_dir.path().to_path_buf(), config);
        assert!(matches!(result, Err(WorktreeError::NotAGitRepository(_))));
    }

    #[test]
    fn worktree_manager_list_returns_main_worktree() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let worktrees = manager.list().unwrap();
        assert_eq!(worktrees.len(), 1); // Main worktree only
    }

    #[test]
    fn worktree_manager_create_and_remove() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        // Create a worktree
        let options = WorktreeCreateOptions {
            path: PathBuf::new(),
            branch: Some("test-branch".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };

        let info = manager.create("test-agent", options).unwrap();
        assert!(info.path.exists());
        assert_eq!(info.branch, Some("test-branch".to_string()));

        // Verify it's in the list
        let worktrees = manager.list().unwrap();
        assert_eq!(worktrees.len(), 2);

        // Remove the worktree
        manager.remove("test-agent", false).unwrap();
        assert!(!info.path.exists());

        // Verify it's removed from list
        let worktrees = manager.list().unwrap();
        assert_eq!(worktrees.len(), 1);
    }

    #[test]
    fn worktree_manager_create_for_agent() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let info = manager.create_for_agent("agent_001", Some("task-123")).unwrap();
        assert!(info.path.exists());
        assert_eq!(info.branch, Some("agent/task-123".to_string()));

        // Cleanup
        manager.remove("agent_001", false).unwrap();
    }

    #[test]
    fn worktree_manager_max_worktrees_limit() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig {
            max_worktrees: 1,
            ..Default::default()
        };
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        // Create first worktree - should succeed
        let options = WorktreeCreateOptions {
            path: PathBuf::new(),
            branch: Some("branch-1".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        manager.create("agent-1", options).unwrap();

        // Try to create second - should fail due to limit
        let options2 = WorktreeCreateOptions {
            path: PathBuf::new(),
            branch: Some("branch-2".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        let result = manager.create("agent-2", options2);
        assert!(matches!(result, Err(WorktreeError::MaxWorktreesReached(_))));

        // Cleanup
        manager.remove("agent-1", false).unwrap();
    }

    #[test]
    fn parse_porcelain_output_basic() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let output = "worktree /path/to/main\nHEAD abc123def456\nbranch refs/heads/main\n\nworktree /path/to/.worktrees/agent-1\nHEAD def456abc123\nbranch refs/heads/feature/test\n";

        let worktrees = manager.parse_porcelain_output(output).unwrap();
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].head, "abc123def456");
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[1].branch, Some("feature/test".to_string()));
    }

    #[test]
    fn parse_porcelain_output_detached() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let output = "worktree /path/to/.worktrees/temp\nHEAD abc123\ndetached\n";

        let worktrees = manager.parse_porcelain_output(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_detached);
        assert_eq!(worktrees[0].branch, None);
    }

    #[test]
    fn parse_porcelain_output_locked() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let output = "worktree /path/to/.worktrees/agent\nHEAD abc123\nbranch refs/heads/feature\nlocked work in progress\n";

        let worktrees = manager.parse_porcelain_output(output).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_locked);
        assert_eq!(worktrees[0].lock_reason, Some("work in progress".to_string()));
    }

    #[test]
    fn worktree_config_default() {
        let config = WorktreeConfig::default();
        assert_eq!(config.max_worktrees, 10);
        assert_eq!(config.prefix, "agent");
        assert_eq!(config.default_base_branch, "main");
        assert!(config.auto_cleanup);
        assert_eq!(config.idle_timeout_secs, 3600);
    }
}
