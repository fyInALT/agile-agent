use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;

use crate::agent_runtime::AgentBootstrapKind;
use crate::agent_runtime::AgentRuntime;
use crate::app::AppState;
use crate::app::LoopPhase;
use crate::backlog::BacklogState;
use crate::backlog_store;
use crate::logging;
use crate::provider::ProviderKind;
use crate::shutdown_snapshot::AgentShutdownSnapshot;
use crate::shutdown_snapshot::ShutdownReason;
use crate::shutdown_snapshot::ShutdownSnapshot;
use crate::session_store;
use crate::shared_state::SharedWorkplaceState;
use crate::skills::SkillRegistry;

#[derive(Debug)]
pub struct RuntimeSession {
    pub app: AppState,
    pub agent_runtime: AgentRuntime,
    pub workplace: SharedWorkplaceState,
}

impl RuntimeSession {
    pub fn bootstrap(
        launch_cwd: PathBuf,
        default_provider: ProviderKind,
        resume_snapshot: bool,
    ) -> Result<Self> {
        let bootstrap = AgentRuntime::bootstrap_for_cwd(&launch_cwd, default_provider)?;
        let workplace_id = bootstrap.runtime.meta().workplace_id.clone();
        let backlog = backlog_store::load_backlog_for_workplace(bootstrap.runtime.workplace())?;
        let skills = SkillRegistry::discover(&launch_cwd);

        let mut workplace = SharedWorkplaceState::with_backlog(workplace_id, backlog);
        workplace.skills = skills;

        let mut app = AppState::new(default_provider);
        app.cwd = launch_cwd.clone();

        for warning in bootstrap.runtime.apply_to_app_state(&mut app) {
            app.push_error_message(warning);
        }
        if matches!(bootstrap.kind, AgentBootstrapKind::Restored) {
            // Restore transcript first (so warnings can be appended)
            if let Err(err) = bootstrap.runtime.restore_transcript(&mut app) {
                app.push_error_message(format!("failed to restore agent transcript: {err}"));
                logging::error_event(
                    "agent.restore_transcript",
                    "failed to restore transcript into session app state",
                    serde_json::json!({
                        "error": err.to_string(),
                    }),
                );
            }
            // Restore agent state (input, task, loop settings)
            match bootstrap.runtime.restore_state(&mut app) {
                Ok(result) => {
                    for warning in result.warnings {
                        app.push_error_message(warning);
                    }
                }
                Err(err) => {
                    app.push_error_message(format!("failed to restore agent state: {err}"));
                    logging::error_event(
                        "agent.restore_state",
                        "failed to restore state into session app state",
                        serde_json::json!({
                            "error": err.to_string(),
                        }),
                    );
                }
            }
        }
        announce_bootstrap_kind(&mut app, &bootstrap.kind, &bootstrap.runtime);

        let mut session = Self {
            app,
            agent_runtime: bootstrap.runtime,
            workplace,
        };

        if resume_snapshot {
            session.restore_snapshot_or_legacy(&launch_cwd);
        }

        session.persist_if_changed()?;
        logging::debug_event(
            "agent.session.bootstrap",
            "bootstrapped runtime session",
            serde_json::json!({
                "agent_id": session.agent_runtime.agent_id().as_str(),
                "provider": session.app.selected_provider.label(),
                "resume_snapshot": resume_snapshot,
            }),
        );
        Ok(session)
    }

    /// Get workplace (shared state) reference
    pub fn workplace(&self) -> &SharedWorkplaceState {
        &self.workplace
    }

    /// Get workplace (shared state) mutable reference
    pub fn workplace_mut(&mut self) -> &mut SharedWorkplaceState {
        &mut self.workplace
    }

    /// Sync shared fields from app to workplace
    /// Called before persisting to ensure workplace has latest shared state
    pub fn sync_app_to_workplace(&mut self) {
        self.workplace.loop_control.should_quit = self.app.should_quit;
        self.workplace.loop_control.loop_run_active = self.app.loop_run_active;
        self.workplace.loop_control.current_iteration =
            self.workplace.loop_control.max_iterations.saturating_sub(self.app.remaining_loop_iterations);
        self.workplace.skills.enabled_names = self.app.skills.enabled_names.clone();
        // Note: backlog is already synced through separate calls
    }

