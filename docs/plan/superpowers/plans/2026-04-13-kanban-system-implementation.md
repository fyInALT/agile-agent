# Kanban System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the core kanban system with domain types, file-based repository, service layer, and event bus, ready for multi-agent integration.

**Architecture:** Clean architecture with separate domain/repository/service layers. Domain types are pure Rust structs with no external dependencies. Repository trait enables future database backend. Service layer contains business logic. Event bus decouples updates from consumers.

**Tech Stack:** Rust (no new dependencies beyond existing workspace), serde for JSON, git2 for future Git operations.

---

## File Structure

```
core/src/
├── kanban/
│   ├── mod.rs              # Module exports
│   ├── error.rs            # KanbanError enum
│   ├── domain.rs           # KanbanElement, Status, Priority, ElementId, StatusHistoryEntry
│   ├── repository.rs       # KanbanElementRepository trait
│   ├── file_repository.rs   # FileKanbanRepository implementation
│   ├── service.rs          # KanbanService
│   ├── events.rs           # KanbanEvent, KanbanEventBus, KanbanEventSubscriber
│   └── git_ops.rs          # GitOperations (basic)
└── lib.rs                  # Add: pub mod kanban;

core/tests/
└── kanban/
    ├── domain.rs           # Domain type tests
    ├── repository.rs       # Repository tests
    └── service.rs          # Service tests
```

---

## Task 1: Create kanban module scaffold and error types

**Files:**
- Create: `core/src/kanban/mod.rs`
- Create: `core/src/kanban/error.rs`

- [ ] **Step 1: Create core/src/kanban/mod.rs**

```rust
pub mod error;
pub mod domain;
pub mod repository;
pub mod file_repository;
pub mod service;
pub mod events;
pub mod git_ops;

pub use error::KanbanError;
pub use domain::{KanbanElement, Status, Priority, ElementId, ElementType, StatusHistoryEntry};
pub use repository::KanbanElementRepository;
pub use file_repository::FileKanbanRepository;
pub use service::KanbanService;
pub use events::{KanbanEvent, KanbanEventBus, KanbanEventSubscriber};
```

- [ ] **Step 2: Create core/src/kanban/error.rs**

```rust
use crate::domain::ElementId;
use crate::domain::Status;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KanbanError {
    #[error("element not found: {0}")]
    NotFound(ElementId),

    #[error("invalid status transition from {current:?} to {requested:?}")]
    InvalidStatusTransition { current: Status, requested: Status },

    #[error("dependencies not met for {element}: blocked by {blockers:?}")]
    DependenciesNotMet { element: ElementId, blockers: Vec<ElementId> },

    #[error("dangling dependency: {element} references {dependency} which does not exist")]
    DanglingDependency { element: ElementId, dependency: ElementId },

    #[error("permission denied: {element} is assigned to {required}")]
    PermissionDenied { element: ElementId, required: String },

    #[error("invalid tip target: {0} is not a task")]
    InvalidTipTarget(ElementId),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("id parse error: {0}")]
    IdParse(String),
}
```

- [ ] **Step 3: Add module to lib.rs**

Modify `core/src/lib.rs` to add:
```rust
pub mod kanban;
```

- [ ] **Step 4: Run tests to verify module compiles**

Run: `cargo build -p agent-core`
Expected: BUILD SUCCESS

- [ ] **Step 5: Commit**

```bash
git add core/src/kanban/ core/src/lib.rs
git commit -m "feat(kanban): create kanban module scaffold"
```

---

## Task 2: Implement domain types (Status, Priority, ElementId, StatusHistoryEntry)

**Files:**
- Create: `core/src/kanban/domain.rs`
- Create: `core/tests/kanban/domain.rs`

- [ ] **Step 1: Write failing test for Status**

Create `core/tests/kanban/domain.rs`:

```rust
use agent_core::kanban::Status;

#[test]
fn test_status_valid_transitions() {
    assert!(Status::Plan.can_transition_to(Status::Backlog));
    assert!(Status::Backlog.can_transition_to(Status::Ready));
    assert!(Status::Backlog.can_transition_to(Status::Blocked));
    assert!(Status::InProgress.can_transition_to(Status::Done));
    assert!(Status::InProgress.can_transition_to(Status::Blocked));
    assert!(Status::Done.can_transition_to(Status::Verified));
}

#[test]
fn test_status_invalid_transitions() {
    assert!(!Status::Plan.can_transition_to(Status::InProgress));
    assert!(!Status::Verified.can_transition_to(Status::Backlog));
}

#[test]
fn test_status_lead_time_calculation() {
    // Status history entries can be used to calculate time in each status
    let in_progress_time = Status::InProgress.lead_time_estimate();
    assert!(in_progress_time.is_none()); // No estimate without history
}
```

Run: `cargo test -p agent-core --test kanban domain::test_status`
Expected: FAIL - function not found

- [ ] **Step 2: Write Status enum implementation**

