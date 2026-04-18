//! Persistence layer for task concept (Sprint 12)
//!
//! Provides task storage, registry, and execution history logging.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::automation::AutoCheckResult;
use crate::task::{Task, TaskId, TaskStatus, TransitionError};
use crate::workflow::{StageId, WorkflowAction};

// ============================================================================
// Story 12.3: Execution History (defined first as Task depends on it)
// ============================================================================

/// Execution record for action logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Action taken
    pub action: WorkflowAction,
    /// Timestamp of action
    pub timestamp: DateTime<Utc>,
    /// Stage when action was taken
    pub stage: StageId,
    /// Auto-check result (if applicable)
    pub auto_check_result: Option<String>,
    /// Whether human was requested
    pub human_requested: bool,
    /// Human response (if provided)
    pub human_response: Option<String>,
    /// Output that triggered this action
    pub triggering_output: Option<String>,
}

impl ExecutionRecord {
    /// Create a new execution record
    pub fn new(action: WorkflowAction, stage: StageId) -> Self {
        Self {
            action,
            timestamp: Utc::now(),
            stage,
            auto_check_result: None,
            human_requested: false,
            human_response: None,
            triggering_output: None,
        }
    }

    /// Create with auto-check result
    pub fn with_auto_check(action: WorkflowAction, stage: StageId, result: &AutoCheckResult) -> Self {
        Self {
            action,
            timestamp: Utc::now(),
            stage,
            auto_check_result: Some(format!("{:?}", result)),
            human_requested: matches!(result, AutoCheckResult::NeedsHuman { .. }),
            human_response: None,
            triggering_output: None,
        }
    }

    /// Add human response
    pub fn with_human_response(mut self, response: String) -> Self {
        self.human_response = Some(response);
        self
    }

    /// Add triggering output
    pub fn with_output(mut self, output: String) -> Self {
        self.triggering_output = Some(output);
        self
    }
}

// ============================================================================
// Story 12.1: Task Store Backend
// ============================================================================

/// Error type for storage operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Task not found: {0}")]
    NotFound(String),
    #[error("Corrupted data: {0}")]
    Corrupted(String),
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serialization(e.to_string())
    }
}

/// Task storage trait
pub trait TaskStore: Send + Sync {
    /// Save a task
    fn save(&self, task: &Task) -> Result<(), StoreError>;
    /// Load a task by ID
    fn load(&self, id: &TaskId) -> Result<Task, StoreError>;
    /// Delete a task
    fn delete(&self, id: &TaskId) -> Result<(), StoreError>;
    /// List all pending (active) tasks
    fn list_pending(&self) -> Result<Vec<Task>, StoreError>;
    /// List all completed tasks
    fn list_completed(&self) -> Result<Vec<Task>, StoreError>;
    /// Move a task to completed storage
    fn move_to_completed(&self, task: &Task) -> Result<(), StoreError>;
}

/// File-based task store
pub struct FileTaskStore {
    base_path: PathBuf,
}

impl FileTaskStore {
    /// Create a new file store
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Get path for a task file
    fn task_path(&self, id: &TaskId) -> PathBuf {
        self.base_path.join("tasks").join(format!("{}.json", id))
    }

    /// Get path for tasks directory
    fn tasks_dir(&self) -> PathBuf {
        self.base_path.join("tasks")
    }

    /// Get path for completed tasks directory
    fn completed_dir(&self) -> PathBuf {
        self.base_path.join("completed")
    }

    /// Ensure directories exist
    fn ensure_dirs(&self) -> Result<(), StoreError> {
        fs::create_dir_all(self.tasks_dir())?;
        fs::create_dir_all(self.completed_dir())?;
        Ok(())
    }
}

impl TaskStore for FileTaskStore {
    fn save(&self, task: &Task) -> Result<(), StoreError> {
        self.ensure_dirs()?;
        let path = self.task_path(&task.id);
        let json = serde_json::to_string(task)?;
        fs::write(&path, json)?;
        Ok(())
    }

