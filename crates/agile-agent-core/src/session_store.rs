use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::app::AppState;
use crate::app::TranscriptEntry;
use crate::provider::ProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedSession {
    pub saved_at: String,
    pub cwd: String,
    pub selected_provider: ProviderKind,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub enabled_skill_names: Vec<String>,
    pub transcript: Vec<TranscriptEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentSessionPointer {
    session_path: String,
}

impl PersistedSession {
    pub fn from_app_state(state: &AppState, cwd: &Path) -> Self {
        Self {
            saved_at: Utc::now().to_rfc3339(),
            cwd: cwd.display().to_string(),
            selected_provider: state.selected_provider,
            claude_session_id: state.claude_session_id.clone(),
            codex_thread_id: state.codex_thread_id.clone(),
            enabled_skill_names: state.skills.enabled_names.iter().cloned().collect(),
            transcript: state.transcript.clone(),
        }
    }

    pub fn apply_to_app_state(&self, state: &mut AppState) {
        state.selected_provider = self.selected_provider;
        state.claude_session_id = self.claude_session_id.clone();
        state.codex_thread_id = self.codex_thread_id.clone();
        state.transcript = self.transcript.clone();

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
    }
}

pub fn save_recent_session(state: &AppState, cwd: &Path) -> Result<()> {
    let root = default_session_root()?;
    save_recent_session_to_root(state, cwd, &root)
}

pub fn load_recent_session() -> Result<PersistedSession> {
    let root = default_session_root()?;
    load_recent_session_from_root(&root)
}

fn save_recent_session_to_root(state: &AppState, cwd: &Path, root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("sessions")).context("failed to create session directory")?;

    let session = PersistedSession::from_app_state(state, cwd);
    let file_name = format!("session-{}.json", session.saved_at.replace(':', "-"));
    let session_path = root.join("sessions").join(file_name);

    let session_json =
        serde_json::to_string_pretty(&session).context("failed to serialize persisted session")?;
    fs::write(&session_path, session_json).context("failed to write persisted session")?;

    let pointer = RecentSessionPointer {
        session_path: session_path.display().to_string(),
    };
    let pointer_json =
        serde_json::to_string_pretty(&pointer).context("failed to serialize recent pointer")?;
    fs::write(root.join("recent-session.json"), pointer_json)
        .context("failed to write recent session pointer")?;

    Ok(())
}

fn load_recent_session_from_root(root: &Path) -> Result<PersistedSession> {
    let pointer_path = root.join("recent-session.json");
    let pointer_json = fs::read_to_string(&pointer_path)
        .with_context(|| format!("failed to read {}", pointer_path.display()))?;
    let pointer: RecentSessionPointer =
        serde_json::from_str(&pointer_json).context("failed to parse recent session pointer")?;

    let session_json = fs::read_to_string(&pointer.session_path)
        .with_context(|| format!("failed to read {}", pointer.session_path))?;
    serde_json::from_str(&session_json).context("failed to parse persisted session")
}

fn default_session_root() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir().context("local data directory is unavailable")?;
    Ok(data_dir.join("agile-agent"))
}

#[cfg(test)]
mod tests {
    use super::PersistedSession;
    use super::load_recent_session_from_root;
    use super::save_recent_session_to_root;
    use crate::app::AppState;
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;
    use crate::skills::SkillMetadata;
    use crate::skills::SkillRegistry;

    use tempfile::TempDir;

    #[test]
    fn saves_and_loads_recent_session() {
        let temp = TempDir::new().expect("tempdir");
        let mut registry = SkillRegistry::default();
        registry.discovered.push(SkillMetadata {
            name: "reviewer".to_string(),
            description: "Reviews code".to_string(),
            path: "reviewer/SKILL.md".into(),
            body: "body".to_string(),
        });

        let mut state = AppState::with_skills(ProviderKind::Claude, registry);
        state
            .transcript
            .push(TranscriptEntry::User("hello".to_string()));
        state.claude_session_id = Some("sess-1".to_string());
        state.skills.toggle("reviewer");

        save_recent_session_to_root(&state, temp.path(), temp.path()).expect("save session");
        let restored = load_recent_session_from_root(temp.path()).expect("load session");

        assert_eq!(restored.selected_provider, ProviderKind::Claude);
        assert_eq!(restored.claude_session_id.as_deref(), Some("sess-1"));
        assert_eq!(restored.transcript.len(), 1);
        assert_eq!(restored.enabled_skill_names, vec!["reviewer".to_string()]);
    }

    #[test]
    fn persisted_session_applies_back_to_app_state() {
        let mut registry = SkillRegistry::default();
        registry.discovered.push(SkillMetadata {
            name: "reviewer".to_string(),
            description: "Reviews code".to_string(),
            path: "reviewer/SKILL.md".into(),
            body: "body".to_string(),
        });
        let mut state = AppState::with_skills(ProviderKind::Mock, registry);

        let persisted = PersistedSession {
            saved_at: "2026-01-01T00:00:00Z".to_string(),
            cwd: ".".to_string(),
            selected_provider: ProviderKind::Codex,
            claude_session_id: None,
            codex_thread_id: Some("thr-1".to_string()),
            enabled_skill_names: vec!["reviewer".to_string()],
            transcript: vec![TranscriptEntry::Assistant("pong".to_string())],
        };

        persisted.apply_to_app_state(&mut state);

        assert_eq!(state.selected_provider, ProviderKind::Codex);
        assert_eq!(state.codex_thread_id.as_deref(), Some("thr-1"));
        assert_eq!(state.transcript.len(), 1);
        assert!(state.skills.is_enabled("reviewer"));
    }
}
