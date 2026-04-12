use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::app::AppState;
use crate::app::LoopPhase;
use crate::app::TranscriptEntry;
use crate::skills::SkillRegistry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentState {
    pub cwd: String,
    pub draft_input: String,
    pub enabled_skill_names: Vec<String>,
    pub transcript: Vec<TranscriptEntry>,
    pub active_task_id: Option<String>,
    pub active_task_had_error: bool,
    pub continuation_attempts: u8,
    pub loop_phase: LoopPhase,
    pub loop_run_active: bool,
    pub remaining_loop_iterations: usize,
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
            transcript: state.transcript.clone(),
            active_task_id: state.active_task_id.clone(),
            active_task_had_error: state.active_task_had_error,
            continuation_attempts: state.continuation_attempts,
            loop_phase: state.loop_phase,
            loop_run_active: state.loop_run_active,
            remaining_loop_iterations: state.remaining_loop_iterations,
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
        state.transcript = self.transcript.clone();
        state.active_task_id = self.active_task_id.clone();
        state.active_task_had_error = self.active_task_had_error;
        state.continuation_attempts = self.continuation_attempts;
        state.loop_phase = self.loop_phase;
        state.loop_run_active = self.loop_run_active;
        state.remaining_loop_iterations = self.remaining_loop_iterations;

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

        RestoreAgentStateResult { warnings }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentState;
    use crate::app::AppState;
    use crate::app::LoopPhase;
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;

    #[test]
    fn round_trips_basic_runtime_state() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.input = "draft".to_string();
        state
            .transcript
            .push(TranscriptEntry::User("hello".to_string()));
        state.active_task_id = Some("task-1".to_string());
        state.loop_phase = LoopPhase::Executing;
        state.loop_run_active = true;

        let snapshot = AgentState::from_app_state(&state);
        let mut restored = AppState::new(ProviderKind::Mock);
        let result = snapshot.apply_to_app_state(&mut restored);

        assert!(result.warnings.is_empty());
        assert_eq!(restored.input, "draft");
        assert_eq!(restored.active_task_id.as_deref(), Some("task-1"));
        assert_eq!(restored.loop_phase, LoopPhase::Executing);
        assert!(restored.loop_run_active);
        assert_eq!(restored.transcript.len(), 1);
    }
}
