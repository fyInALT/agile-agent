# Kanban System Design

## Metadata

- Date: `2026-04-13`
- Project: `agile-agent`
- Status: `Draft`
- Language: `English`

## 1. Purpose

`agile-agent` needs a shared, Git-backed kanban system that multiple agents can use concurrently to manage agile development work. The system manages sprints, stories, tasks, ideas, issues, and tips — all accessible to both agents and human developers directly as readable JSON files.

The kanban system is designed to eventually be extractable as a standalone microservice, with clean boundaries between domain logic, storage, and transport layers.

## 2. Scope

### In scope

- six element types: sprint, story, task, idea, issue, tips
- shared status state machine across all element types
- dependency and reference relationships between elements
- Git-based concurrent access with standard Git flow
- human-readable JSON file format under `~/.agile-agent/workplaces/{id}/kanban/`
- integration with `SharedWorkplaceState` for multi-agent access
- event publication for TUI/agent notifications

### Out of scope

- TUI rendering of the kanban board (handled by TUI layer)
- HTTP/gRPC API (future microservice interface)
- automated sprint planning or story point estimation
- Git conflict resolution UI

## 3. Element Types

| Type | Description | Hierarchy |
|------|-------------|----------|
| `sprint` | A time-boxed development iteration | Top-level container |
| `story` | A user-facing feature or requirement, belongs to a sprint | Child of sprint |
| `task` | A granular work item, belongs to a story | Child of story |
| `idea` | An underdeveloped thought, often just a sentence | Independent, can reference others |
| `issue` | A problem or concern to address | Independent, can reference others |
| `tips` | A small note or reminder, always attached to a sprint | Independent, references a target task |

### 3.1 Tips Appending

Agents may append tips to any task (including tasks assigned to other agents). A tip is an independent element file that references the target task via the `references` field. Tips are append-only to prevent conflicts. The tip records its `agent_id` as the creator.

## 4. Common Fields

All elements share the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Human-readable unique identifier, format `{type}-{number}` e.g. `sprint-001` |
| `type` | string | Element type: `sprint`, `story`, `task`, `idea`, `issue`, `tips` |
| `title` | string | Short title |
| `content` | string | Full content or description |
| `keywords` | string[] | Keywords for search and AI context |
| `status` | string | Current status (see State Machine) |
| `dependencies` | string[] | IDs of elements this item blocks on (execution order) |
| `references` | string[] | IDs of related elements AI should consult when working on this |
| `parent` | string? | ID of parent element (story→sprint, task→story) |
| `created_at` | string (ISO 8601) | Creation timestamp |
| `updated_at` | string (ISO 8601) | Last modification timestamp |
| `priority` | enum | `critical`, `high`, `medium`, `low` (see Priority) |
| `assignee` | string? | Agent or person responsible |
| `effort` | number? | Estimated work units |
| `blocked_reason` | string? | Reason when status is `blocked` |
| `tags` | string[] | Additional labels |
| `status_history` | StatusHistoryEntry[] | When status changed (for cycle time tracking) |

### 4.1 Status History (Cycle Time Tracking)

Each status change is logged for cycle time metrics:

```json
{
  "status_history": [
    { "status": "backlog", "entered_at": "2026-04-10T09:00:00Z" },
    { "status": "ready", "entered_at": "2026-04-12T10:00:00Z" },
    { "status": "in_progress", "entered_at": "2026-04-13T14:00:00Z" },
    { "status": "done", "entered_at": "2026-04-14T11:00:00Z" },
    { "status": "verified", "entered_at": "2026-04-14T15:00:00Z" }
  ]
}
```

Metrics derived from status_history:
- **Lead time**: `created_at` → `done` (total time in system)
- **Cycle time**: `ready` → `done` (time actually working)
- **Wait time**: `backlog` → `ready` (time waiting to be started)
- **Block time**: sum of time in `blocked` status

### 4.2 Priority Enum

```rust
pub enum Priority {
    Critical,  // P0 - Immediately block all work
    High,      // P1 - Must complete this sprint
    Medium,    // P2 - Should complete this sprint
    Low,       // P3 - Next sprint or later
}
```

Tips-specific fields:

