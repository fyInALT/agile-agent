# Sprint 1: Task Metadata Extraction and Branch Naming

## Sprint Goal
Implement task metadata extraction logic to generate meaningful Git Flow branch names from task descriptions.

## Duration
2-3 days

## Stories

### Story 1.1: Create GitFlowConfig Structure
**Description**: Define configuration structure for Git Flow settings.

**Tasks**:
- [ ] Create `GitFlowConfig` struct in `core/src/git_flow_config.rs`
- [ ] Add default configuration values
- [ ] Integrate with existing `WorktreeConfig`

**Acceptance Criteria**:
- Config struct defines base_branch, branch_pattern, auto_sync options
- Default values match common Git Flow conventions
- Config can be loaded/saved if needed

### Story 1.2: Implement TaskType Classification
**Description**: Classify tasks into types (feature, bugfix, refactor, docs, test).

**Tasks**:
- [ ] Create `TaskType` enum in `decision/src/task_metadata.rs`
- [ ] Implement classification logic from task description keywords
- [ ] Add unit tests for classification

**Acceptance Criteria**:
- TaskType enum covers: Feature, Bugfix, Refactor, Docs, Test
- Classification correctly identifies type from keywords
- 80%+ accuracy on test cases

### Story 1.3: Generate Branch Name from Task
**Description**: Generate Git Flow compliant branch name from task metadata.

**Tasks**:
- [ ] Create `BranchNameGenerator` in `decision/src/git_operations.rs`
- [ ] Implement naming pattern: `<type>/<task-id>-<short-desc>`
- [ ] Sanitize description for valid branch names
- [ ] Add unit tests

**Acceptance Criteria**:
- Branch names follow Git Flow convention
- Invalid characters are sanitized
- Names are human-readable and concise

### Story 1.4: Create TaskMetadataExtractor
**Description**: Combine all metadata extraction into cohesive module.

**Tasks**:
- [ ] Create `TaskMetadata` struct containing: branch_name, summary, task_type
- [ ] Implement `TaskMetadataExtractor::extract(task_description)` 
- [ ] Add integration tests

**Acceptance Criteria**:
- Single function call extracts all metadata
- Output includes branch_name, summary, task_type
- Works with various task description formats

## Technical Notes

- Leverage existing `decision/src/task.rs` Task entity
- Keep extraction logic simple (keyword-based, not LLM)
- Ensure branch names are valid Git identifiers

## Dependencies

- None (foundation sprint)

## Risks

- Keyword-based classification may have edge cases
- Branch name sanitization needs careful handling

---

**Sprint Status**: Ready for Development
