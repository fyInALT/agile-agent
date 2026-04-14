//! Concrete implementations of KanbanElementTrait
//!
//! Direct field storage without RwLock overhead.

use crate::domain::{ElementId, KanbanElement, Priority};
use crate::traits::KanbanElementTrait;
use crate::types::{ElementTypeIdentifier, StatusType};

/// SprintElement - concrete implementation for Sprint
pub struct SprintElement {
    inner: KanbanElement,
}

impl SprintElement {
    pub fn new(title: &str, goal: &str) -> Self {
        Self {
            inner: KanbanElement::new_sprint(title, goal),
        }
    }

    pub fn new_with_dates(title: &str, goal: &str, start: &str, end: &str) -> Self {
        Self {
            inner: KanbanElement::new_sprint_with_dates(title, goal, start, end),
        }
    }

    /// Get the sprint goal
    pub fn goal(&self) -> String {
        match &self.inner {
            KanbanElement::Sprint(s) => s.goal.clone(),
            _ => String::new(),
        }
    }

    /// Get the sprint start date
    pub fn start_date(&self) -> Option<String> {
        match &self.inner {
            KanbanElement::Sprint(s) => s.start_date.clone(),
            _ => None,
        }
    }

    /// Get the sprint end date
    pub fn end_date(&self) -> Option<String> {
        match &self.inner {
            KanbanElement::Sprint(s) => s.end_date.clone(),
            _ => None,
        }
    }
}

impl KanbanElementTrait for SprintElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("sprint")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        // For Sprint, content is stored in goal field
        match &self.inner {
            KanbanElement::Sprint(s) => s.goal.clone(),
            _ => self.inner.content().to_string(),
        }
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "SprintElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(SprintElement {
            inner: self.inner.clone(),
        })
    }
}

/// StoryElement - concrete implementation for Story
pub struct StoryElement {
    inner: KanbanElement,
}

impl StoryElement {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            inner: KanbanElement::new_story(title, content),
        }
    }

    pub fn new_with_parent(title: &str, content: &str, parent: ElementId) -> Self {
        Self {
            inner: KanbanElement::new_story_with_parent(title, content, parent),
        }
    }
}

impl KanbanElementTrait for StoryElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("story")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        self.inner.content().to_string()
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "StoryElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(StoryElement {
            inner: self.inner.clone(),
        })
    }
}

/// TaskElement - concrete implementation for Task
pub struct TaskElement {
    inner: KanbanElement,
}

impl TaskElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: KanbanElement::new_task(title),
        }
    }

    pub fn new_with_parent(title: &str, parent: ElementId) -> Self {
        Self {
            inner: KanbanElement::new_task_with_parent(title, parent),
        }
    }
}

impl KanbanElementTrait for TaskElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("task")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        self.inner.content().to_string()
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "TaskElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(TaskElement {
            inner: self.inner.clone(),
        })
    }
}

/// IdeaElement - concrete implementation for Idea
pub struct IdeaElement {
    inner: KanbanElement,
}

impl IdeaElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: KanbanElement::new_idea(title),
        }
    }
}

impl KanbanElementTrait for IdeaElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("idea")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        self.inner.content().to_string()
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "IdeaElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(IdeaElement {
            inner: self.inner.clone(),
        })
    }
}

/// IssueElement - concrete implementation for Issue
pub struct IssueElement {
    inner: KanbanElement,
}

impl IssueElement {
    pub fn new(title: &str) -> Self {
        Self {
            inner: KanbanElement::new_issue(title),
        }
    }
}

impl KanbanElementTrait for IssueElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("issue")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        self.inner.content().to_string()
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "IssueElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(IssueElement {
            inner: self.inner.clone(),
        })
    }
}

/// TipsElement - concrete implementation for Tips
pub struct TipsElement {
    inner: KanbanElement,
}

impl TipsElement {
    pub fn new(title: &str, target_task: ElementId, agent_id: &str) -> Self {
        Self {
            inner: KanbanElement::new_tips(title, target_task, agent_id),
        }
    }

    /// Get the target task ID
    pub fn target_task(&self) -> ElementId {
        match &self.inner {
            KanbanElement::Tips(t) => t.target_task.clone(),
            _ => ElementId::new(crate::domain::ElementType::Task, 0),
        }
    }

    /// Get the agent ID
    pub fn agent_id(&self) -> String {
        match &self.inner {
            KanbanElement::Tips(t) => t.agent_id.clone(),
            _ => String::new(),
        }
    }
}

impl KanbanElementTrait for TipsElement {
    fn id(&self) -> Option<ElementId> {
        self.inner.id().cloned()
    }

    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("tips")
    }

    fn status(&self) -> StatusType {
        self.inner.status().to_status_type()
    }

    fn title(&self) -> String {
        self.inner.title().to_string()
    }

    fn content(&self) -> String {
        self.inner.content().to_string()
    }

    fn dependencies(&self) -> Vec<ElementId> {
        self.inner.dependencies().to_vec()
    }

    fn parent(&self) -> Option<ElementId> {
        self.inner.parent().cloned()
    }

    fn assignee(&self) -> Option<String> {
        self.inner.assignee().cloned()
    }

    fn priority(&self) -> Priority {
        self.inner.priority()
    }

    fn effort(&self) -> Option<u32> {
        self.inner.effort()
    }

    fn blocked_reason(&self) -> Option<String> {
        self.inner.blocked_reason().map(|s| s.to_string())
    }

    fn tags(&self) -> Vec<String> {
        self.inner.base().tags.clone()
    }

    fn set_id(&mut self, id: ElementId) {
        self.inner.set_id(id);
    }

    fn set_status(&mut self, status: StatusType) {
        let status_enum: crate::domain::Status = status.into();
        self.inner.set_status(status_enum);
    }

    fn implementation_type(&self) -> &'static str {
        "TipsElement"
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
        Box::new(TipsElement {
            inner: self.inner.clone(),
        })
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