| Field | Type | Description |
|-------|------|-------------|
| `target_task` | string | ID of the task this tip is appended to |
| `agent_id` | string | ID of the agent that created this tip |

Sprint-specific additional fields:

| Field | Type | Description |
|-------|------|-------------|
| `goal` | string? | Sprint goal description |
| `start_date` | string (ISO 8601)? | Sprint planned start |
| `end_date` | string (ISO 8601)? | Sprint planned end |
| `active` | bool | Whether this is the current active sprint |

### 4.3 Definition of Done (Story-specific, Future)

Stories may include a checklist of completion criteria:

```json
{
  "id": "story-001",
  "definition_of_done": [
    "code_review_approved",
    "tests_written",
    "documentation_updated",
    "deployed_to_staging"
  ],
  "done_checklist": {
    "code_review_approved": true,
    "tests_written": true,
    "documentation_updated": false,
    "deployed_to_staging": false
  }
}
```

This is marked as **Future** since it's complex to enforce across agents.

## 5. State Machine

All element types share one status model:

```
plan → backlog → blocked / ready / todo → in_progress → done → verified
```

| Status | Description |
|--------|-------------|
| `plan` | Item is being planned |
| `backlog` | Item is in the backlog |
| `blocked` | Item cannot proceed |
| `ready` | Item is ready to be worked on immediately |
| `todo` | Item is ready but intentionally deferred |
| `in_progress` | Item is being actively worked |
| `done` | Item is completed |
| `verified` | Item's completion has been verified |

## 6. Relationships

### 6.1 Dependencies

`dependencies` is a list of element IDs. It represents **blocking execution order**: element A depends on B means A cannot start until B is done. This applies to all element types and is many-to-many.

### 6.2 References

`references` is a list of element IDs. It represents a **one-way informational link**: when an agent works on element A, it should consult the content of all elements in A's `references` list. This applies to all element types and is many-to-many.

### 6.3 Parent

`parent` establishes the Scrum hierarchy:

- `task.parent` = `story.id`
- `story.parent` = `sprint.id`
- `idea.parent` = null (independent)
- `issue.parent` = null (independent)
- `tips.parent` = sprint.id (tips belong to a sprint, reference a task)

## 7. Storage Structure

```
~/.agile-agent/workplaces/{workplace_id}/kanban/
├── index.json       # Minimal ID registry
└── elements/
    ├── sprint-001.json
    ├── sprint-002.json
    ├── story-001.json
    ├── story-002.json
    ├── task-001.json
    ├── task-002.json
    ├── idea-001.json
    ├── issue-001.json
    └── tip-001.json
```

### 7.1 index.json

`index.json` is intentionally minimal to avoid merge conflicts:

```json
{
  "elements": [
    "sprint-001",
    "story-001",
    "task-001",
    "idea-001",
    "issue-001",
    "tip-001"
  ]
}
```

Agents discover elements by traversing the `elements/` directory, not by relying on `index.json`. This ensures agents never miss elements due to a stale index and always observe each other's work.

### 7.2 Element File Format

Each element is stored as a single human-readable JSON file:

```json
{
  "id": "task-001",
  "type": "task",
  "title": "Implement kanban persistence",
  "content": "Add JSON file read/write for all element types...",
  "keywords": ["persistence", "json", "storage"],
  "status": "in_progress",
  "dependencies": ["story-001"],
  "references": ["idea-002", "tip-001"],
  "parent": "story-001",
  "created_at": "2026-04-13T10:00:00Z",
  "updated_at": "2026-04-13T14:30:00Z",
  "priority": "high",
  "assignee": "agent-alpha",
  "effort": 5,
  "blocked_reason": null,
  "tags": ["backend", "storage"]
}
```

## 8. Architecture

