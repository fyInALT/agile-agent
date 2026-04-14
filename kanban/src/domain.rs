//! Core domain types for the kanban system

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

/// Status represents the current state of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Plan,
    Backlog,
    Blocked,
    Ready,
    Todo,
    InProgress,
    Done,
    Verified,
}

impl Status {
    /// Returns the valid status transitions from this status
    pub fn valid_transitions(&self) -> Vec<Status> {
        match self {
            Status::Plan => vec![Status::Backlog],
            Status::Backlog => vec![Status::Blocked, Status::Ready, Status::Todo, Status::Plan],
            Status::Blocked => vec![Status::Backlog],
            Status::Ready => vec![Status::Todo, Status::Backlog],
            Status::Todo => vec![Status::InProgress, Status::Ready],
            Status::InProgress => vec![Status::Done, Status::Todo],
            Status::Done => vec![Status::Verified, Status::Todo],
            Status::Verified => vec![], // Terminal state
        }
    }

    /// Checks if transitioning to the target status is valid
    pub fn can_transition_to(&self, target: &Status) -> bool {
        self.valid_transitions().contains(target)
    }

    /// Returns true if this is a terminal status (no further transitions possible)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Verified)
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Plan => write!(f, "Plan"),
            Status::Backlog => write!(f, "Backlog"),
            Status::Blocked => write!(f, "Blocked"),
            Status::Ready => write!(f, "Ready"),
            Status::Todo => write!(f, "Todo"),
            Status::InProgress => write!(f, "InProgress"),
            Status::Done => write!(f, "Done"),
            Status::Verified => write!(f, "Verified"),
        }
    }
}

/// Priority represents the urgency of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Option<Priority> {
        match s.to_lowercase().as_str() {
            "critical" => Some(Priority::Critical),
            "high" => Some(Priority::High),
            "medium" => Some(Priority::Medium),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }
}

/// ElementType represents the type of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Sprint,
    Story,
    Task,
    Idea,
    Issue,
    Tips,
}

impl ElementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementType::Sprint => "sprint",
            ElementType::Story => "story",
            ElementType::Task => "task",
            ElementType::Idea => "idea",
            ElementType::Issue => "issue",
            ElementType::Tips => "tips",
        }
    }

    pub fn from_str(s: &str) -> Option<ElementType> {
        match s.to_lowercase().as_str() {
            "sprint" => Some(ElementType::Sprint),
            "story" => Some(ElementType::Story),
            "task" => Some(ElementType::Task),
            "idea" => Some(ElementType::Idea),
            "issue" => Some(ElementType::Issue),
            "tips" | "tip" => Some(ElementType::Tips), // Accept both
            _ => None,
        }
    }
}

/// ElementId is a unique identifier for kanban elements
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementId(String);

impl ElementId {
    /// Creates a new ElementId from a type and number
    pub fn new(element_type: ElementType, number: u32) -> Self {
        ElementId(format!("{}-{:03}", element_type.as_str(), number))
    }

    /// Parses an ElementId from a string (e.g., "sprint-001", "task-042")
    pub fn parse(s: &str) -> Result<Self, ElementIdParseError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(ElementIdParseError::InvalidFormat(s.to_string()));
        }

        let type_str = parts[0];
        let num_str = parts[1];

        let _element_type = ElementType::from_str(type_str)
            .ok_or(ElementIdParseError::InvalidType(type_str.to_string()))?;

        let number = num_str
            .parse::<u32>()
            .map_err(|_| ElementIdParseError::InvalidNumber(num_str.to_string()))?;

        Ok(ElementId(format!("{}-{:03}", type_str, number)))
    }

    /// Returns the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the numeric portion of the ID
    pub fn number(&self) -> u32 {
        let parts: Vec<&str> = self.0.split('-').collect();
        parts[1].parse().unwrap_or(0)
    }

    /// Returns the type portion of the ID
    pub fn type_(&self) -> ElementType {
        let parts: Vec<&str> = self.0.split('-').collect();
        ElementType::from_str(parts[0]).unwrap_or(ElementType::Task)
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for ElementId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Serialize for ElementId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ElementId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ElementId::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for ElementId {
    type Err = ElementIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ElementId::parse(s)
    }
}

/// Error type for ElementId parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementIdParseError {
    InvalidFormat(String),
    InvalidType(String),
    InvalidNumber(String),
}

impl fmt::Display for ElementIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementIdParseError::InvalidFormat(s) => write!(f, "invalid element ID format: {}", s),
            ElementIdParseError::InvalidType(s) => write!(f, "invalid element type: {}", s),
            ElementIdParseError::InvalidNumber(s) => write!(f, "invalid element number: {}", s),
        }
    }
}

