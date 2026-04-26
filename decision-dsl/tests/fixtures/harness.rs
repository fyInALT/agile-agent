#![allow(dead_code)]

//! Integration test harness for decision-dsl behavior trees.
//!
//! Provides a high-level API for writing scenario-based integration tests
//! that exercise the full decision pipeline: YAML parse → desugar → tick
//! loop → command output.

use decision_dsl::ast::document::Tree;
use decision_dsl::ast::eval::EvaluatorRegistry;
use decision_dsl::ast::parser::{DslParser, YamlParser};
use decision_dsl::ast::runtime::{DslRunner, Executor, TickContext, TickResult, TraceEntry};
use decision_dsl::ext::blackboard::Blackboard;
use decision_dsl::ext::command::DecisionCommand;
use decision_dsl::ext::traits::{Logger, MockClock, NullLogger};

use super::mock_llm::MockLlm;

// ── IntegrationHarness ───────────────────────────────────────────────────────

/// High-level harness for running behavior tree integration tests.
///
/// # Example
///
/// ```rust
/// let harness = IntegrationHarness::new()
///     .with_yaml_tree(YAML)
///     .with_blackboard(|bb| bb.provider_output = "error".into());
///
/// let result = harness.tick_until_complete(10).unwrap();
/// harness.assert_commands_contain(|cmd| matches!(cmd, DecisionCommand::Human(_)));
/// ```
pub struct IntegrationHarness<'a> {
    pub tree: Option<Tree>,
    pub blackboard: Blackboard,
    pub llm: MockLlm,
    pub clock: MockClock,
    pub logger: &'a dyn Logger,
    pub executor: Executor,
}

impl<'a> IntegrationHarness<'a> {
    /// Create a new harness with default state.
    pub fn new() -> Self {
        Self {
            tree: None,
            blackboard: Blackboard::default(),
            llm: MockLlm::new(),
            clock: MockClock::new(),
            logger: &NullLogger,
            executor: Executor::new(),
        }
    }

    /// Parse a behavior tree from YAML and set it as the active tree.
    pub fn with_yaml_tree(mut self, yaml: &str) -> Self {
        let parser = YamlParser::new();
        let doc = parser.parse_document(yaml).expect("YAML parse failed");
        let registry = EvaluatorRegistry::new();
        let tree = doc.desugar(&registry).expect("desugar failed");
        self.tree = Some(tree);
        self
    }