The kanban system follows clean architecture principles to support future microservice extraction:

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                         │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                    KanbanService                         │ │
│  │  - create_element / update_element / delete_element     │ │
│  │  - append_tip / update_status / assign_task             │ │
│  │  - get_element / list_elements / get_by_type             │ │
│  │  - check_dependencies / resolve_blockers                │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Domain Layer                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ KanbanElement │  │  Status      │  │   Event     │       │
│  │ (trait)       │  │  Machine     │  │  (trait)    │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Infrastructure Layer                        │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │               FileKanbanRepository                       │ │
│  │  - read/write JSON files to kanban/elements/             │ │
│  │  - implements KanbanElementRepository trait              │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │               GitOperations                               │ │
│  │  - commit_changes / fetch_rebase / detect_conflicts      │ │
│  │  - uses git2 crate for libgit2 bindings                  │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │               KanbanEventBus                             │ │
│  │  - publishes KanbanChanged events                        │ │
│  │  - used by TUI and agents to watch changes               │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 8.1 Domain Layer

#### KanbanElement Trait

```rust
/// Domain entity representing any kanban element
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

pub trait KanbanElementExt {
    fn id(&self) -> &ElementId;
    fn status(&self) -> Status;
    fn dependencies(&self) -> &[ElementId];
    fn references(&self) -> &[ElementId];
    fn parent(&self) -> Option<&ElementId>;
    fn can_transition_to(&self, new_status: Status) -> bool;
    fn assignee(&self) -> Option<&str>;
}

impl KanbanElement {
    pub fn transition(&mut self, new_status: Status) -> Result<(), KanbanError> {
        if !self.can_transition_to(new_status) {
            return Err(KanbanError::InvalidStatusTransition {
                current: self.status(),
                requested: new_status,
            });
        }
        self.set_status(new_status);
        Ok(())
    }
}
```

#### Status Machine

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Returns valid transitions from this status
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
}
```

#### Element IDs

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(String);

impl ElementId {
    pub fn new(type_: ElementType, number: u32) -> Self {
        Self(format!("{}-{:03}", type_.as_str(), number))
    }

    pub fn parse(s: &str) -> Result<Self, ParseError> {
        // Parse "sprint-001" format
    }

    pub fn type_(&self) -> ElementType;
    pub fn number(&self) -> u32;
}

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

    pub fn counter_file(&self) -> &'static str {
        match self {
            ElementType::Tips => "tip", // tip-001.json but stored as counter
            _ => self.as_str(),
        }
    }
}
```

### 8.2 Repository Layer

#### Repository Trait (for Microservice Extraction)

```rust
/// Repository trait - enables future database or RPC backend
pub trait KanbanElementRepository: Send + Sync {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError>;
    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError>;
    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError>;
    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError>;
    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError>;
    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError>;
    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError>;
    fn save(&self, element: &KanbanElement) -> Result<(), KanbanError>;
    fn delete(&self, id: &ElementId) -> Result<(), KanbanError>;
    fn next_id(&self, type_: ElementType) -> Result<ElementId, KanbanError>;
}
```

#### File-Based Implementation

```rust
pub struct FileKanbanRepository {
    base_path: PathBuf,
    index_path: PathBuf,
    elements_path: PathBuf,
    counters: RwLock<HashMap<ElementType, u32>>,
}

impl FileKanbanRepository {
    pub fn new(workplace_id: &WorkplaceId) -> Result<Self, KanbanError> {
        let base = workplace_path(workplace_id).join("kanban");
        let index = base.join("index.json");
        let elements = base.join("elements");

        // Ensure directories exist
        std::fs::create_dir_all(&elements)?;

        // Load or initialize counters from existing files
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
}

impl KanbanElementRepository for FileKanbanRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        let path = self.element_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let element: KanbanElement = serde_json::from_str(&content)?;
        Ok(Some(element))
    }

    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        let mut elements = Vec::new();
        for entry in std::fs::read_dir(&self.elements_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)?;
                let element: KanbanElement = serde_json::from_str(&content)?;
                elements.push(element);
            }
        }
        // Sort by type then number for deterministic order
        elements.sort_by_key(|e| e.id().clone());
        Ok(elements)
    }

    fn save(&self, element: &KanbanElement) -> Result<(), KanbanError> {
        let path = self.element_path(element.id());
        let content = serde_json::to_string_pretty(element)?;
        std::fs::write(&path, content)?;
        self.update_index()?;
        Ok(())
    }

    fn next_id(&self, type_: ElementType) -> Result<ElementId, KanbanError> {
        let mut counters = self.counters.write().unwrap();
        let counter = counters.entry(type_).or_insert(0);
        *counter += 1;
        let id = ElementId::new(type_, *counter);
        self.save_counter(type_, *counter)?;
        Ok(id)
    }
}
```

