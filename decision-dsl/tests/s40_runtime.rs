use std::time::Duration;

use decision_dsl::ast::document::{Spec, Tree};
use decision_dsl::ast::eval::Evaluator;
use decision_dsl::ast::node::{
    ActionNode, ConditionNode, Node, NodeStatus, ParallelNode, ParallelPolicy, PromptNode,
    RepeaterNode, SelectorNode, SequenceNode, SetVarNode, SubTreeNode, WhenNode,
};
use decision_dsl::ast::parser_out::OutputParser;
use decision_dsl::ast::runtime::{Executor, TickContext, DslRunner};
use decision_dsl::ext::blackboard::{Blackboard, BlackboardValue};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand};
use decision_dsl::ext::error::RuntimeError;
use decision_dsl::ext::traits::{Clock, Logger, MockClock, NullLogger, Session};
use decision_dsl::ext::{SessionError, SessionErrorKind};

// ── Mock Session ────────────────────────────────────────────────────────────

struct MockSession {
    ready: bool,
    reply: Option<String>,
    sent: Vec<String>,
}

impl MockSession {
    fn new() -> Self {
        Self {
            ready: false,
            reply: None,
            sent: Vec::new(),
        }
    }

    fn with_reply(reply: impl Into<String>) -> Self {
        Self {
            ready: true,
            reply: Some(reply.into()),
            sent: Vec::new(),
        }
    }

    fn set_ready(&mut self, reply: impl Into<String>) {
        self.ready = true;
        self.reply = Some(reply.into());
    }
}

