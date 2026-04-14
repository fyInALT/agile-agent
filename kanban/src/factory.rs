//! ElementFactory for dynamic element creation
//!
//! Factory pattern for creating elements based on element type identifiers.

use crate::elements::{SprintElement, StoryElement, TaskElement, IdeaElement, IssueElement, TipsElement};
use crate::registry::ElementTypeRegistry;
use crate::serde::ElementSerde;
use crate::traits::KanbanElementTrait;
use crate::types::{ElementTypeIdentifier, StatusType};
use std::sync::Arc;

/// ElementFactory - creates elements dynamically based on type
///
/// Uses ElementTypeRegistry for extensibility (future custom element types).
pub struct ElementFactory {
    #[allow(dead_code)]
    registry: Option<Arc<ElementTypeRegistry>>,
}

impl ElementFactory {
    /// Create a new factory with default builtin element types
    pub fn new() -> Self {
        Self { registry: None }
    }

    /// Create a factory with a custom registry
    pub fn with_registry(registry: ElementTypeRegistry) -> Self {
        Self {
            registry: Some(Arc::new(registry)),
        }
    }

    /// Create a factory with shared registry
    pub fn with_shared_registry(registry: Arc<ElementTypeRegistry>) -> Self {
        Self { registry: Some(registry) }
    }

    /// Check if this factory can create an element of the given type
    pub fn can_create(&self, type_: &ElementTypeIdentifier) -> bool {
        matches!(
            type_.name(),
            "sprint" | "story" | "task" | "idea" | "issue" | "tips"
        )
    }

    /// Create an element with just a title
    pub fn create(&self, type_: &ElementTypeIdentifier, title: &str) -> Option<Box<dyn KanbanElementTrait>> {
        match type_.name() {
            "sprint" => Some(Box::new(SprintElement::new(title, ""))),
            "story" => Some(Box::new(StoryElement::new(title, ""))),
            "task" => Some(Box::new(TaskElement::new(title))),
            "idea" => Some(Box::new(IdeaElement::new(title))),
            "issue" => Some(Box::new(IssueElement::new(title))),
            _ => None,
        }
    }

    /// Create an element with title and content
    pub fn create_with_content(
        &self,
        type_: &ElementTypeIdentifier,
        title: &str,
        content: &str,
    ) -> Option<Box<dyn KanbanElementTrait>> {
        match type_.name() {
            "sprint" => Some(Box::new(SprintElement::new(title, content))),
            "story" => Some(Box::new(StoryElement::new(title, content))),
            "task" => Some(Box::new(TaskElement::new(title))),
            "idea" => Some(Box::new(IdeaElement::new(title))),
            "issue" => Some(Box::new(IssueElement::new(title))),
            _ => None,
        }
    }

    /// Create a sprint element with goal
    pub fn create_sprint(&self, title: &str, goal: &str) -> Box<dyn KanbanElementTrait> {
        Box::new(SprintElement::new(title, goal))
    }

    /// Create a sprint with dates
    pub fn create_sprint_with_dates(
        &self,
        title: &str,
        goal: &str,
        start: &str,
        end: &str,
    ) -> Box<dyn KanbanElementTrait> {
        Box::new(SprintElement::new_with_dates(title, goal, start, end))
    }

    /// Create a story with parent
    pub fn create_story_with_parent(
        &self,
        title: &str,
        content: &str,
        parent: crate::domain::ElementId,
    ) -> Box<dyn KanbanElementTrait> {
        Box::new(StoryElement::new_with_parent(title, content, parent))
    }

    /// Create a task with parent
    pub fn create_task_with_parent(
        &self,
        title: &str,
        parent: crate::domain::ElementId,
    ) -> Box<dyn KanbanElementTrait> {
        Box::new(TaskElement::new_with_parent(title, parent))
    }

    /// Create a tips element
    pub fn create_tips(
        &self,
        title: &str,
        target_task: crate::domain::ElementId,
        agent_id: &str,
    ) -> Box<dyn KanbanElementTrait> {
        Box::new(TipsElement::new(title, target_task, agent_id))
    }

    /// Create an element from ElementSerde (deserialization)
    ///
    /// Reconstructs an element from its serialized form.
    pub fn from_serde(&self, serde: &ElementSerde) -> Option<Box<dyn KanbanElementTrait>> {
        match serde.element_type.as_str() {
            "sprint" => {
                let mut element = SprintElement::new(&serde.title, &serde.content);
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            "story" => {
                let mut element = StoryElement::new(&serde.title, &serde.content);
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            "task" => {
                let mut element = TaskElement::new(&serde.title);
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            "idea" => {
                let mut element = IdeaElement::new(&serde.title);
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            "issue" => {
                let mut element = IssueElement::new(&serde.title);
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            "tips" => {
                // Tips needs target_task and agent_id - use defaults for now
                let target_task = crate::domain::ElementId::new(crate::domain::ElementType::Task, 0);
                let mut element = TipsElement::new(&serde.title, target_task, "unknown");
                if let Some(id_str) = &serde.id {
                    if let Ok(id) = crate::domain::ElementId::parse(id_str) {
                        element.set_id(id);
                    }
                }
                element.set_status(StatusType::new(&serde.status));
                Some(Box::new(element))
            }
            _ => None,
        }
    }
}

impl Default for ElementFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_new() {
        let factory = ElementFactory::new();
        assert!(factory.can_create(&ElementTypeIdentifier::new("task")));
    }

    #[test]
    fn test_factory_create_task() {
        let factory = ElementFactory::new();
        let task = factory.create(&ElementTypeIdentifier::new("task"), "Task");
        assert!(task.is_some());
        assert_eq!(task.unwrap().title(), "Task");
    }
}