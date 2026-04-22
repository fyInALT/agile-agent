//! Integration between Decision Layer and Kanban
//!
//! Story 8.3: Connects task completion/failure to Kanban status updates,
//! and provides next task selection from Kanban backlog.

use std::sync::Arc;

use crate::{
    domain::{ElementId, ElementType, KanbanElement, Status},
    file_repository::FileKanbanRepository,
    service::KanbanService,
};

use agent_backlog::{BacklogState, TaskItem, TaskStatus};

/// Decision-Kanban integration configuration
#[derive(Debug, Clone)]
pub struct DecisionKanbanConfig {
    /// Auto-sync task status to Kanban
    pub auto_sync: bool,
    /// Select next task from Kanban Todo
    pub select_from_kanban: bool,
    /// Notify on task completion
    pub notify_completion: bool,
    /// Notify on task failure
    pub notify_failure: bool,
}

impl Default for DecisionKanbanConfig {
    fn default() -> Self {
        Self {
            auto_sync: true,
            select_from_kanban: true,
            notify_completion: true,
            notify_failure: true,
        }
    }
}

/// Task-Kanban mapping for integration
#[derive(Debug, Clone)]
pub struct TaskKanbanMapping {
    /// Backlog task ID
    pub task_id: String,
    /// Kanban element ID
    pub kanban_id: ElementId,
    /// Mapping created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Decision-Kanban integration result
#[derive(Debug, Clone)]
pub enum KanbanSyncResult {
    /// Task synced successfully
    Synced {
        task_id: String,
        kanban_id: ElementId,
    },
    /// No Kanban element found for task
    NotFound { task_id: String },
    /// Sync failed
    Failed { task_id: String, error: String },
    /// Kanban service unavailable
    NoKanbanService,
}

/// Next task selection result
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum NextTaskResult {
    /// Task selected successfully
    Selected {
        kanban_element: KanbanElement,
        backlog_task: TaskItem,
    },
    /// No tasks available
    NoTasksAvailable,
    /// Kanban service unavailable
    NoKanbanService,
    /// Selection failed
    Failed { error: String },
}

/// Decision-Kanban integration service
pub struct DecisionKanbanIntegration {
    /// Kanban service
    kanban: Option<Arc<KanbanService<FileKanbanRepository>>>,
    /// Integration configuration
    config: DecisionKanbanConfig,
    /// Task-Kanban mappings
    mappings: Vec<TaskKanbanMapping>,
}

impl DecisionKanbanIntegration {
    /// Create new integration with optional Kanban service
    pub fn new(kanban: Option<Arc<KanbanService<FileKanbanRepository>>>) -> Self {
        Self {
            kanban,
            config: DecisionKanbanConfig::default(),
            mappings: Vec::new(),
        }
    }

    /// Create integration with configuration
    pub fn with_config(
        kanban: Option<Arc<KanbanService<FileKanbanRepository>>>,
        config: DecisionKanbanConfig,
    ) -> Self {
        Self {
            kanban,
            config,
            mappings: Vec::new(),
        }
    }

    /// Get configuration
    pub fn config(&self) -> &DecisionKanbanConfig {
        &self.config
    }

    /// Check if Kanban service is available
    pub fn has_kanban(&self) -> bool {
        self.kanban.is_some()
    }

    /// Get mappings
    pub fn mappings(&self) -> &[TaskKanbanMapping] {
        &self.mappings
    }

    /// Register task-Kanban mapping
    pub fn register_mapping(&mut self, task_id: String, kanban_id: ElementId) {
        self.mappings.push(TaskKanbanMapping {
            task_id,
            kanban_id,
            created_at: chrono::Utc::now(),
        });
    }

    /// Find Kanban ID for task
    pub fn find_kanban_id(&self, task_id: &str) -> Option<&ElementId> {
        self.mappings
            .iter()
            .find(|m| m.task_id == task_id)
            .map(|m| &m.kanban_id)
    }

