# Sprint 7: Task Completion Git Workflow

## Goal

Handle git workflow when task completes: final commit verification, PR preparation, and branch cleanup.

## Duration

2-3 days

## Stories

### Story 7.1: TaskCompletionGitState

**Description**: Define git state specific to task completion.

**Acceptance Criteria**:
- Extends GitState with completion-specific info
- Tracks all commits made during task
- Identifies ready-for-merge status

**Implementation**:
```rust
// In decision/src/task_completion.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletionGitState {
    /// Base git state
    pub git_state: GitState,
    /// Commits made during this task
    pub task_commits: Vec<CommitInfo>,
    /// Total commits count
    pub commit_count: usize,
    /// Whether branch is ready for merge/PR
    pub ready_for_merge: bool,
    /// PR title suggestion
    pub suggested_pr_title: Option<String>,
    /// PR description template
    pub pr_description_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub files_changed: usize,
}
```

---

### Story 7.2: FinalCommitVerification

**Description**: Verify all changes are committed before marking task complete.

**Acceptance Criteria**:
- Check for any remaining uncommitted files
- If found, trigger commit action
- Ensure commit message is appropriate

**Implementation**:
```rust
pub struct FinalCommitVerifier;

impl FinalCommitVerifier {
    pub fn verify(&self, git_state: &GitState) -> FinalCommitResult {
        if git_state.has_uncommitted {
            // Uncommitted changes remain
            return FinalCommitResult::NeedsCommit {
                files: git_state.uncommitted_files.clone(),
                suggested_message: self.generate_final_commit_message(git_state),
            };
        }
        
        // All changes committed
        FinalCommitResult::Verified {
            commit_count: self.count_task_commits(),
        }
    }
    
    fn generate_final_commit_message(&self, git_state: &GitState) -> String {
        // Generate based on remaining changes
        "chore: final cleanup for task completion".to_string()
    }
}

#[derive(Debug, Clone)]
pub enum FinalCommitResult {
    Verified { commit_count: usize },
    NeedsCommit { files: Vec<FileStatus>, suggested_message: String },
}
```

---

### Story 7.3: PR Title Generator

**Description**: Generate PR title from task commits.

**Acceptance Criteria**:
- Aggregate commits into coherent title
- Follow conventional format
- Max 72 characters

**Implementation**:
```rust
pub struct PrTitleGenerator;

impl PrTitleGenerator {
    pub fn generate(&self, commits: &[CommitInfo], task_meta: &TaskMeta) -> String {
        // If single commit, use its message
        if commits.len() == 1 {
            return self.format_single_commit(&commits[0]);
        }
        
        // Multiple commits - aggregate
        let work_type = self.detect_dominant_type(commits);
        let scope = self.detect_scope(commits);
        let summary = task_meta.task_summary.clone();
        
        if scope.is_empty() {
            format!("{}: {}", work_type.to_commit_prefix(), summary)
        } else {
            format!("{}({}): {}", work_type.to_commit_prefix(), scope, summary)
        }
    }
    
    fn detect_dominant_type(&self, commits: &[CommitInfo]) -> WorkType {
        // Count commit types from messages
        let mut counts = HashMap::new();
        for commit in commits {
            let msg_type = self.parse_commit_type(&commit.message);
            *counts.entry(msg_type).or_insert(0) += 1;
        }
        
        counts.into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t)
            .unwrap_or(WorkType::Feature)
    }
}
```

---

### Story 7.4: PR Description Template

**Description**: Generate PR description from commits and task context.

**Acceptance Criteria**:
- Lists all changes
- Includes testing information
- Follows PR template format

**Implementation**:
```rust
impl PrDescriptionGenerator {
    pub fn generate(&self, commits: &[CommitInfo], task_meta: &TaskMeta) -> String {
        let mut description = String::new();
        
        // Summary section
        description.push_str("## Summary\n\n");
        description.push_str(&task_meta.task_summary);
        description.push_str("\n\n");
        
        // Changes section
        description.push_str("## Changes\n\n");
        for commit in commits {
            description.push_str(&format!("- {}\n", commit.message));
        }
        description.push_str("\n");
        
        // Test plan section (placeholder)
        description.push_str("## Test Plan\n\n");
        description.push_str("- [ ] Tests added/updated\n");
        description.push_str("- [ ] Manual testing completed\n");
        description.push_str("\n");
        
        // AI attribution
        description.push_str("\n🤖 Generated with Claude Code\n");
        
        description
    }
}
```

---

### Story 7.5: TaskCompletionReady Situation

**Description**: Create situation for task completion git workflow.

**Acceptance Criteria**:
- Registered as new situation type
- Contains completion git state
- Determines merge readiness

**Implementation**:
```rust
pub fn task_completion_ready() -> SituationType {
    SituationType::new("task_completion_ready")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletionReadySituation {
    /// Task metadata
    pub task_meta: TaskMeta,
    /// Completion git state
    pub git_state: TaskCompletionGitState,
    /// Agent ID
    pub agent_id: String,
}

impl DecisionSituation for TaskCompletionReadySituation {
    fn situation_type(&self) -> SituationType {
        task_completion_ready()
    }
    
    fn requires_human(&self) -> bool {
        // PR creation typically requires human review
        true
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Medium
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("prepare_pr"),
            ActionType::new("final_commit"),
            ActionType::new("request_human"),
            ActionType::new("continue"),  // Skip PR for now
        ]
    }
}
```

---

### Story 7.6: PreparePRAction

**Description**: Create action to prepare PR.