impl Session for MockSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent.push(message.into());
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        self.ready = false;
        self.reply.take().ok_or(SessionError {
            kind: SessionErrorKind::UnexpectedFormat,
            message: "no reply".into(),
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn simple_tree(root: Node) -> Tree {
    Tree {
        api_version: "1.0".into(),
        kind: decision_dsl::ast::document::TreeKind::BehaviorTree,
        metadata: decision_dsl::ast::document::Metadata {
            name: "test".into(),
            description: None,
        },
        spec: Spec { root },
    }
}

fn tick_ctx<'a>(
    bb: &'a mut Blackboard,
    session: &'a mut dyn Session,
    clock: &'a dyn Clock,
    logger: &'a dyn Logger,
) -> TickContext<'a> {
    TickContext {
        blackboard: bb,
        session,
        clock,
        logger,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.1: Executor & Tick Loop
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn executor_tick_condition_true_returns_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Condition(ConditionNode {
        name: "c".into(),
        evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Success);
    assert!(result.commands.is_empty());
}

#[test]
fn executor_tick_condition_false_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Condition(ConditionNode {
        name: "c".into(),
        evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn executor_tick_action_drains_command() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Action(ActionNode {
        name: "a".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    assert!(matches!(result.commands[0], DecisionCommand::Agent(AgentCommand::WakeUp)));
}

#[test]
fn executor_reset_clears_running_state() {
    let mut executor = Executor::new();
    executor.reset();
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.2: Composite Nodes
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn selector_first_success_wins() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
        ],
        active_child: None,
        rule_name: None,
        rule_priority: None,
        matched: false,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn selector_all_fail_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
        ],
        active_child: None,
        rule_name: None,
        rule_priority: None,
        matched: false,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn sequence_all_success_returns_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn sequence_first_failure_aborts() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
            Node::Action(ActionNode {
                name: "a1".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    assert!(result.commands.is_empty());
}

#[test]
fn parallel_all_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn parallel_any_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AnySuccess,
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn parallel_majority() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::Majority,
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c3".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.4: Leaf Nodes
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn condition_evaluator_true() {
    let mut bb = Blackboard::default();
    bb.provider_output = "error".into();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Condition(ConditionNode {
        name: "c".into(),
        evaluator: Evaluator::OutputContains {
            pattern: "error".into(),
            case_sensitive: true,
        },
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn condition_evaluator_false() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Condition(ConditionNode {
        name: "c".into(),
        evaluator: Evaluator::OutputContains {
            pattern: "error".into(),
            case_sensitive: true,
        },
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn action_with_when_guard_false() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Action(ActionNode {
        name: "a".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: Some(Evaluator::Script { expression: r#"provider_output == "x""#.into() }),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    assert!(result.commands.is_empty());
}

#[test]
fn setvar_writes_to_blackboard() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::SetVar(SetVarNode {
        name: "set".into(),
        key: "my_var".into(),
        value: BlackboardValue::String("hello".into()),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(ctx.blackboard.get("my_var"), Some(&BlackboardValue::String("hello".into())));
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.5: Prompt Node Async Lifecycle
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn prompt_first_tick_returns_running() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Hello".into(),
        parser: OutputParser::Json { schema: None },
        sets: vec![],
        timeout_ms: 1000,
        pending: false,
        sent_at: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Running);
    assert_eq!(session.sent.len(), 1);
    assert_eq!(session.sent[0], "Hello");
}

#[test]
fn prompt_second_tick_success_when_ready() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::with_reply(r#"{"status": "ok"}"#);
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Hello".into(),
        parser: OutputParser::Json { schema: None },
        sets: vec![],
        timeout_ms: 1000,
        pending: true,
        sent_at: Some(clock.now()),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn prompt_timeout_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let mut clock = MockClock::new();
    clock.advance(Duration::from_millis(2000));
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let sent_at = clock.now() - Duration::from_millis(2000);
    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Hello".into(),
        parser: OutputParser::Json { schema: None },
        sets: vec![],
        timeout_ms: 1000,
        pending: true,
        sent_at: Some(sent_at),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.6: SubTree Node & Scope Isolation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn subtree_unresolved_returns_error() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::SubTree(SubTreeNode {
        name: "st".into(),
        ref_name: "missing".into(),
        resolved_root: None,
    }));

    let mut executor = Executor::new();
    let err = executor.tick(&mut tree, &mut ctx).unwrap_err();
    assert!(matches!(err, RuntimeError::SubTreeNotResolved { .. }));
}

#[test]
fn subtree_resolved_executes_child() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::SubTree(SubTreeNode {
        name: "st".into(),
        ref_name: "child".into(),
        resolved_root: Some(Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        }))),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Story 4.1: Resume / Running path
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn executor_resume_running_path() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Prompt(PromptNode {
                name: "p".into(),
                model: None,
                template: "Ask".into(),
                parser: OutputParser::Json { schema: None },
                sets: vec![],
                timeout_ms: 1000,
                pending: false,
                sent_at: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: Prompt sends message, returns Running
    let result = {
        let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
        executor.tick(&mut tree, &mut ctx).unwrap()
    };
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: session not ready, still Running
    let result = {
        let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
        executor.tick(&mut tree, &mut ctx).unwrap()
    };
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 3: session ready with reply
    session.set_ready(r#"{"status": "ok"}"#);
    let result = {
        let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
        executor.tick(&mut tree, &mut ctx).unwrap()
    };
    assert_eq!(result.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Bug Fix Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn selector_with_rule_name_does_not_execute_skipped_children() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    // First child succeeds, second should be skipped without executing
    let mut tree = simple_tree(Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Action(ActionNode {
                name: "a2".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
        ],
        active_child: None,
        rule_name: Some("test_rule".into()),
        rule_priority: Some(1),
        matched: false,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    // The second child (Action) should NOT have been executed
    assert_eq!(result.commands.len(), 0);
    // Should have RuleSkipped trace for second child
    let skipped = result.trace.iter().any(|e| matches!(e, decision_dsl::ast::runtime::TraceEntry::RuleSkipped { .. }));
    assert!(skipped);
}

#[test]
fn repeater_retries_on_failure_until_max_attempts() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into(); // Will fail each time
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
        current: 0,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Should fail after 3 attempts, not immediately
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn repeater_succeeds_on_success_within_attempts() {
    let mut bb = Blackboard::default();
    bb.provider_output = "".into(); // Will succeed
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
        current: 0,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Should succeed after 3 successful attempts
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn resume_when_rechecks_condition() {
    let mut bb = Blackboard::default();
    bb.set("flag", BlackboardValue::Boolean(true));
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::When(WhenNode {
        name: "w".into(),
        condition: Evaluator::VariableIs {
            key: "flag".into(),
            expected: BlackboardValue::Boolean(true),
        },
        action: Box::new(Node::Prompt(PromptNode {
            name: "prompt".into(),
            model: None,
            template: "Hello".into(),
            parser: OutputParser::Json { schema: None },
            sets: vec![],
            timeout_ms: 1000,
            pending: true,
            sent_at: Some(clock.now()),
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Drop ctx to release borrow of bb, then change condition
    drop(ctx);
    bb.set("flag", BlackboardValue::Boolean(false));
    // Create new ctx for resume tick
    let mut ctx2 = tick_ctx(&mut bb, &mut session, &clock, &logger);
    // Resume - should fail because condition changed
    let result = executor.tick(&mut tree, &mut ctx2).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn parallel_ticks_all_children_concurrently() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![
            Node::Action(ActionNode {
                name: "a1".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
            Node::Action(ActionNode {
                name: "a2".into(),
                command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Both actions should execute
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 2);
}

#[test]
fn parallel_all_success_with_one_running() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new(); // Not ready, so Prompt returns Running
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Prompt(PromptNode {
                name: "prompt".into(),
                model: None,
                template: "Hello".into(),
                parser: OutputParser::Json { schema: None },
                sets: vec![],
                timeout_ms: 1000,
                pending: false,
                sent_at: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // First child succeeds, second is Running - overall Running (AllSuccess requires all)
    assert_eq!(result.status, NodeStatus::Running);
}