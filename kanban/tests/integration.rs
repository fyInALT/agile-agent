//! Integration tests for WorkplaceStore + FileKanbanRepository

use agent_kanban::FileKanbanRepository;
use agent_kanban::domain::{ElementType, KanbanElement};
use agent_kanban::repository::KanbanElementRepository;

#[test]
fn test_file_repository_from_workplace() {
    let temp = tempfile::TempDir::new().unwrap();
    let workplace = temp.path().join("workplace");

    // Create repo using from_workplace
    let _repo = FileKanbanRepository::from_workplace(&workplace).unwrap();

    // Verify kanban directory was created
    assert!(workplace.join("kanban").join("elements").exists());
}

#[test]
fn test_file_repository_from_workplace_nested() {
    let temp = tempfile::TempDir::new().unwrap();
    let workplace = temp.path().join("workspace").join("project");

    // Create repo using from_workplace
    let _repo = FileKanbanRepository::from_workplace(&workplace).unwrap();

    // Verify kanban directory structure was created
    assert!(workplace.join("kanban").join("elements").exists());
}

#[test]
fn test_crud_operations_through_repository() {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = FileKanbanRepository::new(temp.path().join("kanban")).unwrap();

    // Create
    let mut task = KanbanElement::new_task("Test Task");
    let id = repo.new_id(ElementType::Task).unwrap();
    task.set_id(id.clone());
    repo.save(task).unwrap();

    // Read
    let retrieved = repo.get(&id).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().title(), "Test Task");

    // Update
    let mut updated = KanbanElement::new_task("Updated Task");
    updated.set_id(id.clone());
    repo.save(updated).unwrap();

    let retrieved = repo.get(&id).unwrap();
    assert_eq!(retrieved.unwrap().title(), "Updated Task");

    // Delete
    repo.delete(&id).unwrap();
    assert!(repo.get(&id).unwrap().is_none());
}

#[test]
fn test_list_by_type_through_repository() {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = FileKanbanRepository::new(temp.path().join("kanban")).unwrap();

    // Create tasks and stories
    let mut task1 = KanbanElement::new_task("Task 1");
    task1.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task1).unwrap();

    let mut task2 = KanbanElement::new_task("Task 2");
    task2.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task2).unwrap();

    let mut story = KanbanElement::new_story("Story 1", "Content");
    story.set_id(repo.new_id(ElementType::Story).unwrap());
    repo.save(story).unwrap();

    // List all
    let all = repo.list().unwrap();
    assert_eq!(all.len(), 3);

    // List by type
    let tasks = repo.list_by_type(ElementType::Task).unwrap();
    assert_eq!(tasks.len(), 2);

    let stories = repo.list_by_type(ElementType::Story).unwrap();
    assert_eq!(stories.len(), 1);
}

#[test]
fn test_list_by_status_through_repository() {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = FileKanbanRepository::new(temp.path().join("kanban")).unwrap();

    // Create elements
    let mut task1 = KanbanElement::new_task("Task 1");
    task1.set_id(repo.new_id(ElementType::Task).unwrap());
    repo.save(task1).unwrap();

    let mut task2 = KanbanElement::new_task("Task 2");
    task2.set_id(repo.new_id(ElementType::Task).unwrap());
    task2.set_status(agent_kanban::domain::Status::Backlog);
    repo.save(task2).unwrap();

    // List all
    let all = repo.list().unwrap();
    assert_eq!(all.len(), 2);

    // List by status
    let plan = repo
        .list_by_status(agent_kanban::domain::Status::Plan)
        .unwrap();
    assert_eq!(plan.len(), 1);

    let backlog = repo
        .list_by_status(agent_kanban::domain::Status::Backlog)
        .unwrap();
    assert_eq!(backlog.len(), 1);
}