    /// Sync shared fields from workplace to app
    /// Called after restoring to ensure app has latest shared state
    pub fn sync_workplace_to_app(&mut self) {
        self.app.should_quit = self.workplace.loop_control.should_quit;
        self.app.loop_run_active = self.workplace.loop_control.loop_run_active;
        self.app.remaining_loop_iterations = self.workplace.loop_control.remaining_iterations();
        self.app.skills.enabled_names = self.workplace.skills.enabled_names.clone();
    }

    pub fn persist_if_changed(&mut self) -> Result<()> {
        self.sync_app_to_workplace();
        if self.agent_runtime.sync_from_app_state(&self.app) {
            self.persist_all()?;
        }
        Ok(())
    }

    pub fn persist_all(&self) -> Result<()> {
        self.agent_runtime.persist()?;
        self.agent_runtime.persist_state(&self.app)?;
        self.agent_runtime.persist_transcript(&self.app)?;
        self.agent_runtime.persist_messages(&self.app)?;
        self.agent_runtime.persist_memory(&self.app)?;
        backlog_store::save_backlog_for_workplace(
            &self.workplace.backlog,
            self.agent_runtime.workplace(),
        )?;
        session_store::save_recent_session_for_workplace(
            &self.app,
            self.agent_runtime.workplace(),
        )?;
        logging::debug_event(
            "agent.persist",
            "persisted runtime bundle",
            serde_json::json!({
                "agent_id": self.agent_runtime.agent_id().as_str(),
                "workplace_id": self.agent_runtime.meta().workplace_id.as_str(),
                "provider": self.app.selected_provider.label(),
            }),
        );
        Ok(())
    }

    pub fn mark_stopped_and_persist(&mut self) -> Result<()> {
        self.sync_app_to_workplace();
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        self.persist_all()
    }

    /// Mark agent as interrupted and persist (for unexpected shutdown during execution)
    pub fn mark_interrupted_and_persist(&mut self) -> Result<()> {
        self.sync_app_to_workplace();
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        // Save state with interrupted flag
        self.agent_runtime.persist_interrupted_state(&self.app)?;
        self.agent_runtime.persist()?;
        self.agent_runtime.persist_transcript(&self.app)?;
        self.agent_runtime.persist_messages(&self.app)?;
        self.agent_runtime.persist_memory(&self.app)?;
        backlog_store::save_backlog_for_workplace(
            &self.workplace.backlog,
            self.agent_runtime.workplace(),
        )?;
        session_store::save_recent_session_for_workplace(
            &self.app,
            self.agent_runtime.workplace(),
        )?;
        logging::debug_event(
            "agent.persist_interrupted",
            "persisted interrupted agent state",
            serde_json::json!({
                "agent_id": self.agent_runtime.agent_id().as_str(),
                "loop_phase": self.app.loop_phase,
            }),
        );
        Ok(())
    }

    /// Check if the current agent state indicates an interrupted session
    pub fn was_interrupted(&self) -> bool {
        matches!(self.app.loop_phase, LoopPhase::Executing | LoopPhase::Planning | LoopPhase::Verifying)
    }

    /// Create shutdown snapshot for current session
    pub fn create_shutdown_snapshot(&self, reason: ShutdownReason) -> ShutdownSnapshot {
        let agent_snapshot = if self.was_interrupted() {
            AgentShutdownSnapshot::active(
                self.agent_runtime.meta().clone(),
                self.app.active_task_id.clone(),
                crate::shutdown_snapshot::ProviderThreadSnapshot::waiting_for_response(
                    None,
                    "2026-04-14T00:00:00Z".to_string(), // placeholder
                ),
            )
        } else {
            AgentShutdownSnapshot::idle(self.agent_runtime.meta().clone())
        };

        ShutdownSnapshot::new(
            self.agent_runtime.meta().workplace_id.as_str().to_string(),
            vec![agent_snapshot],
            self.workplace.backlog.clone(),
            reason,
        )
    }

    /// Perform graceful shutdown with snapshot
    pub fn graceful_shutdown(&mut self, reason: ShutdownReason) -> Result<ShutdownSnapshot> {
        // Persist current state
        self.persist_all()?;

        // Create and save shutdown snapshot
        let snapshot = self.create_shutdown_snapshot(reason);
        self.agent_runtime.workplace().save_shutdown_snapshot(&snapshot)?;

        // Mark agent as stopped
        self.agent_runtime.mark_stopped();
        self.agent_runtime.persist()?;

        logging::debug_event(
            "session.shutdown",
            "completed graceful shutdown",
            serde_json::json!({
                "agent_id": self.agent_runtime.agent_id().as_str(),
                "reason": snapshot.shutdown_reason,
                "was_interrupted": self.was_interrupted(),
            }),
        );

        Ok(snapshot)
    }

