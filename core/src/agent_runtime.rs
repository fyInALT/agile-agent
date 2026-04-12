use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_memory::AgentMemory;
use crate::agent_messages::AgentMessages;
use crate::agent_state::AgentState;
use crate::agent_state::RestoreAgentStateResult;
use crate::agent_store::AgentStore;
use crate::agent_transcript::AgentTranscript;
use crate::app::AppState;
use crate::app::AppStatus;
use crate::app::LoopPhase;
use crate::provider::ProviderKind;
use crate::provider::SessionHandle;
use crate::workplace_store::WorkplaceStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkplaceId(String);

impl WorkplaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentCodename(String);

impl AgentCodename {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Mock,
    Claude,
    Codex,
    Opencode,
}

impl ProviderType {
    pub fn from_provider_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Mock => Self::Mock,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Codex => Self::Codex,
        }
    }

    pub fn to_provider_kind(self) -> Option<ProviderKind> {
        match self {
            Self::Mock => Some(ProviderKind::Mock),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Codex => Some(ProviderKind::Codex),
            Self::Opencode => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderSessionId(String);

impl ProviderSessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMeta {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub workplace_id: WorkplaceId,
    pub provider_type: ProviderType,
    pub provider_session_id: Option<ProviderSessionId>,
    pub created_at: String,
    pub updated_at: String,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntime {
    meta: AgentMeta,
    workplace: WorkplaceStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentBootstrapKind {
    Created,
    Restored,
    RecreatedAfterError { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentBootstrap {
    pub runtime: AgentRuntime,
    pub kind: AgentBootstrapKind,
}

impl AgentRuntime {
    pub fn new(
        workplace: &WorkplaceStore,
        agent_index: usize,
        provider_kind: ProviderKind,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            meta: AgentMeta {
                agent_id: AgentId::new(format!("agent_{agent_index:03}")),
                codename: AgentCodename::new(generate_codename(agent_index)),
                workplace_id: workplace.workplace_id().clone(),
                provider_type: ProviderType::from_provider_kind(provider_kind),
                provider_session_id: None,
                created_at: now.clone(),
                updated_at: now,
                status: AgentStatus::Idle,
            },
            workplace: workplace.clone(),
        }
    }

    pub fn from_meta(meta: AgentMeta, workplace: WorkplaceStore) -> Self {
        Self { meta, workplace }
    }

    pub fn meta(&self) -> &AgentMeta {
        &self.meta
    }

    pub fn workplace_path(&self) -> &Path {
        self.workplace.path()
    }

    pub fn workplace(&self) -> &WorkplaceStore {
        &self.workplace
    }

    pub fn agent_id(&self) -> &AgentId {
        &self.meta.agent_id
    }

    pub fn codename(&self) -> &AgentCodename {
        &self.meta.codename
    }

    pub fn summary(&self) -> String {
        format!(
            "{} ({})",
            self.meta.codename.as_str(),
            self.meta.agent_id.as_str()
        )
    }

    pub fn apply_to_app_state(&self, state: &mut AppState) -> Vec<String> {
        let mut warnings = Vec::new();
        let store = AgentStore::new(self.workplace.clone());
        state.agent_storage_root = Some(store.agent_dir(&self.meta.agent_id));
        if let Some(provider_kind) = self.meta.provider_type.to_provider_kind() {
            state.selected_provider = provider_kind;
            if let Some(session_id) = self.meta.provider_session_id.as_ref() {
                match self.meta.provider_type {
                    ProviderType::Claude => {
                        state.apply_session_handle(SessionHandle::ClaudeSession {
                            session_id: session_id.as_str().to_string(),
                        });
                    }
                    ProviderType::Codex => {
                        state.apply_session_handle(SessionHandle::CodexThread {
                            thread_id: session_id.as_str().to_string(),
                        });
                    }
                    ProviderType::Mock => {}
                    ProviderType::Opencode => {
                        warnings.push(
                            "restored provider type `opencode`, but runtime support is not available yet"
                                .to_string(),
                        );
                    }
                }
            }
        } else {
            warnings.push(format!(
                "restored provider type `{}` is not supported by the current runtime",
                self.meta.provider_type.label()
            ));
        }

        warnings
    }

    pub fn sync_from_app_state(&mut self, state: &AppState) -> bool {
        let mut changed = false;

        let provider_session_id = provider_session_id_from_app(state);
        if self.meta.provider_session_id != provider_session_id {
            self.meta.provider_session_id = provider_session_id;
            changed = true;
        }

        let status = agent_status_from_app(state);
        if self.meta.status != status {
            self.meta.status = status;
            changed = true;
        }

        if changed {
            self.meta.updated_at = Utc::now().to_rfc3339();
        }

        changed
    }

    pub fn mark_stopped(&mut self) {
        self.meta.status = AgentStatus::Stopped;
        self.meta.updated_at = Utc::now().to_rfc3339();
    }

    pub fn create_sibling(&self, provider_kind: ProviderKind) -> Result<Self> {
        let store = AgentStore::new(self.workplace.clone());
        let runtime = Self::new(&self.workplace, store.next_agent_index()?, provider_kind);
        runtime.persist()?;
        Ok(runtime)
    }

    pub fn persist(&self) -> Result<std::path::PathBuf> {
        AgentStore::new(self.workplace.clone()).save_meta(&self.meta)
    }

    pub fn persist_state(&self, state: &AppState) -> Result<std::path::PathBuf> {
        AgentStore::new(self.workplace.clone())
            .save_state(&self.meta.agent_id, &AgentState::from_app_state(state))
    }

    pub fn restore_state(&self, state: &mut AppState) -> Result<RestoreAgentStateResult> {
        let snapshot = AgentStore::new(self.workplace.clone()).load_state(&self.meta.agent_id)?;
        Ok(snapshot.apply_to_app_state(state))
    }

    pub fn persist_transcript(&self, state: &AppState) -> Result<std::path::PathBuf> {
        AgentStore::new(self.workplace.clone())
            .save_transcript(&self.meta.agent_id, &AgentTranscript::from_app_state(state))
    }

    pub fn restore_transcript(&self, state: &mut AppState) -> Result<()> {
        let snapshot =
            AgentStore::new(self.workplace.clone()).load_transcript(&self.meta.agent_id)?;
        snapshot.apply_to_app_state(state);
        Ok(())
    }

    pub fn persist_messages(&self, state: &AppState) -> Result<std::path::PathBuf> {
        AgentStore::new(self.workplace.clone())
            .save_messages(&self.meta.agent_id, &AgentMessages::from_app_state(state))
    }

    pub fn persist_memory(&self, state: &AppState) -> Result<std::path::PathBuf> {
        AgentStore::new(self.workplace.clone()).save_memory(
            &self.meta.agent_id,
            &AgentMemory::from_runtime_and_app(self, state),
        )
    }

    pub fn bootstrap_for_cwd(cwd: &Path, default_provider: ProviderKind) -> Result<AgentBootstrap> {
        let workplace = WorkplaceStore::for_cwd(cwd)?;
        workplace.ensure()?;
        let store = AgentStore::new(workplace.clone());

        match store.load_most_recent_meta() {
            Ok(Some(meta)) => Ok(AgentBootstrap {
                runtime: Self::from_meta(meta, workplace),
                kind: AgentBootstrapKind::Restored,
            }),
            Ok(None) => {
                let runtime = Self::new(&workplace, store.next_agent_index()?, default_provider);
                runtime.persist()?;
                Ok(AgentBootstrap {
                    runtime,
                    kind: AgentBootstrapKind::Created,
                })
            }
            Err(error) => {
                let runtime = Self::new(&workplace, store.next_agent_index()?, default_provider);
                runtime.persist()?;
                Ok(AgentBootstrap {
                    runtime,
                    kind: AgentBootstrapKind::RecreatedAfterError {
                        error: error.to_string(),
                    },
                })
            }
        }
    }
}

fn provider_session_id_from_app(state: &AppState) -> Option<ProviderSessionId> {
    match state.current_session_handle() {
        Some(SessionHandle::ClaudeSession { session_id }) => {
            Some(ProviderSessionId::new(session_id))
        }
        Some(SessionHandle::CodexThread { thread_id }) => Some(ProviderSessionId::new(thread_id)),
        None => None,
    }
}

fn agent_status_from_app(state: &AppState) -> AgentStatus {
    if state.should_quit {
        return AgentStatus::Stopped;
    }
    if state.status == AppStatus::Responding || state.loop_phase != LoopPhase::Idle {
        AgentStatus::Running
    } else {
        AgentStatus::Idle
    }
}

fn generate_codename(index: usize) -> String {
    const NAMES: &[&str] = &[
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliet", "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo", "sierra",
        "tango", "uniform", "victor", "whiskey", "xray", "yankee", "zulu",
    ];

    let zero_based = index.saturating_sub(1);
    let base = NAMES[zero_based % NAMES.len()];
    let round = zero_based / NAMES.len();
    if round == 0 {
        base.to_string()
    } else {
        format!("{base}-{}", round + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::AgentBootstrapKind;
    use super::AgentRuntime;
    use super::AgentStatus;
    use super::ProviderSessionId;
    use super::ProviderType;
    use crate::app::AppState;
    use crate::app::AppStatus;
    use crate::app::LoopPhase;
    use crate::provider::ProviderKind;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    #[test]
    fn new_runtime_creates_expected_identity_fields() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Claude);

        assert_eq!(runtime.meta().agent_id.as_str(), "agent_001");
        assert_eq!(runtime.meta().codename.as_str(), "alpha");
        assert_eq!(runtime.meta().provider_type, ProviderType::Claude);
        assert!(runtime.meta().provider_session_id.is_none());
    }

    #[test]
    fn sync_from_app_updates_provider_session_and_status() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let mut runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Mock);
        let mut app = AppState::new(ProviderKind::Codex);
        app.codex_thread_id = Some("thr-1".to_string());
        app.status = AppStatus::Responding;
        app.loop_phase = LoopPhase::Executing;

        let changed = runtime.sync_from_app_state(&app);

        assert!(changed);
        assert_eq!(runtime.meta().provider_type, ProviderType::Mock);
        assert_eq!(
            runtime.meta().provider_session_id,
            Some(ProviderSessionId::new("thr-1"))
        );
        assert_eq!(runtime.meta().status, AgentStatus::Running);
    }

    #[test]
    fn mark_stopped_updates_status() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let mut runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Mock);

        runtime.mark_stopped();

        assert_eq!(runtime.meta().status, AgentStatus::Stopped);
    }

    #[test]
    fn bootstrap_restores_existing_runtime() {
        let temp = TempDir::new().expect("tempdir");
        let first =
            AgentRuntime::bootstrap_for_cwd(temp.path(), ProviderKind::Claude).expect("bootstrap");
        assert!(matches!(first.kind, AgentBootstrapKind::Created));

        let restored =
            AgentRuntime::bootstrap_for_cwd(temp.path(), ProviderKind::Mock).expect("bootstrap");
        assert!(matches!(restored.kind, AgentBootstrapKind::Restored));
        assert_eq!(
            restored.runtime.agent_id().as_str(),
            first.runtime.agent_id().as_str()
        );
        assert_eq!(restored.runtime.meta().provider_type, ProviderType::Claude);
    }

    #[test]
    fn bootstrap_recreates_runtime_after_broken_meta() {
        let temp = TempDir::new().expect("tempdir");
        let first =
            AgentRuntime::bootstrap_for_cwd(temp.path(), ProviderKind::Claude).expect("bootstrap");
        let meta_path = first
            .runtime
            .workplace_path()
            .join("agents")
            .join(first.runtime.agent_id().as_str())
            .join("meta.json");
        std::fs::write(&meta_path, "{ not valid json").expect("corrupt meta");

        let recreated =
            AgentRuntime::bootstrap_for_cwd(temp.path(), ProviderKind::Mock).expect("bootstrap");

        assert!(matches!(
            recreated.kind,
            AgentBootstrapKind::RecreatedAfterError { .. }
        ));
        assert_ne!(
            recreated.runtime.agent_id().as_str(),
            first.runtime.agent_id().as_str()
        );
    }

    #[test]
    fn sync_does_not_mutate_provider_binding() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let mut runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Claude);
        let app = AppState::new(ProviderKind::Codex);

        runtime.sync_from_app_state(&app);

        assert_eq!(runtime.meta().provider_type, ProviderType::Claude);
    }

    #[test]
    fn create_sibling_creates_new_agent_id_for_new_provider() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Claude);
        runtime.persist().expect("persist");

        let sibling = runtime
            .create_sibling(ProviderKind::Codex)
            .expect("sibling");

        assert_ne!(sibling.agent_id().as_str(), runtime.agent_id().as_str());
        assert_eq!(sibling.meta().provider_type, ProviderType::Codex);
    }
}
