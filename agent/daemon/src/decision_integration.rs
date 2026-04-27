//! Decision Integration — connects Work Agent output to decision flows
//!
//! Bridges provider events and decision agent slots, enabling:
//! - Automatic decision triggering on Work Agent output
//! - Command dispatch from DecisionCommand to Work Agent
//! - Poll scheduling for Running status

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use decision_dsl::ast::document::Tree;
use decision_dsl::ast::parser::{DslParser, YamlParser};
use decision_dsl::ast::runtime::TickResult;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand, TaskCommand};
use decision_dsl::ext::blackboard::SprintGoal;
use tokio::sync::{Mutex, broadcast};

use crate::decision_agent_slot::{
    DecisionAgentSlot, DecisionCommandInterpreter, DecisionSlotConfig,
};
use crate::event_pump::EventPump;
use agent_core::ProviderEvent;
use agent_protocol::events::Event;

/// Helper to load a Tree from a YAML file.
fn load_tree_from_file(path: &PathBuf) -> Result<Tree> {
    let yaml = std::fs::read_to_string(path)
        .context("failed to read template file")?;

    let parser = YamlParser::new();
    let doc = parser.parse_document(&yaml)
        .context("failed to parse template")?;

    match doc {
        decision_dsl::ast::document::DslDocument::BehaviorTree { api_version, metadata, root } => {
            Ok(Tree {
                api_version,
                kind: decision_dsl::ast::document::TreeKind::BehaviorTree,
                metadata,
                spec: decision_dsl::ast::document::Spec { root },
            })
        }
        decision_dsl::ast::document::DslDocument::SubTree { api_version, metadata, root } => {
            Ok(Tree {
                api_version,
                kind: decision_dsl::ast::document::TreeKind::SubTree,
                metadata,
                spec: decision_dsl::ast::document::Spec { root },
            })
        }
        decision_dsl::ast::document::DslDocument::DecisionRules { .. } => {
            anyhow::bail!("DecisionRules cannot be used as decision flow template")
        }
    }
}

/// Decision integration configuration.
#[derive(Debug, Clone)]
pub struct DecisionIntegrationConfig {
    /// Decision flow template path
    pub template_path: Option<PathBuf>,
    /// Provider kind for decision AI
    pub decision_provider: String,
    /// Total sprints for multi-sprint workflows
    pub total_sprints: u8,
    /// Sprint goals
    pub sprint_goals: Vec<SprintGoal>,
    /// Enable decision on Work Agent output
    pub enabled: bool,
}

impl Default for DecisionIntegrationConfig {
    fn default() -> Self {
        Self {
            template_path: None,
            decision_provider: "claude".to_string(),
            total_sprints: 1,
            sprint_goals: Vec::new(),
            enabled: false,
        }
    }
}

/// Decision slot state for a Work Agent.
struct DecisionSlotState {
    /// The decision agent slot
    slot: DecisionAgentSlot,
    /// Work Agent ID this slot monitors
    work_agent_id: String,
    /// Current accumulated output
    output_buffer: String,
    /// Pending commands awaiting dispatch
    pending_commands: Vec<DecisionCommand>,
    /// Poll scheduled for Running status
    poll_scheduled: bool,
}

/// Manages decision agent slots for multiple Work Agents.
pub struct DecisionIntegration {
    /// Decision slots per Work Agent
    slots: Arc<Mutex<HashMap<String, DecisionSlotState>>>,
    /// Configuration
    config: DecisionIntegrationConfig,
    /// Event pump for generating protocol events
    event_pump: Arc<Mutex<EventPump>>,
    /// Event broadcaster for notifying clients
    event_tx: broadcast::Sender<Event>,
}

