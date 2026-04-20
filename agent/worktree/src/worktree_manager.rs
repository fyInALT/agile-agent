//! Git Worktree Manager for multi-agent isolation
//!
//! Provides worktree creation, management, and cleanup operations
//! to support isolated development environments for multiple agents.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::logging;

/// Default timeout for git commands (30 seconds)
const GIT_COMMAND_TIMEOUT_SECS: u64 = 30;

/// Ahead/behind commit count for branch comparison
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AheadBehindCount {
    /// Number of commits ahead of base branch
    pub ahead: u32,
    /// Number of commits behind base branch
    pub behind: u32,
}

impl AheadBehindCount {
    /// Check if branch is synced (neither ahead nor behind)
    pub fn is_synced(&self) -> bool {
        self.ahead == 0 && self.behind == 0
    }

    /// Check if branch needs rebase (behind base)
    pub fn needs_rebase(&self) -> bool {
        self.behind > 0
    }
}

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
/// Clone is safe since Arc<Mutex<()>> can be cloned.
#[derive(Clone)]
pub struct WorktreeManager {
    /// Main repository root path
    repo_root: PathBuf,
    /// Worktrees storage directory
    worktrees_dir: PathBuf,
    /// Configuration
    config: WorktreeConfig,
    /// Mutex for synchronizing git operations
    git_lock: Arc<Mutex<()>>,
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
            git_lock: Arc::new(Mutex::new(())),
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
        let output = self.run_git_command_internal(&["worktree", "list", "--porcelain"])?;
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
    pub fn create(
        &self,
        name: &str,
        options: WorktreeCreateOptions,
    ) -> Result<WorktreeInfo, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Check worktree limit
        let current_count = self.list_internal()?.len();
        if current_count >= self.config.max_worktrees + 1 {
            // +1 for main worktree
            return Err(WorktreeError::MaxWorktreesReached(
                self.config.max_worktrees,
            ));
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

        // Add start-point for the worktree
        // - For new branches: base commit is specified after path
        // - For existing branches: use the branch name as start-point
        // - For detached HEAD: optionally specify commit
        if let Some(branch) = &options.branch {
            if options.create_branch {
                // New branch: add base if specified
                if let Some(base) = &options.base {
                    args.push(base);
                }
            } else {
                // Use existing branch as start-point
                args.push(branch);
            }
        } else if let Some(base) = &options.base {
            // Detached HEAD with specific base commit
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

        logging::debug_event(
            "worktree.prune.start",
            "Pruning worktree records",
            serde_json::json!({}),
        );

        self.run_git_command_internal(&["worktree", "prune"])?;

        logging::debug_event(
            "worktree.prune.complete",
            "Worktree records pruned",
            serde_json::json!({}),
        );

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

        self.run_git_command_internal(&["worktree", "unlock", path.to_str().unwrap_or("")])?;
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
        let output = self
            .run_git_command_internal(&["symbolic-ref", "refs/remotes/origin/HEAD"])
            .ok();

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
        let output =
            self.run_git_command_internal(&["rev-parse", &format!("refs/heads/{}", branch)])?;
        Ok(output.trim().to_string())
    }

    /// Count the number of worktrees (excluding main)
    pub fn count_worktrees(&self) -> Result<usize, WorktreeError> {
        let all = self.list()?;
        Ok(all.len().saturating_sub(1)) // Subtract main worktree
    }

    fn run_git_command_internal(&self, args: &[&str]) -> Result<String, WorktreeError> {
        let start = Instant::now();
        let timeout = Duration::from_secs(GIT_COMMAND_TIMEOUT_SECS);

        // Spawn the process instead of using blocking .output()
        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        // Wait with timeout using try_wait in a loop
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process completed
                    let stdout = child.stdout.take().expect("stdout was piped");
                    let stderr = child.stderr.take().expect("stderr was piped");
                    let stdout_content = std::io::read_to_string(stdout)
                        .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;
                    let stderr_content = std::io::read_to_string(stderr)
                        .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

                    let duration_ms = start.elapsed().as_millis();

                    if status.success() {
                        logging::debug_event(
                            "worktree.git_command.complete",
                            "git command completed",
                            serde_json::json!({
                                "args": args,
                                "duration_ms": duration_ms,
                                "success": true,
                            }),
                        );
                        return Ok(stdout_content);
                    } else {
                        logging::debug_event(
                            "worktree.git_command.failed",
                            "git command failed",
                            serde_json::json!({
                                "args": args,
                                "duration_ms": duration_ms,
                                "stderr": stderr_content,
                            }),
                        );
                        return Err(WorktreeError::GitCommandFailed(stderr_content));
                    }
                }
                Ok(None) => {
                    // Process still running, check timeout
                    if start.elapsed() >= timeout {
                        // Timeout - kill the process
                        logging::warn_event(
                            "worktree.git_command.timeout",
                            "git command timed out, killing process",
                            serde_json::json!({
                                "args": args,
                                "timeout_secs": GIT_COMMAND_TIMEOUT_SECS,
                            }),
                        );
                        let _ = child.kill();
                        let _ = child.wait(); // Wait for kill to complete
                        return Err(WorktreeError::GitCommandFailed(format!(
                            "Command timed out after {} seconds",
                            GIT_COMMAND_TIMEOUT_SECS
                        )));
                    }
                    // Short sleep to avoid busy-waiting
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    // Error checking process status
                    return Err(WorktreeError::GitCommandFailed(e.to_string()));
                }
            }
        }
    }

    fn list_internal(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let output = self.run_git_command_internal(&["worktree", "list", "--porcelain"])?;
        self.parse_porcelain_output(&output)
    }

    /// Parse porcelain output from git worktree list --porcelain
    ///
    /// Porcelain format:
    /// ```text
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
                            info.branch = Some(
                                branch_ref
                                    .strip_prefix("refs/heads/")
                                    .unwrap_or(branch_ref)
                                    .to_string(),
                            );
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

    /// Check if a worktree has merge/rebase conflicts
    ///
    /// Returns true if there are unmerged paths
    pub fn has_conflicts(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

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

        let status_output = String::from_utf8_lossy(&output.stdout);
        // Check for conflict markers: UU, AA, DD
        Ok(status_output.lines().any(|line| {
            line.starts_with("UU") || line.starts_with("AA") || line.starts_with("DD")
        }))
    }

    /// Get the current branch name for a worktree
    ///
    /// Returns "detached" if on detached HEAD
    pub fn get_current_branch(&self, worktree_path: &Path) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(WorktreeError::GitCommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if branch.is_empty() { "detached".to_string() } else { branch })
    }

    /// Checkout a branch in a worktree
    ///
    /// Switches to the specified branch
    pub fn checkout_branch(&self, worktree_path: &Path, branch: &str) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.checkout.started",
            "checking out branch",
            serde_json::json!({"branch": branch}),
        );

        let output = Command::new("git")
            .args(["checkout", branch])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(WorktreeError::GitCommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        logging::debug_event(
            "git_flow.checkout.completed",
            "branch checked out",
            serde_json::json!({"branch": branch}),
        );

        Ok(())
    }

    /// Get ahead/behind count relative to base branch
    ///
    /// Returns (ahead, behind) counts
    pub fn get_ahead_behind_count(&self, worktree_path: &Path, base_branch: &str) -> Result<AheadBehindCount, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Try origin/base first
        let origin_base = format!("origin/{}", base_branch);
        let base_rev = if self.run_git_command_internal(&["rev-parse", "--verify", &origin_base]).is_ok() {
            origin_base
        } else if self.run_git_command_internal(&["rev-parse", "--verify", base_branch]).is_ok() {
            base_branch.to_string()
        } else {
            "HEAD".to_string()
        };

        let output = Command::new("git")
            .args(["rev-list", "--left-right", "--count", &format!("{}...HEAD", base_rev)])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Ok(AheadBehindCount { ahead: 0, behind: 0 });
        }

        let count_output = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = count_output.trim().split('\t').collect();

        let ahead = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
        let behind = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        Ok(AheadBehindCount { ahead, behind })
    }

    /// Commit all changes with a message
    ///
    /// Adds all tracked changes and commits
    pub fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Add all tracked changes
        Command::new("git")
            .args(["add", "-u"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        // Commit
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("nothing to commit") {
                return Ok("no-changes".to_string());
            }
            return Err(WorktreeError::GitCommandFailed(stderr.to_string()));
        }

        // Get commit SHA
        let sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        Ok(String::from_utf8_lossy(&sha_output.stdout).trim().to_string())
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

    // ========================================================================
    // Git Flow Operations for Task Preparation
    // ========================================================================

    /// Fetch from origin to update remote tracking branches
    ///
    /// This is a prerequisite for sync_base_branch and getting latest remote HEAD.
    pub fn fetch_origin(&self) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.fetch.started",
            "fetching from origin",
            serde_json::json!({}),
        );

        self.run_git_command_internal(&["fetch", "origin", "--quiet"])?;

        logging::debug_event(
            "git_flow.fetch.completed",
            "fetch completed successfully",
            serde_json::json!({}),
        );

        Ok(())
    }

    /// Get the remote HEAD commit SHA for a branch
    ///
    /// Returns the commit that origin/<branch> points to.
    pub fn get_remote_head(&self, branch: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        let output = self.run_git_command_internal(&[
            "rev-parse",
            &format!("origin/{}", branch),
        ])?;
        Ok(output.trim().to_string())
    }

    /// Check if local branch is synced with remote
    ///
    /// Returns true if local branch HEAD equals remote HEAD.
    pub fn is_branch_synced(&self, branch: &str) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Get local HEAD
        let local_head = self.run_git_command_internal(&[
            "rev-parse",
            &format!("refs/heads/{}", branch),
        ])?;

        // Get remote HEAD (may not exist for new branches)
        let remote_result = self.run_git_command_internal(&[
            "rev-parse",
            &format!("origin/{}", branch),
        ]);

        match remote_result {
            Ok(remote_head) => Ok(local_head.trim() == remote_head.trim()),
            Err(_) => {
                // Remote doesn't exist - considered synced for new branches
                Ok(true)
            }
        }
    }

    /// Sync the base branch by resetting to origin
    ///
    /// This updates the local base branch to match remote.
    /// Should only be called on base branch (main/master).
    pub fn sync_base_branch(&self, branch: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.sync.started",
            "syncing base branch",
            serde_json::json!({"branch": branch}),
        );

        // First ensure we have fetched
        self.run_git_command_internal(&["fetch", "origin", "--quiet"])?;

        // Check if we're on the base branch
        let current_branch = self.run_git_command_internal(&["branch", "--show-current"])?;
        if current_branch.trim() != branch {
            // Checkout base branch first
            self.run_git_command_internal(&["checkout", branch])?;
        }

        // Reset to origin
        self.run_git_command_internal(&[
            "reset",
            "--hard",
            &format!("origin/{}", branch),
        ])?;

        // Get new HEAD
        let new_head = self.run_git_command_internal(&["rev-parse", "HEAD"])?;
        let head_sha = new_head.trim().to_string();

        logging::debug_event(
            "git_flow.sync.completed",
            "base branch synced",
            serde_json::json!({"branch": branch, "head": head_sha}),
        );

        Ok(head_sha)
    }

    /// Create a feature branch from base branch HEAD
    ///
    /// Creates a new branch from the specified base branch.
    /// Returns the created branch info.
    pub fn create_feature_branch(
        &self,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Check if branch already exists
        if self.branch_exists_internal(branch_name)? {
            return Err(WorktreeError::BranchAlreadyExists(branch_name.to_string()));
        }

        logging::debug_event(
            "git_flow.branch.create.started",
            "creating feature branch",
            serde_json::json!({
                "branch": branch_name,
                "base": base_branch,
            }),
        );

        // Create branch from base
        self.run_git_command_internal(&[
            "branch",
            branch_name,
            &format!("origin/{}", base_branch),
        ])?;

        // Get the commit SHA
        let head = self.run_git_command_internal(&[
            "rev-parse",
            branch_name,
        ])?;

        logging::debug_event(
            "git_flow.branch.create.completed",
            "feature branch created",
            serde_json::json!({
                "branch": branch_name,
                "base": base_branch,
                "head": head.trim(),
            }),
        );

        Ok(head.trim().to_string())
    }

    /// Rebase a branch to the base branch
    ///
    /// Returns the new HEAD after rebase, or error with conflict info.
    pub fn rebase_to_base(
        &self,
        branch: &str,
        base_branch: &str,
    ) -> Result<RebaseResult, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.rebase.started",
            "rebasing branch to base",
            serde_json::json!({"branch": branch, "base": base_branch}),
        );

        // Checkout the branch
        self.run_git_command_internal(&["checkout", branch])?;

        // Fetch latest
        self.run_git_command_internal(&["fetch", "origin", "--quiet"])?;

        // Attempt rebase
        let rebase_result = self.run_git_command_internal(&[
            "rebase",
            &format!("origin/{}", base_branch),
        ]);

        match rebase_result {
            Ok(_) => {
                let new_head = self.run_git_command_internal(&["rev-parse", "HEAD"])?;
                logging::debug_event(
                    "git_flow.rebase.completed",
                    "rebase successful",
                    serde_json::json!({"branch": branch, "new_head": new_head.trim()}),
                );
                Ok(RebaseResult::Success {
                    new_head: new_head.trim().to_string(),
                })
            }
            Err(e) => {
                // Check if it's a conflict
                let status = self.run_git_command_internal(&["status", "--porcelain"])?;
                let has_conflicts = status.lines().any(|line| line.starts_with("UU") || line.starts_with("AA"));

                if has_conflicts {
                    // Abort rebase to clean state
                    self.run_git_command_internal(&["rebase", "--abort"]).ok();

                    logging::warn_event(
                        "git_flow.rebase.conflict",
                        "rebase had conflicts, aborted",
                        serde_json::json!({"branch": branch, "base": base_branch}),
                    );

                    Ok(RebaseResult::Conflict {
                        message: "Rebase conflicts detected. Branch restored to pre-rebase state.".to_string(),
                    })
                } else {
                    // Other error
                    logging::warn_event(
                        "git_flow.rebase.failed",
                        "rebase failed",
                        serde_json::json!({"branch": branch, "error": e.to_string()}),
                    );
                    Err(e)
                }
            }
        }
    }

    /// Stash uncommitted changes with a descriptive message
    ///
    /// Returns the stash reference (e.g., "stash@{0}")
    pub fn stash_changes(&self, worktree_path: &Path, message: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.stash.started",
            "stashing uncommitted changes",
            serde_json::json!({"path": worktree_path.to_string_lossy(), "message": message}),
        );

        // Create stash
        let output = Command::new("git")
            .args(["stash", "push", "-m", message])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "No local changes to save" is not an error
            if stderr.contains("No local changes") {
                return Ok("none".to_string());
            }
            return Err(WorktreeError::GitCommandFailed(stderr.to_string()));
        }

        // Get the stash ref
        let stash_list = Command::new("git")
            .args(["stash", "list"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        let list_output = String::from_utf8_lossy(&stash_list.stdout);
        let stash_ref = list_output.lines().next()
            .map(|line| line.split(':').next().unwrap_or("stash@{0}"))
            .unwrap_or("stash@{0}");

        logging::debug_event(
            "git_flow.stash.completed",
            "changes stashed",
            serde_json::json!({"stash_ref": stash_ref}),
        );

        Ok(stash_ref.to_string())
    }

    /// Pop the most recent stash
    pub fn stash_pop(&self, worktree_path: &Path) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        logging::debug_event(
            "git_flow.stash.pop.started",
            "popping stash",
            serde_json::json!({"path": worktree_path.to_string_lossy()}),
        );

        let output = Command::new("git")
            .args(["stash", "pop"])
            .current_dir(worktree_path)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WorktreeError::GitCommandFailed(stderr.to_string()));
        }

        logging::debug_event(
            "git_flow.stash.pop.completed",
            "stash restored",
            serde_json::json!({}),
        );

        Ok(())
    }

    /// Get detailed uncommitted changes information
    ///
    /// Returns list of files with their status.
    pub fn get_uncommitted_info(&self, worktree_path: &Path) -> Result<UncommittedChangesInfo, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

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

        let status_output = String::from_utf8_lossy(&output.stdout);
        let files: Vec<UncommittedFile> = status_output
            .lines()
            .filter_map(|line| {
                let status = line.chars().next()?;
                let file_path = line[3..].to_string();
                Some(UncommittedFile {
                    path: file_path,
                    status: FileChangeStatus::from_git_status(status),
                })
            })
            .collect();

        // Calculate flags before moving files
        let has_staged = files.iter().any(|f| f.status.is_staged());
        let has_unstaged = files.iter().any(|f| f.status.is_unstaged());
        let has_untracked = files.iter().any(|f| f.status == FileChangeStatus::Untracked);

        Ok(UncommittedChangesInfo {
            files,
            has_staged,
            has_unstaged,
            has_untracked,
        })
    }

    /// Delete a merged branch
    ///
    /// Only deletes branches that have been merged to base.
    pub fn delete_merged_branch(&self, branch: &str, base_branch: &str) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();

        // Check if branch is merged
        let merge_check = self.run_git_command_internal(&[
            "branch",
            "--merged",
            base_branch,
            "--list",
            branch,
        ])?;

        if !merge_check.trim().contains(branch) {
            logging::warn_event(
                "git_flow.branch.delete.not_merged",
                "branch not merged, refusing to delete",
                serde_json::json!({"branch": branch, "base": base_branch}),
            );
            return Ok(false);
        }

        logging::debug_event(
            "git_flow.branch.delete.started",
            "deleting merged branch",
            serde_json::json!({"branch": branch}),
        );

        self.run_git_command_internal(&["branch", "-d", branch])?;

        logging::debug_event(
            "git_flow.branch.delete.completed",
            "branch deleted",
            serde_json::json!({"branch": branch}),
        );

        Ok(true)
    }
}

