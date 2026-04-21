//! Shared test utilities for agent-core integration tests.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use agent_core::ProviderEvent;
use agent_core::agent_runtime::{AgentId, WorkplaceId};
use agent_core::agent_slot::TaskId;
use agent_core::backlog::{BacklogState, TodoItem, TodoStatus};
use agent_core::multi_agent_session::MultiAgentSession;
use agent_core::ProviderKind;
use agent_toolkit::ExecCommandStatus;

// ============================================================================
// MockProviderChannel
// ============================================================================

pub struct MockProviderChannel {
    tx: Sender<ProviderEvent>,
}

impl MockProviderChannel {
    pub fn new() -> (Self, Receiver<ProviderEvent>) {
        let (tx, rx) = channel();
        (Self { tx }, rx)
    }

    pub fn sender(&self) -> &Sender<ProviderEvent> {
        &self.tx
    }

    pub fn send_status(&self, text: impl Into<String>) {
        self.tx.send(ProviderEvent::Status(text.into())).unwrap();
    }

    pub fn send_assistant_chunk(&self, text: impl Into<String>) {
        self.tx.send(ProviderEvent::AssistantChunk(text.into())).unwrap();
    }

    pub fn send_thinking_chunk(&self, text: impl Into<String>) {
        self.tx.send(ProviderEvent::ThinkingChunk(text.into())).unwrap();
    }

    pub fn send_exec_command_started(
        &self,
        call_id: impl Into<String>,
        input_preview: impl Into<String>,
        source: impl Into<String>,
    ) {
        self.tx
            .send(ProviderEvent::ExecCommandStarted {
                call_id: Some(call_id.into()),
                input_preview: Some(input_preview.into()),
                source: Some(source.into()),
            })
            .unwrap();
    }

    pub fn send_exec_command_finished(
        &self,
        call_id: impl Into<String>,
        output_preview: impl Into<String>,
        status: ExecCommandStatus,
        exit_code: i32,
    ) {
        self.tx
            .send(ProviderEvent::ExecCommandFinished {
                call_id: Some(call_id.into()),
                output_preview: Some(output_preview.into()),
                status,
                exit_code: Some(exit_code),
                duration_ms: Some(100),
                source: Some("bash".to_string()),
            })
            .unwrap();
    }

    pub fn send_error(&self, text: impl Into<String>) {
        self.tx.send(ProviderEvent::Error(text.into())).unwrap();
    }

    pub fn send_finished(&self) {
        self.tx.send(ProviderEvent::Finished).unwrap();
    }

    pub fn send_session_handle(&self, handle: agent_core::SessionHandle) {
        self.tx.send(ProviderEvent::SessionHandle(handle)).unwrap();
    }
}

impl Default for MockProviderChannel {
    fn default() -> Self {
        let (chan, _rx) = Self::new();
        chan
    }
}

// ============================================================================
// EventSequence
// ============================================================================

pub struct EventSequence {
    events: Vec<ProviderEvent>,
}

impl EventSequence {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn status(mut self, text: impl Into<String>) -> Self {
        self.events.push(ProviderEvent::Status(text.into()));
        self
    }

    pub fn assistant(mut self, text: impl Into<String>) -> Self {
        self.events.push(ProviderEvent::AssistantChunk(text.into()));
        self
    }

    pub fn thinking(mut self, text: impl Into<String>) -> Self {
        self.events.push(ProviderEvent::ThinkingChunk(text.into()));
        self
    }

    pub fn exec_started(mut self, call_id: impl Into<String>, cmd: impl Into<String>) -> Self {
        self.events.push(ProviderEvent::ExecCommandStarted {
            call_id: Some(call_id.into()),
            input_preview: Some(cmd.into()),
            source: Some("bash".to_string()),
        });
        self
    }

    pub fn exec_finished(
        mut self,
        call_id: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        self.events.push(ProviderEvent::ExecCommandFinished {
            call_id: Some(call_id.into()),
            output_preview: Some(output.into()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(100),
            source: Some("bash".to_string()),
        });
        self
    }

    pub fn error(mut self, text: impl Into<String>) -> Self {
        self.events.push(ProviderEvent::Error(text.into()));
        self
    }

    pub fn finished(mut self) -> Self {
        self.events.push(ProviderEvent::Finished);
        self
    }

    pub fn events(&self) -> &[ProviderEvent] {
        &self.events
    }

    pub fn send_all(&self, channel: &MockProviderChannel) {
        for event in &self.events {
            channel.tx.send(event.clone()).unwrap();
        }
    }
}

impl Default for EventSequence {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MockProviderStarter — implements ProviderStarter for loop_runner tests
// ============================================================================

use std::collections::HashMap;
use std::sync::Mutex;

pub struct MockProviderStarter {
    sequences: Mutex<HashMap<String, Vec<ProviderEvent>>>,
}

impl MockProviderStarter {
    pub fn new() -> Self {
        Self {
            sequences: Mutex::new(HashMap::new()),
        }
    }