    fn load(&self, id: &TaskId) -> Result<Task, StoreError> {
        let path = self.task_path(id);
        if !path.exists() {
            // Check completed directory
            let completed_path = self.completed_dir().join(format!("{}.json", id));
            if completed_path.exists() {
                let json = fs::read_to_string(&completed_path)?;
                return serde_json::from_str(&json).map_err(StoreError::from);
            }
            return Err(StoreError::NotFound(id.to_string()));
        }

        let json = fs::read_to_string(&path)?;
        serde_json::from_str(&json).map_err(|e| StoreError::Corrupted(e.to_string()))
    }

    fn delete(&self, id: &TaskId) -> Result<(), StoreError> {
        let path = self.task_path(id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let completed_path = self.completed_dir().join(format!("{}.json", id));
        if completed_path.exists() {
            fs::remove_file(&completed_path)?;
        }
        Ok(())
    }

    fn list_pending(&self) -> Result<Vec<Task>, StoreError> {
        self.ensure_dirs()?;
        let tasks_dir = self.tasks_dir();
        let mut tasks = Vec::new();

        for entry in fs::read_dir(&tasks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let json = fs::read_to_string(&path)?;
                if let Ok(task) = serde_json::from_str::<Task>(&json) {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }

    fn list_completed(&self) -> Result<Vec<Task>, StoreError> {
        let completed_dir = self.completed_dir();
        if !completed_dir.exists() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::new();
        for entry in fs::read_dir(&completed_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let json = fs::read_to_string(&path)?;
                if let Ok(task) = serde_json::from_str::<Task>(&json) {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }

    fn move_to_completed(&self, task: &Task) -> Result<(), StoreError> {
        self.ensure_dirs()?;
        let active_path = self.task_path(&task.id);
        let completed_path = self.completed_dir().join(format!("{}.json", task.id));

        if active_path.exists() {
            fs::rename(&active_path, &completed_path)?;
        } else {
            // Save directly to completed if not in active
            let json = serde_json::to_string(task)?;
            fs::write(&completed_path, json)?;
        }
        Ok(())
    }
}

// ============================================================================
// Story 12.2: Task Registry
// ============================================================================

/// Error type for registry operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum RegistryError {
    #[error("Task not found: {0}")]
    NotFound(String),
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    #[error("Lock poisoned: {0}")]
    LockError(String),
}

impl From<TransitionError> for RegistryError {
    fn from(e: TransitionError) -> Self {
        RegistryError::InvalidTransition(e.to_string())
    }
}

/// Task update specification
#[derive(Debug, Clone)]
pub struct TaskUpdate {
    status: Option<TaskStatus>,
    reflection_count: Option<usize>,
    confirmation_count: Option<usize>,
    add_history: Option<ExecutionRecord>,
}

impl TaskUpdate {
    /// Create status update
    pub fn status(status: TaskStatus) -> Self {
        Self { status: Some(status), reflection_count: None, confirmation_count: None, add_history: None }
    }

    /// Create reflection count update
    pub fn reflection_count(count: usize) -> Self {
        Self { status: None, reflection_count: Some(count), confirmation_count: None, add_history: None }
    }

    /// Create confirmation count update
    pub fn confirmation_count(count: usize) -> Self {
        Self { status: None, reflection_count: None, confirmation_count: Some(count), add_history: None }
    }

    /// Add history record
    pub fn add_history(record: ExecutionRecord) -> Self {
        Self { status: None, reflection_count: None, confirmation_count: None, add_history: Some(record) }
    }

    /// Apply update to task
    pub fn apply(&self, task: &mut Task) {
        if let Some(status) = self.status {
            task.status = status;
        }
        if let Some(count) = self.reflection_count {
            task.reflection_count = count;
        }
        if let Some(count) = self.confirmation_count {
            task.confirmation_count = count;
        }
        if let Some(record) = &self.add_history {
            task.execution_history.push(record.clone());
        }
        task.updated_at = Utc::now();
    }
}

/// Task registry for managing active and completed tasks
pub struct TaskRegistry {
    /// Active tasks
    active_tasks: RwLock<HashMap<TaskId, Task>>,
    /// Completed tasks
    completed_tasks: RwLock<Vec<Task>>,
    /// Storage backend
    store: RwLock<Box<dyn TaskStore>>,
}

impl TaskRegistry {
    /// Create a new registry
    pub fn new(store: Box<dyn TaskStore>) -> Self {
        Self {
            active_tasks: RwLock::new(HashMap::new()),
            completed_tasks: RwLock::new(Vec::new()),
            store: RwLock::new(store),
        }
    }

    /// Create a new task
    pub fn create(&self, description: String, constraints: Vec<String>) -> Result<TaskId, RegistryError> {
        let task = Task::new(description, constraints);
        let id = task.id.clone();

        {
            let mut active = self.active_tasks.write().map_err(|_| RegistryError::LockError("active_tasks write lock poisoned".into()))?;
            active.insert(id.clone(), task.clone());
        }

        {
            let store = self.store.read().map_err(|_| RegistryError::LockError("store read lock poisoned".into()))?;
            store.save(&task)?;
        }

        Ok(id)
    }

    /// Get a task by ID
    pub fn get(&self, id: &TaskId) -> Option<Task> {
        self.active_tasks.read().map(|active| active.get(id).cloned()).unwrap_or(None)
    }

    /// Get a completed task by ID
    pub fn get_completed(&self, id: &TaskId) -> Option<Task> {
        self.completed_tasks.read().map(|completed| completed.iter().find(|t| t.id == *id).cloned()).unwrap_or(None)
    }

    /// Update a task
    pub fn update(&self, id: &TaskId, update: TaskUpdate) -> Result<(), RegistryError> {
        let mut active = self.active_tasks.write().map_err(|_| RegistryError::LockError("active_tasks write lock poisoned".into()))?;
        let task = active.get_mut(id).ok_or_else(|| RegistryError::NotFound(id.to_string()))?;
        update.apply(task);

        let store = self.store.read().map_err(|_| RegistryError::LockError("store read lock poisoned".into()))?;
        store.save(task)?;
        Ok(())
    }

    /// Complete a task
    pub fn complete(&self, id: &TaskId) -> Result<(), RegistryError> {
        let mut active = self.active_tasks.write().map_err(|_| RegistryError::LockError("active_tasks write lock poisoned".into()))?;
        let task = active.remove(id).ok_or_else(|| RegistryError::NotFound(id.to_string()))?;

        let mut task = task;
        task.transition_to(TaskStatus::Completed)?;

        {
            let mut completed = self.completed_tasks.write().map_err(|_| RegistryError::LockError("completed_tasks write lock poisoned".into()))?;
            completed.push(task.clone());
        }

        {
            let store = self.store.read().map_err(|_| RegistryError::LockError("store read lock poisoned".into()))?;
            store.move_to_completed(&task)?;
        }

        Ok(())
    }

    /// Cancel a task
    pub fn cancel(&self, id: &TaskId, reason: String) -> Result<(), RegistryError> {
        let mut active = self.active_tasks.write().map_err(|_| RegistryError::LockError("active_tasks write lock poisoned".into()))?;
        let task = active.get_mut(id).ok_or_else(|| RegistryError::NotFound(id.to_string()))?;

        task.transition_to(TaskStatus::Cancelled)?;

        // Add cancellation record
        let record = ExecutionRecord::new(
            WorkflowAction::Cancel { reason },
            StageId::new("cancelled"),
        );
        task.execution_history.push(record);

        // Remove from active and add to completed
        let task = active.remove(id).ok_or_else(|| RegistryError::NotFound(id.to_string()))?;
        {
            let mut completed = self.completed_tasks.write().map_err(|_| RegistryError::LockError("completed_tasks write lock poisoned".into()))?;
            completed.push(task.clone());
        }

        {
            let store = self.store.read().map_err(|_| RegistryError::LockError("store read lock poisoned".into()))?;
            store.move_to_completed(&task)?;
        }

        Ok(())
    }

    /// List all active tasks
    pub fn list_active(&self) -> Vec<Task> {
        self.active_tasks.read().map(|active| active.values().cloned().collect()).unwrap_or_default()
    }

    /// List all completed tasks
    pub fn list_completed(&self) -> Vec<Task> {
        self.completed_tasks.read().map(|completed| completed.clone()).unwrap_or_default()
    }

    /// Load tasks from storage
    pub fn load(&self) -> Result<(), StoreError> {
        let store = self.store.read().map_err(|_| StoreError::Io("store read lock poisoned".into()))?;

        let pending = store.list_pending()?;
        {
            let mut active = self.active_tasks.write().map_err(|_| StoreError::Io("active_tasks write lock poisoned".into()))?;
            for task in pending {
                active.insert(task.id.clone(), task);
            }
        }

        let completed = store.list_completed()?;
        {
            let mut completed_tasks = self.completed_tasks.write().map_err(|_| StoreError::Io("completed_tasks write lock poisoned".into()))?;
            completed_tasks.extend(completed);
        }

        Ok(())
    }

    /// Save all active tasks
    pub fn save_all(&self) -> Result<(), StoreError> {
        let store = self.store.read().map_err(|_| StoreError::Io("store read lock poisoned".into()))?;
        let active = self.active_tasks.read().map_err(|_| StoreError::Io("active_tasks read lock poisoned".into()))?;
        for task in active.values() {
            store.save(task)?;
        }
        Ok(())
    }

    /// Get count of active tasks
    pub fn active_count(&self) -> usize {
        self.active_tasks.read().map(|active| active.len()).unwrap_or(0)
    }

    /// Get count of completed tasks
    pub fn completed_count(&self) -> usize {
        self.completed_tasks.read().map(|completed| completed.len()).unwrap_or(0)
    }
}

// ============================================================================
// Story 12.4: Recovery Mechanism
// ============================================================================

/// Recovery error
#[derive(Debug, Clone, thiserror::Error)]
pub enum RecoveryError {
    #[error("Transition error: {0}")]
    Transition(String),
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
}

impl From<TransitionError> for RecoveryError {
    fn from(e: TransitionError) -> Self {
        RecoveryError::Transition(e.to_string())
    }
}

impl TaskRegistry {
    /// Recover tasks after crash
    ///
    /// Tasks in InProgress or Reflecting state are moved to Paused.
    pub fn recover(&self) -> Result<Vec<TaskId>, RecoveryError> {
        let mut recovered = Vec::new();

        // First, identify and transition tasks needing recovery
        {
            let mut active = self.active_tasks.write().map_err(|_| RecoveryError::Transition("active_tasks write lock poisoned".into()))?;
            for task in active.values_mut() {
                if matches!(task.status, TaskStatus::InProgress | TaskStatus::Reflecting) {
                    task.transition_to(TaskStatus::Paused)
                        .map_err(RecoveryError::from)?;
                    recovered.push(task.id.clone());
                }
            }
        } // Release write lock before save_all

        self.save_all()?;
        Ok(recovered)
    }

    /// Check if there are tasks needing recovery
    pub fn needs_recovery(&self) -> bool {
        self.active_tasks.read().map(|active| {
            active.values().any(|t| matches!(t.status, TaskStatus::InProgress | TaskStatus::Reflecting))
        }).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Story 12.1 Tests: Task Store Backend

    #[test]
    fn t12_1_t1_task_saved_to_file() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());
        let task = Task::new("Test task".to_string(), vec!["constraint".to_string()]);

        store.save(&task).expect("save");

        let path = store.task_path(&task.id);
        assert!(path.exists(), "Task file should exist");
    }

    #[test]
    fn t12_1_t2_task_loaded_from_file() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());
        let original = Task::new("Test task".to_string(), vec!["c1".to_string()]);

        store.save(&original).expect("save");
        let loaded = store.load(&original.id).expect("load");

        assert_eq!(loaded.id, original.id);
        assert_eq!(loaded.description, original.description);
        assert_eq!(loaded.constraints, original.constraints);
    }

    #[test]
    fn t12_1_t3_missing_file_returns_error() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());
        let id = TaskId::generate();

