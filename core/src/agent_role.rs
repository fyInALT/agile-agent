//! Agent Role System
//!
//! Defines agent roles for Scrum-style coordination.

use serde::{Deserialize, Serialize};

/// Agent role in Scrum-style coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Product Owner - focuses on requirements, priorities, backlog grooming
    #[default]
    ProductOwner,
    /// Scrum Master - focuses on process, blockers, coordination
    ScrumMaster,
    /// Developer - focuses on implementation, testing, delivery
    Developer,
}

impl AgentRole {
    /// Get display label for this role
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProductOwner => "PO",
            Self::ScrumMaster => "SM",
            Self::Developer => "DEV",
        }
    }

    /// Get full name for this role
    pub fn name(&self) -> &'static str {
        match self {
            Self::ProductOwner => "Product Owner",
            Self::ScrumMaster => "Scrum Master",
            Self::Developer => "Developer",
        }
    }

    /// Get role focus description
    pub fn focus(&self) -> &'static str {
        match self {
            Self::ProductOwner => "Requirements, priorities, backlog grooming",
            Self::ScrumMaster => "Process, blockers, coordination",
            Self::Developer => "Implementation, testing, delivery",
        }
    }

    /// Get default skills for this role
    pub fn default_skills(&self) -> &[&'static str] {
        match self {
            Self::ProductOwner => &["requirements", "planning"],
            Self::ScrumMaster => &["process", "standup"],
            Self::Developer => &["coding", "testing"],
        }
    }

    /// Get role-specific prompt prefix
    pub fn prompt_prefix(&self) -> &'static str {
        match self {
            Self::ProductOwner => {
                "As Product Owner, focus on requirements clarity, backlog prioritization, and stakeholder value. "
            }
            Self::ScrumMaster => {
                "As Scrum Master, focus on process health, blocker resolution, and team coordination. "
            }
            Self::Developer => {
                "As Developer, focus on implementation quality, testing coverage, and delivery speed. "
            }
        }
    }

    /// Check if a skill is relevant for this role
    pub fn is_skill_relevant(&self, skill_name: &str) -> bool {
        // Check if skill matches any default skill
        self.default_skills().iter().any(|s| skill_name.contains(s))
    }

    /// Get all available roles
    pub fn all() -> &'static [AgentRole] {
        &[Self::ProductOwner, Self::ScrumMaster, Self::Developer]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_labels() {
        assert_eq!(AgentRole::ProductOwner.label(), "PO");
        assert_eq!(AgentRole::ScrumMaster.label(), "SM");
        assert_eq!(AgentRole::Developer.label(), "DEV");
    }

    #[test]
    fn role_names() {
        assert_eq!(AgentRole::ProductOwner.name(), "Product Owner");
        assert_eq!(AgentRole::ScrumMaster.name(), "Scrum Master");
        assert_eq!(AgentRole::Developer.name(), "Developer");
    }

    #[test]
    fn role_focus() {
        assert!(AgentRole::ProductOwner.focus().contains("priorities"));
        assert!(AgentRole::ScrumMaster.focus().contains("blockers"));
        assert!(AgentRole::Developer.focus().contains("Implementation"));
    }

    #[test]
    fn role_default_skills() {
        assert!(AgentRole::ProductOwner.default_skills().contains(&"requirements"));
        assert!(AgentRole::ScrumMaster.default_skills().contains(&"process"));
        assert!(AgentRole::Developer.default_skills().contains(&"coding"));
    }

    #[test]
    fn role_prompt_prefixes() {
        assert!(AgentRole::ProductOwner.prompt_prefix().contains("backlog"));
        assert!(AgentRole::ScrumMaster.prompt_prefix().contains("blocker"));
        assert!(AgentRole::Developer.prompt_prefix().contains("implementation"));
    }

    #[test]
    fn role_skill_relevance() {
        assert!(AgentRole::ProductOwner.is_skill_relevant("requirements_analysis"));
        assert!(AgentRole::ScrumMaster.is_skill_relevant("process_improvement"));
        assert!(AgentRole::Developer.is_skill_relevant("coding_python"));
    }

    #[test]
    fn role_skill_not_relevant() {
        assert!(!AgentRole::ProductOwner.is_skill_relevant("coding"));
        assert!(!AgentRole::ScrumMaster.is_skill_relevant("requirements"));
        assert!(!AgentRole::Developer.is_skill_relevant("standup"));
    }

    #[test]
    fn role_all() {
        let all = AgentRole::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&AgentRole::ProductOwner));
        assert!(all.contains(&AgentRole::ScrumMaster));
        assert!(all.contains(&AgentRole::Developer));
    }

    #[test]
    fn role_default() {
        assert_eq!(AgentRole::default(), AgentRole::ProductOwner);
    }

    #[test]
    fn role_serialization() {
        let role = AgentRole::ScrumMaster;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"scrum_master\"");
        let parsed: AgentRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AgentRole::ScrumMaster);
    }
}