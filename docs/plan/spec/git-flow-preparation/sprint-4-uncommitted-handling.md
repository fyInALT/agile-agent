# Sprint 4: Uncommitted Changes Handling

## Goal

Implement handling of uncommitted changes before task start, ensuring clean working state.

## Duration

2-3 days

## Stories

### Story 4.1: UncommittedChangesAnalysis

**Description**: Analyze uncommitted changes to determine appropriate action.

**Acceptance Criteria**:
- Classify changes by type (related to task, previous work, experimental)
- Determine if changes should be committed, stashed, or discarded
- Consider change value assessment

**Implementation**:
```rust
// In decision/src/uncommitted_handler.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncommittedAnalysis {
    /// Whether changes are related to current/previous task
    pub changes_context: ChangesContext,
    /// Whether changes appear valuable
    pub is_valuable: bool,
    /// Suggested action
    pub suggested_action: UncommittedAction,
    /// Reason for suggestion
    pub reason: String,
}

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

pub struct UncommittedAnalyzer;

impl UncommittedAnalyzer {
    pub fn analyze(git_state: &GitState, current_task_id: Option<&str>) -> UncommittedAnalysis {
        if git_state.uncommitted_files.is_empty() {
            return UncommittedAnalysis {
                changes_context: ChangesContext::Unknown,
                is_valuable: false,
                suggested_action: UncommittedAction::Commit, // No changes, no action needed
                reason: "No uncommitted changes".to_string(),
            };
        }
        
        // Heuristics for classification:
        // - If .gitignore patterns match heavily → Temporary
        // - If changes match files mentioned in task → CurrentTask
        // - If files are test/debug related → Temporary
        // - If changes have meaningful content → Valuable
        
        let changes_context = self.classify_context(git_state, current_task_id);
        let is_valuable = self.assess_value(git_state);
        let suggested_action = self.determine_action(changes_context, is_valuable);
        
        UncommittedAnalysis {
            changes_context,
            is_valuable,
            suggested_action,
            reason: self.explain_reason(changes_context, is_valuable),
        }
    }
    
    fn classify_context(&self, git_state: &GitState, task_id: Option<&str>) -> ChangesContext {
        // Classification logic based on file patterns
        // ...
    }
    
    fn assess_value(&self, git_state: &GitState) -> bool {
        // Value assessment: are changes meaningful or throwaway?
        // Check for debug prints, commented code, etc.
        // ...
    }
    
    fn determine_action(&self, context: ChangesContext, valuable: bool) -> UncommittedAction {
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
}
```

---

### Story 4.2: Commit Operation with Message Generation

**Description**: Implement commit operation with appropriate message generation.

**Acceptance Criteria**:
- Generate conventional commit message
- Handle WIP commits for incomplete work
- Skip sensitive files

**Implementation**:
```rust
pub struct CommitMessageGenerator;

impl CommitMessageGenerator {
    pub fn generate(task_meta: &TaskMeta, is_wip: bool) -> String {
        let type_prefix = task_meta.work_type.to_commit_prefix();
        let scope = ""; // Optional scope
        
        if is_wip {
            format!("{}(wip): {} [in progress]", type_prefix, task_meta.task_summary)
        } else {
            format!("{}: {}", type_prefix, task_meta.task_summary)
        }
    }
    
    pub fn generate_with_coauthor(message: &str) -> String {
        format!(
            "{}\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
            message
        )
    }
}

impl WorktreeManager {
    /// Commit changes with generated message
    pub fn commit_changes(
        &self,
        worktree_path: &Path,
        message: &str,
        agent_id: &str,
    ) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        // Check for sensitive files first
        if self.has_sensitive_files(worktree_path)? {
            return Err(WorktreeError::SensitiveFilesDetected);
        }
        
        // Stage all changes
        self.run_git_command_internal_in_worktree(worktree_path, &["add", "-A"])?;
        
        // Commit
        let full_message = CommitMessageGenerator::generate_with_coauthor(message);
        self.run_git_command_internal_in_worktree(worktree_path, 
            &["commit", "-m", &full_message])?;
        
        // Return new commit SHA
        let sha = self.get_head_commit(worktree_path).unwrap_or_default();
        Ok(sha)
    }
    
    fn has_sensitive_files(&self, worktree_path: &Path) -> Result<bool, WorktreeError> {
        // Check for .env, credentials, secrets files
        let sensitive_patterns = [".env", "credentials", "secrets", "*.pem", "*.key"];
        // ...
    }
}
```

---

### Story 4.3: Stash Operation

**Description**: Implement stash operation for temporary change storage.

**Acceptance Criteria**:
- Create named stash with description
- Stash can be retrieved later
- Include untracked files option