impl std::error::Error for ElementIdParseError {}

/// StatusHistoryEntry records a status change with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusHistoryEntry {
    pub status: Status,
    pub entered_at: chrono::DateTime<chrono::Utc>,
}

impl StatusHistoryEntry {
    pub fn new(status: Status) -> Self {
        StatusHistoryEntry {
            status,
            entered_at: chrono::Utc::now(),
        }
    }
}

/// BaseElement contains common fields for all kanban elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseElement {
    pub id: Option<ElementId>,
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub status: Status,
    #[serde(default)]
    pub dependencies: Vec<ElementId>,
    #[serde(default)]
    pub references: Vec<ElementId>,
    pub parent: Option<ElementId>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub priority: Priority,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub effort: Option<u32>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_history: Vec<StatusHistoryEntry>,
}

impl BaseElement {
    pub fn new(_element_type: ElementType, title: &str) -> Self {
        let now = chrono::Utc::now();
        let status = Status::Plan;
        BaseElement {
            id: None,
            title: title.to_string(),
            content: String::new(),
            keywords: Vec::new(),
            status,
            dependencies: Vec::new(),
            references: Vec::new(),
            parent: None,
            created_at: now,
            updated_at: now,
            priority: Priority::Medium,
            assignee: None,
            effort: None,
            blocked_reason: None,
            tags: Vec::new(),
            status_history: vec![StatusHistoryEntry::new(status)],
        }
    }

    pub fn can_transition_to(&self, target: &Status) -> bool {
        self.status.can_transition_to(target)
    }

    pub fn transition(&mut self, new_status: Status) -> Result<(), KanbanTransitionError> {
        if !self.can_transition_to(&new_status) {
            return Err(KanbanTransitionError {
                from: self.status,
                to: new_status,
            });
        }
        self.status = new_status;
        self.updated_at = chrono::Utc::now();
        self.status_history
            .push(StatusHistoryEntry::new(new_status));
        Ok(())
    }
}

/// Error type for invalid status transitions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanTransitionError {
    pub from: Status,
    pub to: Status,
}

impl fmt::Display for KanbanTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid transition from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for KanbanTransitionError {}

/// Sprint represents a sprint element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    #[serde(flatten)]
    pub base: BaseElement,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub active: bool,
}

impl Sprint {
    pub fn new(title: &str, goal: &str) -> Self {
        Sprint {
            base: BaseElement::new(ElementType::Sprint, title),
            goal: goal.to_string(),
            start_date: None,
            end_date: None,
            active: false,
        }
    }

    pub fn new_with_dates(title: &str, goal: &str, start: &str, end: &str) -> Self {
        Sprint {
            base: BaseElement::new(ElementType::Sprint, title),
            goal: goal.to_string(),
            start_date: Some(start.to_string()),
            end_date: Some(end.to_string()),
            active: true,
        }
    }
}

/// Story represents a user story element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Story {
    pub fn new(title: &str, content: &str) -> Self {
        let mut base = BaseElement::new(ElementType::Story, title);
        base.content = content.to_string();
        Story { base }
    }

    pub fn new_with_parent(title: &str, content: &str, parent: ElementId) -> Self {
        let mut story = Story::new(title, content);
        story.base.parent = Some(parent);
        story
    }
}

/// Task represents a task element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Task {
    pub fn new(title: &str) -> Self {
        Task {
            base: BaseElement::new(ElementType::Task, title),
        }
    }

    pub fn new_with_parent(title: &str, parent: ElementId) -> Self {
        let mut task = Task::new(title);
        task.base.parent = Some(parent);
        task
    }
}

/// Idea represents an idea element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Idea {
    pub fn new(title: &str) -> Self {
        Idea {
            base: BaseElement::new(ElementType::Idea, title),
        }
    }
}

/// Issue represents an issue element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    #[serde(flatten)]
    pub base: BaseElement,
}

impl Issue {
    pub fn new(title: &str) -> Self {
        Issue {
            base: BaseElement::new(ElementType::Issue, title),
        }
    }
}

/// Tips represents a tip element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tips {
    #[serde(flatten)]
    pub base: BaseElement,
    pub target_task: ElementId,
    pub agent_id: String,
}

impl Tips {
    pub fn new(title: &str, target_task: ElementId, agent_id: &str) -> Self {
        Tips {
            base: BaseElement::new(ElementType::Tips, title),
            target_task,
            agent_id: agent_id.to_string(),
        }
    }
}