Add to `core/src/kanban/domain.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Plan,
    Backlog,
    Blocked,
    Ready,
    Todo,
    InProgress,
    Done,
    Verified,
}

impl Status {
    pub fn valid_transitions(&self) -> Vec<Status> {
        match self {
            Status::Plan => vec![Status::Backlog],
            Status::Backlog => vec![Status::Blocked, Status::Ready, Status::Todo, Status::Plan],
            Status::Blocked => vec![Status::Ready, Status::Todo, Status::Backlog],
            Status::Ready => vec![Status::InProgress, Status::Todo, Status::Backlog],
            Status::Todo => vec![Status::Ready, Status::InProgress, Status::Backlog],
            Status::InProgress => vec![Status::Done, Status::Blocked, Status::Backlog],
            Status::Done => vec![Status::Verified, Status::InProgress],
            Status::Verified => vec![], // Terminal state
        }
    }

    pub fn can_transition_to(&self, target: Status) -> bool {
        self.valid_transitions().contains(&target)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Verified)
    }
}
```

- [ ] **Step 3: Write Priority enum**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "critical" => Some(Priority::Critical),
            "high" => Some(Priority::High),
            "medium" => Some(Priority::Medium),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Write ElementId type**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(String);

impl ElementId {
    pub fn new(type_: ElementType, number: u32) -> Self {
        Self(format!("{}-{:03}", type_.as_str(), number))
    }

    pub fn parse(s: &str) -> Result<Self, crate::kanban::KanbanError> {
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(crate::kanban::KanbanError::IdParse(s.to_string()));
        }
        let type_str = parts[0];
        let num_str = parts[1];

        // Validate type is known
        match type_str {
            "sprint" | "story" | "task" | "idea" | "issue" | "tip" => {}
            _ => return Err(crate::kanban::KanbanError::IdParse(s.to_string())),
        }

        // Validate number
        if num_str.parse::<u32>().is_err() {
            return Err(crate::kanban::KanbanError::IdParse(s.to_string()));
        }

        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

- [ ] **Step 5: Write ElementType enum**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Sprint,
    Story,
    Task,
    Idea,
    Issue,
    Tips,
}

impl ElementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementType::Sprint => "sprint",
            ElementType::Story => "story",
            ElementType::Task => "task",
            ElementType::Idea => "idea",
            ElementType::Issue => "issue",
            ElementType::Tips => "tip",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sprint" => Some(ElementType::Sprint),
            "story" => Some(ElementType::Story),
            "task" => Some(ElementType::Task),
            "idea" => Some(ElementType::Idea),
            "issue" => Some(ElementType::Issue),
            "tip" | "tips" => Some(ElementType::Tips),
            _ => None,
        }
    }
}
```

- [ ] **Step 6: Write StatusHistoryEntry struct**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusHistoryEntry {
    pub status: Status,
    pub entered_at: chrono::DateTime<chrono::Utc>,
}

impl StatusHistoryEntry {
    pub fn new(status: Status) -> Self {
        Self {
            status,
            entered_at: chrono::Utc::now(),
        }
    }
}
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p agent-core --test kanban domain::`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add core/src/kanban/domain.rs core/tests/kanban/domain.rs
git commit -m "feat(kanban): add domain types - Status, Priority, ElementId, ElementType, StatusHistoryEntry"
```

---

## Task 3: Implement KanbanElement enum (Sprint, Story, Task, Idea, Issue, Tips)

**Files:**
- Modify: `core/src/kanban/domain.rs`

- [ ] **Step 1: Write KanbanElement test**

Create `core/tests/kanban/element.rs`:

```rust
use agent_core::kanban::{KanbanElement, ElementType, Status, Priority};
use chrono::Utc;

#[test]
fn test_sprint_creation() {
    let sprint = KanbanElement::new_sprint("Sprint 1", "Complete auth module");
    assert!(matches!(sprint, KanbanElement::Sprint(_)));
}

#[test]
fn test_task_can_transition() {
    let mut task = KanbanElement::new_task("Implement login", "story-001");
    assert!(task.can_transition_to(Status::Ready));
    task.transition(Status::Ready).expect("transition should work");
    assert!(task.can_transition_to(Status::InProgress));
}

#[test]
fn test_tips_has_target_task() {
    let tip = KanbanElement::new_tips(
        "Remember to validate input",
        "task-001",
        "agent-alpha",
    );
    match &tip {
        KanbanElement::Tips(t) => {
            assert_eq!(t.target_task.as_str(), "task-001");
            assert_eq!(t.agent_id.as_str(), "agent-alpha");
        }
        _ => panic!("expected Tips variant"),
    }
}
```

Run: `cargo test -p agent-core --test kanban element::`
Expected: FAIL - function not found

- [ ] **Step 2: Write base element struct with common fields**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseElement {
    pub id: ElementId,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub status: Status,
    #[serde(default)]
    pub dependencies: Vec<ElementId>,
    #[serde(default)]
    pub references: Vec<ElementId>,
    pub parent: Option<ElementId>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub priority: Option<Priority>,
    pub assignee: Option<String>,
    pub effort: Option<u32>,
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub status_history: Vec<StatusHistoryEntry>,
}

impl BaseElement {
    pub fn new(type_: ElementType, title: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: ElementId::new(type_, 0), // Temporary, will be replaced by repository
            title: title.to_string(),
            content: String::new(),
            keywords: Vec::new(),
            status: Status::Plan,
            dependencies: Vec::new(),
            references: Vec::new(),
            parent: None,
            created_at: now,
            updated_at: now,
            priority: None,
            assignee: None,
            effort: None,
            blocked_reason: None,
            tags: Vec::new(),
            status_history: vec![StatusHistoryEntry::new(Status::Plan)],
        }
    }

    pub fn can_transition_to(&self, new_status: Status) -> bool {
        self.status.can_transition_to(new_status)
    }

    pub fn transition(&mut self, new_status: Status) -> Result<(), KanbanError> {
        if !self.can_transition_to(new_status) {
            return Err(KanbanError::InvalidStatusTransition {
                current: self.status,
                requested: new_status,
            });
        }
        self.status_history.push(StatusHistoryEntry::new(new_status));
        self.status = new_status;
        self.updated_at = chrono::Utc::now();
        Ok(())
    }
}
```

