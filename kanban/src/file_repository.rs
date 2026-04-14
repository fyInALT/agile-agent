//! File-based implementation of KanbanElementRepository

use crate::domain::{ElementId, ElementType, KanbanElement, Status};
use crate::error::KanbanError;
use crate::repository::KanbanElementRepository;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

/// Index structure for minimal index.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Index {
    elements: Vec<String>,
}

/// FileKanbanRepository implements file-based storage for kanban elements
pub struct FileKanbanRepository {
    #[allow(dead_code)]
    base_path: PathBuf,
    index_path: PathBuf,
    elements_path: PathBuf,
    counters: RwLock<HashMap<ElementType, u32>>,
}

impl FileKanbanRepository {
    /// Create a new FileKanbanRepository at the given base path
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self, KanbanError> {
        let base_path = base_path.into();
        let elements_path = base_path.join("elements");
        let index_path = base_path.join("index.json");

        // Create directories if they don't exist
        fs::create_dir_all(&elements_path).map_err(|e| {
            KanbanError::RepositoryError(format!("failed to create elements dir: {}", e))
        })?;

        let mut repo = FileKanbanRepository {
            base_path,
            index_path,
            elements_path,
            counters: RwLock::new(HashMap::new()),
        };

        // Load counters from existing files
        repo.load_counters()?;

        Ok(repo)
    }

    /// Create a new FileKanbanRepository from a workplace path
    pub fn from_workplace(workplace_path: impl Into<PathBuf>) -> Result<Self, KanbanError> {
        let kanban_path = workplace_path.into().join("kanban");
        Self::new(kanban_path)
    }

    /// Get the path for an element's JSON file
    fn element_path(&self, id: &ElementId) -> PathBuf {
        self.elements_path.join(format!("{}.json", id.as_str()))
    }

    /// Load counters by scanning existing element files
    fn load_counters(&mut self) -> Result<(), KanbanError> {
        if !self.elements_path.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.elements_path).map_err(|e| {
            KanbanError::RepositoryError(format!("failed to read elements dir: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                KanbanError::RepositoryError(format!("failed to read entry: {}", e))
            })?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = ElementId::parse(stem) {
                        let type_ = id.type_();
                        let num = id.number();
                        let mut counters = self.counters.write().unwrap();
                        let current = counters.get(&type_).copied().unwrap_or(0);
                        if num > current {
                            counters.insert(type_, num);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Update the index file
    fn update_index(&self, ids: &[ElementId]) -> Result<(), KanbanError> {
        let index = Index {
            elements: ids.iter().map(|id| id.as_str().to_string()).collect(),
        };
        let json = serde_json::to_string_pretty(&index)
            .map_err(|e| KanbanError::SerializationError(e.to_string()))?;
        fs::write(&self.index_path, json)
            .map_err(|e| KanbanError::RepositoryError(format!("failed to write index: {}", e)))?;
        Ok(())
    }

    /// Read the index file
    fn read_index(&self) -> Result<Vec<ElementId>, KanbanError> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.index_path)
            .map_err(|e| KanbanError::RepositoryError(format!("failed to read index: {}", e)))?;
        let index: Index = serde_json::from_str(&content)
            .map_err(|e| KanbanError::SerializationError(e.to_string()))?;

        index
            .elements
            .iter()
            .map(|s| ElementId::parse(s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| KanbanError::SerializationError(format!("invalid index: {}", e)))
    }
}

impl KanbanElementRepository for FileKanbanRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        let path = self.element_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| KanbanError::RepositoryError(format!("failed to read element: {}", e)))?;
        let element: KanbanElement = serde_json::from_str::<KanbanElement>(&content)
            .map_err(|e| KanbanError::SerializationError(e.to_string()))?;

        Ok(Some(element))
    }

    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        let ids = self.read_index()?;
        let mut elements = Vec::new();

        for id in &ids {
            if let Some(element) = self.get(id)? {
                elements.push(element);
            }
        }

        // Sort by ID
        elements.sort_by(|a, b| {
            let a_id = a.id().map(|id| id.as_str()).unwrap_or("");
            let b_id = b.id().map(|id| id.as_str()).unwrap_or("");
            a_id.cmp(&b_id)
        });