    /// Quick shutdown (just persist and mark stopped)
    pub fn quick_shutdown(&mut self) -> Result<()> {
        self.mark_stopped_and_persist()
    }

    /// Check for shutdown snapshot and restore if exists
    pub fn check_shutdown_snapshot(&self) -> Option<ShutdownSnapshot> {
        self.agent_runtime
            .workplace()
            .load_shutdown_snapshot()
            .ok()
            .flatten()
    }

    /// Restore session from shutdown snapshot
    pub fn restore_from_snapshot(
        launch_cwd: PathBuf,
        snapshot: ShutdownSnapshot,
        default_provider: ProviderKind,
    ) -> Result<Self> {
        // Use first agent's meta if available, otherwise create new
        let (agent_meta, was_active, assigned_task_id) = if let Some(first_agent) = snapshot.agents.first() {
            (first_agent.meta.clone(), first_agent.was_active, first_agent.assigned_task_id.clone())
        } else {
            // No agent in snapshot, create default
            let workplace = crate::workplace_store::WorkplaceStore::for_cwd(&launch_cwd)?;
            workplace.ensure()?;
            let store = crate::agent_store::AgentStore::new(workplace.clone());
            let index = store.next_agent_index()?;
            let runtime = AgentRuntime::new(&workplace, index, default_provider);
            runtime.persist()?;
            (runtime.meta().clone(), false, None)
        };

        let workplace = crate::workplace_store::WorkplaceStore::for_cwd(&launch_cwd)?;
        workplace.ensure()?;
        let agent_runtime = AgentRuntime::from_meta(agent_meta, workplace.clone());

        let backlog = snapshot.backlog.clone();
        let skills = SkillRegistry::discover(&launch_cwd);

        let mut workplace_state = SharedWorkplaceState::with_backlog(
            agent_runtime.meta().workplace_id.clone(),
            backlog,
        );
        workplace_state.skills = skills;

        let mut app = AppState::new(default_provider);
        app.cwd = launch_cwd.clone();

        // Restore agent state
        for warning in agent_runtime.apply_to_app_state(&mut app) {
            app.push_error_message(warning);
        }

        // Restore transcript
        if let Err(err) = agent_runtime.restore_transcript(&mut app) {
            app.push_error_message(format!("failed to restore transcript: {err}"));
        }

        // Restore state
        match agent_runtime.restore_state(&mut app) {
            Ok(result) => {
                for warning in result.warnings {
                    app.push_error_message(warning);
                }
            }
            Err(err) => {
                app.push_error_message(format!("failed to restore state: {err}"));
            }
        }

        // Add restore message
        if was_active {
            app.push_status_message(format!(
                "restored agent {} (was interrupted during execution)",
                agent_runtime.summary()
            ));
        } else {
            app.push_status_message(format!("restored agent {}", agent_runtime.summary()));
        }

        // Set task if agent had one
        if let Some(task_id) = assigned_task_id {
            app.active_task_id = Some(task_id);
        }

        let session = Self {
            app,
            agent_runtime,
            workplace: workplace_state,
        };

        // Clear the snapshot after successful restore
        workplace.clear_shutdown_snapshot()?;

        logging::debug_event(
            "session.restore_from_snapshot",
            "restored session from shutdown snapshot",
            serde_json::json!({
                "agent_id": session.agent_runtime.agent_id().as_str(),
                "was_active": was_active,
                "resume_count": snapshot.resume_count(),
            }),
        );

        Ok(session)
    }

    pub fn switch_agent(&mut self, provider_kind: ProviderKind) -> Result<String> {
        self.sync_app_to_workplace();
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        self.persist_all()?;

        let next_runtime = self.agent_runtime.create_sibling(provider_kind)?;
        let cwd = self.app.cwd.clone();
        let mut skills = SkillRegistry::discover(&cwd);
        skills.enabled_names = self.workplace.skills.enabled_names.clone();

        let mut next_app = AppState::new(provider_kind);
        next_app.cwd = cwd.clone();
        for warning in next_runtime.apply_to_app_state(&mut next_app) {
            next_app.push_error_message(warning);
        }
        let summary = next_runtime.summary();
        next_app.push_status_message(format!("created agent: {summary}"));

        // Update workplace with new skills (backlog stays the same)
        self.workplace.skills = skills;

        self.app = next_app;
        self.agent_runtime = next_runtime;
        self.sync_workplace_to_app(); // Ensure app has workplace state
        self.persist_all()?;

        Ok(summary)
    }

