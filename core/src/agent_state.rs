use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::app::AppState;
use crate::app::LoopPhase;
use crate::skills::SkillRegistry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentState {
    pub cwd: String,
    pub draft_input: String,
    pub enabled_skill_names: Vec<String>,
    pub active_task_id: Option<String>,
    pub active_task_had_error: bool,
    pub continuation_attempts: u8,
    pub loop_phase: LoopPhase,
    /// Whether the agent was interrupted during execution (crash, force quit, etc.)
    pub was_interrupted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreAgentStateResult {
    pub warnings: Vec<String>,
}

impl AgentState {
    pub fn from_app_state(state: &AppState) -> Self {
        Self {
            cwd: state.cwd.display().to_string(),
            draft_input: state.input.clone(),
            enabled_skill_names: state.skills.enabled_names.iter().cloned().collect(),
            active_task_id: state.active_task_id.clone(),
            active_task_had_error: state.active_task_had_error,
            continuation_attempts: state.continuation_attempts,
            loop_phase: state.loop_phase,
            was_interrupted: false, // Will be set to true on shutdown if executing
        }
    }

    /// Create a state snapshot marking the agent as interrupted
    pub fn interrupted_from_app_state(state: &AppState) -> Self {
        Self {
            cwd: state.cwd.display().to_string(),
            draft_input: state.input.clone(),
            enabled_skill_names: state.skills.enabled_names.iter().cloned().collect(),
            active_task_id: state.active_task_id.clone(),
            active_task_had_error: state.active_task_had_error,
            continuation_attempts: state.continuation_attempts,
            loop_phase: state.loop_phase,
            was_interrupted: true,
        }
    }

    pub fn apply_to_app_state(&self, state: &mut AppState) -> RestoreAgentStateResult {
        let restored_cwd = PathBuf::from(&self.cwd);
        let mut warnings = Vec::new();
        let effective_cwd = if restored_cwd.is_dir() {
            restored_cwd
        } else {
            warnings.push(format!("saved agent cwd is unavailable: {}", self.cwd));
            state.cwd.clone()
        };

        state.cwd = effective_cwd.clone();
        state.skills = SkillRegistry::discover(&effective_cwd);
        state.input = self.draft_input.clone();
        state.active_task_id = self.active_task_id.clone();
        state.active_task_had_error = self.active_task_had_error;
        state.continuation_attempts = self.continuation_attempts;
        state.loop_phase = self.loop_phase;

        let restored_enabled: BTreeSet<String> = self
            .enabled_skill_names
            .iter()
            .filter(|name| {
                state
                    .skills
                    .discovered
                    .iter()
                    .any(|skill| &skill.name == *name)
            })
            .cloned()
            .collect();
        state.skills.enabled_names = restored_enabled;

        if self.was_interrupted {
            warnings.push("agent was interrupted during previous execution".to_string());
        }

        RestoreAgentStateResult { warnings }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentState;
    use crate::app::AppState;
    use crate::app::LoopPhase;
    use crate::ProviderKind;

    #[test]
    fn round_trips_basic_runtime_state() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.input = "draft".to_string();
        state.active_task_id = Some("task-1".to_string());
        state.loop_phase = LoopPhase::Executing;

        let snapshot = AgentState::from_app_state(&state);
        let mut restored = AppState::new(ProviderKind::Mock);
        let result = snapshot.apply_to_app_state(&mut restored);

        assert!(result.warnings.is_empty());
        assert_eq!(restored.input, "draft");
        assert_eq!(restored.active_task_id.as_deref(), Some("task-1"));
        assert_eq!(restored.loop_phase, LoopPhase::Executing);
    }

    #[test]
    fn interrupted_state_produces_warning_on_restore() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.loop_phase = LoopPhase::Executing;

        let snapshot = AgentState::interrupted_from_app_state(&state);
        assert!(snapshot.was_interrupted);

        let mut restored = AppState::new(ProviderKind::Mock);
        let result = snapshot.apply_to_app_state(&mut restored);

        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("interrupted"));
    }
}
