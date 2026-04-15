//! Builtin trait implementations for kanban statuses and element types
//!
//! Concrete implementations of KanbanStatus and KanbanElementTypeTrait traits.

use crate::traits::{KanbanElementTypeTrait, KanbanStatus};
use crate::types::{ElementTypeIdentifier, StatusType};

// ============================================================================
// Builtin Status Implementations
// ============================================================================

/// Plan status - initial planning phase
pub struct PlanStatus;

impl KanbanStatus for PlanStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn implementation_type(&self) -> &'static str {
        "PlanStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(PlanStatus)
    }
}

/// Backlog status - ready to be scheduled
pub struct BacklogStatus;

impl KanbanStatus for BacklogStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("backlog")
    }

    fn implementation_type(&self) -> &'static str {
        "BacklogStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(BacklogStatus)
    }
}

/// Blocked status - cannot proceed
pub struct BlockedStatus;

impl KanbanStatus for BlockedStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("blocked")
    }

    fn implementation_type(&self) -> &'static str {
        "BlockedStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(BlockedStatus)
    }
}

/// Ready status - ready to start
pub struct ReadyStatus;

impl KanbanStatus for ReadyStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("ready")
    }

    fn implementation_type(&self) -> &'static str {
        "ReadyStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(ReadyStatus)
    }
}

/// Todo status - scheduled for work
pub struct TodoStatus;

impl KanbanStatus for TodoStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("todo")
    }

    fn implementation_type(&self) -> &'static str {
        "TodoStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(TodoStatus)
    }
}

/// InProgress status - actively being worked on
pub struct InProgressStatus;

impl KanbanStatus for InProgressStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("in_progress")
    }

    fn implementation_type(&self) -> &'static str {
        "InProgressStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(InProgressStatus)
    }
}

/// Done status - completed
pub struct DoneStatus;

impl KanbanStatus for DoneStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("done")
    }

    fn implementation_type(&self) -> &'static str {
        "DoneStatus"
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(DoneStatus)
    }
}

/// Verified status - verified and accepted (terminal)
pub struct VerifiedStatus;

impl KanbanStatus for VerifiedStatus {
    fn status_type(&self) -> StatusType {
        StatusType::new("verified")
    }

    fn implementation_type(&self) -> &'static str {
        "VerifiedStatus"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
        Box::new(VerifiedStatus)
    }
}

/// Builtin status implementations factory functions
pub mod builtin_statuses_impl {
    use super::*;

    pub fn plan() -> Box<dyn KanbanStatus> {
        Box::new(PlanStatus)
    }

    pub fn backlog() -> Box<dyn KanbanStatus> {
        Box::new(BacklogStatus)
    }

    pub fn blocked() -> Box<dyn KanbanStatus> {
        Box::new(BlockedStatus)
    }

    pub fn ready() -> Box<dyn KanbanStatus> {
        Box::new(ReadyStatus)
    }

    pub fn todo() -> Box<dyn KanbanStatus> {
        Box::new(TodoStatus)
    }

    pub fn in_progress() -> Box<dyn KanbanStatus> {
        Box::new(InProgressStatus)
    }

    pub fn done() -> Box<dyn KanbanStatus> {
        Box::new(DoneStatus)
    }

    pub fn verified() -> Box<dyn KanbanStatus> {
        Box::new(VerifiedStatus)
    }

    pub fn all() -> Vec<Box<dyn KanbanStatus>> {
        vec![
            plan(),
            backlog(),
            blocked(),
            ready(),
            todo(),
            in_progress(),
            done(),
            verified(),
        ]
    }
}

// ============================================================================
// Builtin Element Type Implementations
// ============================================================================

/// Sprint element type
pub struct SprintElementType;

impl KanbanElementTypeTrait for SprintElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("sprint")
    }

    fn implementation_type(&self) -> &'static str {
        "SprintElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(SprintElementType)
    }
}

/// Story element type
pub struct StoryElementType;

impl KanbanElementTypeTrait for StoryElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("story")
    }

    fn implementation_type(&self) -> &'static str {
        "StoryElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(StoryElementType)
    }
}

/// Task element type
pub struct TaskElementType;

impl KanbanElementTypeTrait for TaskElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("task")
    }

    fn implementation_type(&self) -> &'static str {
        "TaskElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(TaskElementType)
    }
}

/// Idea element type
pub struct IdeaElementType;

impl KanbanElementTypeTrait for IdeaElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("idea")
    }

    fn implementation_type(&self) -> &'static str {
        "IdeaElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(IdeaElementType)
    }
}

/// Issue element type
pub struct IssueElementType;

impl KanbanElementTypeTrait for IssueElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("issue")
    }

    fn implementation_type(&self) -> &'static str {
        "IssueElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(IssueElementType)
    }
}

/// Tips element type
pub struct TipsElementType;

impl KanbanElementTypeTrait for TipsElementType {
    fn element_type(&self) -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("tips")
    }

    fn implementation_type(&self) -> &'static str {
        "TipsElementType"
    }

    fn default_status(&self) -> StatusType {
        StatusType::new("plan")
    }

    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
        Box::new(TipsElementType)
    }
}

/// Builtin element type implementations factory functions
pub mod builtin_element_types_impl {
    use super::*;

    pub fn sprint() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(SprintElementType)
    }

    pub fn story() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(StoryElementType)
    }

    pub fn task() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(TaskElementType)
    }

    pub fn idea() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(IdeaElementType)
    }

    pub fn issue() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(IssueElementType)
    }

    pub fn tips() -> Box<dyn KanbanElementTypeTrait> {
        Box::new(TipsElementType)
    }

    pub fn all() -> Vec<Box<dyn KanbanElementTypeTrait>> {
        vec![sprint(), story(), task(), idea(), issue(), tips()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_status_plan() {
        let status = builtin_statuses_impl::plan();
        assert_eq!(status.status_type().name(), "plan");
        assert_eq!(status.implementation_type(), "PlanStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_builtin_status_verified() {
        let status = builtin_statuses_impl::verified();
        assert!(status.is_terminal());
    }

    #[test]
    fn test_builtin_element_type_task() {
        let elem_type = builtin_element_types_impl::task();
        assert_eq!(elem_type.element_type().name(), "task");
        assert_eq!(elem_type.implementation_type(), "TaskElementType");
    }
}
