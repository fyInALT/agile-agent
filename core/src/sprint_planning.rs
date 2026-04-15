//! Sprint Planning Session
//!
//! Provides sprint planning support for Scrum ProductOwner role.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use agent_kanban::{ElementType, FileKanbanRepository, KanbanElement, KanbanService, Status};

/// Sprint planning session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintPlanningSession {
    /// Sprint being planned (ID)
    pub sprint_id: Option<String>,
    /// Sprint goal being defined
    pub goal: String,
    /// Stories selected for sprint
    pub selected_stories: Vec<StorySelection>,
    /// Total committed effort
    pub total_effort: u32,
    /// Planning session status
    pub status: PlanningStatus,
    /// Timestamp when planning started
    pub started_at: String,
}

/// Story selection in sprint planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySelection {
    /// Story element ID
    pub story_id: String,
    /// Story title
    pub title: String,
    /// Estimated effort (story points)
    pub effort: u32,
    /// Priority in sprint (1 = highest)
    pub sprint_priority: u32,
    /// Assignee hint (agent codename)
    pub assignee_hint: Option<String>,
}

/// Planning session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningStatus {
    /// Selecting stories for sprint
    Selecting,
    /// Estimating effort for selected stories
    Estimating,
    /// Defining sprint goal
    DefiningGoal,
    /// Finalizing commitment
    Committing,
    /// Planning complete
    Complete,
}

impl Default for PlanningStatus {
    fn default() -> Self {
        Self::Selecting
    }
}

impl SprintPlanningSession {
    /// Create a new planning session
    pub fn new() -> Self {
        Self {
            sprint_id: None,
            goal: String::new(),
            selected_stories: Vec::new(),
            total_effort: 0,
            status: PlanningStatus::default(),
            started_at: Utc::now().to_rfc3339(),
        }
    }

    /// Create planning session for a specific sprint
    pub fn for_sprint(sprint_id: String) -> Self {
        Self {
            sprint_id: Some(sprint_id),
            goal: String::new(),
            selected_stories: Vec::new(),
            total_effort: 0,
            status: PlanningStatus::default(),
            started_at: Utc::now().to_rfc3339(),
        }
    }

    /// Add a story to the sprint selection
    pub fn add_story(&mut self, story_id: String, title: String, effort: u32) {
        let priority = self.selected_stories.len() as u32 + 1;
        self.selected_stories.push(StorySelection {
            story_id,
            title,
            effort,
            sprint_priority: priority,
            assignee_hint: None,
        });
        self.total_effort += effort;
    }

    /// Remove a story from selection
    pub fn remove_story(&mut self, story_id: &str) {
        if let Some(pos) = self
            .selected_stories
            .iter()
            .position(|s| s.story_id == story_id)
        {
            let removed = self.selected_stories.remove(pos);
            self.total_effort -= removed.effort;
            // Re-number priorities
            for (i, story) in self.selected_stories.iter_mut().enumerate() {
                story.sprint_priority = i as u32 + 1;
            }
        }
    }

    /// Set sprint goal
    pub fn set_goal(&mut self, goal: String) {
        self.goal = goal;
    }

    /// Set assignee hint for a story
    pub fn set_assignee_hint(&mut self, story_id: &str, assignee: String) {
        if let Some(story) = self
            .selected_stories
            .iter_mut()
            .find(|s| s.story_id == story_id)
        {
            story.assignee_hint = Some(assignee);
        }
    }

    /// Reorder story priority
    pub fn reorder_story(&mut self, story_id: &str, new_priority: u32) {
        if new_priority == 0 || new_priority > self.selected_stories.len() as u32 {
            return;
        }
        if let Some(pos) = self
            .selected_stories
            .iter()
            .position(|s| s.story_id == story_id)
        {
            // Remove from old position
            let story = self.selected_stories.remove(pos);
            // Insert at new position (priority - 1)
            self.selected_stories
                .insert(new_priority as usize - 1, story);
            // Re-number all priorities
            for (i, s) in self.selected_stories.iter_mut().enumerate() {
                s.sprint_priority = i as u32 + 1;
            }
        }
    }

    /// Advance to next planning phase
    pub fn advance_phase(&mut self) {
        self.status = match self.status {
            PlanningStatus::Selecting => PlanningStatus::Estimating,
            PlanningStatus::Estimating => PlanningStatus::DefiningGoal,
            PlanningStatus::DefiningGoal => PlanningStatus::Committing,
            PlanningStatus::Committing => PlanningStatus::Complete,
            PlanningStatus::Complete => PlanningStatus::Complete,
        };
    }

    /// Check if planning is complete
    pub fn is_complete(&self) -> bool {
        self.status == PlanningStatus::Complete && !self.selected_stories.is_empty()
    }

    /// Get summary for display
    pub fn summary(&self) -> String {
        format!(
            "Sprint Planning: {} stories, {} points total\nGoal: {}",
            self.selected_stories.len(),
            self.total_effort,
            if self.goal.is_empty() {
                "Not defined"
            } else {
                &self.goal
            }
        )
    }
}

impl Default for SprintPlanningSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Sprint planning helper functions
pub struct SprintPlanningHelper;

impl SprintPlanningHelper {
    /// Get stories ready for sprint planning (Backlog or Ready status)
    pub fn get_plannable_stories(
        kanban: &Arc<KanbanService<FileKanbanRepository>>,
    ) -> Vec<KanbanElement> {
        kanban
            .list_by_type(ElementType::Story)
            .unwrap_or_default()
            .into_iter()
            .filter(|s| {
                let status = s.status();
                status == Status::Backlog || status == Status::Ready || status == Status::Todo
            })
            .collect()
    }