- [ ] **Step 3: Write element variants (Sprint, Story, Task, Idea, Issue, Tips)**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    #[serde(flatten)]
    pub base: BaseElement,
    pub goal: Option<String>,
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub active: bool,
}

impl Sprint {
    pub fn new(title: &str, goal: Option<&str>) -> Self {
        let mut base = BaseElement::new(ElementType::Sprint, title);
        base.status = Status::Backlog; // Sprints start in backlog
        Self {
            base,
            goal: goal.map(|s| s.to_string()),
            start_date: None,
            end_date: None,
            active: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Story {
    pub fn new(title: &str, parent_sprint: &ElementId) -> Self {
        let mut base = BaseElement::new(ElementType::Story, title);
        base.status = Status::Backlog;
        base.parent = Some(parent_sprint.clone());
        Self { base }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Task {
    pub fn new(title: &str, parent_story: &ElementId) -> Self {
        let mut base = BaseElement::new(ElementType::Task, title);
        base.status = Status::Backlog;
        base.parent = Some(parent_story.clone());
        Self { base }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Idea {
    pub fn new(title: &str) -> Self {
        let mut base = BaseElement::new(ElementType::Idea, title);
        base.status = Status::Backlog;
        Self { base }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Issue {
    pub fn new(title: &str) -> Self {
        let mut base = BaseElement::new(ElementType::Issue, title);
        base.status = Status::Backlog;
        Self { base }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tips {
    #[serde(flatten)]
    pub base: BaseElement,
    pub target_task: ElementId,
    pub agent_id: String,
}

impl Tips {
    pub fn new(title: &str, target_task: ElementId, agent_id: &str) -> Self {
        let mut base = BaseElement::new(ElementType::Tips, title);
        base.status = Status::Backlog;
        Self {
            base,
            target_task,
            agent_id: agent_id.to_string(),
        }
    }
}
```

- [ ] **Step 4: Write KanbanElement enum with constructors**

Add to `core/src/kanban/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KanbanElement {
    Sprint(Sprint),
    Story(Story),
    Task(Task),
    Idea(Idea),
    Issue(Issue),
    Tips(Tips),
}

impl KanbanElement {
    // Constructors
    pub fn new_sprint(title: &str, goal: Option<&str>) -> Self {
        KanbanElement::Sprint(Sprint::new(title, goal))
    }

    pub fn new_story(title: &str, parent_sprint: &ElementId) -> Self {
        KanbanElement::Story(Story::new(title, parent_sprint))
    }

    pub fn new_task(title: &str, parent_story: &ElementId) -> Self {
        KanbanElement::Task(Task::new(title, parent_story))
    }

    pub fn new_idea(title: &str) -> Self {
        KanbanElement::Idea(Idea::new(title))
    }

    pub fn new_issue(title: &str) -> Self {
        KanbanElement::Issue(Issue::new(title))
    }

    pub fn new_tips(title: &str, target_task: &ElementId, agent_id: &str) -> Self {
        KanbanElement::Tips(Tips::new(title, target_task.clone(), agent_id))
    }

    // Accessors
    pub fn id(&self) -> &ElementId {
        match self {
            KanbanElement::Sprint(s) => &s.base.id,
            KanbanElement::Story(s) => &s.base.id,
            KanbanElement::Task(t) => &t.base.id,
            KanbanElement::Idea(i) => &i.base.id,
            KanbanElement::Issue(i) => &i.base.id,
            KanbanElement::Tips(t) => &t.base.id,
        }
    }

    pub fn element_type(&self) -> ElementType {
        match self {
            KanbanElement::Sprint(_) => ElementType::Sprint,
            KanbanElement::Story(_) => ElementType::Story,
            KanbanElement::Task(_) => ElementType::Task,
            KanbanElement::Idea(_) => ElementType::Idea,
            KanbanElement::Issue(_) => ElementType::Issue,
            KanbanElement::Tips(_) => ElementType::Tips,
        }
    }

    pub fn status(&self) -> Status {
        match self {
            KanbanElement::Sprint(s) => s.base.status,
            KanbanElement::Story(st) => st.base.status,
            KanbanElement::Task(t) => t.base.status,
            KanbanElement::Idea(i) => i.base.status,
            KanbanElement::Issue(iss) => iss.base.status,
            KanbanElement::Tips(tip) => tip.base.status,
        }
    }

    pub fn can_transition_to(&self, new_status: Status) -> bool {
        match self {
            KanbanElement::Sprint(s) => s.base.can_transition_to(new_status),
            KanbanElement::Story(st) => st.base.can_transition_to(new_status),
            KanbanElement::Task(t) => t.base.can_transition_to(new_status),
            KanbanElement::Idea(i) => i.base.can_transition_to(new_status),
            KanbanElement::Issue(iss) => iss.base.can_transition_to(new_status),
            KanbanElement::Tips(tip) => tip.base.can_transition_to(new_status),
        }
    }

    pub fn transition(&mut self, new_status: Status) -> Result<(), KanbanError> {
        match self {
            KanbanElement::Sprint(s) => s.base.transition(new_status),
            KanbanElement::Story(st) => st.base.transition(new_status),
            KanbanElement::Task(t) => t.base.transition(new_status),
            KanbanElement::Idea(i) => i.base.transition(new_status),
            KanbanElement::Issue(iss) => iss.base.transition(new_status),
            KanbanElement::Tips(tip) => tip.base.transition(new_status),
        }
    }

    pub fn assignee(&self) -> Option<&str> {
        match self {
            KanbanElement::Sprint(s) => s.base.assignee.as_deref(),
            KanbanElement::Story(st) => st.base.assignee.as_deref(),
            KanbanElement::Task(t) => t.base.assignee.as_deref(),
            KanbanElement::Idea(i) => i.base.assignee.as_deref(),
            KanbanElement::Issue(iss) => iss.base.assignee.as_deref(),
            KanbanElement::Tips(tip) => tip.base.assignee.as_deref(),
        }
    }

    pub fn dependencies(&self) -> &[ElementId] {
        match self {
            KanbanElement::Sprint(s) => &s.base.dependencies,
            KanbanElement::Story(st) => &st.base.dependencies,
            KanbanElement::Task(t) => &t.base.dependencies,
            KanbanElement::Idea(i) => &i.base.dependencies,
            KanbanElement::Issue(iss) => &iss.base.dependencies,
            KanbanElement::Tips(tip) => &tip.base.dependencies,
        }
    }

    pub fn references(&self) -> &[ElementId] {
        match self {
            KanbanElement::Sprint(s) => &s.base.references,
            KanbanElement::Story(st) => &st.base.references,
            KanbanElement::Task(t) => &t.base.references,
            KanbanElement::Idea(i) => &i.base.references,
            KanbanElement::Issue(iss) => &iss.base.references,
            KanbanElement::Tips(tip) => &tip.base.references,
        }
    }

    pub fn parent(&self) -> Option<&ElementId> {
        match self {
            KanbanElement::Sprint(s) => s.base.parent.as_ref(),
            KanbanElement::Story(st) => st.base.parent.as_ref(),
            KanbanElement::Task(t) => t.base.parent.as_ref(),
            KanbanElement::Idea(i) => i.base.parent.as_ref(),
            KanbanElement::Issue(iss) => iss.base.parent.as_ref(),
            KanbanElement::Tips(tip) => tip.base.parent.as_ref(),
        }
    }

    // Mutable accessors for updates
    pub fn set_id(&mut self, id: ElementId) {
        match self {
            KanbanElement::Sprint(s) => s.base.id = id,
            KanbanElement::Story(st) => st.base.id = id,
            KanbanElement::Task(t) => t.base.id = id,
            KanbanElement::Idea(i) => i.base.id = id,
            KanbanElement::Issue(iss) => iss.base.id = id,
            KanbanElement::Tips(tip) => tip.base.id = id,
        }
    }

    pub fn set_status(&mut self, status: Status) {
        match self {
            KanbanElement::Sprint(s) => s.base.status = status,
            KanbanElement::Story(st) => st.base.status = status,
            KanbanElement::Task(t) => t.base.status = status,
            KanbanElement::Idea(i) => i.base.status = status,
            KanbanElement::Issue(iss) => iss.base.status = status,
            KanbanElement::Tips(tip) => tip.base.status = status,
        }
    }

    pub fn set_updated_at(&mut self, time: chrono::DateTime<chrono::Utc>) {
        match self {
            KanbanElement::Sprint(s) => s.base.updated_at = time,
            KanbanElement::Story(st) => st.base.updated_at = time,
            KanbanElement::Task(t) => t.base.updated_at = time,
            KanbanElement::Idea(i) => i.base.updated_at = time,
            KanbanElement::Issue(iss) => iss.base.updated_at = time,
            KanbanElement::Tips(tip) => tip.base.updated_at = time,
        }
    }

    pub fn set_created_at(&mut self, time: chrono::DateTime<chrono::Utc>) {
        match self {
            KanbanElement::Sprint(s) => s.base.created_at = time,
            KanbanElement::Story(st) => st.base.created_at = time,
            KanbanElement::Task(t) => t.base.created_at = time,
            KanbanElement::Idea(i) => i.base.created_at = time,
            KanbanElement::Issue(iss) => iss.base.created_at = time,
            KanbanElement::Tips(tip) => tip.base.created_at = time,
        }
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p agent-core --test kanban`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add core/src/kanban/domain.rs core/tests/kanban/
git commit -m "feat(kanban): add KanbanElement enum with all variant types"
```

---

## Task 4: Implement Repository trait

**Files:**
- Modify: `core/src/kanban/repository.rs`

- [ ] **Step 1: Write repository trait test**

Create `core/tests/kanban/repository.rs`:

```rust
use agent_core::kanban::{KanbanElementRepository, FileKanbanRepository, ElementId, ElementType};
use tempfile::TempDir;

#[test]
fn test_file_repository_creates_directories() {
    let temp = TempDir::new().expect("tempdir");
    let repo = FileKanbanRepository::new(temp.path()).expect("create repo");
    assert!(temp.path().join("kanban/elements").exists());
}

#[test]
fn test_save_and_get_element() {
    let temp = TempDir::new().expect("tempdir");
    let repo = FileKanbanRepository::new(temp.path()).expect("create repo");

    let sprint = repo.new_id(ElementType::Sprint).expect("new id");
    let mut element = agent_core::kanban::KanbanElement::new_sprint("Sprint 1", Some("Goal"));
    element.set_id(sprint.clone());

    repo.save(&element).expect("save");

    let loaded = repo.get(&sprint).expect("get");
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().id(), &sprint);
}

#[test]
fn test_list_returns_all_elements() {
    let temp = TempDir::new().expect("tempdir");
    let repo = FileKanbanRepository::new(temp.path()).expect("create repo");

    let sprint_id = repo.new_id(ElementType::Sprint).expect("new id");
    let mut sprint = agent_core::kanban::KanbanElement::new_sprint("Sprint 1", None);
    sprint.set_id(sprint_id);
    repo.save(&sprint).expect("save sprint");

    let story_id = repo.new_id(ElementType::Story).expect("new id");
    let mut story = agent_core::kanban::KanbanElement::new_story("Story 1", &sprint_id);
    story.set_id(story_id);
    repo.save(&story).expect("save story");

    let elements = repo.list().expect("list");
    assert_eq!(elements.len(), 2);
}
```

Run: `cargo test -p agent-core --test kanban repository::`
Expected: FAIL - FileKanbanRepository not found

- [ ] **Step 2: Write KanbanElementRepository trait**

Create `core/src/kanban/repository.rs`:

```rust
use crate::domain::{ElementId, ElementType, KanbanElement, Status};

/// Repository trait - enables future database or RPC backend
pub trait KanbanElementRepository: Send + Sync {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, crate::KanbanError>;
    fn list(&self) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn list_blocked(&self) -> Result<Vec<KanbanElement>, crate::KanbanError>;
    fn save(&self, element: &KanbanElement) -> Result<(), crate::KanbanError>;
    fn delete(&self, id: &ElementId) -> Result<(), crate::KanbanError>;
    fn new_id(&self, type_: ElementType) -> Result<ElementId, crate::KanbanError>;
}
```

- [ ] **Step 3: Run tests to verify trait compiles**

Run: `cargo build -p agent-core`
Expected: BUILD SUCCESS (trait only, no implementation yet)

- [ ] **Step 4: Commit**

```bash
git add core/src/kanban/repository.rs core/tests/kanban/repository.rs
git commit -m "feat(kanban): add KanbanElementRepository trait"
```

---

## Task 5: Implement FileKanbanRepository

**Files:**
- Create: `core/src/kanban/file_repository.rs`

- [ ] **Step 1: Write FileKanbanRepository implementation**

Create `core/src/kanban/file_repository.rs`:

```rust
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::domain::{ElementId, ElementType, KanbanElement, Status};
use crate::kanban::error::KanbanError;
use crate::kanban::repository::KanbanElementRepository;

pub struct FileKanbanRepository {
    base_path: PathBuf,
    index_path: PathBuf,
    elements_path: PathBuf,
    counters: RwLock<HashMap<ElementType, u32>>,
}

#[derive(Debug, Default)]
struct Index {
    elements: Vec<String>,
}

impl FileKanbanRepository {
    pub fn new(base_path: &Path) -> Result<Self, KanbanError> {
        let base = base_path.join("kanban");
        let index = base.join("index.json");
        let elements = base.join("elements");

        fs::create_dir_all(&elements)?;

        let counters = Self::load_counters(&elements)?;

        Ok(Self {
            base_path: base,
            index_path: index,
            elements_path: elements,
            counters: RwLock::new(counters),
        })
    }

    fn element_path(&self, id: &ElementId) -> PathBuf {
        self.elements_path.join(format!("{}.json", id.as_str()))
    }

    fn load_counters(elements_path: &Path) -> Result<HashMap<ElementType, u32>, KanbanError> {
        let mut counters: HashMap<ElementType, u32> = HashMap::new();

        for entry in fs::read_dir(elements_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = ElementId::parse(filename) {
                        let type_ = id.type_();
                        let num = id.number();
                        let current = counters.entry(type_).or_insert(0);
                        if num > *current {
                            *current = num;
                        }
                    }
                }
            }
        }

        Ok(counters)
    }

    fn update_index(&self) -> Result<(), KanbanError> {
        let elements = self.list()?;
        let ids: Vec<String> = elements.iter().map(|e| e.id().as_str().to_string()).collect();
        let index = Index { elements: ids };
        let content = serde_json::to_string_pretty(&index)?;
        fs::write(&self.index_path, content)?;
        Ok(())
    }
}

impl KanbanElementRepository for FileKanbanRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        let path = self.element_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let element: KanbanElement = serde_json::from_str(&content)?;
        Ok(Some(element))
    }

    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        let mut elements = Vec::new();
        for entry in fs::read_dir(&self.elements_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let element: KanbanElement = serde_json::from_str(&content)?;
                elements.push(element);
            }
        }
        elements.sort_by_key(|e| e.id().clone());
        Ok(elements)
    }

    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|e| e.element_type() == type_).collect())
    }

    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|e| e.status() == status).collect())
    }

    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|e| e.assignee() == Some(assignee)).collect())
    }

    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|e| e.parent() == Some(parent)).collect())
    }

    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.list_by_status(Status::Blocked)
    }

    fn save(&self, element: &KanbanElement) -> Result<(), KanbanError> {
        let path = self.element_path(element.id());
        let content = serde_json::to_string_pretty(element)?;
        fs::write(&path, content)?;
        self.update_index()?;
        Ok(())
    }

    fn delete(&self, id: &ElementId) -> Result<(), KanbanError> {
        let path = self.element_path(id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        self.update_index()?;
        Ok(())
    }

    fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError> {
        let mut counters = self.counters.write().unwrap();
        let counter = counters.entry(type_).or_insert(0);
        *counter += 1;
        let id = ElementId::new(type_, *counter);
        Ok(id)
    }
}
```

Note: You'll need to add `use std::collections::HashMap;` at the top.

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p agent-core --test kanban repository::`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add core/src/kanban/file_repository.rs
git commit -m "feat(kanban): implement FileKanbanRepository"
```

---

## Task 6: Implement KanbanEvent and KanbanEventBus

**Files:**
- Create: `core/src/kanban/events.rs`

- [ ] **Step 1: Write event system tests**

Create `core/tests/kanban/events.rs`:

```rust
use agent_core::kanban::events::{KanbanEvent, KanbanEventBus, KanbanEventSubscriber};
use agent_core::kanban::{ElementId, ElementType, Status};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn test_event_bus_publishes_to_subscriber() {
    let bus = KanbanEventBus::new();
    let count = AtomicUsize::new(0);

    bus.subscribe(Box::new(|event| {
        count.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Sprint, 1),
        element_type: ElementType::Sprint,
    });

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_event_bus_multiple_subscribers() {
    let bus = KanbanEventBus::new();
    let count1 = AtomicUsize::new(0);
    let count2 = AtomicUsize::new(0);

    bus.subscribe(Box::new(move |_| {
        count1.fetch_add(1, Ordering::SeqCst);
    }));
    bus.subscribe(Box::new(move |_| {
        count2.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(KanbanEvent::StatusChanged {
        element_id: ElementId::new(ElementType::Task, 1),
        old_status: Status::Ready,
        new_status: Status::InProgress,
        changed_by: "test".to_string(),
    });

    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}
```

Run: `cargo test -p agent-core --test kanban events::`
Expected: FAIL - module not found

- [ ] **Step 2: Write KanbanEvent and KanbanEventBus**

Create `core/src/kanban/events.rs`:

```rust
use crate::domain::{ElementId, ElementType, Status};
use std::sync::RwLock;
use std::vec::Vec;

/// Event types published by the kanban system
#[derive(Debug, Clone)]
pub enum KanbanEvent {
    Created {
        element_id: ElementId,
        element_type: ElementType,
    },
    Updated {
        element_id: ElementId,
        changes: Vec<String>,
    },
    StatusChanged {
        element_id: ElementId,
        old_status: Status,
        new_status: Status,
        changed_by: String,
    },
    Deleted {
        element_id: ElementId,
    },
    TipAppended {
        task_id: ElementId,
        tip_id: ElementId,
        agent_id: String,
    },
    DependencyAdded {
        element_id: ElementId,
        dependency: ElementId,
    },
    DependencyRemoved {
        element_id: ElementId,
        dependency: ElementId,
    },
}

/// Subscriber trait for kanban events
pub trait KanbanEventSubscriber: Send {
    fn on_event(&self, event: &KanbanEvent);
}

/// Event bus for publish/subscribe
pub struct KanbanEventBus {
    subscribers: RwLock<Vec<Box<dyn KanbanEventSubscriber + Send>>>,
}

impl KanbanEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, subscriber: Box<dyn KanbanEventSubscriber + Send>) {
        self.subscribers.write().unwrap().push(subscriber);
    }

    pub fn publish(&self, event: KanbanEvent) {
        for subscriber in self.subscribers.read().unwrap().iter() {
            subscriber.on_event(&event);
        }
    }
}

impl Default for KanbanEventBus {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p agent-core --test kanban events::`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add core/src/kanban/events.rs core/tests/kanban/events.rs
git commit -m "feat(kanban): add KanbanEvent and KanbanEventBus"
```

---

## Task 7: Implement KanbanService

**Files:**
- Create: `core/src/kanban/service.rs`
- Create: `core/tests/kanban/service.rs`

- [ ] **Step 1: Write KanbanService tests**

Create `core/tests/kanban/service.rs`:

```rust
use agent_core::kanban::{FileKanbanRepository, KanbanService, ElementId, ElementType, Status};
use tempfile::TempDir;
use std::sync::Arc;

#[test]
fn test_create_element_assigns_id() {
    let temp = TempDir::new().expect("tempdir");
    let repo = Arc::new(FileKanbanRepository::new(temp.path()).expect("create repo"));
    let bus = Arc::new(agent_core::kanban::KanbanEventBus::new());
    let service = KanbanService::new(repo.clone(), bus);

    let element = agent_core::kanban::KanbanElement::new_sprint("Sprint 1", Some("Goal"));
    let created = service.create_element(element).expect("create");

    assert!(created.id().as_str().starts_with("sprint-"));
    assert_eq!(created.id().as_str(), "sprint-001");
}

#[test]
fn test_update_status_validates_transition() {
    let temp = TempDir::new().expect("tempdir");
    let repo = Arc::new(FileKanbanRepository::new(temp.path()).expect("create repo"));
    let bus = Arc::new(agent_core::kanban::KanbanEventBus::new());
    let service = KanbanService::new(repo, bus);

    let element = agent_core::kanban::KanbanElement::new_sprint("Sprint 1", None);
    let created = service.create_element(element).expect("create");

    // Plan -> Backlog is valid, Plan -> Done is not
    let result = service.update_status(created.id(), Status::Done, "test");
    assert!(result.is_err());
}

#[test]
fn test_find_blocking_dependencies() {
    let temp = TempDir::new().expect("tempdir");
    let repo = Arc::new(FileKanbanRepository::new(temp.path()).expect("create repo"));
    let bus = Arc::new(agent_core::kanban::KanbanEventBus::new());
    let service = KanbanService::new(repo, bus);

    // Create sprint first
    let sprint = service.create_element(agent_core::kanban::KanbanElement::new_sprint("Sprint 1", None)).expect("create sprint");

    // Create story that depends on sprint
    let mut story = agent_core::kanban::KanbanElement::new_story("Story 1", sprint.id());
    story.base_mut().dependencies.push(sprint.id().clone());
    let story = service.create_element(story).expect("create story");

    // Sprint is done, so story should have no blockers
    service.update_status(sprint.id(), Status::Done, "test").expect("complete sprint");
    let blockers = service.find_blocking_dependencies(story.id()).expect("find blockers");
    assert!(blockers.is_empty());
}
```

Run: `cargo test -p agent-core --test kanban service::`
Expected: FAIL - service not found

- [ ] **Step 2: Write KanbanService**

Create `core/src/kanban/service.rs`:

```rust
use std::sync::Arc;
use crate::domain::{ElementId, ElementType, KanbanElement, Status};
use crate::kanban::error::KanbanError;
use crate::kanban::events::{KanbanEvent, KanbanEventBus};
use crate::kanban::repository::KanbanElementRepository;

pub struct KanbanService<R: KanbanElementRepository> {
    repository: Arc<R>,
    event_bus: Arc<KanbanEventBus>,
}

impl<R: KanbanElementRepository> KanbanService<R> {
    pub fn new(repository: Arc<R>, event_bus: Arc<KanbanEventBus>) -> Self {
        Self { repository, event_bus }
    }

    pub fn create_element(&self, mut element: KanbanElement) -> Result<KanbanElement, KanbanError> {
        // Assign ID if not already set
        let id = self.repository.new_id(element.element_type())?;
        element.set_id(id);
        element.set_created_at(chrono::Utc::now());
        element.set_updated_at(chrono::Utc::now());

        self.repository.save(&element)?;

        self.event_bus.publish(KanbanEvent::Created {
            element_id: element.id().clone(),
            element_type: element.element_type(),
        });

        Ok(element)
    }

    pub fn get_element(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        self.repository.get(id)
    }

    pub fn list_elements(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list()
    }

    pub fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list_by_type(type_)
    }

    pub fn update_status(&self, id: &ElementId, new_status: Status, agent_id: &str)
        -> Result<KanbanElement, KanbanError>
    {
        let mut element = self.repository.get(id)?
            .ok_or(KanbanError::NotFound(id.clone()))?;

        if !element.can_transition_to(new_status) {
            return Err(KanbanError::InvalidStatusTransition {
                current: element.status(),
                requested: new_status,
            });
        }

        // Check dependencies when moving to in_progress or done
        if new_status == Status::InProgress || new_status == Status::Done {
            let blockers = self.find_blocking_dependencies(id)?;
            if !blockers.is_empty() {
                return Err(KanbanError::DependenciesNotMet {
                    element: id.clone(),
                    blockers,
                });
            }
        }

        let old_status = element.status();
        element.transition(new_status)?;

        self.repository.save(&element)?;

        self.event_bus.publish(KanbanEvent::StatusChanged {
            element_id: id.clone(),
            old_status,
            new_status,
            changed_by: agent_id.to_string(),
        });

        Ok(element)
    }

    pub fn find_blocking_dependencies(&self, id: &ElementId)
        -> Result<Vec<ElementId>, KanbanError>
    {
        let element = self.repository.get(id)?
            .ok_or(KanbanError::NotFound(id.clone()))?;

        let mut blockers = Vec::new();
        for dep_id in element.dependencies() {
            let dep = self.repository.get(dep_id)?
                .ok_or(KanbanError::DanglingDependency {
                    element: id.clone(),
                    dependency: dep_id.clone(),
                })?;

            if dep.status() != Status::Done && dep.status() != Status::Verified {
                blockers.push(dep_id.clone());
            }
        }

        Ok(blockers)
    }

    pub fn can_start(&self, id: &ElementId) -> Result<bool, KanbanError> {
        let blockers = self.find_blocking_dependencies(id)?;
        Ok(blockers.is_empty())
    }

    pub fn append_tip(&self, task_id: &ElementId, title: &str, agent_id: &str)
        -> Result<(), KanbanError>
    {
        let task = self.repository.get(task_id)?
            .ok_or(KanbanError::NotFound(task_id.clone()))?;

        if task.element_type() != ElementType::Task {
            return Err(KanbanError::InvalidTipTarget(task_id.clone()));
        }

        let tip = KanbanElement::new_tips(title, task_id, agent_id);
        let created = self.create_element(tip)?;

        self.event_bus.publish(KanbanEvent::TipAppended {
            task_id: task_id.clone(),
            tip_id: created.id().clone(),
            agent_id: agent_id.to_string(),
        });

        Ok(())
    }
}
```

Note: Need to add `base_mut()` method to KanbanElement - see Step 3.

- [ ] **Step 3: Add base_mut accessor to KanbanElement**

Modify `core/src/kanban/domain.rs` to add:

```rust
pub fn base_mut(&mut self) -> &mut BaseElement {
    match self {
        KanbanElement::Sprint(s) => &mut s.base,
        KanbanElement::Story(st) => &mut st.base,
        KanbanElement::Task(t) => &mut t.base,
        KanbanElement::Idea(i) => &mut i.base,
        KanbanElement::Issue(iss) => &mut iss.base,
        KanbanElement::Tips(tip) => &mut tip.base,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p agent-core --test kanban`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add core/src/kanban/service.rs core/src/kanban/domain.rs core/tests/kanban/service.rs
git commit -m "feat(kanban): add KanbanService with create, status update, and dependency tracking"
```

---

## Task 8: Basic Git Operations (placeholder for future)

**Files:**
- Create: `core/src/kanban/git_ops.rs`

- [ ] **Step 1: Write placeholder GitOperations**

Create `core/src/kanban/git_ops.rs`:

```rust
use std::path::PathBuf;

/// Git operations for kanban collaboration
/// Full implementation deferred - placeholder for now
pub struct GitOperations {
    repo_path: PathBuf,
}

impl GitOperations {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// Stage and commit all kanban changes
    /// TODO: Implement with git2 crate
    pub fn commit_changes(&self, agent_id: &str, message: &str) -> Result<(), GitError> {
        // TODO: Implement
        Ok(())
    }

    /// Fetch and rebase on latest changes
    /// TODO: Implement with git2 crate
    pub fn fetch_and_rebase(&self, branch: &str) -> Result<(), GitError> {
        // TODO: Implement
        Ok(())
    }

    /// Check for unresolved conflicts
    pub fn has_conflicts(&self) -> bool {
        // TODO: Implement
        false
    }
}

#[derive(Debug)]
pub struct GitError {
    message: String,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GitError: {}", self.message)
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add core/src/kanban/git_ops.rs
git commit -m "feat(kanban): add placeholder GitOperations for future implementation"
```

---

## Task 9: Integration with SharedWorkplaceState

**Files:**
- Modify: `core/src/workplace_store.rs`
- Modify: `core/src/shared_state.rs` (assumed to exist in multi-agent design)

- [ ] **Step 1: Add kanban_dir method to WorkplaceStore**

Modify `core/src/workplace_store.rs` to add:

```rust
pub fn kanban_dir(&self) -> PathBuf {
    self.path.join("kanban")
}

pub fn kanban_elements_dir(&self) -> PathBuf {
    self.kanban_dir().join("elements")
}
```

- [ ] **Step 2: Commit integration**

```bash
git add core/src/workplace_store.rs
git commit -m "feat(kanban): add kanban directory accessors to WorkplaceStore"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] Domain types (Status, Priority, ElementId, ElementType) - Task 2
- [x] KanbanElement enum with all variants (Sprint, Story, Task, Idea, Issue, Tips) - Task 3
- [x] StatusHistoryEntry for cycle time tracking - Task 2
- [x] Repository trait - Task 4
- [x] FileKanbanRepository implementation - Task 5
- [x] KanbanEvent + KanbanEventBus - Task 6
- [x] KanbanService with create, update_status, find_blocking_dependencies - Task 7
- [x] GitOperations placeholder - Task 8
- [x] WorkplaceStore integration - Task 9

**2. Placeholder scan:**
- All steps have actual code, no TBD/TODO in implementation
- git_ops.rs has explicit TODO comments but is marked as placeholder for future

**3. Type consistency:**
- ElementId::new() takes ElementType and u32
- KanbanElement constructors match spec
- Status enum uses snake_case serialization

**4. Gaps identified:**
- Tips::target_task and Tips::agent_id need accessor methods on Tips struct
- FileRepository load_counters could be simplified
- No integration tests for multi-agent scenarios yet

---

## Execution Options

**Plan complete and saved to `docs/superpowers/plans/2026-04-13-kanban-system-implementation.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