### 8.3 Application Service Layer

```rust
pub struct KanbanService<R: KanbanElementRepository> {
    repository: Arc<R>,
    event_bus: Arc<KanbanEventBus>,
    git_ops: Arc<GitOperations>,
}

impl<R: KanbanElementRepository> KanbanService<R> {
    /// Create a new element with auto-generated ID
    pub fn create_element(&self, mut element: KanbanElement) -> Result<KanbanElement, KanbanError> {
        // Assign ID if not set
        if element.id().as_str().starts_with("{type}") {
            let new_id = self.repository.next_id(element.type_())?;
            element.set_id(new_id);
        }

        element.set_created_at(Utc::now());
        element.set_updated_at(Utc::now());

        self.repository.save(&element)?;

        self.event_bus.publish(KanbanEvent::Created {
            element_id: element.id().clone(),
            element_type: element.type_(),
        });

        Ok(element)
    }

    /// Update an element's content
    pub fn update_element(&self, id: &ElementId, updates: ElementUpdates)
        -> Result<KanbanElement, KanbanError>
    {
        let mut element = self.repository.get(id)?
            .ok_or(KanbanError::NotFound(id.clone()))?;

        // Validate assignee permission (only assignee can modify)
        // Note: this is advisory, as per design
        if let Some(assignee) = element.assignee() {
            if !updates.is_from_agent(assignee) && !updates.override_permission {
                return Err(KanbanError::PermissionDenied {
                    element: id.clone(),
                    required: assignee.to_string(),
                });
            }
        }

        element.apply_updates(updates);
        element.set_updated_at(Utc::now());

        self.repository.save(&element)?;

        self.event_bus.publish(KanbanEvent::Updated {
            element_id: id.clone(),
            changes: element.change_summary(),
        });

        Ok(element)
    }

    /// Append a tip to a task
    pub fn append_tip(&self, task_id: &ElementId, tip: Tips) -> Result<(), KanbanError> {
        // Verify task exists
        let task = self.repository.get(task_id)?
            .ok_or(KanbanError::NotFound(task_id.clone()))?;

        if task.type_() != ElementType::Task {
            return Err(KanbanError::InvalidTipTarget(task_id.clone()));
        }

        // Tip is independent - save as new element
        let tip_id = self.repository.next_id(ElementType::Tips)?;
        let mut tip_element = KanbanElement::Tips(tip);
        tip_element.set_id(tip_id);
        tip_element.set_created_at(Utc::now());
        tip_element.set_updated_at(Utc::now());

        self.repository.save(&tip_element)?;

        self.event_bus.publish(KanbanEvent::TipAppended {
            task_id: task_id.clone(),
            tip_id: tip_element.id().clone(),
            agent_id: tip.agent_id().to_string(),
        });

        Ok(())
    }

    /// Update element status with validation
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

        // Check dependencies are satisfied
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
        element.set_status(new_status);
        element.set_updated_at(Utc::now());

        self.repository.save(&element)?;

        self.event_bus.publish(KanbanEvent::StatusChanged {
            element_id: id.clone(),
            old_status,
            new_status,
            changed_by: agent_id.to_string(),
        });

        Ok(element)
    }

    /// Find elements blocking the given element
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

    /// Check if element can start (no blocking dependencies)
    pub fn can_start(&self, id: &ElementId) -> Result<bool, KanbanError> {
        let blockers = self.find_blocking_dependencies(id)?;
        Ok(blockers.is_empty())
    }

    /// List elements by sprint
    pub fn list_by_sprint(&self, sprint_id: &ElementId)
        -> Result<(Sprint, Vec<Story>, Vec<Task>), KanbanError>
    {
        let sprint = self.repository.get(sprint_id)?
            .and_then(|e| match e {
                KanbanElement::Sprint(s) => Some(s),
                _ => None,
            })
            .ok_or(KanbanError::NotFound(sprint_id.clone()))?;

        let stories = self.repository.list_by_parent(sprint_id)?
            .into_iter()
            .filter_map(|e| match e {
                KanbanElement::Story(s) => Some(s),
                _ => None,
            })
            .collect();

        let mut tasks = Vec::new();
        for story in self.repository.list_by_parent(sprint_id)? {
            if let KanbanElement::Story(s) = story {
                for task in self.repository.list_by_parent(s.id())? {
                    if let KanbanElement::Task(t) = task {
                        tasks.push(t);
                    }
                }
            }
        }

        Ok((sprint, stories, tasks))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementUpdates {
    pub title: Option<String>,
    pub content: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub effort: Option<u32>,
    pub blocked_reason: Option<String>,
    pub tags: Option<Vec<String>>,
    pub dependencies: Option<Vec<ElementId>>,
    pub references: Option<Vec<ElementId>>,
    #[serde(default)]
    pub override_permission: bool,
}
```

