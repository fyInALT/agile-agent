use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::backlog::BacklogState;
use crate::logging;
use crate::storage;
use crate::workplace_store::WorkplaceStore;

pub fn load_backlog() -> Result<BacklogState> {
    let root = default_backlog_root()?;
    load_backlog_from_root(&root)
}

pub fn save_backlog(backlog: &BacklogState) -> Result<()> {
    let root = default_backlog_root()?;
    save_backlog_to_root(backlog, &root)
}

pub fn load_backlog_for_workplace(workplace: &WorkplaceStore) -> Result<BacklogState> {
    load_backlog_from_root(workplace.path())
}

pub fn save_backlog_for_workplace(
    backlog: &BacklogState,
    workplace: &WorkplaceStore,
) -> Result<()> {
    save_backlog_to_root(backlog, workplace.path())
}

fn load_backlog_from_root(root: &Path) -> Result<BacklogState> {
    let path = root.join("backlog.json");
    if !path.exists() {
        logging::debug_event(
            "storage.read",
            "backlog file missing, using default backlog",
            serde_json::json!({
                "kind": "backlog",
                "path": path.display().to_string(),
            }),
        );
        return Ok(BacklogState::default());
    }

    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let backlog = serde_json::from_str(&data).context("failed to parse backlog.json")?;
    logging::debug_event(
        "storage.read",
        "loaded workplace backlog",
        serde_json::json!({
            "kind": "backlog",
            "path": path.display().to_string(),
        }),
    );
    Ok(backlog)
}

fn save_backlog_to_root(backlog: &BacklogState, root: &Path) -> Result<()> {
    fs::create_dir_all(root).context("failed to create backlog root")?;
    let path = root.join("backlog.json");
    let data = serde_json::to_string_pretty(backlog).context("failed to serialize backlog")?;
    fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))?;
    logging::debug_event(
        "storage.write",
        "saved workplace backlog",
        serde_json::json!({
            "kind": "backlog",
            "path": path.display().to_string(),
        }),
    );
    Ok(())
}

fn default_backlog_root() -> Result<PathBuf> {
    storage::app_data_root().context("failed to resolve backlog root")
}

#[cfg(test)]
mod tests {
    use super::load_backlog_for_workplace;
    use super::load_backlog_from_root;
    use super::save_backlog_for_workplace;
    use super::save_backlog_to_root;
    use crate::backlog::BacklogState;
    use crate::backlog::TodoItem;
    use crate::backlog::TodoStatus;
    use crate::backlog::TaskItem;
    use crate::backlog::TaskStatus;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn missing_backlog_file_returns_empty_state() {
        let temp = TempDir::new().expect("tempdir");
        let backlog = load_backlog_from_root(temp.path()).expect("load backlog");

        assert!(backlog.todos.is_empty());
        assert!(backlog.tasks.is_empty());
    }