/// Result of a rebase operation
#[derive(Debug, Clone)]
pub enum RebaseResult {
    /// Rebase completed successfully
    Success { new_head: String },
    /// Rebase had conflicts (aborted)
    Conflict { message: String },
}


/// Information about uncommitted changes
#[derive(Debug, Clone)]
pub struct UncommittedChangesInfo {
    /// List of changed files
    pub files: Vec<UncommittedFile>,
    /// Has staged changes
    pub has_staged: bool,
    /// Has unstaged changes
    pub has_unstaged: bool,
    /// Has untracked files
    pub has_untracked: bool,
}

impl UncommittedChangesInfo {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.files.is_empty()
    }

    /// Get count of changed files
    pub fn count(&self) -> usize {
        self.files.len()
    }
}

/// Information about a single uncommitted file
#[derive(Debug, Clone)]
pub struct UncommittedFile {
    /// File path
    pub path: String,
    /// Change status
    pub status: FileChangeStatus,
}

/// Git file change status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeStatus {
    /// Staged for addition
    Added,
    /// Staged for modification
    ModifiedStaged,
    /// Modified but not staged
    ModifiedUnstaged,
    /// Staged for deletion
    DeletedStaged,
    /// Deleted but not staged
    DeletedUnstaged,
    /// Renamed
    Renamed,
    /// Copied
    Copied,
    /// Untracked file
    Untracked,
    /// Both modified (conflict)
    BothModified,
    /// Unknown status
    Unknown,
}