    /// Configure the blackboard with a closure before running.
    pub fn with_blackboard<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut Blackboard),
    {
        f(&mut self.blackboard);
        self
    }

    /// Configure the Mock LLM with scenarios.
    pub fn with_llm(mut self, llm: MockLlm) -> Self {
        self.llm = llm;
        self
    }

    /// Set a custom logger.
    pub fn with_logger(mut self, logger: &'a dyn Logger) -> Self {
        self.logger = logger;
        self
    }

    /// Advance the mock clock by a duration.
    pub fn advance_clock(&mut self, duration: std::time::Duration) {
        self.clock.advance(duration);
    }

    /// Execute a single tick against the behavior tree.
    ///
    /// Panics if no tree has been set.
    pub fn tick(&mut self) -> TickResult {
        let tree = self.tree.as_mut().expect("no tree set; call with_yaml_tree() first");
        // Advance LLM internal tick counter for delay simulation
        self.llm.advance_tick();
        let mut ctx = TickContext::new(
            &mut self.blackboard,
            &mut self.llm,
            &self.clock,
            self.logger,
        );
        self.executor.tick(tree, &mut ctx).expect("tick failed")
    }

    /// Tick until the tree reaches a terminal state (Success or Failure),
    /// or until `max_ticks` is exceeded.
    ///
    /// Returns the final `TickResult` on success, or panics if max_ticks
    /// is exceeded while still Running.
    pub fn tick_until_complete(&mut self, max_ticks: usize) -> TickResult {
        for _ in 0..max_ticks {
            let result = self.tick();
            if !matches!(result.status, decision_dsl::ast::node::NodeStatus::Running) {
                return result;
            }
        }
        panic!(
            "tree did not complete within {} ticks. sent_messages={:?}",
            max_ticks,
            self.llm.sent_messages()
        );
    }

    /// Tick a specific number of times, returning the last result.
    pub fn tick_n(&mut self, n: usize) -> TickResult {
        let mut last = None;
        for _ in 0..n {
            last = Some(self.tick());
        }
        last.expect("tick_n called with n=0")
    }

    // ── Assertions ───────────────────────────────────────────────────────────

    /// Assert that the last tick produced at least one command matching
    /// the given predicate.
    pub fn assert_commands_contain<F>(&self, result: &TickResult, predicate: F)
    where
        F: Fn(&DecisionCommand) -> bool,
    {
        assert!(
            result.commands.iter().any(predicate),
            "expected at least one matching command, got {:?}",
            result.commands
        );
    }

    /// Assert that the last tick produced exactly N commands.
    pub fn assert_command_count(&self, result: &TickResult, expected: usize) {
        assert_eq!(
            result.commands.len(),
            expected,
            "expected {} commands, got {:?}",
            expected,
            result.commands
        );
    }

    /// Assert that the trace contains at least one entry matching the predicate.
    pub fn assert_trace_contains<F>(&self, result: &TickResult, predicate: F)
    where
        F: Fn(&TraceEntry) -> bool,
    {
        assert!(
            result.trace.iter().any(predicate),
            "expected trace to contain matching entry. trace entries: {:?}",
            result.trace
        );
    }

    /// Assert that a specific prompt was sent to the LLM.
    pub fn assert_prompt_sent(&self, expected_substring: &str) {
        let sent = self.llm.sent_messages();
        assert!(
            sent.iter().any(|msg| msg.contains(expected_substring)),
            "expected prompt containing '{}' to be sent. sent messages: {:?}",
            expected_substring,
            sent
        );
    }

    /// Assert that NO prompt containing the substring was sent.
    pub fn assert_prompt_not_sent(&self, unexpected_substring: &str) {
        let sent = self.llm.sent_messages();
        assert!(
            !sent.iter().any(|msg| msg.contains(unexpected_substring)),
            "did NOT expect prompt containing '{}'. sent messages: {:?}",
            unexpected_substring,
            sent
        );
    }

    /// Assert that the blackboard contains a variable with the given value.
    pub fn assert_blackboard_has(&self, key: &str, expected: &str) {
        let actual = self
            .blackboard
            .get(key)
            .map(|v| format!("{:?}", v))
            .unwrap_or_else(|| "<missing>".into());
        assert_eq!(
            actual, expected,
            "blackboard key '{}' expected '{}', got '{}'",
            key, expected, actual
        );
    }

    /// Assert that the blackboard does NOT contain a variable.
    pub fn assert_blackboard_missing(&self, key: &str) {
        assert!(
            self.blackboard.get(key).is_none(),
            "expected blackboard key '{}' to be absent",
            key
        );
    }
}

impl Default for IntegrationHarness<'_> {
    fn default() -> Self {
        Self::new()
    }
}

// ── TestDslRunner ───────────────────────────────────────────────────────────

/// A test-specific runner that records every tick result for post-hoc analysis.
pub struct RecordingRunner {
    pub executor: Executor,
    pub history: Vec<TickResult>,
}

impl RecordingRunner {
    pub fn new() -> Self {
        Self {
            executor: Executor::new(),
            history: Vec::new(),
        }
    }

    pub fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, decision_dsl::ext::error::RuntimeError> {
        let result = self.executor.tick(tree, ctx)?;
        self.history.push(result.clone());
        Ok(result)
    }

    /// Find the first tick that produced a command matching the predicate.
    pub fn find_tick_with_command<F>(&self, predicate: F) -> Option<&TickResult>
    where
        F: Fn(&DecisionCommand) -> bool,
    {
        self.history.iter().find(|r| r.commands.iter().any(|c| predicate(c)))
    }

    /// Total number of Running ticks before completion.
    pub fn running_tick_count(&self) -> usize {
        self.history
            .iter()
            .filter(|r| matches!(r.status, decision_dsl::ast::node::NodeStatus::Running))
            .count()
    }
}

impl Default for RecordingRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ── Scenario presets for common integration test patterns ────────────────────

/// Build a MockLlm that approves any prompt immediately.
pub fn llm_always_approves() -> MockLlm {
    use super::presets::Preset;
    use super::mock_llm::{Scenario, PromptMatcher, ResponseStrategy};
    MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::Preset(Preset::CodexApprove),
        "always approves",
    ))
}

/// Build a MockLlm that escalates any prompt to human.
pub fn llm_always_escalates() -> MockLlm {
    use super::presets::Preset;
    use super::mock_llm::{Scenario, PromptMatcher, ResponseStrategy};
    MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::Preset(Preset::ClaudeEscalate { reason: "integration test" }),
        "always escalates",
    ))
}

/// Build a MockLlm that simulates a delayed Codex response.
pub fn llm_delayed_approval(delay_ticks: usize) -> MockLlm {
    use super::presets::Preset;
    use super::mock_llm::{Scenario, PromptMatcher, ResponseStrategy};
    MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::AfterTicks(delay_ticks, Preset::CodexApprove.render()),
        "delayed approval",
    ))
}