    /// Sync task completion to Kanban
    ///
    /// Updates Kanban element status to Done when backlog task completes.
    pub fn sync_task_completion(
        &mut self,
        task_id: &str,
        backlog: &mut BacklogState,
    ) -> KanbanSyncResult {
        if let Some(kanban) = &self.kanban {
            // Find mapping
            let kanban_id = self.find_kanban_id(task_id);

            if let Some(kanban_id) = kanban_id {
                // Update Kanban status to Done
                let result = kanban.update_status(kanban_id, Status::Done, "decision_layer");

                match result {
                    Ok(_) => {
                        // Update backlog task to Done
                        backlog.complete_task(task_id, Some("Completed successfully".to_string()));

                        KanbanSyncResult::Synced {
                            task_id: task_id.to_string(),
                            kanban_id: kanban_id.clone(),
                        }
                    }
                    Err(e) => KanbanSyncResult::Failed {
                        task_id: task_id.to_string(),
                        error: e.to_string(),
                    },
                }
            } else {
                // Try to find by title matching
                if let Some(task) = backlog.find_task(task_id) {
                    // Search Kanban for matching task
                    if let Ok(elements) = kanban.list_elements() {
                        for elem in elements {
                            if elem.element_type() == ElementType::Task
                                && elem.title() == task.objective
                            {
                                // Update status
                                let result = kanban.update_status(
                                    elem.id().unwrap(),
                                    Status::Done,
                                    "decision_layer",
                                );

                                if result.is_ok() {
                                    // Register mapping
                                    self.register_mapping(
                                        task_id.to_string(),
                                        elem.id().unwrap().clone(),
                                    );
                                    backlog.complete_task(
                                        task_id,
                                        Some("Completed successfully".to_string()),
                                    );

                                    return KanbanSyncResult::Synced {
                                        task_id: task_id.to_string(),
                                        kanban_id: elem.id().unwrap().clone(),
                                    };
                                }
                            }
                        }
                    }
                }

                KanbanSyncResult::NotFound {
                    task_id: task_id.to_string(),
                }
            }
        } else {
            // No Kanban service, just update backlog
            backlog.complete_task(task_id, Some("Completed successfully".to_string()));
            KanbanSyncResult::NoKanbanService
        }
    }

    /// Sync task failure to Kanban
    ///
    /// Updates Kanban element status to Todo (for rework) when backlog task fails.
    /// Note: InProgress -> Blocked is not valid, so we use Todo instead.
    pub fn sync_task_failure(
        &mut self,
        task_id: &str,
        error: String,
        backlog: &mut BacklogState,
    ) -> KanbanSyncResult {
        if let Some(kanban) = &self.kanban {
            // Find mapping
            let kanban_id = self.find_kanban_id(task_id);

            if let Some(kanban_id) = kanban_id {
                // Update Kanban status to Todo (for rework after failure)
                // InProgress -> Todo is valid transition
                let result = kanban.update_status(kanban_id, Status::Todo, "decision_layer");

                match result {
                    Ok(_) => {
                        // Update backlog task to Failed
                        backlog.fail_task(task_id, error);

                        KanbanSyncResult::Synced {
                            task_id: task_id.to_string(),
                            kanban_id: kanban_id.clone(),
                        }
                    }
                    Err(e) => KanbanSyncResult::Failed {
                        task_id: task_id.to_string(),
                        error: e.to_string(),
                    },
                }
            } else {
                KanbanSyncResult::NotFound {
                    task_id: task_id.to_string(),
                }
            }
        } else {
            // No Kanban service, just update backlog
            backlog.fail_task(task_id, error);
            KanbanSyncResult::NoKanbanService
        }
    }

