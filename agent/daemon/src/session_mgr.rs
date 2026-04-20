//! SessionManager — owns runtime state previously held by `TuiState`.
//!
//! Encapsulates `AppState`, `AgentPool`, `EventAggregator`, and `Mailbox`.
//! Snapshot generation maps core types into protocol wire types.

use agent_core::agent_mail::AgentMailbox;
use agent_core::agent_pool::AgentPool;
use agent_core::agent_slot::{AgentSlot, AgentSlotStatus as CoreAgentStatus};
use agent_core::app::{AppStatus, TranscriptEntry};
use agent_core::event_aggregator::EventAggregator;
use agent_core::runtime_session::RuntimeSession;
use agent_protocol::state::*;
use agent_types::WorkplaceId;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Owns all daemon-side runtime state.
pub struct SessionManager {
    inner: Arc<Mutex<SessionInner>>,
    session_id: String,
    workplace_id: WorkplaceId,
}

struct SessionInner {
    session: RuntimeSession,
    agent_pool: AgentPool,
    #[allow(dead_code)]
    event_aggregator: EventAggregator,
    #[allow(dead_code)]
    mailbox: AgentMailbox,
}

impl SessionManager {
    /// Bootstrap a new session for the given workplace.
    ///
    /// This performs filesystem I/O and should be called once during daemon startup.
    pub async fn bootstrap(cwd: PathBuf, workplace_id: WorkplaceId) -> Result<Self> {
        let default_provider = agent_core::default_provider();
        let session = RuntimeSession::bootstrap(cwd, default_provider, true)
            .context("bootstrap runtime session")?;

        let agent_pool = AgentPool::with_cwd(
            workplace_id.clone(),
            4,
            session.app.cwd.clone(),
        );

        let event_aggregator = EventAggregator::new();
        let mailbox = AgentMailbox::new();

        let session_id = format!("sess-{}", uuid::Uuid::new_v4());
        tracing::info!(session_id = %session_id, workplace = %workplace_id.as_str(), "SessionManager bootstrapped");

        Ok(Self {
            inner: Arc::new(Mutex::new(SessionInner {
                session,
                agent_pool,
                event_aggregator,
                mailbox,
            })),
            session_id,
            workplace_id,
        })
    }

    /// Generate a live [`SessionState`] snapshot from current runtime data.
    pub async fn snapshot(&self) -> Result<SessionState> {
        let inner = self.inner.lock().await;

        let app = &inner.session.app;
        let pool = &inner.agent_pool;

        let transcript: Vec<TranscriptItem> = app
            .transcript
            .iter()
            .enumerate()
            .map(|(idx, entry)| map_transcript_entry(idx, entry))
            .collect();

        let agents: Vec<AgentSnapshot> = pool
            .slots()
            .iter()
            .map(map_agent_slot)
            .collect();

        let workplace = WorkplaceSnapshot {
            id: self.workplace_id.as_str().to_string(),
            path: app.cwd.display().to_string(),
            backlog: BacklogSnapshot { items: vec![] },
            skills: vec![],
        };

        let status = match app.status {
            AppStatus::Idle => SessionStatus::Idle,
            AppStatus::Responding => SessionStatus::Running,
        };

        let state = SessionState {
            session_id: self.session_id.clone(),
            alias: "default".to_string(),
            server_time: chrono::Utc::now().to_rfc3339(),
            last_event_seq: 0,
            app_state: AppStateSnapshot {
                transcript,
                input: InputState {
                    text: app.input.clone(),
                    multiline: false,
                },
                status,
            },
            agents,
            workplace,
            focused_agent_id: None,
            protocol_version: agent_protocol::PROTOCOL_VERSION.to_string(),
            capabilities: vec![
                "session.initialize".to_string(),
                "session.heartbeat".to_string(),
                "session.health".to_string(),
                "agent.spawn".to_string(),
                "agent.stop".to_string(),
                "agent.list".to_string(),
            ],
        };

        Ok(state)
    }

    /// Spawn a new agent in the pool.
    pub async fn spawn_agent(&self, provider: agent_types::ProviderKind) -> Result<AgentSnapshot> {
        let mut inner = self.inner.lock().await;
        let agent_id = inner
            .agent_pool
            .spawn_agent(provider)
            .map_err(|e| anyhow::anyhow!("spawn failed: {e}"))?;

        // Find the newly created slot.
        let slot = inner
            .agent_pool
            .slots()
            .iter()
            .find(|s| s.agent_id().as_str() == agent_id.as_str())
            .context("spawned agent not found in pool")?;

        Ok(map_agent_slot(slot))
    }

