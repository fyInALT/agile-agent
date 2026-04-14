//! Concrete implementations of KanbanElementTrait
//!
//! Wraps existing domain structs to implement the trait-based element interface.

use crate::domain::{ElementId, KanbanElement};
use crate::traits::KanbanElementTrait;
use crate::types::{ElementTypeIdentifier, StatusType};
use std::sync::RwLock;

/// SprintElement - concrete implementation wrapping Sprint
pub struct SprintElement {
    inner: RwLock<KanbanElement>,
}

impl SprintElement {
    pub fn new(title: &str, goal: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_sprint(title, goal)),
        }
    }

    pub fn new_with_dates(title: &str, goal: &str, start: &str, end: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_sprint_with_dates(title, goal, start, end)),
        }
    }
}

impl KanbanElementTrait for SprintElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("sprint")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "SprintElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(SprintElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

/// Additional methods for SprintElement
impl SprintElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

/// StoryElement - concrete implementation wrapping Story
pub struct StoryElement {
    inner: RwLock<KanbanElement>,
}

impl StoryElement {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_story(title, content)),
        }
    }

    pub fn new_with_parent(title: &str, content: &str, parent: ElementId) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_story_with_parent(title, content, parent)),
        }
    }
}

impl KanbanElementTrait for StoryElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("story")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "StoryElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(StoryElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

impl StoryElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

/// TaskElement - concrete implementation wrapping Task
pub struct TaskElement {
    inner: RwLock<KanbanElement>,
}

impl TaskElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_task(title)),
        }
    }

    pub fn new_with_parent(title: &str, parent: ElementId) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_task_with_parent(title, parent)),
        }
    }
}

impl KanbanElementTrait for TaskElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("task")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "TaskElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(TaskElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

impl TaskElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

/// IdeaElement - concrete implementation wrapping Idea
pub struct IdeaElement {
    inner: RwLock<KanbanElement>,
}

impl IdeaElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_idea(title)),
        }
    }
}

impl KanbanElementTrait for IdeaElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("idea")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "IdeaElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(IdeaElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

impl IdeaElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

/// IssueElement - concrete implementation wrapping Issue
pub struct IssueElement {
    inner: RwLock<KanbanElement>,
}

impl IssueElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_issue(title)),
        }
    }
}

impl KanbanElementTrait for IssueElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("issue")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "IssueElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(IssueElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

impl IssueElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

/// TipsElement - concrete implementation wrapping Tips
pub struct TipsElement {
    inner: RwLock<KanbanElement>,
}

impl TipsElement {
    pub fn new(title: &str, target_task: ElementId, agent_id: &str) -> Self {
        Self {
            inner: RwLock::new(KanbanElement::new_tips(title, target_task, agent_id)),
        }
    }
}

impl KanbanElementTrait for TipsElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.read().unwrap().id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("tips")
    }

    fn status(&self) -> StatusType {
        self.inner.read().unwrap().status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.read().unwrap().title().to_string()
    }

    fn implementation_type(&self) -> &'static str {
        "TipsElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(TipsElement {
            inner: RwLock::new(self.inner.read().unwrap().clone()),
        })
    }
}

impl TipsElement {
    pub fn set_id(&mut self, id: ElementId) {
        self.inner.write().unwrap().set_id(id);
    }

    pub fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.write().unwrap().set_status(status_enum);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprint_element() {
        let sprint = SprintElement::new("Sprint 1", "Goal");
        assert_eq!(sprint.title(), "Sprint 1");
        assert_eq!(sprint.implementation_type(), "SprintElement");
    }

    #[test]
    fn test_task_element() {
        let task = TaskElement::new("Task 1");
        assert_eq!(task.title(), "Task 1");
        assert_eq!(task.implementation_type(), "TaskElement");
    }
}