    /// Select next task from Kanban Todo
    ///
    /// Finds the next available task from Kanban backlog and syncs to backlog.
    pub fn select_next_task(&self, backlog: &mut BacklogState) -> NextTaskResult {
        if let Some(kanban) = &self.kanban {
            // Get tasks with Todo status
            let result = kanban.list_by_status(Status::Todo);

            match result {
                Ok(todo_tasks) => {
                    // Find first unassigned task
                    for kanban_elem in todo_tasks {
                        if kanban_elem.element_type() == ElementType::Task {
                            // Check if already in backlog
                            let kanban_id = kanban_elem.id().unwrap();
                            let exists_in_backlog = backlog.find_task(kanban_id.as_str()).is_some();

                            if !exists_in_backlog {
                                // Create backlog task from Kanban element
                                let task = TaskItem {
                                    id: kanban_id.as_str().to_string(),
                                    todo_id: kanban_elem
                                        .parent()
                                        .map(|p| p.as_str().to_string())
                                        .unwrap_or_default(),
                                    objective: kanban_elem.title().to_string(),
                                    scope: kanban_elem.content().to_string(),
                                    constraints: Vec::new(),
                                    verification_plan: Vec::new(),
                                    status: TaskStatus::Ready,
                                    result_summary: None,
                                };

                                backlog.push_task(task.clone());

                                return NextTaskResult::Selected {
                                    kanban_element: kanban_elem,
                                    backlog_task: task,
                                };
                            }
                        }
                    }

                    NextTaskResult::NoTasksAvailable
                }
                Err(e) => NextTaskResult::Failed {
                    error: e.to_string(),
                },
            }
        } else {
            // No Kanban, select from backlog ready tasks
            let ready = backlog.ready_tasks();
            if let Some(task) = ready.first() {
                let task_clone = (*task).clone();
                return NextTaskResult::Selected {
                    kanban_element: KanbanElement::new_task(&task.objective),
                    backlog_task: task_clone,
                };
            }

            NextTaskResult::NoKanbanService
        }
    }

    /// Load story definition from Kanban
    ///
    /// Gets the story content from Kanban for context loading.
    pub fn load_story_definition(&self, story_id: &str) -> Option<String> {
        if let Some(kanban) = &self.kanban {
            let element_id = ElementId::parse(story_id).ok()?;

            if let Ok(Some(element)) = kanban.get_element(&element_id)
                && element.element_type() == ElementType::Story
            {
                return Some(element.content().to_string());
            }
        }
        None
    }

    /// Load task definition from Kanban
    ///
    /// Gets the task details from Kanban for context loading.
    pub fn load_task_definition(&self, task_id: &str) -> Option<KanbanElement> {
        if let Some(kanban) = &self.kanban {
            let element_id = ElementId::parse(task_id).ok()?;

            if let Ok(Some(element)) = kanban.get_element(&element_id)
                && element.element_type() == ElementType::Task
            {
                return Some(element);
            }
        }
        None
    }