    /// Stop an agent by ID.
    pub async fn stop_agent(&self, agent_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        inner
            .agent_pool
            .stop_agent(&id)
            .map_err(|e| anyhow::anyhow!("stop failed: {e}"))?;
        Ok(())
    }

    /// List agents in the pool.
    pub async fn list_agents(&self, include_stopped: bool) -> Vec<AgentSnapshot> {
        let inner = self.inner.lock().await;
        inner
            .agent_pool
            .slots()
            .iter()
            .filter(|s| include_stopped || !matches!(s.status(), CoreAgentStatus::Stopped { .. }))
            .map(map_agent_slot)
            .collect()
    }

    /// Write a snapshot file to disk for session restore on next startup.
    pub async fn write_snapshot(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let snapshot = self.snapshot().await?;
        let mut file = SnapshotFile {
            version: 1,
            session_id: snapshot.session_id.clone(),
            written_at: chrono::Utc::now().to_rfc3339(),
            last_event_seq: snapshot.last_event_seq,
            state: snapshot,
            checksum: None,
        };
        // Compute checksum of the JSON *without* the checksum field.
        let json = serde_json::to_string_pretty(&file).context("serialize snapshot file")?;
        let checksum = format!("{:08x}", crc32fast::hash(json.as_bytes()));
        file.checksum = Some(checksum);
        let json = serde_json::to_string_pretty(&file).context("serialize snapshot file")?;
        tokio::fs::write(path.as_ref(), json)
            .await
            .with_context(|| format!("write snapshot {}", path.as_ref().display()))?;
        Ok(())
    }

    /// Read a snapshot file and verify its checksum. Returns an empty state on mismatch or parse error.
    pub async fn read_snapshot(path: impl AsRef<std::path::Path>) -> Result<SnapshotFile> {
        let bytes = tokio::fs::read(path.as_ref()).await?;
        let file: SnapshotFile = match serde_json::from_slice(&bytes) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Snapshot parse error: {}. Falling back to empty state.", e);
                return Ok(SnapshotFile {
                    version: 1,
                    session_id: String::new(),
                    written_at: String::new(),
                    last_event_seq: 0,
                    state: SessionState::default(),
                    checksum: None,
                });
            }
        };

        if let Some(ref expected) = file.checksum {
            let mut file_without_checksum = file.clone();
            file_without_checksum.checksum = None;
            let json = serde_json::to_string_pretty(&file_without_checksum)
                .context("re-serialize snapshot for checksum")?;
            let actual = format!("{:08x}", crc32fast::hash(json.as_bytes()));
            if actual != *expected {
                tracing::warn!(
                    "Snapshot checksum mismatch: expected {}, got {}. Falling back to empty state.",
                    expected,
                    actual
                );
                return Ok(SnapshotFile {
                    version: 1,
                    session_id: String::new(),
                    written_at: String::new(),
                    last_event_seq: 0,
                    state: SessionState::default(),
                    checksum: None,
                });
            }
        }
        Ok(file)
    }

    /// Return a debug dump of internal session state.
    pub async fn debug_dump(&self) -> Result<serde_json::Value> {
        let inner = self.inner.lock().await;
        let agents: Vec<String> = inner
            .agent_pool
            .slots()
            .iter()
            .map(|s| s.agent_id().as_str().to_string())
            .collect();
        let event_queue_depth = inner.event_aggregator.receiver_count();
        Ok(serde_json::json!({
            "session_id": self.session_id,
            "workplace_id": self.workplace_id.as_str(),
            "agent_count": agents.len(),
            "agents": agents,
            "event_queue_depth": event_queue_depth,
        }))
    }
}

/// On-disk snapshot format for graceful shutdown persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SnapshotFile {
    pub version: u32,
    pub session_id: String,
    pub written_at: String,
    pub last_event_seq: u64,
    pub state: SessionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

// ---------------------------------------------------------------------------
// Mappers: core → protocol
// ---------------------------------------------------------------------------