impl DecisionIntegration {
    /// Create a new decision integration.
    pub fn new(
        config: DecisionIntegrationConfig,
        event_pump: Arc<Mutex<EventPump>>,
        event_tx: broadcast::Sender<Event>,
    ) -> Result<Self> {
        Ok(Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
            config,
            event_pump,
            event_tx,
        })
    }

    /// Attach a decision slot to a Work Agent.
    pub async fn attach_decision_slot(
        &self,
        work_agent_id: String,
        task_description: String,
        template: Option<Tree>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let tree = template.or_else(|| {
            // Load default template if configured
            self.config.template_path.as_ref().and_then(|path| {
                load_tree_from_file(path).ok()
            })
        });

        let tree = tree.ok_or_else(|| {
            anyhow::anyhow!("no decision template available")
        })?;

        let cwd = PathBuf::from(".");
        let slot_config = DecisionSlotConfig {
            provider_kind: self.config.decision_provider.clone(),
            cwd,
            max_reflection_rounds: 2,
            total_sprints: self.config.total_sprints,
            sprint_goals: self.config.sprint_goals.clone(),
        };

        let mut slot = DecisionAgentSlot::new(tree, slot_config)?;
        slot.set_work_agent_id(&work_agent_id);
        slot.set_task_description(&task_description);

        let state = DecisionSlotState {
            slot,
            work_agent_id: work_agent_id.clone(),
            output_buffer: String::new(),
            pending_commands: Vec::new(),
            poll_scheduled: false,
        };

        let mut slots = self.slots.lock().await;
        let work_agent_id_ref = work_agent_id.clone();
        slots.insert(work_agent_id, state);

        tracing::info!(
            work_agent_id = %work_agent_id_ref,
            "Decision slot attached"
        );

        Ok(())
    }

    /// Handle a provider event from a Work Agent.
    ///
    /// Returns true if a decision tick was triggered.
    pub async fn handle_provider_event(
        &self,
        agent_id: &str,
        event: &ProviderEvent,
    ) -> Result<bool> {
        if !self.config.enabled {
            return Ok(false);
        }

        let mut slots = self.slots.lock().await;
        let state = slots.get_mut(agent_id);

        if state.is_none() {
            return Ok(false);
        }

        let state = state.unwrap();

        // Accumulate output
        match event {
            ProviderEvent::AssistantChunk(text) => {
                state.output_buffer.push_str(text);
            }
            ProviderEvent::Finished => {
                // Trigger decision tick on completion
                state.slot.sync_work_agent_output(&state.output_buffer);
                state.output_buffer.clear();

                let result = state.slot.tick()?;
                self.handle_tick_result(state, result)?;
                return Ok(true);
            }
            _ => {}
        }

        Ok(false)
    }

    /// Handle a tick result from the decision slot.
    fn handle_tick_result(&self, state: &mut DecisionSlotState, result: TickResult) -> Result<()> {
        tracing::debug!(
            agent_id = %state.work_agent_id,
            status = ?result.status,
            commands = result.commands.len(),
            "Decision tick completed"
        );

        match result.status {
            decision_dsl::ast::node::NodeStatus::Success => {
                // Dispatch commands
                let commands = state.slot.drain_commands();
                self.dispatch_commands(state, commands)?;
            }
            decision_dsl::ast::node::NodeStatus::Running => {
                // Schedule poll
                if !state.poll_scheduled {
                    state.poll_scheduled = true;
                    // In a real implementation, schedule a timer task
                    tracing::debug!(
                        agent_id = %state.work_agent_id,
                        "Decision running, scheduling poll"
                    );
                }
            }
            decision_dsl::ast::node::NodeStatus::Failure => {
                // Handle failure - escalate
                tracing::warn!(
                    agent_id = %state.work_agent_id,
                    "Decision flow failed"
                );
            }
        }

        Ok(())
    }

    /// Poll a running decision slot.
    pub async fn poll_decision_slot(&self, agent_id: &str) -> Result<Option<TickResult>> {
        let mut slots = self.slots.lock().await;
        let state = slots.get_mut(agent_id);

        if state.is_none() {
            return Ok(None);
        }

        let state = state.unwrap();

        if !state.poll_scheduled {
            return Ok(None);
        }

        // Check if ready (session is a Box<dyn Session>)
        if state.slot.is_session_ready() {
            state.poll_scheduled = false;
            let result = state.slot.tick()?;
            self.handle_tick_result(state, result.clone())?;
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Dispatch commands from decision flow to Work Agent.
    fn dispatch_commands(
        &self,
        state: &mut DecisionSlotState,
        commands: Vec<DecisionCommand>,
    ) -> Result<()> {
        for cmd in commands {
            tracing::info!(
                agent_id = %state.work_agent_id,
                command = ?cmd,
                "Dispatching decision command"
            );

            // Interpret each command
            match &cmd {
                DecisionCommand::Agent(AgentCommand::SendInstruction { prompt, target_agent }) => {
                    tracing::debug!(
                        target = %target_agent,
                        prompt_len = prompt.len(),
                        "SendInstruction queued"
                    );
                }
                DecisionCommand::Agent(AgentCommand::WakeUp) => {
                    tracing::debug!("WakeUp command queued");
                }
                DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => {
                    tracing::debug!(
                        prompt_len = prompt.len(),
                        "Reflect command queued"
                    );
                }
                DecisionCommand::Agent(AgentCommand::Terminate { reason }) => {
                    tracing::debug!(
                        reason = %reason,
                        "Terminate command queued"
                    );
                }
                DecisionCommand::Agent(AgentCommand::ApproveAndContinue) => {
                    tracing::debug!("ApproveAndContinue command queued");
                }
                DecisionCommand::Task(TaskCommand::ConfirmCompletion) => {
                    tracing::info!("ConfirmCompletion - task verified complete");
                }
                DecisionCommand::Task(TaskCommand::StopIfComplete { reason }) => {
                    tracing::info!(
                        reason = %reason,
                        "StopIfComplete"
                    );
                }
                DecisionCommand::Task(TaskCommand::PrepareStart { task_id, description }) => {
                    tracing::debug!(
                        task_id = %task_id,
                        "PrepareStart queued"
                    );
                }
                DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
                    tracing::warn!(
                        reason = %reason,
                        context = ?context,
                        "Human escalation required"
                    );
                    // Broadcast escalation event
                    let _ = self.event_tx.send(Event {
                        seq: 0,
                        payload: agent_protocol::events::EventPayload::Error(
                            agent_protocol::events::ErrorData {
                                message: reason.clone(),
                                source: Some(state.work_agent_id.clone()),
                            }
                        ),
                    });
                }
                DecisionCommand::Human(HumanCommand::SelectOption { option_id }) => {
                    tracing::debug!(
                        option_id = %option_id,
                        "SelectOption queued"
                    );
                }
                DecisionCommand::Human(HumanCommand::SkipDecision) => {
                    tracing::debug!("SkipDecision");
                }
                DecisionCommand::Git(_, _) => {
                    tracing::debug!("Git command queued");
                }
                DecisionCommand::Provider(_) => {
                    tracing::debug!("Provider command queued");
                }
            }

            state.pending_commands.push(cmd);
        }

        Ok(())
    }

    /// Get pending commands for an agent.
    pub async fn get_pending_commands(&self, agent_id: &str) -> Vec<DecisionCommand> {
        let slots = self.slots.lock().await;
        slots
            .get(agent_id)
            .map(|s| s.pending_commands.clone())
            .unwrap_or_default()
    }

    /// Clear pending commands for an agent.
    pub async fn clear_pending_commands(&self, agent_id: &str) {
        let mut slots = self.slots.lock().await;
        if let Some(state) = slots.get_mut(agent_id) {
            state.pending_commands.clear();
        }
    }

    /// Detach decision slot from Work Agent.
    pub async fn detach_decision_slot(&self, agent_id: &str) -> Result<()> {
        let mut slots = self.slots.lock().await;
        slots.remove(agent_id);

        tracing::info!(
            agent_id = %agent_id,
            "Decision slot detached"
        );

        Ok(())
    }

    /// Check if decision is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Update decision configuration.
    pub fn update_config(&mut self, config: DecisionIntegrationConfig) {
        self.config = config;
    }
}