impl FileChangeStatus {
    /// Convert git status character to enum
    fn from_git_status(status: char) -> Self {
        match status {
            'A' => FileChangeStatus::Added,
            'M' => FileChangeStatus::ModifiedStaged,
            'm' => FileChangeStatus::ModifiedUnstaged,
            'D' => FileChangeStatus::DeletedStaged,
            'd' => FileChangeStatus::DeletedUnstaged,
            'R' => FileChangeStatus::Renamed,
            'C' => FileChangeStatus::Copied,
            '?' => FileChangeStatus::Untracked,
            'U' => FileChangeStatus::BothModified,
            _ => FileChangeStatus::Unknown,
        }
    }

    /// Check if this is a staged change
    pub fn is_staged(&self) -> bool {
        matches!(
            self,
            FileChangeStatus::Added
                | FileChangeStatus::ModifiedStaged
                | FileChangeStatus::DeletedStaged
                | FileChangeStatus::Renamed
                | FileChangeStatus::Copied
        )
    }

    /// Check if this is an unstaged change
    pub fn is_unstaged(&self) -> bool {
        matches!(
            self,
            FileChangeStatus::ModifiedUnstaged | FileChangeStatus::DeletedUnstaged
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize a git repo with explicit main branch
        Command::new("git")
            .args(["init", "-b", "main"])
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

        // Disable GPG signing for test repo
        Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to disable GPG signing");

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

        let info = manager
            .create_for_agent("agent_001", Some("task-123"))
            .unwrap();
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
        assert_eq!(
            worktrees[0].lock_reason,
            Some("work in progress".to_string())
        );
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

    #[test]
    fn parse_porcelain_empty_output() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let worktrees = manager.parse_porcelain_output("").unwrap();
        assert_eq!(worktrees.len(), 0);
    }

    #[test]
    fn parse_porcelain_malformed_missing_worktree_line() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        // Malformed: missing "worktree" line
        let output = "HEAD abc123\nbranch refs/heads/main\n";
        let worktrees = manager.parse_porcelain_output(output).unwrap();
        // Should skip malformed entry
        assert_eq!(worktrees.len(), 0);
    }

