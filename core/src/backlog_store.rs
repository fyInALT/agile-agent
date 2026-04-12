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
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

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
}