    /// Calculate recommended sprint capacity based on team velocity
    pub fn calculate_capacity(team_velocity: u32, buffer_percent: u32) -> u32 {
        // Subtract buffer (usually 10-20% for uncertainty)
        let buffer = team_velocity * buffer_percent / 100;
        team_velocity.saturating_sub(buffer)
    }

    /// Check if sprint commitment exceeds capacity
    pub fn is_over_capacity(total_effort: u32, capacity: u32) -> bool {
        total_effort > capacity
    }

    /// Generate sprint commitment summary
    pub fn generate_commitment_summary(session: &SprintPlanningSession) -> String {
        let mut summary = format!("Sprint Commitment Summary\n");
        summary.push_str("========================\n\n");
        summary.push_str(&format!("Goal: {}\n\n", session.goal));
        summary.push_str("Stories:\n");
        for story in &session.selected_stories {
            summary.push_str(&format!(
                "{}. {} - {} points ({})\n",
                story.sprint_priority,
                story.title,
                story.effort,
                story.assignee_hint.as_deref().unwrap_or("Unassigned")
            ));
        }
        summary.push_str(&format!("\nTotal: {} points\n", session.total_effort));
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_session_new() {
        let session = SprintPlanningSession::new();
        assert!(session.sprint_id.is_none());
        assert!(session.selected_stories.is_empty());
        assert_eq!(session.total_effort, 0);
        assert_eq!(session.status, PlanningStatus::Selecting);
    }

    #[test]
    fn planning_session_add_stories() {
        let mut session = SprintPlanningSession::new();
        session.add_story("story-001".to_string(), "Story 1".to_string(), 5);
        session.add_story("story-002".to_string(), "Story 2".to_string(), 3);

        assert_eq!(session.selected_stories.len(), 2);
        assert_eq!(session.total_effort, 8);
        assert_eq!(session.selected_stories[0].sprint_priority, 1);
        assert_eq!(session.selected_stories[1].sprint_priority, 2);
    }

    #[test]
    fn planning_session_remove_story() {
        let mut session = SprintPlanningSession::new();
        session.add_story("story-001".to_string(), "Story 1".to_string(), 5);
        session.add_story("story-002".to_string(), "Story 2".to_string(), 3);

        session.remove_story("story-001");

        assert_eq!(session.selected_stories.len(), 1);
        assert_eq!(session.total_effort, 3);
        assert_eq!(session.selected_stories[0].sprint_priority, 1);
    }

    #[test]
    fn planning_session_set_goal() {
        let mut session = SprintPlanningSession::new();
        session.set_goal("Deliver user authentication".to_string());
        assert_eq!(session.goal, "Deliver user authentication");
    }

    #[test]
    fn planning_session_set_assignee() {
        let mut session = SprintPlanningSession::new();
        session.add_story("story-001".to_string(), "Story 1".to_string(), 5);
        session.set_assignee_hint("story-001", "alpha".to_string());
        assert_eq!(
            session.selected_stories[0].assignee_hint,
            Some("alpha".to_string())
        );
    }

    #[test]
    fn planning_session_reorder() {
        let mut session = SprintPlanningSession::new();
        session.add_story("story-001".to_string(), "Story 1".to_string(), 5);
        session.add_story("story-002".to_string(), "Story 2".to_string(), 3);
        session.add_story("story-003".to_string(), "Story 3".to_string(), 2);

        session.reorder_story("story-003", 1);

        assert_eq!(session.selected_stories[0].story_id, "story-003");
        assert_eq!(session.selected_stories[0].sprint_priority, 1);
        assert_eq!(session.selected_stories[1].sprint_priority, 2);
        assert_eq!(session.selected_stories[2].sprint_priority, 3);
    }

    #[test]
    fn planning_session_advance_phase() {
        let mut session = SprintPlanningSession::new();
        assert_eq!(session.status, PlanningStatus::Selecting);

        session.advance_phase();
        assert_eq!(session.status, PlanningStatus::Estimating);

        session.advance_phase();
        assert_eq!(session.status, PlanningStatus::DefiningGoal);

        session.advance_phase();
        assert_eq!(session.status, PlanningStatus::Committing);

        session.advance_phase();
        assert_eq!(session.status, PlanningStatus::Complete);
    }

    #[test]
    fn planning_session_is_complete() {
        let mut session = SprintPlanningSession::new();
        assert!(!session.is_complete());

        session.add_story("story-001".to_string(), "Story 1".to_string(), 5);
        session.status = PlanningStatus::Complete;
        assert!(session.is_complete());
    }

    #[test]
    fn capacity_calculation() {
        // Team velocity 40, buffer 20%
        let capacity = SprintPlanningHelper::calculate_capacity(40, 20);
        assert_eq!(capacity, 32);
    }

    #[test]
    fn over_capacity_check() {
        assert!(SprintPlanningHelper::is_over_capacity(40, 32));
        assert!(!SprintPlanningHelper::is_over_capacity(30, 32));
    }

    #[test]
    fn commitment_summary() {
        let mut session = SprintPlanningSession::new();
        session.set_goal("Test goal".to_string());
        session.add_story("story-001".to_string(), "Test Story".to_string(), 5);

        let summary = SprintPlanningHelper::generate_commitment_summary(&session);
        assert!(summary.contains("Test goal"));
        assert!(summary.contains("Test Story"));
        assert!(summary.contains("5 points"));
    }

    #[test]
    fn planning_status_serialization() {
        let status = PlanningStatus::Estimating;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"estimating\"");
        let parsed: PlanningStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PlanningStatus::Estimating);
    }
}