        let result = store.load(&id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StoreError::NotFound(_)));
    }

    #[test]
    fn t12_1_t4_corrupted_file_handled_gracefully() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());
        let task = Task::new("Test".to_string(), vec![]);
        store.save(&task).expect("save");

        // Corrupt the file
        let path = store.task_path(&task.id);
        fs::write(&path, "not valid json").expect("write corrupt");

        let result = store.load(&task.id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StoreError::Corrupted(_)));
    }

    #[test]
    fn t12_1_t5_list_pending_returns_tasks() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());

        let task1 = Task::new("Task 1".to_string(), vec![]);
        let task2 = Task::new("Task 2".to_string(), vec![]);
        store.save(&task1).expect("save");
        store.save(&task2).expect("save");

        let pending = store.list_pending().expect("list");
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn t12_1_t6_delete_removes_task() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());
        let task = Task::new("Test".to_string(), vec![]);
        store.save(&task).expect("save");

        store.delete(&task.id).expect("delete");

        let result = store.load(&task.id);
        assert!(result.is_err());
    }

    // Story 12.2 Tests: Task Registry

    #[test]
    fn t12_2_t1_task_created_with_correct_id() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Test task".to_string(), vec!["c1".to_string()]).expect("create");

        let task = registry.get(&id).expect("task");
        assert_eq!(task.description, "Test task");
        assert_eq!(task.constraints, vec!["c1"]);
    }

    #[test]
    fn t12_2_t2_task_retrieved_by_id() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        let task = registry.get(&id);

        assert!(task.is_some());
        assert_eq!(task.unwrap().id, id);
    }

    #[test]
    fn t12_2_t3_task_updated_correctly() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        registry.update(&id, TaskUpdate::reflection_count(3)).expect("update");

        let task = registry.get(&id).expect("task");
        assert_eq!(task.reflection_count, 3);
    }

    #[test]
    fn t12_2_t4_task_completed_moves_to_history() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        // Transition through proper workflow: Pending → InProgress → PendingConfirmation → Completed
        registry.update(&id, TaskUpdate::status(TaskStatus::InProgress)).expect("update to InProgress");
        registry.update(&id, TaskUpdate::status(TaskStatus::PendingConfirmation)).expect("update to PendingConfirmation");
        registry.complete(&id).expect("complete");

        // Not in active
        assert!(registry.get(&id).is_none());
        // In completed
        assert!(registry.get_completed(&id).is_some());
        assert_eq!(registry.active_count(), 0);
        assert_eq!(registry.completed_count(), 1);
    }

    #[test]
    fn t12_2_t5_task_cancelled_handled() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        registry.cancel(&id, "User cancelled".to_string()).expect("cancel");

        let task = registry.get_completed(&id).expect("task");
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn t12_2_t6_active_tasks_listed() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        registry.create("Task 1".to_string(), vec![]).expect("create");
        registry.create("Task 2".to_string(), vec![]).expect("create");
        registry.create("Task 3".to_string(), vec![]).expect("create");

        let active = registry.list_active();
        assert_eq!(active.len(), 3);
    }

    #[test]
    fn t12_2_t7_load_restores_from_storage() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().to_path_buf();

        // Create and save tasks with first registry
        let store1 = Box::new(FileTaskStore::new(path.clone()));
        let registry1 = TaskRegistry::new(store1);
        let id = registry1.create("Saved task".to_string(), vec![]).expect("create");

        // Create second registry and load
        let store2 = Box::new(FileTaskStore::new(path));
        let registry2 = TaskRegistry::new(store2);
        registry2.load().expect("load");

        let task = registry2.get(&id).expect("task");
        assert_eq!(task.description, "Saved task");
    }

    #[test]
    fn t12_2_t8_save_persists_to_storage() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().to_path_buf();

        let store = Box::new(FileTaskStore::new(path.clone()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        registry.update(&id, TaskUpdate::reflection_count(5)).expect("update");
        registry.save_all().expect("save");

        // Load fresh and verify
        let store2 = FileTaskStore::new(path);
        let task = store2.load(&id).expect("load");
        assert_eq!(task.reflection_count, 5);
    }

    // Story 12.3 Tests: Execution History

    #[test]
    fn t12_3_t1_record_added_to_task() {
        let mut task = Task::new("Test".to_string(), vec![]);
        let record = ExecutionRecord::new(
            WorkflowAction::Reflect { reason: "test".to_string() },
            StageId::new("reflecting"),
        );

        task.execution_history.push(record);

        assert_eq!(task.execution_history.len(), 1);
    }

    #[test]
    fn t12_3_t2_all_records_retrieved() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Reflect { reason: "test".to_string() },
            StageId::new("reflecting"),
        ));

        let history = &task.execution_history;
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn t12_3_t3_stage_specific_records_retrieved() {
        let mut task = Task::new("Test".to_string(), vec![]);
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Reflect { reason: "test".to_string() },
            StageId::new("reflecting"),
        ));
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));

        let developing_records: Vec<_> = task.execution_history.iter()
            .filter(|r| r.stage == StageId::new("developing"))
            .collect();

        assert_eq!(developing_records.len(), 2);
    }

    #[test]
    fn t12_3_t4_history_persisted_with_task() {
        let temp = TempDir::new().expect("tempdir");
        let store = FileTaskStore::new(temp.path().to_path_buf());

        let mut task = Task::new("Test".to_string(), vec![]);
        task.execution_history.push(ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        ));

        store.save(&task).expect("save");
        let loaded = store.load(&task.id).expect("load");

        assert_eq!(loaded.execution_history.len(), 1);
    }

    #[test]
    fn t12_3_t5_history_timestamped_correctly() {
        let before = Utc::now();
        let record = ExecutionRecord::new(
            WorkflowAction::Continue,
            StageId::new("developing"),
        );
        let after = Utc::now();

        assert!(record.timestamp >= before);
        assert!(record.timestamp <= after);
    }

    #[test]
    fn t12_3_t6_record_with_auto_check() {
        let record = ExecutionRecord::with_auto_check(
            WorkflowAction::Reflect { reason: "test".to_string() },
            StageId::new("reflecting"),
            &AutoCheckResult::NeedsReflection { reason: "error".to_string() },
        );

        assert!(record.auto_check_result.is_some());
        assert!(!record.human_requested);
    }

    #[test]
    fn t12_3_t7_record_with_human_flag() {
        let record = ExecutionRecord::with_auto_check(
            WorkflowAction::RequestHuman { question: "test".to_string() },
            StageId::new("human_decision"),
            &AutoCheckResult::NeedsHuman { reason: "boundary".to_string() },
        );

        assert!(record.human_requested);
    }

    // Story 12.4 Tests: Recovery Mechanism

    #[test]
    fn t12_4_t1_inprogress_restored_as_paused() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        registry.update(&id, TaskUpdate::status(TaskStatus::InProgress)).expect("update");

        let recovered = registry.recover().expect("recover");

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0], id);

        let task = registry.get(&id).expect("task");
        assert_eq!(task.status, TaskStatus::Paused);
    }

    #[test]
    fn t12_4_t2_reflecting_restored_as_paused() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        // Transition through proper workflow: Pending → InProgress → Reflecting
        registry.update(&id, TaskUpdate::status(TaskStatus::InProgress)).expect("update to InProgress");
        registry.update(&id, TaskUpdate::status(TaskStatus::Reflecting)).expect("update to Reflecting");

        let recovered = registry.recover().expect("recover");

        assert_eq!(recovered.len(), 1);
        let task = registry.get(&id).expect("task");
        assert_eq!(task.status, TaskStatus::Paused);
    }

    #[test]
    fn t12_4_t3_pending_restored_as_pending() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        // Status is already Pending

        let recovered = registry.recover().expect("recover");

        // Pending should not be recovered (not in active execution)
        assert_eq!(recovered.len(), 0);

        let task = registry.get(&id).expect("task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn t12_4_t4_completed_not_recovered() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        // Transition through proper workflow to complete first task
        registry.update(&id, TaskUpdate::status(TaskStatus::InProgress)).expect("update to InProgress");
        registry.update(&id, TaskUpdate::status(TaskStatus::PendingConfirmation)).expect("update to PendingConfirmation");
        registry.complete(&id).expect("complete");

        // Create another in-progress task
        let id2 = registry.create("Task2".to_string(), vec![]).expect("create");
        registry.update(&id2, TaskUpdate::status(TaskStatus::InProgress)).expect("update");

        let recovered = registry.recover().expect("recover");

        // Only in-progress should be recovered
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0], id2);
    }

    #[test]
    fn t12_4_t5_needs_recovery_check() {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(FileTaskStore::new(temp.path().to_path_buf()));
        let registry = TaskRegistry::new(store);

        let id = registry.create("Task".to_string(), vec![]).expect("create");
        assert!(!registry.needs_recovery());

        registry.update(&id, TaskUpdate::status(TaskStatus::InProgress)).expect("update");
        assert!(registry.needs_recovery());

        registry.recover().expect("recover");
        assert!(!registry.needs_recovery());
    }
}