**Implementation**:
```rust
impl WorktreeManager {
    /// Stash changes with description
    pub fn stash_changes(
        &self,
        worktree_path: &Path,
        description: &str,
        include_untracked: bool,
    ) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        let mut args = vec!["stash", "push"];
        
        if include_untracked {
            args.push("--include-untracked");
        }
        
        args.push("-m");
        args.push(description);
        
        self.run_git_command_internal_in_worktree(worktree_path, &args)?;
        
        // Return stash reference
        Ok("stash@{0}".to_string())
    }
    
    /// List stashes
    pub fn list_stashes(&self, worktree_path: &Path) -> Result<Vec<StashInfo>, WorktreeError> {
        let output = self.run_git_command_internal_in_worktree(worktree_path, 
            &["stash", "list"])?;
        
        // Parse stash list output
        // ...
    }
    
    /// Pop stash
    pub fn pop_stash(&self, worktree_path: &Path, stash_ref: &str) -> Result<bool, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        let result = self.run_git_command_internal_in_worktree(worktree_path,
            &["stash", "pop", stash_ref]);
        
        match result {
            Ok(_) => Ok(true),
            Err(e) => {
                // Check for conflict on pop
                if self.has_uncommitted_changes(worktree_path)? {
                    Err(WorktreeError::StashPopConflict)
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StashInfo {
    pub reference: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}
```

---

### Story 4.4: UncommittedChangesSituation

**Description**: Create situation for uncommitted changes detection.

**Acceptance Criteria**:
- Registered as new situation type
- Provides analysis and action options
- Works with decision framework

**Implementation**:
```rust
// In decision/src/builtin_situations.rs
pub fn uncommitted_changes_detected() -> SituationType {
    SituationType::new("uncommitted_changes_detected")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncommittedChangesSituation {
    /// Git state with uncommitted files
    pub git_state: GitState,
    /// Analysis of the changes
    pub analysis: UncommittedAnalysis,
    /// Current task ID (if task switch is happening)
    pub pending_task_id: Option<String>,
}

impl DecisionSituation for UncommittedChangesSituation {
    fn situation_type(&self) -> SituationType {
        uncommitted_changes_detected()
    }
    
    fn requires_human(&self) -> bool {
        self.analysis.suggested_action == UncommittedAction::RequestHuman
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("commit_changes"),
            ActionType::new("stash_changes"),
            ActionType::new("discard_changes"),
            ActionType::new("request_human"),
        ]
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Uncommitted changes detected:\nFiles: {}\nContext: {}\nSuggested: {}",
            self.git_state.uncommitted_files.len(),
            self.analysis.changes_context,
            self.analysis.suggested_action,
        )
    }
}
```

---

### Story 4.5: HandleUncommittedAction

**Description**: Create action for handling uncommitted changes.

**Acceptance Criteria**:
- Executes commit/stash/discard based on decision
- Updates git state after action
- Returns appropriate result

**Implementation**:
```rust
// In decision/src/builtin_actions.rs
pub fn handle_uncommitted() -> ActionType {
    ActionType::new("handle_uncommitted")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleUncommittedAction {
    /// Action to take
    pub action: UncommittedAction,
    /// Commit message (if committing)
    pub commit_message: Option<String>,
    /// Stash description (if stashing)
    pub stash_description: Option<String>,
}

impl DecisionAction for HandleUncommittedAction {
    fn action_type(&self) -> ActionType {
        handle_uncommitted()
    }
    
    fn to_prompt_format(&self) -> String {
        match self.action {
            UncommittedAction::Commit => format!("Commit: {}", self.commit_message.unwrap_or_default()),
            UncommittedAction::Stash => format!("Stash: {}", self.stash_description.unwrap_or_default()),
            UncommittedAction::Discard => "Discard changes".to_string(),
            UncommittedAction::RequestHuman => "Request human decision".to_string(),
        }
    }
}
```

---

### Story 4.6: Unit Tests

**Description**: Comprehensive unit tests for uncommitted handling.

**Test Cases**:
```rust
#[test]
fn test_analyze_current_task_changes() {
    let git_state = GitState {
        uncommitted_files: vec![FileStatus { path: "src/auth.rs", status: FileChangeType::Modified }],
        ..
    };
    let analysis = UncommittedAnalyzer::analyze(&git_state, Some("auth-task"));
    assert_eq!(analysis.changes_context, ChangesContext::CurrentTask);
}

#[test]
fn test_commit_message_generation() {
    let task_meta = TaskMeta {
        work_type: WorkType::Feature,
        task_summary: "add user authentication",
        ..
    };
    let message = CommitMessageGenerator::generate(&task_meta, false);
    assert!(message.starts_with("feat:"));
}

#[test]
fn test_wip_commit_format() {
    let task_meta = TaskMeta { work_type: WorkType::Feature, task_summary: "auth", .. };
    let message = CommitMessageGenerator::generate(&task_meta, true);
    assert!(message.contains("[in progress]"));
}

#[test]
fn test_stash_creation() {
    let manager = create_test_manager();
    let result = manager.stash_changes(worktree_path, "wip: partial auth", true);
    assert!(result.is_ok());
}
```

---

## Integration Points

- `decision/src/uncommitted_handler.rs`: New module
- `decision/src/builtin_actions.rs`: Register actions
- `decision/src/builtin_situations.rs`: Register situation
- `core/src/worktree_manager.rs`: Add stash/commit methods

## Dependencies

- Sprint 1 (TaskMeta) - for commit message generation
- Sprint 2 (GitState) - for state analysis

## Risks

- Incorrect classification of changes (mitigate: improve heuristics, allow human override)
- Sensitive files committed (mitigate: explicit check before commit)

## Definition of Done

- [ ] UncommittedAnalyzer implemented
- [ ] CommitMessageGenerator implemented
- [ ] Stash operations added to WorktreeManager
- [ ] UncommittedChangesSituation defined
- [ ] HandleUncommittedAction defined
- [ ] Unit tests passing
- [ ] Code reviewed
