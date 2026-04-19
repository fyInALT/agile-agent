# Sprint 2: Git State Analysis

## Goal

Implement git state analysis to detect uncommitted changes, branch status, and worktree health before task start.

## Duration

2-3 days

## Stories

### Story 2.1: GitState Data Structure

**Description**: Define the GitState struct that holds analysis results.

**Acceptance Criteria**:
- GitState contains: current_branch, uncommitted_files, ahead_behind, has_conflicts
- GitState is serializable for decision context
- GitState can be constructed from git command outputs

**Implementation**:
```rust
// In decision/src/git_state.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitState {
    /// Current branch name
    pub current_branch: String,
    /// Whether there are uncommitted changes
    pub has_uncommitted: bool,
    /// List of uncommitted files with their status
    pub uncommitted_files: Vec<FileStatus>,
    /// Commits ahead of main/master
    pub commits_ahead: usize,
    /// Commits behind main/master
    pub commits_behind: usize,
    /// Whether there are merge/rebase conflicts
    pub has_conflicts: bool,
    /// Last commit SHA (short)
    pub last_commit_sha: Option<String>,
    /// Last commit message
    pub last_commit_message: Option<String>,
    /// Worktree is healthy (no lock, exists on disk)
    pub is_healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: FileChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChangeType {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
}
```

**Files**:
- Create: `decision/src/git_state.rs`
- Update: `decision/src/lib.rs` (add module)

---

### Story 2.2: Git Command Executor

**Description**: Implement safe git command execution for state queries.

**Acceptance Criteria**:
- Execute git commands with timeout protection
- Parse command outputs correctly
- Handle command failures gracefully
- Thread-safe execution

**Implementation**:
```rust
pub struct GitCommandExecutor {
    /// Default timeout for git commands
    timeout_ms: u64,
}

impl GitCommandExecutor {
    pub fn run_git_status(&self, worktree_path: &Path) -> Result<String, GitStateError> {
        self.execute(worktree_path, &["status", "--porcelain"])
    }
    
    pub fn run_git_branch_info(&self, worktree_path: &Path, base_branch: &str) -> Result<String, GitStateError> {
        self.execute(worktree_path, &["rev-list", "--left-right", "--count", 
            &format!("{}...HEAD", base_branch)])
    }
    
    pub fn run_git_log_last(&self, worktree_path: &Path) -> Result<String, GitStateError> {
        self.execute(worktree_path, &["log", "-1", "--format=%h %s"])
    }
    
    fn execute(&self, cwd: &Path, args: &[&str]) -> Result<String, GitStateError> {
        // Execute with timeout, return stdout or error
    }
}
```

**Security Considerations**:
- No shell interpolation of arguments
- Timeout to prevent hanging
- Validate paths before execution

---

### Story 2.3: Git State Parser

**Description**: Parse git command outputs into structured data.

**Acceptance Criteria**:
- Parse `git status --porcelain` output
- Parse `git rev-list --left-right --count` for ahead/behind
- Parse `git log` for commit info
- Handle edge cases (empty output, malformed output)

**Implementation**:
```rust
pub struct GitStateParser;

impl GitStateParser {
    pub fn parse_status(output: &str) -> Vec<FileStatus> {
        output.lines()
            .filter_map(|line| Self::parse_status_line(line))
            .collect()
    }
    
    fn parse_status_line(line: &str) -> Option<FileStatus> {
        // Porcelain format: XY path
        // X = staged status, Y = unstaged status
        // ' M' = modified unstaged
        // 'M ' = modified staged
        // '??' = untracked
        // ...
    }
    
    pub fn parse_ahead_behind(output: &str) -> (usize, usize) {
        // Format: "N\tM" where N = ahead, M = behind
    }
    
    pub fn parse_log(output: &str) -> Option<(String, String)> {
        // Format: "sha message"
    }
}
```

---

### Story 2.4: GitStateAnalyzer Component

**Description**: Create the main analyzer component that orchestrates analysis.

**Acceptance Criteria**:
- Runs all git commands needed
- Combines parsed results into GitState
- Handles worktree path validation
- Returns meaningful error for invalid worktrees