/// KanbanElement is the main enum representing all kanban board elements
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KanbanElement {
    #[serde(rename = "sprint")]
    Sprint(Sprint),
    #[serde(rename = "story")]
    Story(Story),
    #[serde(rename = "task")]
    Task(Task),
    #[serde(rename = "idea")]
    Idea(Idea),
    #[serde(rename = "issue")]
    Issue(Issue),
    #[serde(rename = "tips")]
    Tips(Tips),
}

impl KanbanElement {
    pub fn new_sprint(title: &str, goal: &str) -> Self {
        KanbanElement::Sprint(Sprint::new(title, goal))
    }

    pub fn new_sprint_with_dates(title: &str, goal: &str, start: &str, end: &str) -> Self {
        KanbanElement::Sprint(Sprint::new_with_dates(title, goal, start, end))
    }

    pub fn new_story(title: &str, content: &str) -> Self {
        KanbanElement::Story(Story::new(title, content))
    }

    pub fn new_story_with_parent(title: &str, content: &str, parent: ElementId) -> Self {
        KanbanElement::Story(Story::new_with_parent(title, content, parent))
    }

    pub fn new_task(title: &str) -> Self {
        KanbanElement::Task(Task::new(title))
    }

    pub fn new_task_with_parent(title: &str, parent: ElementId) -> Self {
        KanbanElement::Task(Task::new_with_parent(title, parent))
    }

    pub fn new_idea(title: &str) -> Self {
        KanbanElement::Idea(Idea::new(title))
    }

    pub fn new_issue(title: &str) -> Self {
        KanbanElement::Issue(Issue::new(title))
    }

    pub fn new_tips(title: &str, target_task: ElementId, agent_id: &str) -> Self {
        KanbanElement::Tips(Tips::new(title, target_task, agent_id))
    }

    pub fn id(&self) -> Option<&ElementId> {
        match self {
            KanbanElement::Sprint(s) => s.base.id.as_ref(),
            KanbanElement::Story(s) => s.base.id.as_ref(),
            KanbanElement::Task(t) => t.base.id.as_ref(),
            KanbanElement::Idea(i) => i.base.id.as_ref(),
            KanbanElement::Issue(i) => i.base.id.as_ref(),
            KanbanElement::Tips(t) => t.base.id.as_ref(),
        }
    }

    pub fn set_id(&mut self, id: ElementId) {
        match self {
            KanbanElement::Sprint(s) => s.base.id = Some(id),
            KanbanElement::Story(s) => s.base.id = Some(id),
            KanbanElement::Task(t) => t.base.id = Some(id),
            KanbanElement::Idea(i) => i.base.id = Some(id),
            KanbanElement::Issue(i) => i.base.id = Some(id),
            KanbanElement::Tips(t) => t.base.id = Some(id),
        }
    }

    pub fn element_type(&self) -> ElementType {
        match self {
            KanbanElement::Sprint(_) => ElementType::Sprint,
            KanbanElement::Story(_) => ElementType::Story,
            KanbanElement::Task(_) => ElementType::Task,
            KanbanElement::Idea(_) => ElementType::Idea,
            KanbanElement::Issue(_) => ElementType::Issue,
            KanbanElement::Tips(_) => ElementType::Tips,
        }
    }

    pub fn status(&self) -> Status {
        match self {
            KanbanElement::Sprint(s) => s.base.status,
            KanbanElement::Story(s) => s.base.status,
            KanbanElement::Task(t) => t.base.status,
            KanbanElement::Idea(i) => i.base.status,
            KanbanElement::Issue(i) => i.base.status,
            KanbanElement::Tips(t) => t.base.status,
        }
    }

    pub fn set_status(&mut self, status: Status) {
        match self {
            KanbanElement::Sprint(s) => s.base.status = status,
            KanbanElement::Story(s) => s.base.status = status,
            KanbanElement::Task(t) => t.base.status = status,
            KanbanElement::Idea(i) => i.base.status = status,
            KanbanElement::Issue(i) => i.base.status = status,
            KanbanElement::Tips(t) => t.base.status = status,
        }
    }

    pub fn set_created_at(&mut self, timestamp: chrono::DateTime<chrono::Utc>) {
        match self {
            KanbanElement::Sprint(s) => s.base.created_at = timestamp,
            KanbanElement::Story(s) => s.base.created_at = timestamp,
            KanbanElement::Task(t) => t.base.created_at = timestamp,
            KanbanElement::Idea(i) => i.base.created_at = timestamp,
            KanbanElement::Issue(i) => i.base.created_at = timestamp,
            KanbanElement::Tips(t) => t.base.created_at = timestamp,
        }
    }

