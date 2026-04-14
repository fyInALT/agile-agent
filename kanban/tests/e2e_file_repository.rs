//! E2E tests for FileKanbanRepository
//!
//! Tests using real file storage with temporary directories.

use agent_kanban::FileKanbanRepository;
use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::repository::KanbanElementRepository;
use tempfile::TempDir;

fn create_temp_repo() -> (FileKanbanRepository, TempDir) {
    let temp = TempDir::new().unwrap();
    let repo = FileKanbanRepository::new(temp.path().join("kanban")).unwrap();
    (repo, temp)
}

#[test]
fn test_file_repo_creates_directory_structure() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("kanban");

    FileKanbanRepository::new(&path).unwrap();

    assert!(path.exists());
    assert!(path.join("elements").exists());
    // index.json is created lazily on first save
}

#[test]
fn test_file_repo_save_and_retrieve() {
    let (repo, _temp) = create_temp_repo();

    let mut task = KanbanElement::new_task("Test Task");
    let id = repo.new_id(ElementType::Task).unwrap();
    task.set_id(id.clone());

    repo.save(task).unwrap();

    let retrieved = repo.get(&id).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().title(), "Test Task");
}

#[test]
fn test_file_repo_update_existing() {
    let (repo, _temp) = create_temp_repo();

    let mut task = KanbanElement::new_task("Original");
    let id = repo.new_id(ElementType::Task).unwrap();
    task.set_id(id.clone());
    repo.save(task).unwrap();

    let mut updated = KanbanElement::new_task("Updated Title");
    updated.set_id(id.clone());
    repo.save(updated).unwrap();

    let retrieved = repo.get(&id).unwrap().unwrap();
    assert_eq!(retrieved.title(), "Updated Title");
}

#[test]
fn test_file_repo_delete_removes_file() {
    let (repo, temp) = create_temp_repo();

    let mut task = KanbanElement::new_task("To Delete");
    let id = repo.new_id(ElementType::Task).unwrap();
    task.set_id(id.clone());
    repo.save(task).unwrap();

    let file_path = temp
        .path()
        .join("kanban")
        .join("elements")
        .join(format!("{}.json", id.as_str()));
    assert!(file_path.exists());

    repo.delete(&id).unwrap();
    assert!(!file_path.exists());
    assert!(repo.get(&id).unwrap().is_none());
}

#[test]
fn test_file_repo_list_all() {
    let (repo, _temp) = create_temp_repo();

    let mut task1 = KanbanElement::new_task("Task 1");
    task1.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task1).unwrap();

    let mut task2 = KanbanElement::new_task("Task 2");
    task2.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task2).unwrap();

    let mut story = KanbanElement::new_story("Story 1", "Content");
    story.set_id(repo.new_id(ElementType::Story).unwrap());
    repo.save(story).unwrap();

    let all = repo.list().unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_file_repo_list_by_type() {
    let (repo, _temp) = create_temp_repo();

    let mut task1 = KanbanElement::new_task("Task 1");
    task1.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task1).unwrap();

    let mut task2 = KanbanElement::new_task("Task 2");
    task2.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task2).unwrap();

    let mut story = KanbanElement::new_story("Story", "Content");
    story.set_id(repo.new_id(ElementType::Story).unwrap());
    repo.save(story).unwrap();

    let tasks = repo.list_by_type(ElementType::Task).unwrap();
    let stories = repo.list_by_type(ElementType::Story).unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(stories.len(), 1);
}

#[test]
fn test_file_repo_list_by_status() {
    let (repo, _temp) = create_temp_repo();

    let mut task1 = KanbanElement::new_task("Task 1");
    task1.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task1).unwrap();

    let mut task2 = KanbanElement::new_task("Task 2");
    task2.set_id(repo.new_id(ElementType::Task).unwrap());
    task2.set_status(Status::Backlog);
    repo.save(task2).unwrap();

    let plan = repo.list_by_status(Status::Plan).unwrap();
    let backlog = repo.list_by_status(Status::Backlog).unwrap();

    assert_eq!(plan.len(), 1);
    assert_eq!(backlog.len(), 1);
}

#[test]
fn test_file_repo_sequential_ids() {
    let (repo, _temp) = create_temp_repo();

    let id1 = repo.new_id(ElementType::Task).unwrap();
    let id2 = repo.new_id(ElementType::Task).unwrap();
    let id3 = repo.new_id(ElementType::Story).unwrap();

    assert_eq!(id1.as_str(), "task-001");
    assert_eq!(id2.as_str(), "task-002");
    assert_eq!(id3.as_str(), "story-001");
}

#[test]
fn test_file_repo_id_persistence_across_restarts() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("kanban");

    // First instance
    {
        let repo = FileKanbanRepository::new(&path).unwrap();
        let mut task = KanbanElement::new_task("Task");
        task.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task).unwrap();
    }

    // Second instance should continue from previous counter
    let repo2 = FileKanbanRepository::new(&path).unwrap();
    let id = repo2.new_id(ElementType::Task).unwrap();
    assert_eq!(id.as_str(), "task-002"); // Should be 2, not 1
}

#[test]
fn test_file_repo_from_workplace() {
    let temp = TempDir::new().unwrap();
    let workplace = temp.path().join("workspace");

    let repo = FileKanbanRepository::from_workplace(&workplace).unwrap();

    assert!(workplace.join("kanban").join("elements").exists());
}