    fn restore_snapshot_or_legacy(&mut self, launch_cwd: &Path) {
        match self.agent_runtime.restore_snapshot(&mut self.app) {
            Ok(restored) => {
                self.app.push_status_message("restored recent agent state");
                for warning in restored.warnings {
                    self.app.push_error_message(warning);
                }
                for warning in self.agent_runtime.apply_to_app_state(&mut self.app) {
                    self.app.push_error_message(warning);
                }
                self.sync_app_to_workplace();
            }
            Err(err) => match session_store::restore_recent_session_for_workplace(
                &mut self.app,
                launch_cwd,
                self.agent_runtime.workplace(),
            ) {
                Ok(restored) => {
                    self.app.push_status_message("restored recent session");
                    for warning in restored.warnings {
                        self.app.push_error_message(warning);
                    }
                    for warning in self.agent_runtime.apply_to_app_state(&mut self.app) {
                        self.app.push_error_message(warning);
                    }
                    self.sync_app_to_workplace();
                }
                Err(_) => self
                    .app
                    .push_error_message(format!("failed to restore recent agent state: {err}")),
            },
        }
    }
}

fn announce_bootstrap_kind(app: &mut AppState, kind: &AgentBootstrapKind, runtime: &AgentRuntime) {
    match kind {
        AgentBootstrapKind::Created => {
            app.push_status_message(format!("created agent: {}", runtime.summary()));
        }
        AgentBootstrapKind::Restored => {
            app.push_status_message(format!("restored agent: {}", runtime.summary()));
        }
        AgentBootstrapKind::RecreatedAfterError { error } => {
            app.push_error_message(format!("failed to restore agent runtime: {error}"));
            app.push_status_message(format!("created replacement agent: {}", runtime.summary()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeSession;
    use super::LoopPhase;
    use super::ShutdownReason;
    use crate::app::TranscriptEntry;
    use crate::logging;
    use crate::logging::RunMode;
    use crate::provider::ProviderKind;
    use crate::provider::SessionHandle;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_creates_runtime_session() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap");

        assert_eq!(session.agent_runtime.meta().provider_type.label(), "mock");
    }

    #[test]
    fn switching_agent_changes_identity() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
                .expect("bootstrap");
        let previous = session.agent_runtime.agent_id().as_str().to_string();

        session.switch_agent(ProviderKind::Codex).expect("switch");

        assert_ne!(session.agent_runtime.agent_id().as_str(), previous);
        assert_eq!(session.app.selected_provider, ProviderKind::Codex);
    }

    #[test]
    fn bootstrap_restores_existing_agent_transcript_and_codex_thread_without_resume_snapshot() {
        let _guard = logging::test_guard();
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");
        logging::init_for_workplace(&workplace, RunMode::RunLoop).expect("init logger");
        let mut first = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Codex, false)
            .expect("bootstrap");
        first.app.push_user_message("hello".to_string());
        first.app.begin_provider_response();
        first.app.append_assistant_chunk("world");
        first.app.finish_provider_response();
        first.app.apply_session_handle(SessionHandle::CodexThread {
            thread_id: "thr-restore-1".to_string(),
        });
        first.mark_stopped_and_persist().expect("persist");

        let restored = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
            .expect("bootstrap restored");

        assert_eq!(restored.app.selected_provider, ProviderKind::Codex);
        assert!(
            restored
                .app
                .transcript
                .iter()
                .any(|entry| { matches!(entry, TranscriptEntry::User(text) if text == "hello") })
        );
        assert!(
            restored.app.transcript.iter().any(|entry| {
                matches!(entry, TranscriptEntry::Assistant(text) if text == "world")
            })
        );
        assert_eq!(
            restored.app.codex_thread_id.as_deref(),
            Some("thr-restore-1")
        );
        assert_eq!(
            restored.app.current_session_handle(),
            Some(SessionHandle::CodexThread {
                thread_id: "thr-restore-1".to_string(),
            })
        );

        let log_path = logging::current_log_path().expect("log path");
        let contents = std::fs::read_to_string(log_path).expect("log file");
        assert!(contents.contains("\"event\":\"agent.bootstrap\""));
        assert!(contents.contains("\"event\":\"agent.restore_transcript\""));
    }

    #[test]
    fn bootstrap_restores_agent_state_on_restart() {
        let temp = TempDir::new().expect("tempdir");
        let mut first =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap first");
        first.app.input = "draft input".to_string();
        first.app.active_task_id = Some("task-restore-1".to_string());
        first.app.loop_run_active = true;
        first.app.remaining_loop_iterations = 5;
        first.mark_stopped_and_persist().expect("persist first");

        let restored =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap restored");

        assert_eq!(restored.app.input, "draft input");
        assert_eq!(restored.app.active_task_id.as_deref(), Some("task-restore-1"));
        assert!(restored.app.loop_run_active);
        assert_eq!(restored.app.remaining_loop_iterations, 5);
    }

    #[test]
    fn mark_interrupted_perserves_interrupted_flag() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");
        session.app.loop_phase = LoopPhase::Executing;
        session.mark_interrupted_and_persist().expect("interrupted persist");

        let restored =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap restored");

        // Check that warnings include interrupted message
        let has_interrupted_warning = restored.app.transcript.iter().any(|entry| {
            matches!(entry, TranscriptEntry::Error(text) if text.contains("interrupted"))
        });
        assert!(has_interrupted_warning);
    }

    #[test]
    fn was_interrupted_detects_active_phases() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");

        // Idle is not interrupted
        session.app.loop_phase = LoopPhase::Idle;
        assert!(!session.was_interrupted());

        // Executing is interrupted
        session.app.loop_phase = LoopPhase::Executing;
        assert!(session.was_interrupted());

        // Planning is interrupted
        session.app.loop_phase = LoopPhase::Planning;
        assert!(session.was_interrupted());
    }

    #[test]
    fn graceful_shutdown_creates_snapshot() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");

        let snapshot = session
            .graceful_shutdown(ShutdownReason::UserQuit)
            .expect("shutdown");

        assert_eq!(snapshot.shutdown_reason, ShutdownReason::UserQuit);
        assert_eq!(snapshot.agents.len(), 1);
        assert!(!snapshot.has_active_agents());
    }

    #[test]
    fn graceful_shutdown_saves_snapshot_file() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");

        session.graceful_shutdown(ShutdownReason::UserQuit).expect("shutdown");

        // Snapshot file should exist
        assert!(session.agent_runtime.workplace().has_shutdown_snapshot());
    }

    #[test]
    fn graceful_shutdown_marks_active_agent_in_snapshot() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");
        session.app.loop_phase = LoopPhase::Executing;

        let snapshot = session
            .graceful_shutdown(ShutdownReason::Interrupted)
            .expect("shutdown");

        assert!(snapshot.has_active_agents());
        assert!(snapshot.agents[0].needs_resume());
    }

    #[test]
    fn restore_from_snapshot_recovers_session() {
        let temp = TempDir::new().expect("tempdir");
        let mut first =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap first");
        first.app.input = "draft input".to_string();
        first.app.active_task_id = Some("task-restore-1".to_string());
        let snapshot = first.graceful_shutdown(ShutdownReason::UserQuit).expect("shutdown");

        let restored =
            RuntimeSession::restore_from_snapshot(temp.path().into(), snapshot, ProviderKind::Mock)
                .expect("restore");

        assert_eq!(restored.app.input, "draft input");
        assert_eq!(restored.app.active_task_id.as_deref(), Some("task-restore-1"));
    }

    #[test]
    fn restore_from_snapshot_clears_snapshot_file() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap");
        let snapshot = session.graceful_shutdown(ShutdownReason::UserQuit).expect("shutdown");

        // Snapshot file exists before restore
        assert!(session.agent_runtime.workplace().has_shutdown_snapshot());

        RuntimeSession::restore_from_snapshot(temp.path().into(), snapshot, ProviderKind::Mock)
            .expect("restore");

        // Snapshot file should be cleared after restore
        let workplace = crate::workplace_store::WorkplaceStore::for_cwd(temp.path())
            .expect("workplace");
        assert!(!workplace.has_shutdown_snapshot());
    }
}