    pub fn set_updated_at(&mut self, timestamp: chrono::DateTime<chrono::Utc>) {
        match self {
            KanbanElement::Sprint(s) => s.base.updated_at = timestamp,
            KanbanElement::Story(s) => s.base.updated_at = timestamp,
            KanbanElement::Task(t) => t.base.updated_at = timestamp,
            KanbanElement::Idea(i) => i.base.updated_at = timestamp,
            KanbanElement::Issue(i) => i.base.updated_at = timestamp,
            KanbanElement::Tips(t) => t.base.updated_at = timestamp,
        }
    }

    pub fn can_transition_to(&self, target: &Status) -> bool {
        match self {
            KanbanElement::Sprint(s) => s.base.can_transition_to(target),
            KanbanElement::Story(s) => s.base.can_transition_to(target),
            KanbanElement::Task(t) => t.base.can_transition_to(target),
            KanbanElement::Idea(i) => i.base.can_transition_to(target),
            KanbanElement::Issue(i) => i.base.can_transition_to(target),
            KanbanElement::Tips(t) => t.base.can_transition_to(target),
        }
    }

    pub fn transition(&mut self, new_status: Status) -> Result<(), KanbanTransitionError> {
        match self {
            KanbanElement::Sprint(s) => s.base.transition(new_status),
            KanbanElement::Story(s) => s.base.transition(new_status),
            KanbanElement::Task(t) => t.base.transition(new_status),
            KanbanElement::Idea(i) => i.base.transition(new_status),
            KanbanElement::Issue(i) => i.base.transition(new_status),
            KanbanElement::Tips(t) => t.base.transition(new_status),
        }
    }

    pub fn assignee(&self) -> Option<&String> {
        match self {
            KanbanElement::Sprint(s) => s.base.assignee.as_ref(),
            KanbanElement::Story(s) => s.base.assignee.as_ref(),
            KanbanElement::Task(t) => t.base.assignee.as_ref(),
            KanbanElement::Idea(i) => i.base.assignee.as_ref(),
            KanbanElement::Issue(i) => i.base.assignee.as_ref(),
            KanbanElement::Tips(t) => t.base.assignee.as_ref(),
        }
    }

    pub fn dependencies(&self) -> &[ElementId] {
        match self {
            KanbanElement::Sprint(s) => &s.base.dependencies,
            KanbanElement::Story(s) => &s.base.dependencies,
            KanbanElement::Task(t) => &t.base.dependencies,
            KanbanElement::Idea(i) => &i.base.dependencies,
            KanbanElement::Issue(i) => &i.base.dependencies,
            KanbanElement::Tips(t) => &t.base.dependencies,
        }
    }

    pub fn references(&self) -> &[ElementId] {
        match self {
            KanbanElement::Sprint(s) => &s.base.references,
            KanbanElement::Story(s) => &s.base.references,
            KanbanElement::Task(t) => &t.base.references,
            KanbanElement::Idea(i) => &i.base.references,
            KanbanElement::Issue(i) => &i.base.references,
            KanbanElement::Tips(t) => &t.base.references,
        }
    }

    pub fn parent(&self) -> Option<&ElementId> {
        match self {
            KanbanElement::Sprint(s) => s.base.parent.as_ref(),
            KanbanElement::Story(s) => s.base.parent.as_ref(),
            KanbanElement::Task(t) => t.base.parent.as_ref(),
            KanbanElement::Idea(i) => i.base.parent.as_ref(),
            KanbanElement::Issue(i) => i.base.parent.as_ref(),
            KanbanElement::Tips(t) => t.base.parent.as_ref(),
        }
    }

    pub fn title(&self) -> &str {
        match self {
            KanbanElement::Sprint(s) => &s.base.title,
            KanbanElement::Story(s) => &s.base.title,
            KanbanElement::Task(t) => &t.base.title,
            KanbanElement::Idea(i) => &i.base.title,
            KanbanElement::Issue(i) => &i.base.title,
            KanbanElement::Tips(t) => &t.base.title,
        }
    }

    pub fn base_mut(&mut self) -> &mut BaseElement {
        match self {
            KanbanElement::Sprint(s) => &mut s.base,
            KanbanElement::Story(s) => &mut s.base,
            KanbanElement::Task(t) => &mut t.base,
            KanbanElement::Idea(i) => &mut i.base,
            KanbanElement::Issue(i) => &mut i.base,
            KanbanElement::Tips(t) => &mut t.base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_transitions() {
        assert!(Status::Plan.can_transition_to(&Status::Backlog));
        assert!(!Status::Plan.can_transition_to(&Status::Done));
    }

    #[test]
    fn test_element_id_parse_and_access() {
        let id = ElementId::parse("sprint-001").unwrap();
        assert_eq!(id.number(), 1);
        assert_eq!(id.type_(), ElementType::Sprint);
    }
}
