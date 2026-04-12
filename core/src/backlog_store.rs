use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::backlog::BacklogState;
use crate::storage;

pub fn load_backlog() -> Result<BacklogState> {
    let root = default_backlog_root()?;
    load_backlog_from_root(&root)
}

pub fn save_backlog(backlog: &BacklogState) -> Result<()> {
    let root = default_backlog_root()?;
    save_backlog_to_root(backlog, &root)
}

fn load_backlog_from_root(root: &Path) -> Result<BacklogState> {
    let path = root.join("backlog.json");
    if !path.exists() {
        return Ok(BacklogState::default());
    }

    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&data).context("failed to parse backlog.json")
}

fn save_backlog_to_root(backlog: &BacklogState, root: &Path) -> Result<()> {
    fs::create_dir_all(root).context("failed to create backlog root")?;
    let path = root.join("backlog.json");
    let data = serde_json::to_string_pretty(backlog).context("failed to serialize backlog")?;
    fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn default_backlog_root() -> Result<PathBuf> {
    storage::app_data_root().context("failed to resolve backlog root")
}

#[cfg(test)]
mod tests {
    use super::load_backlog_from_root;
    use super::save_backlog_to_root;
    use crate::backlog::BacklogState;
    use crate::backlog::TodoItem;
    use crate::backlog::TodoStatus;
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
}