**Acceptance Criteria**:
- Generates PR title and description
- Creates PR draft via gh CLI (if available)
- Returns PR URL or draft info

**Implementation**:
```rust
pub fn prepare_pr() -> ActionType {
    ActionType::new("prepare_pr")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparePrAction {
    /// PR title
    pub title: String,
    /// PR description
    pub description: String,
    /// Base branch to merge into
    pub base_branch: String,
    /// Whether to create draft PR
    pub as_draft: bool,
}

impl DecisionAction for PreparePrAction {
    fn action_type(&self) -> ActionType {
        prepare_pr()
    }
    
    fn to_prompt_format(&self) -> String {
        format!(
            "PreparePR: {}\nBase: {}\nDraft: {}",
            self.title,
            self.base_branch,
            self.as_draft
        )
    }
}
```

---

### Story 7.7: Branch Cleanup After Completion

**Description**: Clean up feature branch after task completion/merge.

**Acceptance Criteria**:
- Archive or delete branch
- Reset worktree for next task
- Preserve commit history if needed

**Implementation**:
```rust
impl AgentPool {
    /// Cleanup after task completion
    pub fn cleanup_completed_task(&mut self, agent_id: &AgentId) -> Result<(), AgentPoolError> {
        let slot = self.get_slot_by_id(agent_id)?;
        let worktree_path = slot.cwd();
        let branch = slot.branch();
        
        // Option 1: Archive branch (keep for reference)
        // Option 2: Delete branch (after merge)
        
        // Reset worktree to base branch for next task
        self.run_git_in_worktree(&worktree_path, &["checkout", &self.config.base_branch])?;
        
        // Delete feature branch (optional)
        self.run_git_in_worktree(&worktree_path, &["branch", "-D", &branch])?;
        
        // Clear worktree state
        slot.clear_task_assignment();
        
        Ok(())
    }
}
```

---

### Story 7.8: Integration with ClaimsCompletion Flow

**Description**: Integrate completion git workflow into existing completion flow.

**Acceptance Criteria**:
- Trigger after confirm_completion action
- Check git state before confirming
- Add git steps to completion decision

**Implementation**:
```rust
// In core/src/agent_pool.rs - extend confirm_completion handling
fn execute_confirm_completion(&mut self, agent_id: &AgentId) -> Result<(), Error> {
    // Step 1: Verify all changes committed
    let git_state = self.analyze_git_state(agent_id)?;
    if git_state.has_uncommitted {
        // Need to commit first
        self.trigger_final_commit_decision(agent_id)?;
        return Ok(());  // Don't confirm yet
    }
    
    // Step 2: Check branch status
    if git_state.commits_ahead > 0 {
        // Branch has commits to merge
        // Trigger PR preparation
        self.trigger_pr_preparation(agent_id)?;
    }
    
    // Step 3: Confirm completion
    let slot = self.get_slot_mut_by_id(agent_id)?;
    slot.transition_to(AgentSlotStatus::idle());
    slot.clear_task_assignment();
    
    Ok(())
}
```

---

### Story 7.9: Unit Tests

**Description**: Comprehensive tests for completion workflow.

**Test Cases**:
```rust
#[test]
fn test_final_commit_verification() {
    let verifier = FinalCommitVerifier::default();
    
    // Clean state
    let clean_state = GitState { has_uncommitted: false, .. };
    assert!(matches!(verifier.verify(&clean_state), FinalCommitResult::Verified { .. }));
    
    // Uncommitted state
    let dirty_state = GitState { 
        has_uncommitted: true, 
        uncommitted_files: vec![FileStatus::new("src/main.rs", Modified)],
        .. 
    };
    assert!(matches!(verifier.verify(&dirty_state), FinalCommitResult::NeedsCommit { .. }));
}

#[test]
fn test_pr_title_generation() {
    let gen = PrTitleGenerator::default();
    
    let commits = vec![
        CommitInfo { message: "feat(auth): add login".into(), .. },
        CommitInfo { message: "test(auth): add login tests".into(), .. },
    ];
    let meta = TaskMeta { task_summary: "Add login feature".into(), .. };
    
    let title = gen.generate(&commits, &meta);
    assert!(title.starts_with("feat(auth):"));
}

#[test]
fn test_pr_description_format() {
    let gen = PrDescriptionGenerator::default();
    let commits = vec![CommitInfo { message: "test commit".into(), .. }];
    let meta = TaskMeta { task_summary: "Test".into(), .. };
    
    let desc = gen.generate(&commits, &meta);
    assert!(desc.contains("## Summary"));
    assert!(desc.contains("## Changes"));
    assert!(desc.contains("## Test Plan"));
}
```

---

## Integration Points

- `decision/src/task_completion.rs`: New module
- `decision/src/builtin_actions.rs`: Register actions
- `decision/src/builtin_situations.rs`: Register situation
- `core/src/agent_pool.rs`: Execute actions and cleanup

## Dependencies

- Sprint 1-5 (P0 features)
- Sprint 6 (Commit hygiene - optional)

## Risks

- PR creation requires gh CLI (mitigate: fallback to manual PR prep)
- Breaking existing completion flow (mitigate: careful integration)

## Definition of Done

- [ ] TaskCompletionGitState defined
- [ ] FinalCommitVerifier implemented
- [ ] PR title/description generators implemented
- [ ] TaskCompletionReadySituation registered
- [ ] PreparePRAction registered
- [ ] Branch cleanup implemented
- [ ] Integration with ClaimsCompletion
- [ ] Unit tests passing
- [ ] Code reviewed