    /// Notify PR submission trigger
    ///
    /// Marks task as Ready for PR review in Kanban.
    /// If already at InProgress, returns success without changing status.
    pub fn notify_pr_submission(&self, task_id: &str) -> KanbanSyncResult {
        if let Some(kanban) = &self.kanban {
            let kanban_id = self.find_kanban_id(task_id);

            if let Some(kanban_id) = kanban_id {
                // Check current status
                if let Ok(Some(element)) = kanban.get_element(kanban_id) {
                    // If already InProgress, no need to update
                    if element.status() == Status::InProgress {
                        return KanbanSyncResult::Synced {
                            task_id: task_id.to_string(),
                            kanban_id: kanban_id.clone(),
                        };
                    }
                }

                // Try to update to InProgress (PR review stage)
                let result = kanban.update_status(kanban_id, Status::InProgress, "pr_submitted");

                match result {
                    Ok(_) => KanbanSyncResult::Synced {
                        task_id: task_id.to_string(),
                        kanban_id: kanban_id.clone(),
                    },
                    Err(e) => KanbanSyncResult::Failed {
                        task_id: task_id.to_string(),
                        error: e.to_string(),
                    },
                }
            } else {
                KanbanSyncResult::NotFound {
                    task_id: task_id.to_string(),
                }
            }
        } else {
            KanbanSyncResult::NoKanbanService
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KanbanEventBus;
    use tempfile::TempDir;

    fn create_test_kanban_service(temp: &TempDir) -> Arc<KanbanService<FileKanbanRepository>> {
        let repo = FileKanbanRepository::from_workplace(temp.path()).unwrap();
        let event_bus = Arc::new(KanbanEventBus::new());
        Arc::new(KanbanService::new(Arc::new(repo), event_bus))
    }

    fn make_test_task() -> TaskItem {
        TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Test Task".to_string(),
            scope: "Test scope".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Running,
            result_summary: None,
        }
    }

    fn make_element_id(id_str: &str) -> ElementId {
        ElementId::parse(id_str).unwrap()
    }

    #[test]
    fn decision_kanban_config_default() {
        let config = DecisionKanbanConfig::default();
        assert!(config.auto_sync);
        assert!(config.select_from_kanban);
        assert!(config.notify_completion);
        assert!(config.notify_failure);
    }

    #[test]
    fn decision_kanban_integration_new() {
        let integration = DecisionKanbanIntegration::new(None);
        assert!(!integration.has_kanban());
        assert!(integration.mappings().is_empty());
    }

    #[test]
    fn decision_kanban_integration_with_kanban() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let integration = DecisionKanbanIntegration::new(Some(kanban));
        assert!(integration.has_kanban());
    }

    #[test]
    fn register_mapping() {
        let mut integration = DecisionKanbanIntegration::new(None);
        integration.register_mapping("task-001".to_string(), make_element_id("task-001"));

        assert_eq!(integration.mappings().len(), 1);
        assert_eq!(
            integration.find_kanban_id("task-001"),
            Some(&make_element_id("task-001"))
        );
    }

    #[test]
    fn sync_task_completion_no_kanban() {
        let mut integration = DecisionKanbanIntegration::new(None);
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task());

        let result = integration.sync_task_completion("task-001", &mut backlog);

        assert!(matches!(result, KanbanSyncResult::NoKanbanService));

