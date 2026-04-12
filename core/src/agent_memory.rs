use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_runtime::AgentRuntime;
use crate::app::AppState;
use crate::app::TranscriptEntry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMemory {
    pub updated_at: String,
    pub agent_summary: String,
    pub provider_type: String,
    pub provider_session_id: Option<String>,
    pub last_assistant_summary: Option<String>,
    pub active_task_id: Option<String>,
}

impl AgentMemory {
    pub fn from_runtime_and_app(runtime: &AgentRuntime, state: &AppState) -> Self {
        Self {
            updated_at: Utc::now().to_rfc3339(),
            agent_summary: runtime.summary(),
            provider_type: runtime.meta().provider_type.label().to_string(),
            provider_session_id: runtime
                .meta()
                .provider_session_id
                .as_ref()
                .map(|value| value.as_str().to_string()),
            last_assistant_summary: state.transcript.iter().rev().find_map(|entry| match entry {
                TranscriptEntry::Assistant(text) if !text.is_empty() => Some(text.clone()),
                _ => None,
            }),
            active_task_id: state.active_task_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentMemory;
    use crate::agent_runtime::AgentRuntime;
    use crate::app::AppState;
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    #[test]
    fn captures_agent_runtime_summary() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Claude);
        let mut state = AppState::new(ProviderKind::Claude);
        state
            .transcript
            .push(TranscriptEntry::Assistant("done".to_string()));

        let memory = AgentMemory::from_runtime_and_app(&runtime, &state);

        assert!(memory.agent_summary.contains("alpha"));
        assert_eq!(memory.provider_type, "claude");
        assert_eq!(memory.last_assistant_summary.as_deref(), Some("done"));
    }
}
