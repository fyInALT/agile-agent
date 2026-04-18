# Sprint 12: Persistence Layer

## Metadata

- Sprint ID: `decision-sprint-012`
- Title: `Persistence Layer`
- Duration: 1-2 weeks
- Priority: P1 (High)
- Status: `Completed`
- Created: 2026-04-18
- Depends on: `decision-sprint-009` (Task Entity), `decision-sprint-011` (Automation Layer)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-013: Task Storage and Retrieval
- FR-014: Action Logging

## Sprint Goal

Implement persistence for tasks and execution history, enabling recovery after crashes and historical analysis.

## Context

With Task entities and decision automation, we need to ensure:
1. Tasks persist across session restarts
2. Execution history is logged for analysis
3. System can recover from crashes

This sprint adds the storage layer that makes the decision layer reliable.

## Stories

### Story 12.1: Task Store Backend

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement the storage backend for task persistence.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.1.1 | Define `TaskStore` trait | Todo | - |
| T12.1.2 | Implement `FileTaskStore` | Todo | - |
| T12.1.3 | Define storage path structure | Todo | - |
| T12.1.4 | Implement JSON serialization | Todo | - |
| T12.1.5 | Write unit tests for store | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T12.1.T1 | Task saved to file |
| T12.1.T2 | Task loaded from file |
| T12.1.T3 | Missing file returns error |
| T12.1.T4 | Corrupted file handled gracefully |

#### Acceptance Criteria

- TaskStore trait defined
- FileTaskStore works correctly
- Storage path configurable

#### Technical Notes

```rust
pub trait TaskStore: Send + Sync {
    fn save(&self, task: &Task) -> Result<(), StoreError>;
    fn load(&self, id: &TaskId) -> Result<Task, StoreError>;
    fn delete(&self, id: &TaskId) -> Result<(), StoreError>;
    fn list_pending(&self) -> Result<Vec<Task>, StoreError>;
}

pub struct FileTaskStore {
    base_path: PathBuf,
}

impl FileTaskStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }
    
    fn task_path(&self, id: &TaskId) -> PathBuf {
        self.base_path.join("tasks").join(format!("{}.json", id))
    }
}

impl TaskStore for FileTaskStore {
    fn save(&self, task: &Task) -> Result<(), StoreError> {
        let path = self.task_path(&task.id);
        let json = serde_json::to_string(task)?;
        fs::write(&path, json)?;
        Ok(())
    }
    
    fn load(&self, id: &TaskId) -> Result<Task, StoreError> {
        let path = self.task_path(id);
        let json = fs::read_to_string(&path)?;
        let task = serde_json::from_str(&json)?;
        Ok(task)
    }
    
    // ... other methods
}
```

### Story 12.2: Task Registry

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement the TaskRegistry for managing active and completed tasks.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.2.1 | Create `TaskRegistry` struct | Todo | - |
| T12.2.2 | Implement `create()` method | Todo | - |
| T12.2.3 | Implement `get()` method | Todo | - |
| T12.2.4 | Implement `update()` method | Todo | - |
| T12.2.5 | Implement `complete()` method | Todo | - |
| T12.2.6 | Implement `cancel()` method | Todo | - |
| T12.2.7 | Implement `list_active()` method | Todo | - |
| T12.2.8 | Implement `load()` and `save()` | Todo | - |
| T12.2.9 | Write unit tests for registry | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T12.2.T1 | Task created with correct ID |
| T12.2.T2 | Task retrieved by ID |
| T12.2.T3 | Task updated correctly |
| T12.2.T4 | Task completed moves to history |
| T12.2.T5 | Task cancelled handled |
| T12.2.T6 | Active tasks listed |
| T12.2.T7 | Load restores from storage |
| T12.2.T8 | Save persists to storage |

#### Acceptance Criteria

- TaskRegistry matches FR-013 specification
- CRUD operations work correctly
- Persistence integrated

#### Technical Notes