/// Command interpreter that routes to Work Agent via session manager.
///
/// This would be used in the full integration to actually send commands
/// to the Work Agent, but for now we store pending commands.
pub struct SessionManagerInterpreter {
    /// Session manager reference (would be used in full integration)
    #[allow(dead_code)]
    session_mgr: Arc<crate::session_mgr::EventLoop>,
}

impl SessionManagerInterpreter {
    pub fn new(session_mgr: Arc<crate::session_mgr::EventLoop>) -> Self {
        Self { session_mgr }
    }
}

impl DecisionCommandInterpreter for SessionManagerInterpreter {
    fn handle_agent_command(&mut self, cmd: AgentCommand) -> Result<()> {
        // In full integration, would call session_mgr.send_input()
        tracing::info!(command = ?cmd, "Agent command received");
        Ok(())
    }

    fn handle_human_command(&mut self, cmd: HumanCommand) -> Result<()> {
        tracing::info!(command = ?cmd, "Human command received");
        Ok(())
    }

    fn handle_task_command(&mut self, cmd: TaskCommand) -> Result<()> {
        tracing::info!(command = ?cmd, "Task command received");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_tree() -> Tree {
        use decision_dsl::ast::document::{Metadata, Spec, Tree, TreeKind};
        use decision_dsl::ast::node::{Node, PromptNode, SetMapping};
        use decision_dsl::ast::parser_out::OutputParser;

        Tree {
            api_version: "decision.agile-agent.io/v1".to_string(),
            kind: TreeKind::BehaviorTree,
            metadata: Metadata {
                name: "test_tree".to_string(),
                description: None,
            },
            spec: Spec {
                root: Node::Prompt(PromptNode {
                    name: "test_prompt".to_string(),
                    model: None,
                    template: "Test: {{ task_description }}".to_string(),
                    parser: OutputParser::Json { schema: None },
                    sets: vec![SetMapping {
                        key: "result".to_string(),
                        field: "decision".to_string(),
                    }],
                    timeout_ms: 5000,
                    pending: false,
                    sent_at: None,
                }),
            },
        }
    }

    fn make_event_tx() -> broadcast::Sender<Event> {
        let (tx, _) = broadcast::channel(100);
        tx
    }

    #[tokio::test]
    async fn decision_integration_new() {
        let config = DecisionIntegrationConfig::default();
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();
        assert!(!integration.is_enabled());
    }

    #[tokio::test]
    async fn decision_integration_enabled() {
        let config = DecisionIntegrationConfig {
            enabled: true,
            ..Default::default()
        };
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();
        assert!(integration.is_enabled());
    }

    #[tokio::test]
    async fn attach_decision_slot_when_disabled() {
        let config = DecisionIntegrationConfig::default(); // disabled
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();

        let result = integration
            .attach_decision_slot("agent-1".to_string(), "task".to_string(), None)
            .await;

        // Should succeed silently when disabled
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn attach_decision_slot_with_template() {
        let config = DecisionIntegrationConfig {
            enabled: true,
            ..Default::default()
        };
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();

        let tree = make_simple_tree();
        let result = integration
            .attach_decision_slot("agent-1".to_string(), "test task".to_string(), Some(tree))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_provider_event_accumulates_output() {
        let config = DecisionIntegrationConfig {
            enabled: true,
            ..Default::default()
        };
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();

        let tree = make_simple_tree();
        integration
            .attach_decision_slot("agent-1".to_string(), "task".to_string(), Some(tree))
            .await
            .unwrap();

        // Accumulate output
        integration
            .handle_provider_event("agent-1", &ProviderEvent::AssistantChunk("output1".to_string()))
            .await
            .unwrap();

        integration
            .handle_provider_event("agent-1", &ProviderEvent::AssistantChunk("output2".to_string()))
            .await
            .unwrap();

        // No tick triggered yet
        let commands = integration.get_pending_commands("agent-1").await;
        assert_eq!(commands.len(), 0);
    }

    #[tokio::test]
    async fn handle_provider_event_finished_triggers_tick() {
        let config = DecisionIntegrationConfig {
            enabled: true,
            ..Default::default()
        };
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();

        // Create slot with replies
        use crate::decision_agent_slot::DecisionAgentSlot;
        use decision_dsl::ast::document::{Metadata, Spec, Tree, TreeKind};
        use decision_dsl::ast::node::{Node, PromptNode, SetMapping};
        use decision_dsl::ast::parser_out::OutputParser;

        let tree = Tree {
            api_version: "decision.agile-agent.io/v1".to_string(),
            kind: TreeKind::BehaviorTree,
            metadata: Metadata {
                name: "test".to_string(),
                description: None,
            },
            spec: Spec {
                root: Node::Prompt(PromptNode {
                    name: "test".to_string(),
                    model: None,
                    template: "Test: {{ task_description }}".to_string(),
                    parser: OutputParser::Json { schema: None },
                    sets: vec![],
                    timeout_ms: 5000,
                    pending: false,
                    sent_at: None,
                }),
            },
        };

        integration
            .attach_decision_slot("agent-1".to_string(), "task".to_string(), Some(tree))
            .await
            .unwrap();

        // Accumulate output
        integration
            .handle_provider_event("agent-1", &ProviderEvent::AssistantChunk("test output".to_string()))
            .await
            .unwrap();

        // Finish triggers tick (returns Running on first tick)
        let triggered = integration
            .handle_provider_event("agent-1", &ProviderEvent::Finished)
            .await
            .unwrap();

        assert!(triggered);
    }

    #[tokio::test]
    async fn detach_decision_slot() {
        let config = DecisionIntegrationConfig {
            enabled: true,
            ..Default::default()
        };
        let event_pump = Arc::new(Mutex::new(EventPump::new()));
        let event_tx = make_event_tx();

        let integration = DecisionIntegration::new(config, event_pump, event_tx).unwrap();

        let tree = make_simple_tree();
        integration
            .attach_decision_slot("agent-1".to_string(), "task".to_string(), Some(tree))
            .await
            .unwrap();

        integration.detach_decision_slot("agent-1").await.unwrap();

        // No pending commands after detach
        let commands = integration.get_pending_commands("agent-1").await;
        assert_eq!(commands.len(), 0);
    }
}