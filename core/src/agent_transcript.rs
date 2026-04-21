use serde::Deserialize;
use serde::Serialize;

use crate::app::AppState;
use crate::app::TranscriptEntry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTranscript {
    pub entries: Vec<TranscriptEntry>,
}

impl AgentTranscript {
    pub fn from_app_state(state: &AppState) -> Self {
        Self {
            entries: state.transcript.clone(),
        }
    }

    pub fn from_entries(entries: Vec<TranscriptEntry>) -> Self {
        Self { entries }
    }

    pub fn apply_to_app_state(&self, state: &mut AppState) {
        state.transcript = self.entries.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::AgentTranscript;
    use crate::app::AppState;
    use crate::app::TranscriptEntry;
    use crate::ProviderKind;

    #[test]
    fn round_trips_transcript_entries() {
        let mut state = AppState::new(ProviderKind::Mock);
        state
            .transcript
            .push(TranscriptEntry::User("hello".to_string()));

        let snapshot = AgentTranscript::from_app_state(&state);
        let mut restored = AppState::new(ProviderKind::Mock);
        snapshot.apply_to_app_state(&mut restored);

        assert_eq!(restored.transcript.len(), 1);
    }
}