        // Backlog task should be done
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, TaskStatus::Done);
    }

    #[test]
    fn sync_task_failure_no_kanban() {
        let mut integration = DecisionKanbanIntegration::new(None);
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task());

        let result =
            integration.sync_task_failure("task-001", "test error".to_string(), &mut backlog);

        assert!(matches!(result, KanbanSyncResult::NoKanbanService));

        // Backlog task should be failed
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[test]
    fn sync_task_completion_with_kanban() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let mut integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban task
        use crate::domain::KanbanElement;
        let kanban_task = KanbanElement::new_task("Test Task");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(kanban_task)
            .unwrap();
        let kanban_id = created.id().unwrap();

        // Transition to InProgress (Plan -> Backlog -> Todo -> InProgress)
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Backlog, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Todo, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::InProgress, "test")
            .unwrap();

        // Register mapping
        integration.register_mapping("task-001".to_string(), kanban_id.clone());

        // Add backlog task
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task());

        // Sync completion
        let result = integration.sync_task_completion("task-001", &mut backlog);

        assert!(matches!(result, KanbanSyncResult::Synced { .. }));

        // Kanban task should be Done
        let kanban_elem = integration
            .kanban
            .as_ref()
            .unwrap()
            .get_element(kanban_id)
            .unwrap()
            .unwrap();
        assert_eq!(kanban_elem.status(), Status::Done);
    }

    #[test]
    fn sync_task_failure_with_kanban() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let mut integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban task
        use crate::domain::KanbanElement;
        let kanban_task = KanbanElement::new_task("Test Task");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(kanban_task)
            .unwrap();
        let kanban_id = created.id().unwrap();

        // Transition to InProgress first (Plan -> Backlog -> Todo -> InProgress)
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Backlog, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Todo, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::InProgress, "test")
            .unwrap();

        // Register mapping
        integration.register_mapping("task-001".to_string(), kanban_id.clone());

        // Add backlog task
        let mut backlog = BacklogState::default();
        backlog.push_task(make_test_task());

        // Sync failure - InProgress -> Todo is valid transition for rework
        let result =
            integration.sync_task_failure("task-001", "test error".to_string(), &mut backlog);

        // Verify result - the failure should transition InProgress -> Todo
        assert!(matches!(result, KanbanSyncResult::Synced { .. }));

        // Kanban task should be Todo (for rework after failure)
        let kanban_elem = integration
            .kanban
            .as_ref()
            .unwrap()
            .get_element(kanban_id)
            .unwrap()
            .unwrap();
        assert_eq!(kanban_elem.status(), Status::Todo);
    }

    #[test]
    fn select_next_task_no_kanban() {
        let integration = DecisionKanbanIntegration::new(None);
        let mut backlog = BacklogState::default();
        // Empty backlog - no tasks available
        let result = integration.select_next_task(&mut backlog);
        assert!(matches!(result, NextTaskResult::NoKanbanService));
    }

    #[test]
    fn select_next_task_from_kanban() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban task
        use crate::domain::KanbanElement;
        let kanban_task = KanbanElement::new_task("New Task");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(kanban_task)
            .unwrap();
        let kanban_id = created.id().unwrap();

        // Transition to Todo status (Plan -> Backlog -> Todo)
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Backlog, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Todo, "test")
            .unwrap();

        let mut backlog = BacklogState::default();

        let result = integration.select_next_task(&mut backlog);

        assert!(matches!(result, NextTaskResult::Selected { .. }));

        // Task should be added to backlog
        assert!(backlog.find_task(kanban_id.as_str()).is_some());
    }

    #[test]
    fn load_story_definition() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban story
        use crate::domain::KanbanElement;
        let story = KanbanElement::new_story("Test Story", "Story content here");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(story)
            .unwrap();

        // Load story
        let content = integration.load_story_definition(created.id().unwrap().as_str());

        assert!(content.is_some());
        assert_eq!(content.unwrap(), "Story content here");
    }

    #[test]
    fn load_task_definition() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban task
        use crate::domain::KanbanElement;
        let task = KanbanElement::new_task("Test Task");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(task)
            .unwrap();

        // Load task
        let loaded = integration.load_task_definition(created.id().unwrap().as_str());

        assert!(loaded.is_some());
        // Task content is empty by default
        assert_eq!(loaded.unwrap().content(), "");
    }

    #[test]
    fn notify_pr_submission() {
        let temp = TempDir::new().unwrap();
        let kanban = create_test_kanban_service(&temp);
        let mut integration = DecisionKanbanIntegration::new(Some(kanban));

        // Create Kanban task
        use crate::domain::KanbanElement;
        let kanban_task = KanbanElement::new_task("Test Task");
        let created = integration
            .kanban
            .as_ref()
            .unwrap()
            .create_element(kanban_task)
            .unwrap();
        let kanban_id = created.id().unwrap();

        // Transition to InProgress first (Plan -> Backlog -> Todo -> InProgress)
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Backlog, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::Todo, "test")
            .unwrap();
        integration
            .kanban
            .as_ref()
            .unwrap()
            .update_status(kanban_id, Status::InProgress, "test")
            .unwrap();

        // Register mapping
        integration.register_mapping("task-001".to_string(), kanban_id.clone());

        // Notify PR (should keep at InProgress)
        let result = integration.notify_pr_submission("task-001");

        assert!(matches!(result, KanbanSyncResult::Synced { .. }));

        // Kanban task should still be InProgress
        let kanban_elem = integration
            .kanban
            .as_ref()
            .unwrap()
            .get_element(kanban_id)
            .unwrap()
            .unwrap();
        assert_eq!(kanban_elem.status(), Status::InProgress);
    }
}