fn map_transcript_entry(seq: usize, entry: &TranscriptEntry) -> TranscriptItem {
    let (kind, content, agent_id, metadata) = match entry {
        TranscriptEntry::User(text) => (
            ItemKind::UserInput,
            text.clone(),
            None,
            serde_json::Value::Null,
        ),
        TranscriptEntry::Assistant(text) => (
            ItemKind::AssistantOutput,
            text.clone(),
            None,
            serde_json::Value::Null,
        ),
        TranscriptEntry::Thinking(text) => (
            ItemKind::SystemMessage,
            text.clone(),
            None,
            serde_json::json!({ "type": "thinking" }),
        ),
        TranscriptEntry::Decision {
            agent_id,
            situation_type,
            action_type,
            reasoning,
            confidence,
            tier,
        } => (
            ItemKind::SystemMessage,
            reasoning.clone(),
            Some(agent_id.clone()),
            serde_json::json!({
                "type": "decision",
                "situation": situation_type,
                "action": action_type,
                "confidence": confidence,
                "tier": tier,
            }),
        ),
        TranscriptEntry::ExecCommand {
            call_id,
            source,
            allow_exploring_group,
            input_preview,
            output_preview,
            status,
            exit_code,
            duration_ms,
        } => (
            ItemKind::ToolCall,
            input_preview.clone().unwrap_or_default(),
            None,
            serde_json::json!({
                "type": "exec_command",
                "call_id": call_id,
                "source": source,
                "allow_exploring_group": allow_exploring_group,
                "output_preview": output_preview,
                "status": format!("{:?}", status),
                "exit_code": exit_code,
                "duration_ms": duration_ms,
            }),
        ),
        TranscriptEntry::PatchApply {
            call_id,
            changes,
            status,
            output_preview,
        } => (
            ItemKind::ToolResult,
            format!("{} changes", changes.len()),
            None,
            serde_json::json!({
                "type": "patch_apply",
                "call_id": call_id,
                "status": format!("{:?}", status),
                "output_preview": output_preview,
            }),
        ),
        TranscriptEntry::WebSearch {
            call_id,
            query,
            action,
            started,
        } => (
            ItemKind::ToolCall,
            query.clone(),
            None,
            serde_json::json!({
                "type": "web_search",
                "call_id": call_id,
                "action": action.as_ref().map(|a| format!("{:?}", a)),
                "started": started,
            }),
        ),
        TranscriptEntry::ViewImage { call_id, path } => (
            ItemKind::ToolResult,
            path.clone(),
            None,
            serde_json::json!({
                "type": "view_image",
                "call_id": call_id,
            }),
        ),
        TranscriptEntry::ImageGeneration {
            call_id,
            revised_prompt,
            result,
            saved_path,
        } => (
            ItemKind::ToolResult,
            revised_prompt.clone().unwrap_or_default(),
            None,
            serde_json::json!({
                "type": "image_generation",
                "call_id": call_id,
                "result": result,
                "saved_path": saved_path,
            }),
        ),
        TranscriptEntry::McpToolCall {
            call_id,
            invocation,
            result_blocks,
            error,
            status,
            is_error,
        } => (
            ItemKind::ToolCall,
            format!("{:?}", invocation),
            None,
            serde_json::json!({
                "type": "mcp_tool_call",
                "call_id": call_id,
                "result_blocks": result_blocks,
                "error": error,
                "status": format!("{:?}", status),
                "is_error": is_error,
            }),
        ),
        TranscriptEntry::GenericToolCall {
            name,
            call_id,
            input_preview,
            output_preview,
            success,
            started,
            exit_code,
            duration_ms,
        } => (
            ItemKind::ToolCall,
            input_preview.clone().unwrap_or_default(),
            None,
            serde_json::json!({
                "type": "generic_tool_call",
                "name": name,
                "call_id": call_id,
                "output_preview": output_preview,
                "success": success,
                "started": started,
                "exit_code": exit_code,
                "duration_ms": duration_ms,
            }),
        ),
        TranscriptEntry::Status(text) => (
            ItemKind::SystemMessage,
            text.clone(),
            None,
            serde_json::json!({ "type": "status" }),
        ),
        TranscriptEntry::Error(text) => (
            ItemKind::SystemMessage,
            text.clone(),
            None,
            serde_json::json!({ "type": "error" }),
        ),
    };

    TranscriptItem {
        id: format!("item-{}", seq),
        kind,
        agent_id,
        content,
        metadata,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    }
}

