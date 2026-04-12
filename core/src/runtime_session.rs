use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;

use crate::agent_runtime::AgentBootstrapKind;
use crate::agent_runtime::AgentRuntime;
use crate::app::AppState;
use crate::backlog_store;
use crate::provider::ProviderKind;
use crate::session_store;
use crate::skills::SkillRegistry;

#[derive(Debug)]
pub struct RuntimeSession {
    pub app: AppState,
    pub agent_runtime: AgentRuntime,
}

impl RuntimeSession {
    pub fn bootstrap(
        launch_cwd: PathBuf,
        default_provider: ProviderKind,
        resume_snapshot: bool,
    ) -> Result<Self> {
        let bootstrap = AgentRuntime::bootstrap_for_cwd(&launch_cwd, default_provider)?;
        let mut app = AppState::with_skills(
            default_provider,
            launch_cwd.clone(),
            SkillRegistry::discover(&launch_cwd),
        );
        app.backlog = backlog_store::load_backlog_for_workplace(bootstrap.runtime.workplace())?;

        for warning in bootstrap.runtime.apply_to_app_state(&mut app) {
            app.push_error_message(warning);
        }
        if matches!(bootstrap.kind, AgentBootstrapKind::Restored) {
            if let Err(err) = bootstrap.runtime.restore_transcript(&mut app) {
                app.push_error_message(format!("failed to restore agent transcript: {err}"));
            }
        }
        announce_bootstrap_kind(&mut app, &bootstrap.kind, &bootstrap.runtime);

        let mut session = Self {
            app,
            agent_runtime: bootstrap.runtime,
        };

        if resume_snapshot {
            session.restore_snapshot_or_legacy(&launch_cwd);
        }

        session.persist_if_changed()?;
        Ok(session)
    }

    pub fn persist_if_changed(&mut self) -> Result<()> {
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
            &self.app.backlog,
            self.agent_runtime.workplace(),
        )?;
        session_store::save_recent_session_for_workplace(
            &self.app,
            self.agent_runtime.workplace(),
        )?;
        Ok(())
    }

    pub fn mark_stopped_and_persist(&mut self) -> Result<()> {
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        self.persist_all()
    }

    pub fn switch_agent(&mut self, provider_kind: ProviderKind) -> Result<String> {
        self.agent_runtime.sync_from_app_state(&self.app);
        self.agent_runtime.mark_stopped();
        self.persist_all()?;

        let next_runtime = self.agent_runtime.create_sibling(provider_kind)?;
        let cwd = self.app.cwd.clone();
        let backlog = self.app.backlog.clone();
        let mut skills = SkillRegistry::discover(&cwd);
        skills.enabled_names = self.app.skills.enabled_names.clone();

        let mut next_app = AppState::with_skills(provider_kind, cwd, skills);
        next_app.backlog = backlog;
        for warning in next_runtime.apply_to_app_state(&mut next_app) {
            next_app.push_error_message(warning);
        }
        let summary = next_runtime.summary();
        next_app.push_status_message(format!("created agent: {summary}"));

        self.app = next_app;
        self.agent_runtime = next_runtime;
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
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;
    use crate::provider::SessionHandle;
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
        let temp = TempDir::new().expect("tempdir");
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

        let restored =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
                .expect("bootstrap restored");

        assert_eq!(restored.app.selected_provider, ProviderKind::Codex);
        assert!(restored.app.transcript.iter().any(|entry| {
            matches!(entry, TranscriptEntry::User(text) if text == "hello")
        }));
        assert!(restored.app.transcript.iter().any(|entry| {
            matches!(entry, TranscriptEntry::Assistant(text) if text == "world")
        }));
        assert_eq!(restored.app.codex_thread_id.as_deref(), Some("thr-restore-1"));
        assert_eq!(
            restored.app.current_session_handle(),
            Some(SessionHandle::CodexThread {
                thread_id: "thr-restore-1".to_string(),
            })
        );
    }
}