    #[test]
    fn create_duplicate_worktree_fails() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        // Create first worktree
        let options = WorktreeCreateOptions {
            path: PathBuf::new(),
            branch: Some("test-branch".to_string()),
            create_branch: true,
            base: None,
            lock_reason: None,
        };
        manager.create("duplicate-name", options.clone()).unwrap();

        // Try to create with same name - should fail
        let result = manager.create("duplicate-name", options);
        assert!(result.is_err());
    }

    #[test]
    fn remove_nonexistent_worktree_fails() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let result = manager.remove("nonexistent-worktree", false);
        assert!(result.is_err());
    }

    #[test]
    fn get_info_for_nonexistent_worktree_returns_error() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let result = manager.get_worktree_by_name("nonexistent-worktree");
        assert!(result.is_err());
    }

    #[test]
    fn worktree_info_path_exists_after_create() {
        let (_temp, repo_path) = create_test_repo();
        let config = WorktreeConfig::default();
        let manager = WorktreeManager::new(repo_path, config).unwrap();

        let info = manager
            .create_for_agent("test_agent", Some("task-123"))
            .unwrap();

        // The path should actually exist on disk
        assert!(info.path.exists());
        assert!(info.path.is_dir());

        // Cleanup
        manager.remove("test_agent", false).unwrap();
    }
}
