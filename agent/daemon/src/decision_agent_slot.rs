//! Decision Agent Slot — integrates behavior tree executor with Work Agent
//!
//! Runs decision flow behavior trees, syncing Work Agent output to Blackboard,
//! executing AI decisions via Session trait, and dispatching DecisionCommands.

use std::path::PathBuf;

use anyhow::{Context, Result};
use decision_dsl::ast::document::Tree;
use decision_dsl::ast::runtime::{DslRunner, Executor, TickContext, TickResult};
use decision_dsl::ext::blackboard::{Blackboard, DecisionEntry, ReflectionEntry, SprintGoal};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand, TaskCommand};
use decision_dsl::ext::session_impl::{InMemorySession, ProviderSession};
use decision_dsl::ext::traits::{NullLogger, Session, SystemClock};

/// Configuration for the Decision Agent Slot.
#[derive(Debug, Clone)]
pub struct DecisionSlotConfig {
    /// Provider kind (claude, codex)
    pub provider_kind: String,
    /// Working directory for provider
    pub cwd: PathBuf,
    /// Maximum reflection rounds
    pub max_reflection_rounds: u8,
    /// Total sprints in workflow
    pub total_sprints: u8,
    /// Sprint goals for multi-sprint workflows
    pub sprint_goals: Vec<SprintGoal>,
}

impl Default for DecisionSlotConfig {
    fn default() -> Self {
        Self {
            provider_kind: "claude".to_string(),
            cwd: PathBuf::from("."),
            max_reflection_rounds: 2,
            total_sprints: 1,
            sprint_goals: Vec::new(),
        }
    }
}

/// Decision Agent Slot — executes decision flow behavior trees.
///
/// Integration point between behavior tree executor and Work Agent:
/// - Syncs Work Agent output to Blackboard
/// - Executes behavior tree with Session for AI decisions
/// - Dispatches DecisionCommands to Work Agent controller
pub struct DecisionAgentSlot {
    /// Behavior tree to execute
    tree: Tree,

    /// Tree executor (handles Running resume)
    executor: Executor,

    /// Decision session (AI connection)
    session: Box<dyn Session + Send>,

    /// Blackboard (decision state + Work Agent output)
    blackboard: Blackboard,

    /// Clock for timeouts
    clock: SystemClock,

    /// Configuration (for reference)
    #[allow(dead_code)]
    config: DecisionSlotConfig,

    /// Target Work Agent ID
    work_agent_id: String,
}

impl DecisionAgentSlot {
    /// Create a new Decision Agent Slot with a real provider session.
    pub fn new(tree: Tree, config: DecisionSlotConfig) -> Result<Self> {
        let session: Box<dyn Session + Send> = Box::new(
            ProviderSession::new(&config.provider_kind, config.cwd.clone())
                .with_max_history(10)
        );

        let blackboard = Blackboard::new();
        let executor = Executor::new();
        let clock = SystemClock;

        Ok(Self {
            tree,
            executor,
            session,
            blackboard,
            clock,
            config,
            work_agent_id: String::new(),
        })
    }

    /// Create a Decision Agent Slot with a mock session for testing.
    pub fn with_mock_session(tree: Tree, config: DecisionSlotConfig) -> Self {
        let session: Box<dyn Session + Send> = Box::new(InMemorySession::new());

        let mut blackboard = Blackboard::new();
        blackboard.max_reflection_rounds = config.max_reflection_rounds;
        blackboard.set_sprint_config(config.total_sprints, config.sprint_goals.clone());

        Self {
            tree,
            executor: Executor::new(),
            session,
            blackboard,
            clock: SystemClock,
            config,
            work_agent_id: String::new(),
        }
    }

    /// Create with pre-programmed responses for testing.
    pub fn with_replies(tree: Tree, config: DecisionSlotConfig, replies: Vec<String>) -> Self {
        let session: Box<dyn Session + Send> = Box::new(InMemorySession::with_replies(replies));

        let mut blackboard = Blackboard::new();
        blackboard.max_reflection_rounds = config.max_reflection_rounds;
        blackboard.set_sprint_config(config.total_sprints, config.sprint_goals.clone());

        Self {
            tree,
            executor: Executor::new(),
            session,
            blackboard,
            clock: SystemClock,
            config,
            work_agent_id: String::new(),
        }
    }

    /// Set target Work Agent ID for SendInstruction commands.
    pub fn set_work_agent_id(&mut self, agent_id: impl Into<String>) {
        self.work_agent_id = agent_id.into();
        self.blackboard.work_agent_id = self.work_agent_id.clone();
    }

    /// Set task description for decision context.
    pub fn set_task_description(&mut self, description: impl Into<String>) {
        self.blackboard.task_description = description.into();
    }

    /// Set agent ID for decision context.
    pub fn set_agent_id(&mut self, agent_id: impl Into<String>) {
        self.blackboard.agent_id = agent_id.into();
    }

    /// Check if the session has a response ready.
    pub fn is_session_ready(&self) -> bool {
        self.session.is_ready()
    }

    /// Sync Work Agent output to Blackboard.
    pub fn sync_work_agent_output(&mut self, output: impl Into<String>) {
        self.blackboard.provider_output = output.into();
    }