        Ok(elements)
    }

    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all
            .into_iter()
            .filter(|e| e.element_type() == type_)
            .collect())
    }

    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|e| e.status() == status).collect())
    }

    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all
            .into_iter()
            .filter(|e| {
                e.assignee()
                    .map(|a| a.as_str() == assignee)
                    .unwrap_or(false)
            })
            .collect())
    }

    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
        let all = self.list()?;
        Ok(all
            .into_iter()
            .filter(|e| e.parent().map(|p| p == parent).unwrap_or(false))
            .collect())
    }

    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.list_by_status(Status::Blocked)
    }

    fn save(&self, element: KanbanElement) -> Result<(), KanbanError> {
        let id = element
            .id()
            .ok_or_else(|| KanbanError::RepositoryError("element has no ID".to_string()))?;

        let path = self.element_path(id);
        let json = serde_json::to_string_pretty(&element)
            .map_err(|e| KanbanError::SerializationError(e.to_string()))?;

        fs::write(&path, json)
            .map_err(|e| KanbanError::RepositoryError(format!("failed to write element: {}", e)))?;

        // Update index
        let mut ids = self.read_index()?;
        if !ids.contains(id) {
            ids.push(id.clone());
            ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            self.update_index(&ids)?;
        }

        Ok(())
    }

    fn delete(&self, id: &ElementId) -> Result<(), KanbanError> {
        let path = self.element_path(id);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| {
                KanbanError::RepositoryError(format!("failed to delete element: {}", e))
            })?;
        }

        // Update index
        let mut ids = self.read_index()?;
        ids.retain(|i| i != id);
        self.update_index(&ids)?;

        Ok(())
    }

    fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError> {
        let mut counters = self.counters.write().unwrap();
        let next = counters.get(&type_).copied().unwrap_or(0) + 1;
        counters.insert(type_, next);
        Ok(ElementId::new(type_, next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (FileKanbanRepository, TempDir) {
        let temp = TempDir::new().unwrap();
        let repo = FileKanbanRepository::new(temp.path()).unwrap();
        (repo, temp)
    }

    #[test]
    fn test_create_directory() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("kanban");
        let repo = FileKanbanRepository::new(&path).unwrap();
        assert!(repo.elements_path.exists());
    }

    #[test]
    fn test_save_and_get() {
        let (repo, _temp) = create_test_repo();

        let mut element = KanbanElement::new_task("Test Task");
        let id = repo.new_id(ElementType::Task).unwrap();
        element.set_id(id.clone());

        repo.save(element).unwrap();

        let retrieved = repo.get(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Task");
    }

    #[test]
    fn test_list_all() {
        let (repo, _temp) = create_test_repo();

        let mut task1 = KanbanElement::new_task("Task 1");
        task1.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task1).unwrap();

        let mut task2 = KanbanElement::new_task("Task 2");
        task2.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task2).unwrap();

        let all = repo.list().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_list_by_type() {
        let (repo, _temp) = create_test_repo();

        let mut task = KanbanElement::new_task("Task");
        task.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task).unwrap();

        let mut story = KanbanElement::new_story("Story", "Content");
        story.set_id(repo.new_id(ElementType::Story).unwrap());
        repo.save(story).unwrap();

        let tasks = repo.list_by_type(ElementType::Task).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title(), "Task");
    }

    #[test]
    fn test_delete() {
        let (repo, _temp) = create_test_repo();

        let mut element = KanbanElement::new_task("To Delete");
        let id = repo.new_id(ElementType::Task).unwrap();
        element.set_id(id.clone());
        repo.save(element).unwrap();

        assert!(repo.get(&id).unwrap().is_some());

        repo.delete(&id).unwrap();

        assert!(repo.get(&id).unwrap().is_none());
    }

    #[test]
    fn test_new_id_sequential() {
        let (repo, _temp) = create_test_repo();

        let id1 = repo.new_id(ElementType::Task).unwrap();
        assert_eq!(id1.as_str(), "task-001");

        let id2 = repo.new_id(ElementType::Task).unwrap();
        assert_eq!(id2.as_str(), "task-002");

        let id3 = repo.new_id(ElementType::Story).unwrap();
        assert_eq!(id3.as_str(), "story-001");
    }

    #[test]
    fn test_update_existing() {
        let (repo, _temp) = create_test_repo();

        let mut task = KanbanElement::new_task("Original");
        let id = repo.new_id(ElementType::Task).unwrap();
        task.set_id(id.clone());
        repo.save(task).unwrap();

        let mut updated = KanbanElement::new_task("Updated");
        updated.set_id(id.clone());
        repo.save(updated).unwrap();

        let all = repo.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title(), "Updated");
    }

    #[test]
    fn test_index_persistence() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("kanban");

        {
            let repo = FileKanbanRepository::new(&path).unwrap();
            let mut element = KanbanElement::new_task("Task");
            element.set_id(repo.new_id(ElementType::Task).unwrap());
            repo.save(element).unwrap();
        }

        // Open a new repo at the same path - counters should persist
        let repo2 = FileKanbanRepository::new(&path).unwrap();
        let id = repo2.new_id(ElementType::Task).unwrap();
        assert_eq!(id.as_str(), "task-002"); // Should be 2, not 1
    }
}