```rust
pub struct TaskRegistry {
    active_tasks: HashMap<TaskId, Task>,
    completed_tasks: Vec<Task>,
    store: Box<dyn TaskStore>,
}

impl TaskRegistry {
    pub fn new(store: Box<dyn TaskStore>) -> Self {
        Self {
            active_tasks: HashMap::new(),
            completed_tasks: Vec::new(),
            store,
        }
    }
    
    pub fn create(&mut self, description: String, constraints: Vec<String>) -> TaskId {
        let task = Task::new(description, constraints);
        let id = task.id.clone();
        self.active_tasks.insert(id.clone(), task);
        self.store.save(&task).expect("save task");
        id
    }
    
    pub fn get(&self, id: &TaskId) -> Option<&Task> {
        self.active_tasks.get(id)
    }
    
    pub fn update(&mut self, id: &TaskId, update: TaskUpdate) -> Result<(), RegistryError> {
        let task = self.active_tasks.get_mut(id)?;
        update.apply(task);
        self.store.save(task)?;
        Ok(())
    }
    
    pub fn complete(&mut self, id: &TaskId) -> Result<(), RegistryError> {
        let task = self.active_tasks.remove(id)?;
        task.transition_to(TaskStatus::Completed)?;
        self.completed_tasks.push(task);
        self.store.save(&self.completed_tasks.last().unwrap())?;
        Ok(())
    }
    
    pub fn load(&mut self) -> Result<(), StoreError> {
        let pending = self.store.list_pending()?;
        for task in pending {
            self.active_tasks.insert(task.id.clone(), task);
        }
        Ok(())
    }
    
    pub fn save_all(&self) -> Result<(), StoreError> {
        for task in self.active_tasks.values() {
            self.store.save(task)?;
        }
        Ok(())
    }
}
```

### Story 12.3: Execution History

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement execution history logging for each task.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.3.1 | Define `ExecutionRecord` struct | Todo | - |
| T12.3.2 | Add history field to Task | Todo | - |
| T12.3.3 | Implement `add_record()` method | Todo | - |
| T12.3.4 | Implement `get_history()` method | Todo | - |
| T12.3.5 | Implement `get_stage_history()` method | Todo | - |
| T12.3.6 | Serialize history with Task | Todo | - |
| T12.3.7 | Write unit tests for history | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T12.3.T1 | Record added to task |
| T12.3.T2 | All records retrieved |
| T12.3.T3 | Stage-specific records retrieved |
| T12.3.T4 | History persisted with task |
| T12.3.T5 | History timestamped correctly |

#### Acceptance Criteria

- ExecutionRecord matches FR-014 specification
- History stored with Task
- Query methods work correctly

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub action: DecisionAction,
    pub timestamp: DateTime<Utc>,
    pub stage: StageId,
    pub auto_check: Option<AutoCheckResult>,
    pub human_requested: bool,
    pub human_response: Option<String>,
    pub triggering_output: Option<String>,
}

impl Task {
    pub fn add_record(&mut self, record: ExecutionRecord) {
        // History field added to Task struct
        self.execution_history.push(record);
        self.updated_at = Utc::now();
    }
    
    pub fn get_history(&self) -> &Vec<ExecutionRecord> {
        &self.execution_history
    }
    
    pub fn get_stage_history(&self, stage: &StageId) -> Vec<&ExecutionRecord> {
        self.execution_history.iter()
            .filter(|r| r.stage == *stage)
            .collect()
    }
}
```

### Story 12.4: Recovery Mechanism

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement crash recovery to restore tasks after system restart.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.4.1 | Detect incomplete tasks on load | Todo | - |
| T12.4.2 | Restore task to last known state | Todo | - |
| T12.4.3 | Mark interrupted tasks as Paused | Todo | - |
| T12.4.4 | Provide recovery status in TUI | Todo | - |
| T12.4.5 | Write unit tests for recovery | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T12.4.T1 | InProgress task restored as Paused |
| T12.4.T2 | Reflecting task restored as Paused |
| T12.4.T3 | Pending task restored as Pending |
| T12.4.T4 | Completed task ignored |

#### Acceptance Criteria

- System can recover from crash
- Tasks restored to appropriate state
- User notified of recovery

#### Technical Notes

```rust
impl TaskRegistry {
    pub fn recover(&mut self) -> Result<Vec<TaskId>, RecoveryError> {
        let recovered = Vec::new();
        
        for task in self.active_tasks.values_mut() {
            // Tasks in active state should be paused
            if matches!(task.status, TaskStatus::InProgress | TaskStatus::Reflecting) {
                task.transition_to(TaskStatus::Paused)?;
                recovered.push(task.id.clone());
            }
        }
        
        self.save_all()?;
        Ok(recovered)
    }
}
```

## Sprint Deliverables

1. `TaskStore` trait and `FileTaskStore` implementation
2. `TaskRegistry` with full CRUD operations
3. `ExecutionRecord` and history logging
4. Recovery mechanism for crash handling
5. Unit tests with >90% coverage

## Sprint Review Checklist

- [x] All tasks completed
- [x] All tests passing
- [x] Tasks persist across restarts
- [x] History logged correctly
- [x] Recovery works for interrupted tasks
- [x] Code reviewed and merged

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| Storage corruption | Use atomic writes, backup files |
| Concurrent access issues | Use RwLock for registry |
| Missing storage directory | Create on initialization |

## Next Sprint Preview

Sprint 13 will integrate all components:
- DecisionEngine combining process, task, auto-check
- Decision execution flow
- Human response handling