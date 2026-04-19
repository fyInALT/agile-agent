# Sprint 1: Task Meta Extraction

## Goal

Implement task metadata extraction to generate meaningful branch names and task summaries before a work agent begins development.

## Duration

2-3 days

## Stories

### Story 1.1: TaskMeta Data Structure

**Description**: Define the TaskMeta struct that holds extracted task information.

**Acceptance Criteria**:
- TaskMeta contains: branch_name, task_summary, work_type, task_id_ref
- TaskMeta is serializable for persistence
- TaskMeta can be constructed from task description

**Implementation**:
```rust
// In decision/src/task_meta.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta {
    /// Generated branch name (e.g., "feature/add-user-auth")
    pub branch_name: String,
    /// Brief task summary for commit messages (max 72 chars)
    pub task_summary: String,
    /// Work type classification
    pub work_type: WorkType,
    /// Optional task ID reference from backlog
    pub task_id_ref: Option<String>,
    /// Slugified version of task description
    pub slug: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkType {
    Feature,
    Fix,
    Refactor,
    Docs,
    Test,
    Chore,
}
```

**Files**:
- Create: `decision/src/task_meta.rs`
- Update: `decision/src/lib.rs` (add module)

---

### Story 1.2: Branch Name Generator

**Description**: Implement logic to generate valid git branch names from task descriptions.

**Acceptance Criteria**:
- Converts task description to valid branch name format
- Sanitizes special characters and spaces
- Enforces max length (30 chars for slug)
- Handles collision detection with unique suffix

**Implementation**:
```rust
// In decision/src/task_meta.rs
pub struct BranchNameGenerator {
    /// Branch type prefix
    prefix: String,
    /// Maximum slug length
    max_slug_length: usize,
    /// Collision counter for unique suffixes
    collision_counter: HashMap<String, usize>,
}

impl BranchNameGenerator {
    pub fn generate(&self, task_description: &str, work_type: WorkType) -> String {
        let slug = self.slugify(task_description);
        let prefix = work_type.to_branch_prefix();
        format!("{}/{}", prefix, slug)
    }
    
    fn slugify(&self, text: &str) -> String {
        // Convert to lowercase
        // Replace spaces with hyphens
        // Remove special characters
        // Truncate to max length
        // ...
    }
}
```

**Edge Cases**:
- Empty task description → use timestamp-based slug
- Very long descriptions → truncate intelligently
- Multiple tasks with same name → append `-2`, `-3`, etc.

---

### Story 1.3: Work Type Classifier

**Description**: Classify the work type from task description.

**Acceptance Criteria**:
- Correctly classifies: feature, fix, refactor, docs, test, chore
- Uses keyword matching for simple classification
- Returns Feature as default for ambiguous cases

**Implementation**:
```rust
impl WorkTypeClassifier {
    pub fn classify(description: &str) -> WorkType {
        let lower = description.to_lowercase();
        
        // Keyword-based classification
        if lower.contains("fix") || lower.contains("bug") || lower.contains("error") {
            return WorkType::Fix;
        }
        if lower.contains("refactor") || lower.contains("restructure") {
            return WorkType::Refactor;
        }
        if lower.contains("document") || lower.contains("readme") || lower.contains("doc") {
            return WorkType::Docs;
        }
        if lower.contains("test") || lower.contains("spec") {
            return WorkType::Test;
        }
        if lower.contains("config") || lower.contains("ci") || lower.contains("build") {
            return WorkType::Chore;
        }
        
        // Default to feature for new functionality
        WorkType::Feature
    }
}
```

---

### Story 1.4: TaskMetaExtractor Component

**Description**: Create the main extractor component that orchestrates extraction.

**Acceptance Criteria**:
- Integrates branch name generator and work type classifier
- Returns complete TaskMeta from task description
- Handles optional task ID reference

**Implementation**:
```rust
pub struct TaskMetaExtractor {
    branch_generator: BranchNameGenerator,
    work_type_classifier: WorkTypeClassifier,
}

impl TaskMetaExtractor {
    pub fn extract(&self, task_description: &str, task_id: Option<&str>) -> TaskMeta {
        let work_type = self.work_type_classifier.classify(task_description);
        let branch_name = self.branch_generator.generate(task_description, work_type);
        let task_summary = self.create_summary(task_description);
        let slug = self.branch_generator.slugify(task_description);
        
        TaskMeta {
            branch_name,
            task_summary,
            work_type,
            task_id_ref: task_id.map(|s| s.to_string()),
            slug,
        }
    }
}
```

---

### Story 1.5: Unit Tests

**Description**: Comprehensive unit tests for all components.

**Acceptance Criteria**:
- Test branch name generation for various inputs
- Test work type classification accuracy
- Test collision handling
- Test edge cases (empty, long, special chars)

**Test Cases**:
```rust
#[test]
fn test_branch_name_generation() {
    let gen = BranchNameGenerator::default();
    assert_eq!(gen.generate("Add user authentication", WorkType::Feature), 
               "feature/add-user-authentication");
}

#[test]
fn test_slugify_truncation() {
    let gen = BranchNameGenerator::default();
    let long_desc = "This is a very long task description that should be truncated";
    let slug = gen.slugify(long_desc);
    assert!(slug.len() <= 30);
}

#[test]
fn test_work_type_classification() {
    assert_eq!(WorkTypeClassifier::classify("Fix login timeout bug"), WorkType::Fix);
    assert_eq!(WorkTypeClassifier::classify("Add new API endpoint"), WorkType::Feature);
}
```

---

## Integration Points

- `decision/src/lib.rs`: Add `task_meta` module
- Future: Integration with backlog task system

## Dependencies

- No external dependencies
- Uses existing serde for serialization

## Risks

- Classification accuracy depends on keyword matching (mitigate: add more keywords, consider LLM fallback in future)

## Definition of Done

- [ ] TaskMeta struct defined and documented
- [ ] BranchNameGenerator implemented with tests
- [ ] WorkTypeClassifier implemented with tests
- [ ] TaskMetaExtractor implemented with tests
- [ ] Module added to lib.rs
- [ ] All tests passing
- [ ] Code reviewed