### 8.4 Event System

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KanbanEvent {
    Created {
        element_id: ElementId,
        element_type: ElementType,
    },
    Updated {
        element_id: ElementId,
        changes: ChangeSummary,
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

pub struct KanbanEventBus {
    subscribers: RwLock<Vec<Box<dyn KanbanEventSubscriber + Send>>>,
}

pub trait KanbanEventSubscriber: Send {
    fn on_event(&self, event: &KanbanEvent);
}

impl KanbanEventBus {
    pub fn subscribe(&self, subscriber: Box<dyn KanbanEventSubscriber + Send>) {
        self.subscribers.write().unwrap().push(subscriber);
    }

    pub fn publish(&self, event: KanbanEvent) {
        for subscriber in self.subscribers.read().unwrap().iter() {
            subscriber.on_event(&event);
        }
    }
}
```

### 8.5 Git Operations

```rust
pub struct GitOperations {
    repo_path: PathBuf,
}

impl GitOperations {
    /// Commit all kanban changes
    pub fn commit_changes(&self, agent_id: &str, message: &str)
        -> Result<git2::Commit, GitError>
    {
        let repo = git2::Repository::open(&self.repo_path)?;

        // Stage all changes in kanban directory
        let mut index = repo.index()?;
        index.add_path(Path::new("kanban"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        // Get parent commit
        let head = repo.head()?;
        let parent = head.peel_to_commit()?;

        // Create commit
        let signature = git2::Signature::now(agent_id, "agent@agile-agent")?;
        let commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent],
        )?;

        Ok(repo.find_commit(commit)?)
    }

    /// Fetch and rebase on latest changes
    pub fn fetch_and_rebase(&self, branch: &str) -> Result<(), GitError> {
        // Standard fetch + rebase workflow
    }

    /// Check for unresolved conflicts
    pub fn has_conflicts(&self) -> bool {
        // Check for conflict markers in files
    }
}
```

## 9. Integration with Multi-Agent Runtime

### 9.1 SharedWorkplaceState Integration

The kanban service is accessed through `SharedWorkplaceState`:

```rust
// In core/src/shared_state.rs

pub struct SharedWorkplaceState {
    /// Workplace identity
    workplace_id: WorkplaceId,

    /// Kanban service (shared across all agents)
    kanban: Arc<KanbanService<FileKanbanRepository>>,

    /// Kanban event bus for real-time updates
    kanban_events: Arc<KanbanEventBus>,

    /// Skills registry
    skills: SkillRegistry,

    /// Current working directory
    cwd: PathBuf,

    /// Global loop control
    loop_run_active: bool,

    /// Global app flags
    should_quit: bool,
}

impl SharedWorkplaceState {
    pub fn kanban(&self) -> Arc<KanbanService<FileKanbanRepository>> {
        self.kanban.clone()
    }

    pub fn subscribe_to_kanban_events(
        &self,
        subscriber: Box<dyn KanbanEventSubscriber + Send>,
    ) {
        self.kanban_events.subscribe(subscriber);
    }
}
```

### 9.2 Agent Access Pattern

Agents access kanban through their slot's context:

```rust
// In AgentSlot or agent context
impl AgentSlot {
    pub fn read_kanban(&self) -> Arc<KanbanService<FileKanbanRepository>> {
        self.workplace.kanban()
    }

    pub fn update_task_status(&self, task_id: &ElementId, new_status: Status)
        -> Result<(), KanbanError>
    {
        // Agents can only update elements assigned to them
        let kanban = self.workplace.kanban();
        kanban.update_status(task_id, new_status, self.agent_id.as_str())
    }

    pub fn append_tip(&self, task_id: &ElementId, content: &str)
        -> Result<(), KanbanError>
    {
        let tip = Tips::new(
            format!("Tip from {}", self.agent_id),
            content.to_string(),
            task_id.clone(),
            self.agent_id.clone(),
        );
        self.workplace.kanban().append_tip(task_id, tip)
    }
}
```

### 9.3 TUI Integration

TUI subscribes to kanban events to update board view:

```rust
// In TUI, when rendering kanban board
impl TuiState {
    fn subscribe_to_kanban_events(state: &SharedWorkplaceState) {
        state.subscribe_to_kanban_events(Box::new(|event| {
            // Invalidate kanban view cache on any change
            self.kanban_view_dirty = true;
        }));
    }
}
```

## 10. Microservice Extraction Path

The architecture supports future extraction to a standalone microservice:

### 10.1 Service Boundary

```
┌─────────────────────────────────────────────────────────────┐
│                     agile-agent (TUI/CLI)                    │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           KanbanClient (RPC/HTTP client)                 │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │ RPC
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   kanban-service (future)                    │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   API Layer (axum/gRPC)                  │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                 Application Service                       │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   Repository Layer                       │ │
│  │  (Postgres/SQLite backed for production)                │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 10.2 Interface Preservation

The `KanbanElementRepository` trait and `KanbanService` remain the same — only the implementation changes from `FileKanbanRepository` to `DbKanbanRepository` or `RpcKanbanRepository`.

```rust
/// New implementation for microservice
pub struct DbKanbanRepository {
    pool: sqlx::PgPool,
}

#[async_trait]
impl KanbanElementRepository for DbKanbanRepository {
    async fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        // Query from database
    }
}

/// Client for microservice (replaces direct repository access)
pub struct RpcKanbanRepository {
    client: KanbanServiceClient,
}

#[async_trait]
impl KanbanElementRepository for RpcKanbanRepository {
    async fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        let response = self.client.get_element(GetElementRequest {
            id: id.to_string(),
        }).await?;
        Ok(Some(response.element))
    }
}
```

## 11. Git-Based Collaboration

The `kanban/` directory lives inside a Git repository (the workplace itself). Agents collaborate using standard Git flow:

1. Each agent works on its own branch
2. Before committing, agent fetches and rebases/merges from the shared branch
3. Agent commits its changes and pushes
4. Conflicts are resolved via Git merge/rebase tools

### 11.1 Permissions Model

The permission model is advisory, not enforced technically:

- Agents should only modify elements where `assignee` matches their identity
- Agents may append tips to any task (append-only operation)
- In special cases, agents may modify elements outside their assignment — this is allowed but should be reviewed
- Conflicts in Git indicate a design problem: if two agents modify the same element, the design should be reconsidered to give each agent distinct ownership

### 11.2 Conflict Discovery

Since agents traverse the `elements/` directory directly, they naturally discover each other's work. Git's merge conflict markers surface any concurrent modifications to the same file, making conflicts visible and solvable via standard Git workflows.

## 12. ID Generation

Element IDs follow a human-readable format: `{type}-{number}`

Examples: `sprint-001`, `story-042`, `task-123`, `idea-007`, `issue-001`, `tip-001`

The number is sequential within each type, starting from 001. IDs must be unique across all element types.

Counter persistence: the next available number for each type is stored in a `counter-{type}.txt` file or embedded in index.json.

## 13. Storage Architecture Considerations

The design separates storage concerns from the core domain model to allow future refactoring:

- **File format** (JSON) is human-readable and Git-mergeable, not a binary or opaque format
- **Repository trait** abstracts storage, enabling file → database migration
- **Event bus** decouples storage updates from UI/external watchers
- **Service layer** contains all business logic, independent of storage
- **index.json** is intentionally minimal — the source of truth is the actual element files
- **No database** in v1 — plain files enable Git versioning, diffing, and branching
- **Append-only tips** — tips as independent files prevent read-write conflicts

## 14. Resolved Decisions

- All six element types share one status machine
- Dependencies and references are both many-to-many ID lists
- `index.json` stores only an ID list, not full metadata
- Elements live in `elements/{id}.json` (no type subdirectory in path)
- Tips are independent elements, not embedded in task files
- Git flow is the collaboration model; permissions are advisory
- Storage path: `~/.agile-agent/workplaces/{id}/kanban/`
- Architecture follows clean architecture with separate domain/repository/service layers
- Event bus for publish/subscribe notifications

## 15. Future Enhancements (TODO)

These features are identified but deferred to future sprints.

### 15.1 WIP Limits

Per-status WIP (Work In Progress) limits to prevent overloading:

```rust
pub struct WipLimits {
    pub ready: Option<usize>,       // None = unlimited
    pub in_progress: Option<usize>,
    pub blocked: Option<usize>,
}

impl WipLimits {
    pub fn check(&self, current_counts: &HashMap<Status, usize>) -> Vec<WipViolation> {
        // Return list of violations if limits exceeded
    }
}
```

**Implementation consideration**: WIP limits could be stored in sprint or in a separate `wip-limits.json` file.

### 15.2 Automated Daily Standup Generation

Generate daily standup report from status_history:

```
Daily Standup - 2026-04-14

Agent Alpha:
- Yesterday: task-001 (ready→done), task-002 (done)
- Today: task-003 (ready)
- Blockers: none

Agent Bravo:
- Yesterday: story-002 (backlog→in_progress)
- Today: story-002 (in_progress)
- Blockers: story-002 waiting on story-001
```

**Implementation consideration**: This can be derived from status_history without additional fields.

### 15.3 Sprint Burndown Chart Data

Track remaining work over time:

```rust
pub struct BurndownDataPoint {
    date: NaiveDate,
    remaining_tasks: usize,
    remaining_stories: usize,
    ideal_remaining: f32,  // Linear ideal line
}

pub struct SprintBurndown {
    sprint_id: ElementId,
    data_points: Vec<BurndownDataPoint>,
}
```

**Implementation consideration**: Requires daily snapshot of backlog state. Could be computed on-demand from history or stored separately.

### 15.4 Definition of Done Checklist

Stories may include a checklist of completion criteria:

```json
{
  "id": "story-001",
  "definition_of_done": [
    "code_review_approved",
    "tests_written",
    "documentation_updated",
    "deployed_to_staging"
  ],
  "done_checklist": {
    "code_review_approved": true,
    "tests_written": true,
    "documentation_updated": false,
    "deployed_to_staging": false
  }
}
```

**Implementation consideration**: Requires UI support for checklist display and checking. Complex to enforce across agents.

### 15.5 Cross-Sprint Dependencies Visualization

A dependency graph showing:
- Which elements reference items from other sprints
- Circular dependency detection
- Critical path identification

**Implementation consideration**: Requires graph algorithms (DFS for cycle detection, topological sort for ordering).

### 15.6 Agent Productivity Metrics

Track per-agent metrics:
- Tasks completed per sprint
- Average cycle time
- Escalation rate
- Tip contribution count

**Implementation consideration**: Derived from status_history and element creation. Requires aggregation service.

### 15.7 Capacity Planning

Match task workload to agent capacity:

```rust
pub struct AgentCapacity {
    agent_id: String,
    sprint_hours_available: f32,
    estimated_hours_per_task: HashMap<ElementId, f32>,
}

impl AgentCapacity {
    pub fn total_allocated(&self) -> f32;
    pub fn remaining_capacity(&self) -> f32;
    pub fn is_overallocated(&self) -> bool;
}
```

**Implementation consideration**: Requires effort estimation discipline. May not be suitable for AI agents that don't estimate like humans.

## 16. Open Questions

None at this time. All key design decisions have been resolved through the brainstorming process.

## 17. References

- `docs/plan/multi-agent-parallel-runtime-design.md` — related multi-agent architecture
- `docs/superpowers/specs/2026-04-13-debug-logging-and-observability-design.md` — logging system
- `docs/plan/v2-sprint-1-backlog-and-task-spec.md` — existing backlog and task model