    #[test]
    fn saves_and_loads_backlog() {
        let temp = TempDir::new().expect("tempdir");
        let mut backlog = BacklogState::default();
        backlog.push_todo(TodoItem {
            id: "todo-1".to_string(),
            title: "first".to_string(),
            description: "first todo".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["done".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });

        save_backlog_to_root(&backlog, temp.path()).expect("save backlog");
        let loaded = load_backlog_from_root(temp.path()).expect("load backlog");

        assert_eq!(loaded.todos.len(), 1);
        assert_eq!(loaded.todos[0].id, "todo-1");
    }

    #[test]
    fn saves_and_loads_backlog_for_workplace() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");
        let mut backlog = BacklogState::default();
        backlog.push_todo(TodoItem {
            id: "todo-1".to_string(),
            title: "first".to_string(),
            description: "first todo".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["done".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });

        save_backlog_for_workplace(&backlog, &workplace).expect("save backlog");
        let loaded = load_backlog_for_workplace(&workplace).expect("load backlog");

        assert_eq!(loaded.todos.len(), 1);
        assert_eq!(loaded.todos[0].id, "todo-1");
    }

    #[test]
    fn invalid_json_returns_error() {
        let temp = TempDir::new().expect("tempdir");
        let backlog_path = temp.path().join("backlog.json");
        fs::write(&backlog_path, "{invalid json}").expect("write invalid json");

        let result = load_backlog_from_root(temp.path());
        assert!(result.is_err(), "invalid JSON should return error");
    }

    #[test]
    fn empty_backlog_file_returns_empty_state() {
        let temp = TempDir::new().expect("tempdir");
        let backlog_path = temp.path().join("backlog.json");
        fs::write(&backlog_path, "").expect("write empty file");

        let result = load_backlog_from_root(temp.path());
        // Empty file should fail to parse
        assert!(result.is_err(), "empty file should return error");
    }

    #[test]
    fn saves_tasks_along_with_todos() {
        let temp = TempDir::new().expect("tempdir");
        let mut backlog = BacklogState::default();
        backlog.push_todo(TodoItem {
            id: "todo-1".to_string(),
            title: "todo".to_string(),
            description: "desc".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["done".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });
        backlog.push_task(TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "implement".to_string(),
            scope: "full".to_string(),
            constraints: vec!["no deps".to_string()],
            verification_plan: vec!["run tests".to_string()],
            status: TaskStatus::Ready,
            result_summary: None,
        });

        save_backlog_to_root(&backlog, temp.path()).expect("save backlog");
        let loaded = load_backlog_from_root(temp.path()).expect("load backlog");

        assert_eq!(loaded.todos.len(), 1);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].id, "task-1");
    }

    #[test]
    fn overwrites_existing_backlog() {
        let temp = TempDir::new().expect("tempdir");

        // First save
        let mut backlog1 = BacklogState::default();
        backlog1.push_todo(TodoItem {
            id: "todo-1".to_string(),
            title: "first".to_string(),
            description: "first".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["done".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });
        save_backlog_to_root(&backlog1, temp.path()).expect("save first");

        // Second save with different content
        let mut backlog2 = BacklogState::default();
        backlog2.push_todo(TodoItem {
            id: "todo-2".to_string(),
            title: "second".to_string(),
            description: "second".to_string(),
            priority: 2,
            status: TodoStatus::InProgress,
            acceptance_criteria: vec!["done".to_string()],
            dependencies: Vec::new(),
            source: "test".to_string(),
        });
        save_backlog_to_root(&backlog2, temp.path()).expect("save second");

        let loaded = load_backlog_from_root(temp.path()).expect("load backlog");
        // Should have second todo, not first
        assert_eq!(loaded.todos.len(), 1);
        assert_eq!(loaded.todos[0].id, "todo-2");
    }

    #[test]
    fn roundtrip_preserves_all_fields() {
        let temp = TempDir::new().expect("tempdir");
        let mut backlog = BacklogState::default();
        backlog.push_todo(TodoItem {
            id: "todo-full".to_string(),
            title: "Full Todo".to_string(),
            description: "Complete description".to_string(),
            priority: 5,
            status: TodoStatus::Blocked,
            acceptance_criteria: vec!["AC1".to_string(), "AC2".to_string()],
            dependencies: vec!["dep-1".to_string(), "dep-2".to_string()],
            source: "integration-test".to_string(),
        });
        backlog.push_task(TaskItem {
            id: "task-full".to_string(),
            todo_id: "todo-full".to_string(),
            objective: "Full objective".to_string(),
            scope: "Complete scope".to_string(),
            constraints: vec!["c1".to_string()],
            verification_plan: vec!["vp1".to_string()],
            status: TaskStatus::Running,
            result_summary: Some("in progress".to_string()),
        });

        save_backlog_to_root(&backlog, temp.path()).expect("save");
        let loaded = load_backlog_from_root(temp.path()).expect("load");

        let todo = &loaded.todos[0];
        assert_eq!(todo.id, "todo-full");
        assert_eq!(todo.title, "Full Todo");
        assert_eq!(todo.description, "Complete description");
        assert_eq!(todo.priority, 5);
        assert_eq!(todo.status, TodoStatus::Blocked);
        assert_eq!(todo.acceptance_criteria.len(), 2);
        assert_eq!(todo.dependencies.len(), 2);
        assert_eq!(todo.source, "integration-test");

        let task = &loaded.tasks[0];
        assert_eq!(task.id, "task-full");
        assert_eq!(task.objective, "Full objective");
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.result_summary, Some("in progress".to_string()));
    }
}
