use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::FixedOffset;

use crate::agent_memory::AgentMemory;
use crate::agent_messages::AgentMessages;
use crate::agent_runtime::AgentId;
use crate::agent_runtime::AgentMeta;
use crate::agent_state::AgentState;
use crate::agent_transcript::AgentTranscript;
use crate::logging;
use crate::workplace_store::WorkplaceStore;

#[derive(Debug, Clone)]
pub struct AgentStore {
    workplace: WorkplaceStore,
}

impl AgentStore {
    pub fn new(workplace: WorkplaceStore) -> Self {
        Self { workplace }
    }

    pub fn workplace(&self) -> &WorkplaceStore {
        &self.workplace
    }

    pub fn save_meta(&self, meta: &AgentMeta) -> Result<PathBuf> {
        self.workplace.ensure()?;
        let agent_dir = self.agent_dir(&meta.agent_id);
        fs::create_dir_all(&agent_dir)
            .with_context(|| format!("failed to create {}", agent_dir.display()))?;
        let path = agent_dir.join("meta.json");
        let payload =
            serde_json::to_string_pretty(meta).context("failed to serialize agent meta")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "storage.write",
            "saved agent metadata",
            serde_json::json!({
                "kind": "agent_meta",
                "agent_id": meta.agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn load_meta(&self, agent_id: &AgentId) -> Result<AgentMeta> {
        let path = self.meta_path(agent_id);
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let meta = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "storage.read",
            "loaded agent metadata",
            serde_json::json!({
                "kind": "agent_meta",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(meta)
    }

    pub fn save_state(&self, agent_id: &AgentId, state: &AgentState) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        fs::create_dir_all(&agent_dir)
            .with_context(|| format!("failed to create {}", agent_dir.display()))?;
        let path = agent_dir.join("state.json");
        let payload =
            serde_json::to_string_pretty(state).context("failed to serialize agent state")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "storage.write",
            "saved agent state",
            serde_json::json!({
                "kind": "agent_state",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn load_state(&self, agent_id: &AgentId) -> Result<AgentState> {
        let path = self.state_path(agent_id);
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let state = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "storage.read",
            "loaded agent state",
            serde_json::json!({
                "kind": "agent_state",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(state)
    }

    pub fn save_transcript(
        &self,
        agent_id: &AgentId,
        transcript: &AgentTranscript,
    ) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        fs::create_dir_all(&agent_dir)
            .with_context(|| format!("failed to create {}", agent_dir.display()))?;
        let path = agent_dir.join("transcript.json");
        let payload =
            serde_json::to_string_pretty(transcript).context("failed to serialize transcript")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "storage.write",
            "saved agent transcript",
            serde_json::json!({
                "kind": "agent_transcript",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn load_transcript(&self, agent_id: &AgentId) -> Result<AgentTranscript> {
        let path = self.transcript_path(agent_id);
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let transcript = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "storage.read",
            "loaded agent transcript",
            serde_json::json!({
                "kind": "agent_transcript",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(transcript)
    }

    pub fn save_messages(&self, agent_id: &AgentId, messages: &AgentMessages) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        fs::create_dir_all(&agent_dir)
            .with_context(|| format!("failed to create {}", agent_dir.display()))?;
        let path = agent_dir.join("messages.json");
        let payload =
            serde_json::to_string_pretty(messages).context("failed to serialize messages")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "storage.write",
            "saved agent messages",
            serde_json::json!({
                "kind": "agent_messages",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn save_memory(&self, agent_id: &AgentId, memory: &AgentMemory) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        fs::create_dir_all(&agent_dir)
            .with_context(|| format!("failed to create {}", agent_dir.display()))?;
        let path = agent_dir.join("memory.json");
        let payload = serde_json::to_string_pretty(memory).context("failed to serialize memory")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "storage.write",
            "saved agent memory",
            serde_json::json!({
                "kind": "agent_memory",
                "agent_id": agent_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn load_most_recent_meta(&self) -> Result<Option<AgentMeta>> {
        let mut metas = self.list_meta()?;
        metas.sort_by_key(meta_sort_key);
        Ok(metas.pop())
    }

    pub fn list_meta(&self) -> Result<Vec<AgentMeta>> {
        let agents_dir = self.workplace.agents_dir();
        if !agents_dir.exists() {
            return Ok(Vec::new());
        }

        let mut metas = Vec::new();
        for entry in fs::read_dir(&agents_dir)
            .with_context(|| format!("failed to read {}", agents_dir.display()))?
        {
            let entry = entry.context("failed to read agents directory entry")?;
            if !entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?
                .is_dir()
            {
                continue;
            }

            let meta_path = entry.path().join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            let payload = fs::read_to_string(&meta_path)
                .with_context(|| format!("failed to read {}", meta_path.display()))?;
            let meta: AgentMeta = serde_json::from_str(&payload)
                .with_context(|| format!("failed to parse {}", meta_path.display()))?;
            metas.push(meta);
        }

        metas.sort_by_key(meta_sort_key);
        Ok(metas)
    }

    /// List all agent IDs in the workplace
    pub fn list_agent_ids(&self) -> Result<Vec<AgentId>> {
        let metas = self.list_meta()?;
        Ok(metas.iter().map(|m| m.agent_id.clone()).collect())
    }

    pub fn next_agent_index(&self) -> Result<usize> {
        let agents_dir = self.workplace.agents_dir();
        if !agents_dir.exists() {
            return Ok(1);
        }

        let count = fs::read_dir(&agents_dir)
            .with_context(|| format!("failed to read {}", agents_dir.display()))?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_type().ok())
            .filter(|file_type| file_type.is_dir())
            .count();

        Ok(count + 1)
    }

    fn meta_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_dir(agent_id).join("meta.json")
    }

    pub fn agent_dir(&self, agent_id: &AgentId) -> PathBuf {
        self.workplace.agents_dir().join(agent_id.as_str())
    }

    fn state_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_dir(agent_id).join("state.json")
    }

    fn transcript_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_dir(agent_id).join("transcript.json")
    }
}

fn meta_sort_key(meta: &AgentMeta) -> (DateTime<FixedOffset>, String) {
    let updated_at = DateTime::parse_from_rfc3339(&meta.updated_at).unwrap_or_else(|_| {
        DateTime::parse_from_rfc3339(&meta.created_at)
            .unwrap_or_else(|_| DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z").unwrap())
    });
    (updated_at, meta.agent_id.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::AgentStore;
    use crate::agent_runtime::AgentCodename;
    use crate::agent_runtime::AgentId;
    use crate::agent_runtime::AgentMeta;
    use crate::agent_runtime::AgentStatus;
    use crate::agent_runtime::ProviderType;
    use crate::agent_runtime::WorkplaceId;
    use crate::agent_state::AgentState;
    use crate::agent_transcript::AgentTranscript;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    fn meta(id: &str, updated_at: &str) -> AgentMeta {
        AgentMeta {
            agent_id: AgentId::new(id),
            codename: AgentCodename::new("alpha"),
            workplace_id: WorkplaceId::new("wp_test"),
            provider_type: ProviderType::Claude,
            provider_session_id: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            status: AgentStatus::Idle,
        }
    }

    #[test]
    fn saves_and_loads_meta() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        let meta = meta("agent_001", "2026-04-12T00:00:00Z");

        store.save_meta(&meta).expect("save meta");
        let loaded = store.load_meta(&meta.agent_id).expect("load meta");

        assert_eq!(loaded.agent_id.as_str(), "agent_001");
        assert_eq!(loaded.provider_type, ProviderType::Claude);
    }

    #[test]
    fn loads_most_recent_meta() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        let older = meta("agent_001", "2026-04-12T00:00:00Z");
        let newer = meta("agent_002", "2026-04-12T01:00:00Z");

        store.save_meta(&older).expect("save older");
        store.save_meta(&newer).expect("save newer");

        let loaded = store
            .load_most_recent_meta()
            .expect("load most recent")
            .expect("meta");

        assert_eq!(loaded.agent_id.as_str(), "agent_002");
    }

    #[test]
    fn next_agent_index_uses_existing_directories() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        store
            .save_meta(&meta("agent_001", "2026-04-12T00:00:00Z"))
            .expect("save");

        assert_eq!(store.next_agent_index().expect("index"), 2);
    }

    #[test]
    fn saves_and_loads_state() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        let state = AgentState {
            cwd: ".".to_string(),
            draft_input: "draft".to_string(),
            enabled_skill_names: vec!["reviewer".to_string()],
            active_task_id: Some("task-1".to_string()),
            active_task_had_error: false,
            continuation_attempts: 1,
            loop_phase: crate::app::LoopPhase::Executing,
            was_interrupted: false,
        };

        store
            .save_state(&AgentId::new("agent_001"), &state)
            .expect("save state");
        let loaded = store
            .load_state(&AgentId::new("agent_001"))
            .expect("load state");

        assert_eq!(loaded.draft_input, "draft");
        assert_eq!(loaded.active_task_id.as_deref(), Some("task-1"));
        assert!(!loaded.was_interrupted);
    }

    #[test]
    fn saves_and_loads_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        let transcript = AgentTranscript {
            entries: Vec::new(),
        };

        store
            .save_transcript(&AgentId::new("agent_001"), &transcript)
            .expect("save transcript");
        let loaded = store
            .load_transcript(&AgentId::new("agent_001"))
            .expect("load transcript");

        assert_eq!(loaded.entries.len(), 0);
    }

    #[test]
    fn list_agent_ids_returns_all_saved_agents() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let store = AgentStore::new(workplace);
        store
            .save_meta(&meta("agent_001", "2026-04-12T00:00:00Z"))
            .expect("save 1");
        store
            .save_meta(&meta("agent_002", "2026-04-12T01:00:00Z"))
            .expect("save 2");

        let ids = store.list_agent_ids().expect("list agent ids");

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&AgentId::new("agent_001")));
        assert!(ids.contains(&AgentId::new("agent_002")));
    }
}