    /// Sync file changes to Blackboard.
    pub fn sync_file_changes(&mut self, changes: Vec<(String, String)>) {
        use decision_dsl::ext::blackboard::FileChangeRecord;
        self.blackboard.file_changes = changes
            .into_iter()
            .map(|(path, change_type)| FileChangeRecord { path, change_type })
            .collect();
    }

    /// Get current sprint number.
    pub fn current_sprint(&self) -> u8 {
        self.blackboard.current_sprint
    }

    /// Check if all sprints are completed.
    pub fn is_all_sprints_completed(&self) -> bool {
        self.blackboard.is_all_sprints_completed()
    }

    /// Get pending commands (not yet dispatched).
    pub fn pending_commands(&self) -> &[DecisionCommand] {
        &self.blackboard.commands
    }

    /// Get reflection chain history.
    pub fn reflection_chain(&self) -> &[ReflectionEntry] {
        &self.blackboard.reflection_chain
    }

    /// Get decision chain history.
    pub fn decision_chain(&self) -> &[DecisionEntry] {
        &self.blackboard.decision_chain
    }

    /// Execute one decision cycle (tick the behavior tree).
    ///
    /// Returns TickResult with status and commands to dispatch.
    pub fn tick(&mut self) -> Result<TickResult> {
        let logger = NullLogger;

        let mut ctx = TickContext::new(
            &mut self.blackboard,
            self.session.as_mut(),
            &self.clock,
            &logger,
        );

        let result = self.executor
            .tick(&mut self.tree, &mut ctx)
            .context("behavior tree tick failed");

        result
    }

    /// Drain and return commands generated by the decision flow.
    pub fn drain_commands(&mut self) -> Vec<DecisionCommand> {
        self.blackboard.drain_commands()
    }

    /// Reset the executor for a fresh decision cycle.
    pub fn reset(&mut self) {
        self.executor.reset();
    }

    /// Clear all decision state for a new workflow.
    pub fn clear(&mut self) {
        self.executor.reset();
        self.blackboard.reflection_chain.clear();
        self.blackboard.decision_chain.clear();
        self.blackboard.current_sprint = 1;
        self.blackboard.commands.clear();
    }
}

// ── Command Interpreter ──────────────────────────────────────────────────────

/// Interpret DecisionCommand into concrete actions.
///
/// This trait allows different backends (daemon, CLI, mock) to handle
/// commands differently.
pub trait DecisionCommandInterpreter {
    /// Handle an AgentCommand.
    fn handle_agent_command(&mut self, cmd: AgentCommand) -> Result<()>;

    /// Handle a HumanCommand (escalation).
    fn handle_human_command(&mut self, cmd: HumanCommand) -> Result<()>;

    /// Handle a TaskCommand.
    fn handle_task_command(&mut self, cmd: TaskCommand) -> Result<()>;

    /// Handle any DecisionCommand.
    fn interpret(&mut self, cmd: DecisionCommand) -> Result<()> {
        match cmd {
            DecisionCommand::Agent(agent_cmd) => self.handle_agent_command(agent_cmd),
            DecisionCommand::Human(human_cmd) => self.handle_human_command(human_cmd),
            DecisionCommand::Task(task_cmd) => self.handle_task_command(task_cmd),
            DecisionCommand::Git(_, _) => Ok(()), // Git commands handled separately
            DecisionCommand::Provider(_) => Ok(()), // Provider commands handled separately
        }
    }
}

/// Mock interpreter for testing.
pub struct MockCommandInterpreter {
    pub agent_commands: Vec<AgentCommand>,
    pub human_commands: Vec<HumanCommand>,
    pub task_commands: Vec<TaskCommand>,
}

impl MockCommandInterpreter {
    pub fn new() -> Self {
        Self {
            agent_commands: Vec::new(),
            human_commands: Vec::new(),
            task_commands: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.agent_commands.clear();
        self.human_commands.clear();
        self.task_commands.clear();
    }
}

impl Default for MockCommandInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionCommandInterpreter for MockCommandInterpreter {
    fn handle_agent_command(&mut self, cmd: AgentCommand) -> Result<()> {
        self.agent_commands.push(cmd);
        Ok(())
    }

    fn handle_human_command(&mut self, cmd: HumanCommand) -> Result<()> {
        self.human_commands.push(cmd);
        Ok(())
    }

