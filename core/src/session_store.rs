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
use crate::app::LoopPhase;
use crate::app::TranscriptEntry;
use crate::backlog::BacklogState;
use crate::logging;
use crate::provider::ProviderKind;
use crate::skills::SkillRegistry;
use crate::storage;
use crate::workplace_store::WorkplaceStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedSession {
    pub saved_at: String,
    pub cwd: String,
    pub selected_provider: ProviderKind,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub enabled_skill_names: Vec<String>,
    pub transcript: Vec<TranscriptEntry>,
    pub backlog: BacklogState,
    pub active_task_id: Option<String>,
    pub active_task_had_error: bool,
    pub continuation_attempts: u8,
    pub loop_phase: LoopPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentSessionPointer {
    session_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreSessionResult {
    pub warnings: Vec<String>,
}

impl PersistedSession {
    pub fn from_app_state(state: &AppState) -> Self {
        Self {
            saved_at: Utc::now().to_rfc3339(),
            cwd: state.cwd.display().to_string(),
            selected_provider: state.selected_provider,
            claude_session_id: state.claude_session_id.clone(),
            codex_thread_id: state.codex_thread_id.clone(),
            enabled_skill_names: state.skills.enabled_names.iter().cloned().collect(),
            transcript: state.transcript.clone(),
            backlog: state.backlog.clone(),
            active_task_id: state.active_task_id.clone(),
            active_task_had_error: state.active_task_had_error,
            continuation_attempts: state.continuation_attempts,
            loop_phase: state.loop_phase,
        }
    }

    pub fn apply_to_app_state(&self, state: &mut AppState) {
        self.apply_to_app_state_with_cwd(state, PathBuf::from(&self.cwd));
    }

    pub fn apply_to_app_state_with_cwd(&self, state: &mut AppState, cwd: PathBuf) {
        state.cwd = cwd;
        state.selected_provider = self.selected_provider;
        state.claude_session_id = self.claude_session_id.clone();
        state.codex_thread_id = self.codex_thread_id.clone();
        state.transcript = self.transcript.clone();
        state.backlog = self.backlog.clone();
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
    }
}

pub fn save_recent_session(state: &AppState) -> Result<()> {
    let root = default_session_root()?;
    save_recent_session_to_root(state, &root)
}

pub fn load_recent_session() -> Result<PersistedSession> {
    let root = default_session_root()?;
    load_recent_session_from_root(&root)
}

pub fn restore_recent_session(
    state: &mut AppState,
    launch_cwd: &Path,
) -> Result<RestoreSessionResult> {
    let session = load_recent_session()?;
    Ok(apply_restored_session(state, &session, launch_cwd))
}

pub fn save_recent_session_for_workplace(
    state: &AppState,
    workplace: &WorkplaceStore,
) -> Result<()> {
    save_recent_session_to_root(state, workplace.path())
}

pub fn load_recent_session_for_workplace(workplace: &WorkplaceStore) -> Result<PersistedSession> {
    load_recent_session_from_root(workplace.path())
}

pub fn restore_recent_session_for_workplace(
    state: &mut AppState,
    launch_cwd: &Path,
    workplace: &WorkplaceStore,
) -> Result<RestoreSessionResult> {
    let session = load_recent_session_for_workplace(workplace)?;
    Ok(apply_restored_session(state, &session, launch_cwd))
}

fn save_recent_session_to_root(state: &AppState, root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("sessions")).context("failed to create session directory")?;

    let session = PersistedSession::from_app_state(state);
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
    logging::debug_event(
        "storage.write",
        "saved recent session",
        serde_json::json!({
            "kind": "recent_session",
            "session_path": session_path.display().to_string(),
            "pointer_path": root.join("recent-session.json").display().to_string(),
        }),
    );

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
    let session =
        serde_json::from_str(&session_json).context("failed to parse persisted session")?;
    logging::debug_event(
        "storage.read",
        "loaded recent session",
        serde_json::json!({
            "kind": "recent_session",
            "pointer_path": pointer_path.display().to_string(),
            "session_path": pointer.session_path,
        }),
    );
    Ok(session)
}

fn default_session_root() -> Result<PathBuf> {
    storage::app_data_root().context("failed to resolve session root")
}

fn apply_restored_session(
    state: &mut AppState,
    session: &PersistedSession,
    launch_cwd: &Path,
) -> RestoreSessionResult {
    let restored_cwd = PathBuf::from(&session.cwd);
    let mut warnings = Vec::new();
    let effective_cwd = if restored_cwd.is_dir() {
        restored_cwd
    } else {
        warnings.push(format!("saved session cwd is unavailable: {}", session.cwd));
        launch_cwd.to_path_buf()
    };

    state.skills = SkillRegistry::discover(&effective_cwd);
    session.apply_to_app_state_with_cwd(state, effective_cwd);

    RestoreSessionResult { warnings }
}

#[cfg(test)]
mod tests {
    use super::PersistedSession;
    use super::apply_restored_session;
    use super::load_recent_session_for_workplace;
    use super::load_recent_session_from_root;
    use super::save_recent_session_for_workplace;
    use super::save_recent_session_to_root;
    use crate::app::AppState;
    use crate::app::LoopPhase;
    use crate::app::TranscriptEntry;
    use crate::backlog::BacklogState;
    use crate::provider::ProviderKind;
    use crate::skills::SkillMetadata;
    use crate::skills::SkillRegistry;
    use crate::workplace_store::WorkplaceStore;
    use std::path::Path;
    use std::path::PathBuf;

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

        let mut state = AppState::with_skills(ProviderKind::Claude, temp.path().into(), registry);
        state
            .transcript
            .push(TranscriptEntry::User("hello".to_string()));
        state.claude_session_id = Some("sess-1".to_string());
        state.skills.toggle("reviewer");

        save_recent_session_to_root(&state, temp.path()).expect("save session");
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
        let mut state = AppState::with_skills(ProviderKind::Mock, ".".into(), registry);

        let persisted = PersistedSession {
            saved_at: "2026-01-01T00:00:00Z".to_string(),
            cwd: ".".to_string(),
            selected_provider: ProviderKind::Codex,
            claude_session_id: None,
            codex_thread_id: Some("thr-1".to_string()),
            enabled_skill_names: vec!["reviewer".to_string()],
            transcript: vec![TranscriptEntry::Assistant("pong".to_string())],
            backlog: BacklogState::default(),
            active_task_id: Some("task-1".to_string()),
            active_task_had_error: false,
            continuation_attempts: 1,
            loop_phase: LoopPhase::Executing,
        };

        persisted.apply_to_app_state(&mut state);

        assert_eq!(state.cwd, PathBuf::from("."));
        assert_eq!(state.selected_provider, ProviderKind::Codex);
        assert_eq!(state.codex_thread_id.as_deref(), Some("thr-1"));
        assert_eq!(state.transcript.len(), 1);
        assert!(state.skills.is_enabled("reviewer"));
        assert_eq!(state.active_task_id.as_deref(), Some("task-1"));
        assert_eq!(state.continuation_attempts, 1);
    }

    #[test]
    fn restore_warns_when_saved_cwd_is_missing() {
        let mut state = AppState::default();
        let session = PersistedSession {
            saved_at: "2026-01-01T00:00:00Z".to_string(),
            cwd: "/definitely/missing".to_string(),
            selected_provider: ProviderKind::Mock,
            claude_session_id: None,
            codex_thread_id: None,
            enabled_skill_names: Vec::new(),
            transcript: Vec::new(),
            backlog: BacklogState::default(),
            active_task_id: None,
            active_task_had_error: false,
            continuation_attempts: 0,
            loop_phase: LoopPhase::Idle,
        };

        let restored = apply_restored_session(&mut state, &session, Path::new("."));

        assert_eq!(state.cwd, PathBuf::from("."));
        assert_eq!(restored.warnings.len(), 1);
    }

    #[test]
    fn saves_and_loads_recent_session_for_workplace() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");
        let mut state = AppState::new(ProviderKind::Codex);
        state.codex_thread_id = Some("thr-123".to_string());

        save_recent_session_for_workplace(&state, &workplace).expect("save session");
        let restored = load_recent_session_for_workplace(&workplace).expect("load session");

        assert_eq!(restored.selected_provider, ProviderKind::Codex);
        assert_eq!(restored.codex_thread_id.as_deref(), Some("thr-123"));
    }
}