    /// Register a sequence of events that will be emitted when a prompt contains `substring`.
    pub fn when_prompt_contains(&self, substring: &str, events: Vec<ProviderEvent>) {
        self.sequences.lock().unwrap().insert(substring.to_string(), events);
    }

    /// Register a simple assistant reply that completes immediately.
    pub fn when_prompt_contains_reply(&self, substring: &str, reply: &str) {
        self.when_prompt_contains(
            substring,
            vec![
                ProviderEvent::AssistantChunk(reply.to_string()),
                ProviderEvent::Finished,
            ],
        );
    }
}

impl agent_core::loop_runner::ProviderStarter for MockProviderStarter {
    fn start_provider(
        &self,
        _kind: agent_core::ProviderKind,
        prompt: String,
        _cwd: PathBuf,
        _session_handle: Option<agent_core::SessionHandle>,
    ) -> anyhow::Result<std::sync::mpsc::Receiver<ProviderEvent>> {
        let (tx, rx) = channel();
        let sequences = self.sequences.lock().unwrap();

        let events = sequences
            .iter()
            .find(|(key, _)| prompt.contains(key.as_str()))
            .map(|(_, events)| events.clone());

        if let Some(events) = events {
            std::thread::spawn(move || {
                for event in events {
                    let _ = tx.send(event);
                }
            });
        } else {
            // Default fallback: simple done reply
            std::thread::spawn(move || {
                let _ = tx.send(ProviderEvent::AssistantChunk("done".to_string()));
                let _ = tx.send(ProviderEvent::Finished);
            });
        }

        Ok(rx)
    }
}

// ============================================================================
// TestHarness
// ============================================================================

pub struct TestHarness {
    pub temp_dir: tempfile::TempDir,
    pub workdir: PathBuf,
    pub workplace_id: WorkplaceId,
}

impl TestHarness {
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let workdir = temp_dir.path().to_path_buf();
        let workplace_id = WorkplaceId::new("test-workplace");
        Self {
            temp_dir,
            workdir,
            workplace_id,
        }
    }

    pub fn create_session(&self, max_agents: usize) -> MultiAgentSession {
        MultiAgentSession::new(
            self.workdir.clone(),
            self.workplace_id.clone(),
            ProviderKind::Mock,
            max_agents,
        )
    }

    pub fn create_app_state(&self) -> agent_core::app::AppState {
        agent_core::app::AppState::with_skills(
            ProviderKind::Mock,
            self.workdir.clone(),
            agent_core::skills::SkillRegistry::default(),
        )
    }

    pub fn seed_backlog_with_todo(&self, session: &mut MultiAgentSession, title: &str) {
        let todo = TodoItem {
            id: "todo-1".to_string(),
            title: title.to_string(),
            description: title.to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "test".to_string(),
        };
        session.workplace_mut().backlog.push_todo(todo);
    }

    pub fn seed_app_backlog_with_todo(&self, state: &mut agent_core::app::AppState, title: &str) {
        let todo = TodoItem {
            id: "todo-1".to_string(),
            title: title.to_string(),
            description: title.to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "test".to_string(),
        };
        state.backlog.push_todo(todo);
    }

    pub fn register_mock_provider(
        &self,
        session: &mut MultiAgentSession,
        agent_id: &AgentId,
    ) -> MockProviderChannel {
        let (channel, rx) = MockProviderChannel::new();
        session
            .event_aggregator_mut()
            .add_receiver(agent_id.clone(), rx);
        channel
    }

    pub fn assign_task_to_agent(
        &self,
        session: &mut MultiAgentSession,
        agent_id: &AgentId,
        task_id: &str,
    ) {
        if let Some(slot) = session.agents_mut().get_slot_mut_by_id(agent_id) {
            let _ = slot.assign_task(TaskId::new(task_id));
        }
        session.workplace_mut().backlog.start_task(task_id);
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}
