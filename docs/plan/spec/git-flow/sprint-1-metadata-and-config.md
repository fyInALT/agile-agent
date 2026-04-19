# Sprint 1: Task Metadata and Git Flow Configuration

## Sprint Overview

**Duration**: 1-2 days
**Goal**: Establish foundational infrastructure for Git Flow task preparation
**Priority**: P0 (Critical)

## Stories

### Story 1.1: GitFlowConfig Structure

**Description**: Create configuration structure for Git Flow settings.

**Acceptance Criteria**:
- [ ] `GitFlowConfig` struct defined in `core/src/git_flow_config.rs`
- [ ] Config includes: base_branch, branch_pattern, auto_sync, auto_stash, etc.
- [ ] Default configuration values are sensible
- [ ] Config can be loaded from YAML/JSON file
- [ ] Unit tests for config parsing

**Implementation Notes**:
```rust
pub struct GitFlowConfig {
    pub base_branch: String,
    pub branch_pattern: String,
    pub auto_sync_baseline: bool,
    pub auto_stash_changes: bool,
    pub auto_cleanup_merged: bool,
    pub stale_branch_days: u64,
    pub enforce_conventional_commits: bool,
    pub task_types: HashMap<String, String>,
}
```

### Story 1.2: TaskMetadata Structure

**Description**: Create structure for extracting and storing task metadata.

**Acceptance Criteria**:
- [ ] `TaskMetadata` struct defined in `decision/src/task_metadata.rs`
- [ ] Includes: task_id, branch_name, summary, task_type
- [ ] Branch name generation follows Git Flow conventions
- [ ] Task type classification (feature, bugfix, refactor, docs, test)
- [ ] Unit tests for branch name generation

**Implementation Notes**:
```rust
pub struct TaskMetadata {
    pub task_id: String,
    pub branch_name: String,
    pub summary: String,
    pub task_type: TaskType,
    pub priority: TaskPriority,
}

pub enum TaskType {
    Feature,
    Bugfix,
    Refactor,
    Docs,
    Test,
    Chore,
}
```

### Story 1.3: Branch Name Generator

**Description**: Implement branch name generation from task metadata.

**Acceptance Criteria**:
- [ ] Function `generate_branch_name(task_id, task_type, description)` 
- [ ] Sanitizes description for valid branch name characters
- [ ] Limits description length (max ~30 chars)
- [ ] Handles special characters and spaces
- [ ] Tests for various input scenarios

**Branch Naming Rules**:
- Lowercase alphanumeric and hyphens only
- Format: `<type>/<task-id>-<short-desc>`
- Maximum 50 characters total
- No consecutive hyphens

### Story 1.4: Task Type Classifier

**Description**: Classify task type from task description.

**Acceptance Criteria**:
- [ ] Function `classify_task_type(description)` 
- [ ] Uses keyword matching for classification
- [ ] Default to "feature" if no match
- [ ] Confidence score for classification
- [ ] Tests for classification accuracy

**Classification Keywords**:
```
feature: "add", "implement", "create", "new"
bugfix: "fix", "bug", "issue", "error", "resolve"
refactor: "refactor", "simplify", "optimize", "clean"
docs: "document", "readme", "doc", "update docs"
test: "test", "testing", "spec", "coverage"
```

## Dependencies

- None (foundational sprint)

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Branch name collision | Add collision detection |
| Invalid characters | Comprehensive sanitization |
| Config format issues | Validation and defaults |

## Testing Strategy

- Unit tests for all functions
- Integration tests with WorktreeManager
- Edge case testing (long names, special chars)

## Deliverables

1. `core/src/git_flow_config.rs` - Configuration module
2. `decision/src/task_metadata.rs` - Task metadata module  
3. Updated `core/src/lib.rs` and `decision/src/lib.rs` exports
4. Unit test coverage > 80%

---

**Sprint Status**: Planned
