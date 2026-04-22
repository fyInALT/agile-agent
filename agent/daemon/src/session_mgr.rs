//! SessionManager — owns runtime state previously held by `TuiState`.
//!
//! Encapsulates `AppState`, `AgentPool`, `EventAggregator`, and `Mailbox`.
//! Snapshot generation maps core types into protocol wire types.

use agent_core::agent_mail::AgentMailbox;
use agent_core::agent_pool::AgentPool;
use agent_core::agent_runtime::{AgentId as CoreAgentId, AgentMeta, AgentStatus, ProviderType};
use agent_core::agent_slot::{AgentSlot, AgentSlotStatus as CoreAgentStatus};
use agent_core::app::{AppStatus, TranscriptEntry};
use agent_core::event_aggregator::EventAggregator;
use agent_core::runtime_session::RuntimeSession;
use agent_core::shutdown_snapshot::{AgentShutdownSnapshot, ShutdownSnapshot, ShutdownReason};
use agent_protocol::events::Event;
use agent_protocol::methods::SendInputResult;
use agent_protocol::state::*;
use agent_types::WorkplaceId;
use anyhow::{Context, Result};
use sha2::Digest;
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
    event_aggregator: EventAggregator,
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

        // Persist the updated meta so that future restore reads accurate status.
        if let Some(slot) = inner.agent_pool.get_slot_by_id(&id) {
            let meta = AgentMeta {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                workplace_id: self.workplace_id.clone(),
                provider_type: slot.provider_type(),
                provider_session_id: slot.session_handle().map(|h| match h {
                    agent_core::SessionHandle::ClaudeSession { session_id } => {
                        agent_core::agent_runtime::ProviderSessionId::new(session_id.clone())
                    }
                    agent_core::SessionHandle::CodexThread { thread_id } => {
                        agent_core::agent_runtime::ProviderSessionId::new(thread_id.clone())
                    }
                }),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                status: match slot.status() {
                    CoreAgentStatus::Stopped { .. } => AgentStatus::Stopped,
                    _ if slot.status().is_active() => AgentStatus::Running,
                    _ => AgentStatus::Idle,
                },
                role: slot.role(),
            };
            let store = agent_core::agent_store::AgentStore::new(
                inner.session.agent_runtime.workplace().clone(),
            );
            let _ = store.save_meta(&meta);
        }

        Ok(())
    }

    /// Send user input to an agent, starting a provider thread.
    pub async fn send_input(&self, agent_id: &str, text: &str) -> Result<SendInputResult> {
        let mut inner = self.inner.lock().await;
        let id = CoreAgentId::new(agent_id);
        Self::start_provider_for_agent_inner(
            &mut inner,
            &id,
            text,
            true, // record user prompt in transcript
        )
        .await
    }

    /// Internal helper to start a provider thread for an agent.
    ///
    /// `record_user_prompt` controls whether a `TranscriptEntry::User` is
    /// appended. Set to `false` when the prompt was already recorded by the
    /// caller (e.g. decision action executor).
    async fn start_provider_for_agent_inner(
        inner: &mut tokio::sync::MutexGuard<'_, SessionInner>,
        agent_id: &CoreAgentId,
        text: &str,
        record_user_prompt: bool,
    ) -> Result<SendInputResult> {
        let slot = inner
            .agent_pool
            .get_slot_by_id(agent_id)
            .context("agent not found")?;

        let provider_kind = slot
            .provider_type()
            .to_provider_kind()
            .unwrap_or(agent_types::ProviderKind::Mock);
        let session_handle = slot.session_handle().cloned();
        let cwd = slot.cwd();

        if slot.has_provider_thread() {
            anyhow::bail!("agent {} is already busy", agent_id.as_str());
        }

        let thread_name = format!("agent-{}-provider", agent_id.as_str());
        let handle = agent_core::start_provider_with_handle(
            provider_kind,
            text.to_string(),
            cwd.clone(),
            session_handle,
            thread_name,
        )
        .context("failed to start provider thread")?;

        let (event_rx, join_handle) = handle.into_parts();

        let slot = inner
            .agent_pool
            .get_slot_mut_by_id(agent_id)
            .context("agent not found")?;
        if record_user_prompt {
            slot.append_transcript(TranscriptEntry::User(text.to_string()));
        }
        if let Some(jh) = join_handle {
            slot.set_thread_handle(jh);
        }
        let current_status = slot.status().clone();
        if current_status.is_idle() {
            let _ = slot.transition_to(CoreAgentStatus::starting());
            let _ = slot.transition_to(CoreAgentStatus::responding_now());
        } else if current_status.is_active()
            || current_status.is_blocked()
            || current_status.is_waiting_for_input()
        {
            let _ = slot.transition_to(CoreAgentStatus::responding_now());
        }

        inner.event_aggregator.add_receiver(agent_id.clone(), event_rx);

        Ok(SendInputResult {
            accepted: true,
            item_id: format!("item-{}", uuid::Uuid::new_v4()),
        })
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
        let checksum = format!("{:x}", sha2::Sha256::digest(json.as_bytes()));
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
                return Ok(SnapshotFile::empty_fallback());
            }
        };

        // Require checksum for all files (security: reject files without checksum)
        match &file.checksum {
            Some(expected) => {
                let mut file_without_checksum = file.clone();
                file_without_checksum.checksum = None;
                let json = serde_json::to_string_pretty(&file_without_checksum)
                    .context("re-serialize snapshot for checksum")?;
                let actual = format!("{:x}", sha2::Sha256::digest(json.as_bytes()));
                if actual != *expected {
                    tracing::warn!(
                        "Snapshot checksum mismatch: expected {}, got {}. Falling back to empty state.",
                        expected,
                        actual
                    );
                    return Ok(SnapshotFile::empty_fallback());
                }
                Ok(file)
            }
            None => {
                tracing::warn!("Snapshot missing checksum field. Rejecting for security.");
                Ok(SnapshotFile::empty_fallback())
            }
        }
    }

    /// Return a debug dump of internal session state.
    /// Load a paginated slice of transcript history.
    pub async fn load_history(&self, offset: usize, limit: usize) -> Vec<TranscriptItem> {
        let inner = self.inner.lock().await;
        let app = &inner.session.app;
        app.transcript
            .iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(idx, entry)| {
                let (kind, content) = match entry {
                    agent_core::app::TranscriptEntry::User(t) => (ItemKind::UserInput, t.clone()),
                    agent_core::app::TranscriptEntry::Assistant(t) => (ItemKind::AssistantOutput, t.clone()),
                    agent_core::app::TranscriptEntry::Thinking(t) => (ItemKind::SystemMessage, t.clone()),
                    agent_core::app::TranscriptEntry::Decision { reasoning, .. } => (ItemKind::SystemMessage, reasoning.clone()),
                    agent_core::app::TranscriptEntry::ExecCommand { input_preview, .. } => (ItemKind::ToolCall, input_preview.clone().unwrap_or_default()),
                    agent_core::app::TranscriptEntry::PatchApply { output_preview, .. } => (ItemKind::ToolResult, output_preview.clone().unwrap_or_default()),
                    agent_core::app::TranscriptEntry::WebSearch { query, .. } => (ItemKind::ToolCall, query.clone()),
                    agent_core::app::TranscriptEntry::ViewImage { path, .. } => (ItemKind::ToolResult, path.clone()),
                    agent_core::app::TranscriptEntry::ImageGeneration { revised_prompt, .. } => (ItemKind::ToolResult, revised_prompt.clone().unwrap_or_default()),
                    agent_core::app::TranscriptEntry::McpToolCall { invocation, .. } => (ItemKind::ToolCall, format!("{:?}", invocation)),
                    agent_core::app::TranscriptEntry::GenericToolCall { name, .. } => (ItemKind::ToolCall, name.clone()),
                    agent_core::app::TranscriptEntry::Status(t) => (ItemKind::SystemMessage, t.clone()),
                    agent_core::app::TranscriptEntry::Error(t) => (ItemKind::SystemMessage, t.clone()),
                };
                TranscriptItem {
                    id: format!("item-{}", offset + idx),
                    kind,
                    agent_id: None,
                    content,
                    metadata: serde_json::Value::Null,
                    // TranscriptEntry does not store original timestamps; use a stable placeholder.
                    created_at: "1970-01-01T00:00:00+00:00".to_string(),
                    completed_at: None,
                }
            })
            .collect()
    }

    /// Trigger an immediate snapshot write to disk.
    pub async fn force_snapshot(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        self.write_snapshot(path).await
    }

    /// List active connections (placeholder — actual connections tracked externally).
    pub async fn list_connections(&self) -> Vec<serde_json::Value> {
        vec![]
    }

    /// Save a shutdown snapshot in agent-core format so that `RuntimeSession::bootstrap`
    /// can resume from it on the next startup.
    pub async fn save_shutdown_snapshot(&self, reason: ShutdownReason) -> Result<PathBuf> {
        let inner = self.inner.lock().await;
        let cwd = inner.session.app.cwd.clone();
        let store = agent_core::agent_store::AgentStore::new(
            inner.session.agent_runtime.workplace().clone(),
        );

        let agents: Vec<AgentShutdownSnapshot> = inner
            .agent_pool
            .slots()
            .iter()
            .map(|slot| {
                // Preserve original created_at from persisted meta if available.
                let created_at = store
                    .load_meta(slot.agent_id())
                    .map(|m| m.created_at)
                    .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

                let meta = AgentMeta {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    workplace_id: self.workplace_id.clone(),
                    provider_type: slot.provider_type(),
                    provider_session_id: slot.session_handle().map(|h| match h {
                        agent_core::SessionHandle::ClaudeSession { session_id } => {
                            agent_core::agent_runtime::ProviderSessionId::new(session_id.clone())
                        }
                        agent_core::SessionHandle::CodexThread { thread_id } => {
                            agent_core::agent_runtime::ProviderSessionId::new(thread_id.clone())
                        }
                    }),
                    created_at,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    status: match slot.status() {
                        CoreAgentStatus::Stopped { .. } => AgentStatus::Stopped,
                        _ if slot.status().is_active() => AgentStatus::Running,
                        _ => AgentStatus::Idle,
                    },
                    role: slot.role(),
                };
                // Also persist per-agent transcript to disk so it survives even if
                // shutdown snapshot is deleted or corrupted.
                let _ = store.save_transcript(
                    &meta.agent_id,
                    &agent_core::agent_transcript::AgentTranscript::from_entries(
                        slot.transcript().to_vec(),
                    ),
                );
                AgentShutdownSnapshot {
                    meta,
                    assigned_task_id: slot.assigned_task_id().map(|t| t.as_str().to_string()),
                    was_active: slot.status().is_active(),
                    had_error: slot.status().is_blocked(),
                    provider_thread_state: None,
                    captured_at: chrono::Utc::now().to_rfc3339(),
                    transcript: slot.transcript().to_vec(),
                    worktree_path: slot.worktree_path().cloned(),
                    worktree_branch: slot.worktree_branch().cloned(),
                    worktree_id: slot.worktree_id().cloned(),
                }
            })
            .collect();

        let snapshot = ShutdownSnapshot::new(
            self.workplace_id.as_str().to_string(),
            agents,
            inner.session.workplace.backlog.clone(),
            inner.mailbox.pending_mail_for_snapshot(),
            reason,
        );

        let workplace = agent_core::workplace_store::WorkplaceStore::for_cwd(&cwd)
            .context("resolve workplace for snapshot")?;
        workplace.save_shutdown_snapshot(&snapshot)
    }

    /// Restore session from an existing shutdown snapshot.
    ///
    /// This rebuilds the `RuntimeSession` via `RuntimeSession::bootstrap` (which
    /// automatically loads the snapshot when `resume_snapshot=true`) and then
    /// reconstructs the `AgentPool` to match the saved agents, preserving their
    /// original IDs, roles, and transcripts.
    pub async fn restore_from_shutdown_snapshot(
        cwd: PathBuf,
        workplace_id: WorkplaceId,
        default_provider: agent_types::ProviderKind,
        max_agents: usize,
    ) -> Result<Self> {
        // Load shutdown snapshot BEFORE RuntimeSession consumes it.
        let workplace = agent_core::workplace_store::WorkplaceStore::for_cwd(&cwd)?;
        let snapshot = match workplace.load_shutdown_snapshot()? {
            Some(s) => s,
            None => return Self::bootstrap(cwd, workplace_id).await,
        };

        if snapshot.format_version != 1 {
            tracing::warn!(
                format_version = snapshot.format_version,
                "shutdown snapshot format version mismatch; attempting best-effort restore"
            );
        }

        // RuntimeSession::bootstrap with resume_snapshot=true will automatically
        // load shutdown_snapshot.json from the workplace directory.
        let session = RuntimeSession::bootstrap(cwd.clone(), default_provider, true)
            .context("bootstrap runtime session from snapshot")?;

        // Rebuild agent pool from the snapshot data we loaded above.
        // This preserves original agent IDs via AgentPool::restore_slot.
        let mut agent_pool = AgentPool::with_cwd(workplace_id.clone(), max_agents, cwd.clone());

        let store = agent_core::agent_store::AgentStore::new(
            session.agent_runtime.workplace().clone(),
        );

        for agent_shutdown in &snapshot.agents {
            let meta = &agent_shutdown.meta;
            let session_handle = meta.provider_session_id.as_ref().map(|sid| {
                match meta.provider_type {
                    ProviderType::Claude => agent_core::SessionHandle::ClaudeSession {
                        session_id: sid.as_str().to_string(),
                    },
                    ProviderType::Codex => agent_core::SessionHandle::CodexThread {
                        thread_id: sid.as_str().to_string(),
                    },
                    _ => agent_core::SessionHandle::ClaudeSession {
                        session_id: sid.as_str().to_string(),
                    },
                }
            });
            let status = match meta.status {
                AgentStatus::Stopped => CoreAgentStatus::stopped("restored from snapshot"),
                _ => CoreAgentStatus::idle(),
            };
            let assigned_task_id = agent_shutdown
                .assigned_task_id
                .as_ref()
                .map(|t| agent_types::TaskId::new(t));

            // Use snapshot transcript if present; otherwise fall back to per-agent
            // transcript.json on disk (written by save_shutdown_snapshot).
            let transcript = if agent_shutdown.transcript.is_empty() {
                store
                    .load_transcript(&meta.agent_id)
                    .map(|t| t.entries)
                    .unwrap_or_default()
            } else {
                agent_shutdown.transcript.clone()
            };

            let slot = agent_core::agent_slot::AgentSlot::restored_with_worktree(
                meta.agent_id.clone(),
                meta.codename.clone(),
                meta.provider_type,
                meta.role,
                status,
                session_handle,
                transcript,
                assigned_task_id,
                agent_shutdown.worktree_path.clone(),
                agent_shutdown.worktree_branch.clone(),
                agent_shutdown.worktree_id.clone(),
            );
            if let Err(e) = agent_pool.restore_slot(slot) {
                tracing::warn!(
                    agent_id = %meta.agent_id.as_str(),
                    error = %e,
                    "failed to restore agent slot from shutdown snapshot"
                );
            }
        }

        let event_aggregator = EventAggregator::new();
        let mut mailbox = AgentMailbox::new();
        mailbox.restore_pending(&snapshot.pending_mail);

        Ok(Self {
            inner: Arc::new(Mutex::new(SessionInner {
                session,
                agent_pool,
                event_aggregator,
                mailbox,
            })),
            session_id: format!("sess-{}", uuid::Uuid::new_v4()),
            workplace_id,
        })
    }

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

    /// Process one tick of the daemon event loop.
    ///
    /// Polls provider events from the event aggregator, updates slot state,
    /// triggers the decision layer when appropriate, and returns protocol
    /// events that should be broadcast to connected clients.
    pub async fn tick(&self) -> Result<Vec<Event>> {
        let mut inner = self.inner.lock().await;
        let mut pump = crate::event_pump::EventPump::new();
        let mut broadcast_events = Vec::new();

        // ------------------------------------------------------------------
        // 1. Poll provider events
        // ------------------------------------------------------------------
        let poll_result = inner.event_aggregator.poll_all();

        for agent_event in poll_result.events {
            match agent_event {
                agent_core::event_aggregator::AgentEvent::FromProvider {
                    agent_id,
                    event,
                } => {
                    // Update activity timestamp.
                    if let Some(slot) = inner.agent_pool.get_slot_mut_by_id(&agent_id) {
                        slot.touch_activity();
                        if slot.status().is_waiting_for_input() {
                            let _ = slot
                                .transition_to(CoreAgentStatus::responding_now());
                        }
                    }

                    // Update slot state based on event type.
                    match &event {
                        agent_core::ProviderEvent::Finished => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                if slot.status().is_active() {
                                    let _ = slot
                                        .transition_to(CoreAgentStatus::idle());
                                }
                                slot.clear_provider_thread();
                            }
                            inner.event_aggregator.remove_receiver(&agent_id);
                        }
                        agent_core::ProviderEvent::Error(error) => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                slot.append_transcript(TranscriptEntry::Error(
                                    error.clone(),
                                ));
                                let _ = slot.transition_to(
                                    CoreAgentStatus::blocked(error.clone()),
                                );
                                slot.clear_provider_thread();
                            }
                            inner.event_aggregator.remove_receiver(&agent_id);
                        }
                        agent_core::ProviderEvent::AssistantChunk(chunk) => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                slot.append_transcript(
                                    TranscriptEntry::Assistant(chunk.clone()),
                                );
                            }
                        }
                        agent_core::ProviderEvent::ThinkingChunk(chunk) => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                slot.append_transcript(
                                    TranscriptEntry::Thinking(chunk.clone()),
                                );
                            }
                        }
                        agent_core::ProviderEvent::Status(text) => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                slot.append_transcript(TranscriptEntry::Status(
                                    text.clone(),
                                ));
                            }
                        }
                        agent_core::ProviderEvent::ExecCommandStarted {
                            call_id,
                            input_preview,
                            source,
                        } => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                slot.append_transcript(
                                    TranscriptEntry::ExecCommand {
                                        call_id: call_id.clone(),
                                        source: source.clone(),
                                        allow_exploring_group: true,
                                        input_preview: input_preview.clone(),
                                        output_preview: None,
                                        status: agent_core::ExecCommandStatus::InProgress,
                                        exit_code: None,
                                        duration_ms: None,
                                    },
                                );
                            }
                        }
                        agent_core::ProviderEvent::ExecCommandOutputDelta {
                            call_id: _,
                            delta,
                        } => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                // Append to the last ExecCommand entry.
                                if let Some(
                                    TranscriptEntry::ExecCommand {
                                        output_preview, ..
                                    },
                                ) = slot.transcript_mut().last_mut()
                                {
                                    if let Some(preview) = output_preview {
                                        preview.push_str(delta);
                                    } else {
                                        *output_preview = Some(delta.clone());
                                    }
                                }
                            }
                        }
                        agent_core::ProviderEvent::ExecCommandFinished {
                            call_id: _,
                            output_preview,
                            status,
                            exit_code,
                            duration_ms: _,
                            source: _,
                        } => {
                            if let Some(slot) =
                                inner.agent_pool.get_slot_mut_by_id(&agent_id)
                            {
                                if let Some(
                                    TranscriptEntry::ExecCommand {
                                        output_preview: out,
                                        status: st,
                                        exit_code: ec,
                                        duration_ms: dm,
                                        ..
                                    },
                                ) = slot.transcript_mut().last_mut()
                                {
                                    if out.is_none() {
                                        *out = output_preview.clone();
                                    }
                                    *st = *status;
                                    *ec = *exit_code;
                                    *dm = None;
                                }
                            }
                        }
                        _ => {}
                    }

                    // Decision layer integration.
                    let classify_result =
                        inner.agent_pool.classify_event(&agent_id, &event);
                    if classify_result.is_needs_decision() {
                        let situation = classify_result
                            .situation()
                            .map(|s| s.clone_boxed())
                            .unwrap_or_else(|| {
                                let components =
                                    agent_decision::initializer::initialize_decision_layer();
                                components.situation_registry.build(
                                    classify_result
                                        .situation_type()
                                        .unwrap()
                                        .clone(),
                                )
                            });
                        let situation_type = classify_result.situation_type().unwrap();
                        let context = agent_decision::context::DecisionContext::new(
                            situation.clone_boxed(),
                            agent_id.as_str(),
                        );
                        let request = agent_core::decision_mail::DecisionRequest::new(
                            agent_id.clone(),
                            situation_type.clone(),
                            context,
                        );
                        if let Err(e) =
                            inner.agent_pool.send_decision_request(&agent_id, request)
                        {
                            tracing::warn!(
                                agent_id = %agent_id.as_str(),
                                error = %e,
                                "failed to send decision request"
                            );
                        }
                    }

                    // Convert to protocol events and collect for broadcast.
                    let protocol_events =
                        pump.process(agent_id.as_str().to_string(), event);
                    broadcast_events.extend(protocol_events);
                }
                _ => {}
            }
        }

        // Clean up disconnected channels.
        for disconnected_id in poll_result.disconnected_channels {
            if let Some(slot) =
                inner.agent_pool.get_slot_mut_by_id(&disconnected_id)
            {
                slot.clear_provider_thread();
                if slot.status().is_active() {
                    let _ = slot.transition_to(CoreAgentStatus::idle());
                }
            }
            inner.event_aggregator.remove_receiver(&disconnected_id);
        }

        // ------------------------------------------------------------------
        // 2. Check idle agents and trigger decision layer intervention
        // ------------------------------------------------------------------
        const IDLE_DECISION_TRIGGER_SECS: u64 = 60;
        const IDLE_DECISION_COOLDOWN_SECS: u64 = 300;

        let idle_agents: Vec<CoreAgentId> = inner
            .agent_pool
            .slots()
            .iter()
            .filter_map(|slot| {
                if !slot.status().is_idle() {
                    return None;
                }
                let elapsed = slot.last_activity().elapsed().as_secs();
                let recently_triggered = slot
                    .last_idle_trigger_at()
                    .map(|t| t.elapsed().as_secs() < IDLE_DECISION_COOLDOWN_SECS)
                    .unwrap_or(false);
                if elapsed >= IDLE_DECISION_TRIGGER_SECS && !recently_triggered {
                    Some(slot.agent_id().clone())
                } else {
                    None
                }
            })
            .collect();

        for agent_id in idle_agents {
            // Guard: skip if decision agent is already processing
            let decision_agent_busy = inner
                .agent_pool
                .decision_agent_for(&agent_id)
                .map(|da| !da.is_idle())
                .unwrap_or(false);
            if decision_agent_busy {
                tracing::debug!(
                    agent_id = %agent_id.as_str(),
                    "decision agent busy, skipping idle trigger"
                );
                continue;
            }

            let situation_type = agent_decision::types::SituationType::new("agent_idle");
            let components = agent_decision::initializer::initialize_decision_layer();
            let situation = components.situation_registry.build(situation_type.clone());
            let context = agent_decision::context::DecisionContext::new(
                situation,
                agent_id.as_str(),
            );
            let request = agent_core::decision_mail::DecisionRequest::new(
                agent_id.clone(),
                situation_type,
                context,
            );

            if let Err(e) = inner.agent_pool.send_decision_request(&agent_id, request) {
                tracing::warn!(
                    agent_id = %agent_id.as_str(),
                    error = %e,
                    "failed to send idle decision request"
                );
            } else {
                tracing::info!(
                    agent_id = %agent_id.as_str(),
                    "triggered decision layer for idle agent"
                );
                // Record trigger timestamp to enforce cooldown
                if let Some(slot) = inner.agent_pool.get_slot_mut_by_id(&agent_id) {
                    slot.set_last_idle_trigger_at(std::time::Instant::now());
                }
            }
        }

        // ------------------------------------------------------------------
        // 3. Poll decision agents
        // ------------------------------------------------------------------
        let decision_responses = inner.agent_pool.poll_decision_agents();
        for (work_agent_id, response) in decision_responses {
            if let Some(output) = response.output() {
                let result =
                    inner.agent_pool.execute_decision_action(&work_agent_id, output);

                match result {
                    agent_core::agent_pool::DecisionExecutionResult::CustomInstruction {
                        instruction,
                    } => {
                        // Decision executor already appended the instruction as a
                        // TranscriptEntry::User, so we must NOT record it again.
                        let slot_status = inner
                            .agent_pool
                            .get_slot_by_id(&work_agent_id)
                            .map(|s| s.status().label());

                        let is_valid = slot_status.as_deref().map_or(false, |status| {
                            status == "responding"
                                || status == "starting"
                                || status == "idle"
                                || status.starts_with("blocked:")
                                || status == "waiting_for_input"
                        });

                        if is_valid {
                            if let Err(e) = Self::start_provider_for_agent_inner(
                                &mut inner,
                                &work_agent_id,
                                &instruction,
                                false, // do not duplicate transcript entry
                            )
                            .await
                            {
                                tracing::warn!(
                                    agent_id = %work_agent_id.as_str(),
                                    error = %e,
                                    "failed to start provider for custom instruction"
                                );
                            } else {
                                tracing::info!(
                                    agent_id = %work_agent_id.as_str(),
                                    instruction = %instruction,
                                    "started provider for custom instruction"
                                );
                            }
                        } else {
                            tracing::warn!(
                                agent_id = %work_agent_id.as_str(),
                                status = ?slot_status,
                                "agent not in valid state for custom instruction provider start"
                            );
                        }
                    }
                    agent_core::agent_pool::DecisionExecutionResult::TaskPrepared {
                        branch,
                        ..
                    } => {
                        tracing::info!(
                            agent_id = %work_agent_id.as_str(),
                            branch = %branch,
                            "task prepared by decision layer"
                        );
                    }
                    agent_core::agent_pool::DecisionExecutionResult::PreparationFailed {
                        reason,
                    } => {
                        tracing::warn!(
                            agent_id = %work_agent_id.as_str(),
                            reason = %reason,
                            "task preparation failed"
                        );
                    }
                    _ => {
                        // Other results (Executed, Skipped, Cancelled, etc.) do not
                        // require additional provider thread management.
                    }
                }
            }
        }

        // ------------------------------------------------------------------
        // 4. Process pending mailbox deliveries
        // ------------------------------------------------------------------
        if inner.mailbox.pending_count() > 0 {
            let _delivered = inner.mailbox.process_pending();
        }

        Ok(broadcast_events)
    }

    // ---------------------------------------------------------------------------
    // Test-only helpers
    // ---------------------------------------------------------------------------

    /// Test helper: return the number of agents in the pool.
    #[doc(hidden)]
    pub async fn agent_count(&self) -> usize {
        let inner = self.inner.lock().await;
        inner.agent_pool.active_count()
    }

    /// Test helper: check if an agent exists in the pool.
    #[doc(hidden)]
    pub async fn agent_exists(&self, agent_id: &str) -> bool {
        let inner = self.inner.lock().await;
        inner
            .agent_pool
            .slots()
            .iter()
            .any(|s| s.agent_id().as_str() == agent_id)
    }

    /// Test helper: return the status label of an agent.
    #[doc(hidden)]
    pub async fn agent_status(&self, agent_id: &str) -> Option<String> {
        let inner = self.inner.lock().await;
        inner
            .agent_pool
            .slots()
            .iter()
            .find(|s| s.agent_id().as_str() == agent_id)
            .map(|s| s.status().label())
    }

    /// Test helper: read an agent's transcript entries.
    #[doc(hidden)]
    pub async fn agent_transcript(
        &self,
        agent_id: &str,
    ) -> Option<Vec<agent_core::app::TranscriptEntry>> {
        let inner = self.inner.lock().await;
        inner
            .agent_pool
            .slots()
            .iter()
            .find(|s| s.agent_id().as_str() == agent_id)
            .map(|s| s.transcript().to_vec())
    }

    /// Test helper: spawn a decision agent for a work agent.
    #[doc(hidden)]
    pub async fn spawn_decision_agent_for(&self, agent_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        inner
            .agent_pool
            .spawn_decision_agent_for(&id)
            .map_err(|e| anyhow::anyhow!("spawn decision agent failed: {e}"))?;
        Ok(())
    }

    /// Test helper: check if a decision agent exists for a work agent.
    #[doc(hidden)]
    pub async fn decision_agent_exists(&self, agent_id: &str) -> bool {
        let inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        inner.agent_pool.decision_agent_for(&id).is_some()
    }

    /// Test helper: send a decision request directly to a decision agent.
    #[doc(hidden)]
    pub async fn send_decision_request(
        &self,
        agent_id: &str,
        situation_type: agent_decision::types::SituationType,
        context: agent_decision::context::DecisionContext,
    ) -> Result<()> {
        let inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        let request = agent_core::decision_mail::DecisionRequest::new(id.clone(), situation_type, context);
        inner
            .agent_pool
            .send_decision_request(&id, request)
            .map_err(|e| anyhow::anyhow!("send decision request failed: {e}"))?;
        Ok(())
    }

    /// Test helper: set the last activity timestamp for a slot.
    #[doc(hidden)]
    pub async fn set_slot_last_activity(
        &self,
        agent_id: &str,
        instant: std::time::Instant,
    ) {
        let mut inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        inner.agent_pool.set_slot_last_activity(&id, instant);
    }

    /// Test helper: read a decision agent's status label for a work agent.
    #[doc(hidden)]
    pub async fn decision_agent_status(&self, agent_id: &str) -> Option<String> {
        let inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        inner
            .agent_pool
            .decision_agent_for(&id)
            .map(|da| da.status().label())
    }

    /// Test helper: assign a task to an agent.
    #[doc(hidden)]
    pub async fn assign_task(&self, agent_id: &str, task_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        let task = agent_types::TaskId::new(task_id);
        inner
            .agent_pool
            .assign_task(&id, task)
            .map_err(|e| anyhow::anyhow!("assign task failed: {e}"))?;
        Ok(())
    }

    /// Test helper: inject a transcript entry directly into an agent's slot.
    #[doc(hidden)]
    pub async fn inject_transcript_entry(
        &self,
        agent_id: &str,
        entry: agent_core::app::TranscriptEntry,
    ) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let id = agent_types::AgentId::new(agent_id);
        let slot = inner
            .agent_pool
            .get_slot_mut_by_id(&id)
            .context("agent not found")?;
        slot.append_transcript(entry);
        Ok(())
    }

    /// Test helper: return the current working directory.
    #[doc(hidden)]
    pub async fn work_dir(&self) -> PathBuf {
        let inner = self.inner.lock().await;
        inner.session.app.cwd.clone()
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

impl SnapshotFile {
    /// Create an empty fallback snapshot for error cases.
    fn empty_fallback() -> Self {
        Self {
            version: 1,
            session_id: String::new(),
            written_at: String::new(),
            last_event_seq: 0,
            state: SessionState::default(),
            checksum: None,
        }
    }
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
    async fn snapshot_missing_checksum_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let wp = WorkplaceId::new("wp-nosum");
        let mgr = SessionManager::bootstrap(tmp.path().to_path_buf(), wp)
            .await
            .unwrap();

        let path = tmp.path().join("snapshot.json");
        mgr.write_snapshot(&path).await.unwrap();

        // Remove checksum field by re-writing without it.
        let file: SnapshotFile = serde_json::from_str(
            &tokio::fs::read_to_string(&path).await.unwrap()
        ).unwrap();
        let file_no_checksum = SnapshotFile {
            checksum: None,
            ..file
        };
        tokio::fs::write(&path, serde_json::to_string_pretty(&file_no_checksum).unwrap())
            .await
            .unwrap();

        let file = SessionManager::read_snapshot(&path).await.unwrap();
        assert!(file.checksum.is_none(), "missing checksum should cause fallback");
        assert!(file.session_id.is_empty(), "should return empty state for security");
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