    fn handle_task_command(&mut self, cmd: TaskCommand) -> Result<()> {
        self.task_commands.push(cmd);
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use decision_dsl::ast::document::{Metadata, Spec, Tree, TreeKind};
    use decision_dsl::ast::node::{Node, NodeStatus, PromptNode, SetMapping};
    use decision_dsl::ast::parser_out::OutputParser;

    fn make_simple_tree() -> Tree {
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
                    template: "Test prompt: {{ task_description }}".to_string(),
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

    fn make_config() -> DecisionSlotConfig {
        DecisionSlotConfig {
            provider_kind: "mock".to_string(),
            cwd: PathBuf::from("/tmp"),
            max_reflection_rounds: 2,
            total_sprints: 2,
            sprint_goals: vec![
                SprintGoal::new(1, "first sprint"),
                SprintGoal::new(2, "second sprint"),
            ],
        }
    }

    #[test]
    fn decision_slot_new_creates_slot() {
        let tree = make_simple_tree();
        let config = make_config();
        let slot = DecisionAgentSlot::new(tree, config).unwrap();

        assert_eq!(slot.current_sprint(), 1);
        assert!(!slot.is_all_sprints_completed());
    }

    #[test]
    fn decision_slot_with_mock_session() {
        let tree = make_simple_tree();
        let config = make_config();
        let slot = DecisionAgentSlot::with_mock_session(tree, config);

        assert_eq!(slot.current_sprint(), 1);
        assert_eq!(slot.blackboard.max_reflection_rounds, 2);
        assert_eq!(slot.blackboard.total_sprints, 2);
    }

    #[test]
    fn decision_slot_with_replies() {
        let tree = make_simple_tree();
        let config = make_config();
        let replies = vec![
            r#"{"decision": "proceed", "confidence": 0.8}"#.to_string(),
        ];
        let slot = DecisionAgentSlot::with_replies(tree, config, replies);

        assert_eq!(slot.current_sprint(), 1);
    }

    #[test]
    fn decision_slot_set_work_agent_id() {
        let tree = make_simple_tree();
        let config = make_config();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        slot.set_work_agent_id("agent-123");
        assert_eq!(slot.work_agent_id, "agent-123");
        assert_eq!(slot.blackboard.work_agent_id, "agent-123");
    }

    #[test]
    fn decision_slot_set_task_description() {
        let tree = make_simple_tree();
        let config = make_config();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        slot.set_task_description("test task");
        assert_eq!(slot.blackboard.task_description, "test task");
    }

    #[test]
    fn decision_slot_sync_work_agent_output() {
        let tree = make_simple_tree();
        let config = make_config();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        slot.sync_work_agent_output("work output");
        assert_eq!(slot.blackboard.provider_output, "work output");
    }

    #[test]
    fn decision_slot_sync_file_changes() {
        let tree = make_simple_tree();
        let config = make_config();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        slot.sync_file_changes(vec![
            ("file1.rs".to_string(), "modified".to_string()),
            ("file2.rs".to_string(), "added".to_string()),
        ]);

        assert_eq!(slot.blackboard.file_changes.len(), 2);
        assert_eq!(slot.blackboard.file_changes[0].path, "file1.rs");
    }

    #[test]
    fn decision_slot_tick_with_mock_reply() {
        let tree = make_simple_tree();
        let config = DecisionSlotConfig::default();
        let replies = vec![
            r#"{"decision": "proceed", "confidence": 0.8}"#.to_string(),
        ];
        let mut slot = DecisionAgentSlot::with_replies(tree, config, replies);

        slot.set_task_description("test");

        // First tick sends prompt, returns Running
        let result = slot.tick().unwrap();
        assert_eq!(result.status, NodeStatus::Running);

        // Second tick receives response, returns Success
        let result = slot.tick().unwrap();
        assert_eq!(result.status, NodeStatus::Success);
    }

    #[test]
    fn decision_slot_drain_commands() {
        let tree = make_simple_tree();
        let config = DecisionSlotConfig::default();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        // Commands would be generated by tree execution
        slot.blackboard.push_command(DecisionCommand::Agent(AgentCommand::WakeUp));

        let commands = slot.drain_commands();
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0], DecisionCommand::Agent(AgentCommand::WakeUp)));

        // Commands are drained
        assert_eq!(slot.pending_commands().len(), 0);
    }

    #[test]
    fn decision_slot_reset() {
        let tree = make_simple_tree();
        let config = DecisionSlotConfig::default();
        let replies = vec![r#"{"decision": "proceed"}"#.to_string()];
        let mut slot = DecisionAgentSlot::with_replies(tree, config, replies);

        slot.set_task_description("test");
        slot.tick().unwrap(); // Running
        slot.tick().unwrap(); // Success

        // Reset for fresh cycle
        slot.reset();
        // Executor should be ready for new cycle
    }

    #[test]
    fn decision_slot_clear() {
        let tree = make_simple_tree();
        let config = make_config();
        let mut slot = DecisionAgentSlot::with_mock_session(tree, config);

        slot.blackboard.push_reflection(ReflectionEntry::new(1, "proceed", "test"));
        slot.blackboard.advance_sprint();

        slot.clear();

        assert_eq!(slot.reflection_chain().len(), 0);
        assert_eq!(slot.decision_chain().len(), 0);
        assert_eq!(slot.current_sprint(), 1);
    }

    #[test]
    fn mock_interpreter_records_commands() {
        let mut interpreter = MockCommandInterpreter::new();

        interpreter.interpret(DecisionCommand::Agent(AgentCommand::WakeUp)).unwrap();
        interpreter.interpret(DecisionCommand::Human(HumanCommand::Escalate {
            reason: "test".to_string(),
            context: None,
        })).unwrap();

        assert_eq!(interpreter.agent_commands.len(), 1);
        assert_eq!(interpreter.human_commands.len(), 1);
    }
}