fn map_agent_slot(slot: &AgentSlot) -> AgentSnapshot {
    let status = match slot.status() {
        CoreAgentStatus::Idle => AgentSlotStatus::Idle,
        CoreAgentStatus::Starting
        | CoreAgentStatus::Responding { .. }
        | CoreAgentStatus::ToolExecuting { .. }
        | CoreAgentStatus::Finishing
        | CoreAgentStatus::Stopping
        | CoreAgentStatus::Blocked { .. }
        | CoreAgentStatus::BlockedForDecision { .. }
        | CoreAgentStatus::Paused { .. }
        | CoreAgentStatus::WaitingForInput { .. }
        | CoreAgentStatus::Resting { .. } => AgentSlotStatus::Running,
        CoreAgentStatus::Stopped { .. } => AgentSlotStatus::Stopped,
        CoreAgentStatus::Error { .. } => AgentSlotStatus::Error,
    };

    AgentSnapshot {
        id: slot.agent_id().as_str().to_string(),
        codename: slot.codename().as_str().to_string(),
        role: format!("{:?}", slot.role()),
        provider: format!("{:?}", slot.provider_type()),
        status,
        current_task_id: None,
        uptime_seconds: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_manager_bootstrap_produces_valid_state() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-test");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp.clone())
            .await
            .unwrap();

        let snap = mgr.snapshot().await.unwrap();
        assert_eq!(snap.workplace.id, wp.as_str());
        assert_eq!(snap.protocol_version, agent_protocol::PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn session_manager_concurrent_reads_do_not_deadlock() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-concurrent");
        let mgr = Arc::new(
            SessionManager::bootstrap(tmp.path().to_path_buf(), wp)
                .await
                .unwrap(),
        );

        let mut handles = vec![];
        for _ in 0..10 {
            let m = mgr.clone();
            handles.push(tokio::spawn(async move { m.snapshot().await }));
        }

        for h in handles {
            h.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn snapshot_file_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-snap");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp)
            .await
            .unwrap();

        let path = tmp.path().join("snapshot.json");
        mgr.write_snapshot(&path).await.unwrap();

        let bytes = tokio::fs::read(&path).await.unwrap();
        let file: SnapshotFile = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(file.version, 1);
        assert_eq!(file.state.protocol_version, agent_protocol::PROTOCOL_VERSION);
    }

    #[test]
    fn map_transcript_user_entry() {
        let entry = TranscriptEntry::User("hello".into());
        let item = map_transcript_entry(0, &entry);
        assert_eq!(item.kind, ItemKind::UserInput);
        assert_eq!(item.content, "hello");
    }

    #[test]
    fn map_transcript_assistant_entry() {
        let entry = TranscriptEntry::Assistant("world".into());
        let item = map_transcript_entry(1, &entry);
        assert_eq!(item.kind, ItemKind::AssistantOutput);
        assert_eq!(item.content, "world");
    }

    #[test]
    fn map_agent_slot_idle() {
        use agent_core::agent_slot::AgentSlot;
        use agent_core::agent_runtime::ProviderType;
        use agent_types::{AgentCodename, AgentId};
        let slot = AgentSlot::new(
            AgentId::new("a1"),
            AgentCodename::new("alpha"),
            ProviderType::Claude,
        );
        let snap = map_agent_slot(&slot);
        assert_eq!(snap.id, "a1");
        assert_eq!(snap.codename, "alpha");
        assert_eq!(snap.status, AgentSlotStatus::Idle);
    }

    #[tokio::test]
    async fn snapshot_checksum_validates() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-checksum");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp)
            .await
            .unwrap();

        let path = tmp.path().join("snapshot.json");
        mgr.write_snapshot(&path).await.unwrap();

        let file = SessionManager::read_snapshot(&path).await.unwrap();
        assert_eq!(file.version, 1);
        assert!(file.checksum.is_some());
    }

    #[tokio::test]
    async fn snapshot_checksum_mismatch_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-corrupt");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp)
            .await
            .unwrap();

        let path = tmp.path().join("snapshot.json");
        mgr.write_snapshot(&path).await.unwrap();

        // Corrupt the file by appending garbage.
        let mut bytes = tokio::fs::read(&path).await.unwrap();
        bytes.extend_from_slice(b"\n{\"garbage\": true}");
        tokio::fs::write(&path, &bytes).await.unwrap();

        let file = SessionManager::read_snapshot(&path).await.unwrap();
        assert!(file.checksum.is_none());
        assert!(file.session_id.is_empty());
    }

    #[tokio::test]
    async fn debug_dump_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-dump");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp.clone())
            .await
            .unwrap();

        let dump = mgr.debug_dump().await.unwrap();
        assert_eq!(dump["workplace_id"], wp.as_str());
        assert!(dump.get("agent_count").is_some());
        assert!(dump.get("event_queue_depth").is_some());
    }
}