**Implementation**:
```rust
pub struct GitStateAnalyzer {
    executor: GitCommandExecutor,
    parser: GitStateParser,
    /// Base branch to compare against (main or master)
    base_branch: String,
}

impl GitStateAnalyzer {
    pub fn analyze(&self, worktree_path: &Path) -> Result<GitState, GitStateError> {
        // Validate worktree exists
        if !worktree_path.exists() {
            return Err(GitStateError::WorktreeNotFound);
        }
        
        // Run git status
        let status_output = self.executor.run_git_status(worktree_path)?;
        let uncommitted_files = self.parser.parse_status(&status_output);
        
        // Run ahead/behind check
        let ahead_behind = self.executor.run_git_branch_info(worktree_path, &self.base_branch)?;
        let (ahead, behind) = self.parser.parse_ahead_behind(&ahead_behind);
        
        // Run last commit info
        let log_output = self.executor.run_git_log_last(worktree_path)?;
        let (sha, message) = self.parser.parse_log(&log_output).unwrap_or((String::new(), String::new()));
        
        // Check for conflicts
        let has_conflicts = self.check_for_conflicts(worktree_path)?;
        
        Ok(GitState {
            current_branch: self.get_current_branch(worktree_path)?,
            has_uncommitted: !uncommitted_files.is_empty(),
            uncommitted_files,
            commits_ahead: ahead,
            commits_behind: behind,
            has_conflicts,
            last_commit_sha: Some(sha),
            last_commit_message: Some(message),
            is_healthy: true,
        })
    }
    
    fn check_for_conflicts(&self, worktree_path: &Path) -> Result<bool, GitStateError> {
        // Check for files with conflict markers or unmerged status
    }
}
```

---

### Story 2.5: Integration with WorktreeManager

**Description**: Integrate GitStateAnalyzer with existing WorktreeManager.

**Acceptance Criteria**:
- Use WorktreeManager for worktree path resolution
- Extend WorktreeManager with analysis methods
- Don't duplicate existing functionality

**Implementation**:
```rust
// In core/src/worktree_manager.rs - add helper methods
impl WorktreeManager {
    /// Get git state for a specific worktree
    pub fn get_git_state(&self, worktree_path: &Path) -> Result<GitState, WorktreeError> {
        // Delegate to GitStateAnalyzer
    }
}
```

**Note**: Consider keeping GitStateAnalyzer in decision crate and calling it from agent_pool.

---

### Story 2.6: Unit Tests

**Description**: Comprehensive unit tests for all components.

**Acceptance Criteria**:
- Test status parsing for various outputs
- Test ahead/behind parsing
- Test log parsing
- Test integration with mock git outputs

**Test Cases**:
```rust
#[test]
fn test_parse_status_modified() {
    let output = " M src/main.rs\nM  src/lib.rs\n?? test.txt";
    let files = GitStateParser::parse_status(output);
    assert_eq!(files.len(), 3);
    assert_eq!(files[0].status, FileChangeType::Modified);
    assert!(files[0].path.ends_with("staged")); // Check staged vs unstaged
}

#[test]
fn test_parse_ahead_behind() {
    let output = "3\t5";
    let (ahead, behind) = GitStateParser::parse_ahead_behind(output);
    assert_eq!(ahead, 3);
    assert_eq!(behind, 5);
}

#[test]
fn test_empty_status() {
    let output = "";
    let files = GitStateParser::parse_status(output);
    assert!(files.is_empty());
}
```

---

## Integration Points

- `decision/src/lib.rs`: Add `git_state` module
- `core/src/worktree_manager.rs`: May add helper methods

## Dependencies

- Sprint 1 (TaskMeta) - not required for this sprint
- Existing `WorktreeManager` infrastructure

## Risks

- Git command failures on different platforms (mitigate: test on Linux, document assumptions)
- Timeout handling for slow git operations (mitigate: configurable timeout)

## Definition of Done

- [ ] GitState struct defined and documented
- [ ] GitCommandExecutor implemented with tests
- [ ] GitStateParser implemented with tests
- [ ] GitStateAnalyzer implemented with tests
- [ ] Module added to lib.rs
- [ ] All tests passing
- [ ] Code reviewed
