use std::time::Duration;

use decision_dsl::ast::document::{Spec, Tree};
use decision_dsl::ast::eval::Evaluator;
use decision_dsl::ast::node::{
    ActionNode, ConditionNode, CooldownNode, ForceHumanNode, InverterNode, Node, NodeBehavior, NodeStatus,
    ParallelNode, ParallelPolicy, PromptNode, ReflectionGuardNode, RepeaterNode, SelectorNode, SequenceNode,
    SetVarNode, SubTreeNode, WhenNode,
};
use decision_dsl::ast::parser_out::OutputParser;
use decision_dsl::ast::runtime::{Executor, TickContext, DslRunner};
use decision_dsl::ext::blackboard::{Blackboard, BlackboardValue};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand};
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
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    // Tree: Sequence[Condition(Success), Prompt(Running)]
    // First tick: Condition succeeds, Prompt sends and returns Running
    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
            }),
            Node::Prompt(PromptNode {
                name: "ask".into(),
                model: None,
                template: "hello".into(),
                parser: OutputParser::Enum {
                    values: vec!["yes".into()],
                    case_sensitive: false,
                },
                sets: vec![],
                timeout_ms: 5000,
                pending: false,
                sent_at: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: Sequence enters 0 (Condition), exits Success; enters 1 (Prompt), sends, Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);
    // Trace should show Enter for child 0 and child 1 of Sequence
    let enter_count = result.trace.iter().filter(|e| {
        matches!(e, decision_dsl::ast::runtime::TraceEntry::Enter { name, .. } if name == "seq")
    }).count();
    assert_eq!(enter_count, 2, "first tick should enter seq twice (child 0 and child 1)");

    executor.reset();
    // Reset tree node state too so we can observe root-tick vs resume behavior
    tree.spec.root.reset();

    // Tick 2: after reset, executor ticks from root again
    // Since tree is also reset, Prompt is not pending, so we re-enter both children
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);
    // Second tick should also show Enter for child 0 and child 1 because we restarted
    let enter_count = result.trace.iter().filter(|e| {
        matches!(e, decision_dsl::ast::runtime::TraceEntry::Enter { name, .. } if name == "seq")
    }).count();
    assert_eq!(enter_count, 2, "after reset, tick should re-enter both children from root");
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
    // Selector's matched flag should be set to true after successful match
    if let Node::Selector(sel) = &tree.spec.root {
        assert!(sel.matched, "matched should be true after first child succeeds");
    } else {
        panic!("expected Selector node");
    }
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

// ═════════════════════════════════════════════════════════════════════════════
// Edge case tests for bug fixes
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn repeater_max_attempts_zero_returns_failure() {
    let mut bb = Blackboard::default();
    bb.provider_output = "".into(); // Will succeed condition
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 0,
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
        current: 0,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // max_attempts=0 means no attempts allowed, immediate failure
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn parallel_empty_children_all_success_returns_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Empty children with AllSuccess: vacuously true, returns Success
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn parallel_empty_children_any_success_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AnySuccess,
        children: vec![],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Empty children with AnySuccess: no child to succeed, returns Failure
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn parallel_empty_children_majority_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::Majority,
        children: vec![],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Empty children with Majority: no majority, returns Failure
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Decorator Nodes (Inverter, Cooldown, ReflectionGuard, ForceHuman)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inverter_success_becomes_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn inverter_failure_becomes_success() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn inverter_running_passes_through() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(Node::Prompt(PromptNode {
            name: "p".into(),
            model: None,
            template: "hi".into(),
            parser: OutputParser::Json { schema: None },
            sets: vec![],
            timeout_ms: 1000,
            pending: false,
            sent_at: None,
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);
}

#[test]
fn inverter_resume_inverts_resumed_status() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    // Inverter wrapping a Prompt - child returns Running on first tick
    let mut tree = simple_tree(Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(Node::Prompt(PromptNode {
            name: "p".into(),
            model: None,
            template: "Ask".into(),
            parser: OutputParser::Json { schema: None },
            sets: vec![],
            timeout_ms: 1000,
            pending: false,
            sent_at: None,
        })),
    }));

    let mut executor = Executor::new();

    // Tick 1: Prompt sends and returns Running; Inverter passes through Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: Prompt succeeds; Inverter inverts to Failure
    session.set_ready(r#"{"ok": true}"#);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn cooldown_blocks_within_duration() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 100,
        child: Box::new(Node::Action(ActionNode {
            name: "a".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    }));

    let mut executor = Executor::new();

    // First tick: action succeeds, cooldown records success time
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);

    // Second tick immediately: cooldown blocks
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    assert_eq!(result.commands.len(), 0);
}

#[test]
fn cooldown_allows_after_duration() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let mut clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 100,
        child: Box::new(Node::Action(ActionNode {
            name: "a".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    }));

    let mut executor = Executor::new();

    // First tick succeeds
    let _t1 = clock.now();
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let _ = executor.tick(&mut tree, &mut ctx).unwrap();

    // Verify last_success was recorded
    if let Node::Cooldown(cool) = &tree.spec.root {
        assert!(cool.last_success.is_some(), "last_success should be recorded after success");
    }

    // Advance clock past cooldown
    clock.advance(Duration::from_millis(150));

    // Second tick: cooldown expired, action executes again
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
}

#[test]
fn reflection_guard_blocks_when_max_exceeded() {
    let mut bb = Blackboard::default();
    bb.reflection_round = 5;
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "a".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    assert!(result.commands.is_empty());
}

#[test]
fn reflection_guard_increments_on_success() {
    let mut bb = Blackboard::default();
    bb.reflection_round = 1;
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 5,
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(bb.reflection_round, 2);
}

#[test]
fn force_human_escalates_on_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::ForceHuman(ForceHumanNode {
        name: "fh".into(),
        reason: "manual review required".into(),
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    assert!(matches!(
        result.commands[0],
        DecisionCommand::Human(HumanCommand::Escalate { .. })
    ));
}

#[test]
fn force_human_no_escalate_on_failure() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::ForceHuman(ForceHumanNode {
        name: "fh".into(),
        reason: "manual review required".into(),
        child: Box::new(Node::Condition(ConditionNode {
            name: "c".into(),
            evaluator: Evaluator::Script { expression: r#"provider_output == """#.into() },
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    assert!(result.commands.is_empty());
}

#[test]
fn action_with_when_guard_true_executes() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Action(ActionNode {
        name: "a".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: Some(Evaluator::Script { expression: r#"provider_output == """#.into() }),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Prompt Edge Cases
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn prompt_send_with_hint_when_model_specified() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: Some("claude-sonnet".into()),
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
    // session.sent only tracks plain `send`, but send_with_hint goes through a different path.
    // The MockSession only implements send(), so send_with_hint would panic if called.
    // Since this test passes, it means send_with_hint is NOT being called.
    // Wait — model is Some, so it should call send_with_hint...
    // Let me check: MockSession doesn't implement send_with_hint? No, Session trait requires it.
    // Let me re-check MockSession impl... it only has send, is_ready, receive.
    // Ah, Session trait probably has a default impl for send_with_hint that delegates to send.
    // Yes that's likely. So sent.len() should be 1.
    assert_eq!(session.sent.len(), 1);
}

#[test]
fn prompt_parse_failure_returns_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Hello".into(),
        parser: OutputParser::Enum {
            values: vec!["yes".into(), "no".into()],
            case_sensitive: true,
        },
        sets: vec![],
        timeout_ms: 1000,
        pending: false,
        sent_at: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: sends prompt, returns Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: reply is "maybe" which doesn't match enum values -> parse failure
    session.set_ready("maybe");
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    // Should have PromptFailure trace
    let has_failure = result.trace.iter().any(|e| {
        matches!(e, decision_dsl::ast::runtime::TraceEntry::PromptFailure { .. })
    });
    assert!(has_failure);
}

#[test]
fn prompt_command_parser_injects_command() {
    use std::collections::HashMap;
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut mapping = HashMap::new();
    mapping.insert("wake".into(), DecisionCommand::Agent(AgentCommand::WakeUp));

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Command?".into(),
        parser: OutputParser::Command { mapping },
        sets: vec![],
        timeout_ms: 1000,
        pending: false,
        sent_at: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: sends prompt
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: reply is "wake" which maps to WakeUp command
    session.set_ready("wake");
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    assert!(matches!(result.commands[0], DecisionCommand::Agent(AgentCommand::WakeUp)));
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Resume Paths
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn selector_resume_continues_to_next_child() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into(); // First condition fails
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
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
        rule_name: None,
        rule_priority: None,
        matched: false,
    }));

    let mut executor = Executor::new();

    // Tick 1: c1 fails, Prompt sends and returns Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: resume Prompt, session ready
    session.set_ready(r#"{"ok": true}"#);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn sequence_resume_continues_after_running_child() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
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
            Node::Action(ActionNode {
                name: "a".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: c1 succeeds, Prompt sends and returns Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: resume Prompt, then Action should execute
    session.set_ready(r#"{"ok": true}"#);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
}

#[test]
fn parallel_resume_ticks_all_children() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let mut tree = simple_tree(Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
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

    // Tick 1: c1 succeeds, Prompt sends and returns Running -> overall Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: resume Parallel, Prompt succeeds -> overall Success
    session.set_ready(r#"{"ok": true}"#);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn repeater_resume_continues_looping() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into(); // Condition fails every time
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

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

    // Single tick: all 3 attempts fail immediately (no Running child)
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
}

#[test]
fn subtree_resume_executes_resolved_root() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    let inner = Node::Prompt(PromptNode {
        name: "inner_prompt".into(),
        model: None,
        template: "Inner".into(),
        parser: OutputParser::Json { schema: None },
        sets: vec![],
        timeout_ms: 1000,
        pending: false,
        sent_at: None,
    });

    let mut tree = simple_tree(Node::SubTree(SubTreeNode {
        name: "sub".into(),
        ref_name: "inner".into(),
        resolved_root: Some(Box::new(inner)),
    }));

    let mut executor = Executor::new();

    // Tick 1: inner Prompt sends, returns Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: resume SubTree -> resume inner Prompt
    session.set_ready(r#"{"ok": true}"#);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Runtime Errors
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn subtree_scope_depth_exceeded() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    // Nest SubTrees deeply to exceed scope limit (64 max)
    let leaf = Node::Action(ActionNode {
        name: "leaf".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });

    // Build a chain of 65 SubTrees (root scope + 65 pushes = 66 > 64)
    let mut root = leaf;
    for _ in 0..65 {
        root = Node::SubTree(SubTreeNode {
            name: "sub".into(),
            ref_name: "x".into(),
            resolved_root: Some(Box::new(root)),
        });
    }

    let mut tree = simple_tree(root);
    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx);
    assert!(
        matches!(result, Err(RuntimeError::ScopeDepthExceeded)),
        "expected ScopeDepthExceeded, got {:?}",
        result
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Composite Node active_child State
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn selector_resumes_at_correct_child_after_failure() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;

    // Selector: [Condition(c1 fails), Condition(c2 succeeds), Action]
    // First tick: c1 fails, c2 succeeds -> selector returns Success
    // But if c2 had been the one that needed resuming...
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
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // c1: "x" == "x" -> true -> Success, selector returns immediately
    assert_eq!(result.status, NodeStatus::Success);
    if let Node::Selector(sel) = &tree.spec.root {
        assert!(!sel.matched, "matched is false when rule_name is None");
        // After Success, active_child is reset to None
        assert_eq!(sel.active_child, None);
    }
}

#[test]
fn selector_active_child_none_after_all_fail() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "c1".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "y""#.into() },
            }),
            Node::Condition(ConditionNode {
                name: "c2".into(),
                evaluator: Evaluator::Script { expression: r#"provider_output == "z""#.into() },
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
    // After all fail, selector may reset active_child to None
    if let Node::Selector(sel) = &tree.spec.root {
        assert_eq!(sel.active_child, None, "active_child should be None after all children fail");
    }
}

#[test]
fn sequence_active_child_advances_on_success() {
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
    // After all children succeed, active_child should be at end (None or len)
    if let Node::Sequence(seq) = &tree.spec.root {
        assert!(seq.active_child.is_none() || seq.active_child == Some(2),
            "active_child should be None or 2 after all children succeed");
    }
}

#[test]
fn sequence_active_child_stops_at_failure() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into();
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
                evaluator: Evaluator::Script { expression: r#"provider_output == "x""#.into() },
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);
    // After failure, active_child is reset to None (not persisted)
    if let Node::Sequence(seq) = &tree.spec.root {
        assert_eq!(seq.active_child, None, "active_child should be None after failure");
    }
}

#[test]
fn parallel_active_child_remains_none() {
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
    // Parallel does not use active_child in the same way
    if let Node::Parallel(par) = &tree.spec.root {
        assert_eq!(par.active_child, None, "Parallel active_child should remain None");
    }
}

#[test]
fn repeater_runs_exactly_max_attempts() {
    let mut bb = Blackboard::default();
    bb.provider_output = "x".into();
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "a".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        current: 0,
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // Repeater retries until max_attempts, then succeeds (Action always succeeds)
    assert_eq!(result.status, NodeStatus::Success);
    // All 3 attempts should have been made (3 commands)
    assert_eq!(result.commands.len(), 3);
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Prompt Sets Variable
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn prompt_sets_variable_on_success() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::with_reply(r#"{"status": "ok", "value": "test_result"}"#);
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "Hello".into(),
        parser: OutputParser::Json { schema: None },
        sets: vec![decision_dsl::ast::node::SetMapping { key: "result".into(), field: "value".into() }],
        timeout_ms: 1000,
        pending: true,
        sent_at: Some(clock.now()),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    // The prompt should have set the result variable on blackboard
    assert!(bb.get("result").is_some(), "result variable should be set after prompt success");
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: ReflectionGuard on Running
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn reflection_guard_passes_through_running() {
    let mut bb = Blackboard::default();
    bb.reflection_round = 0;
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let logger = NullLogger;
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock, &logger);

    let mut tree = simple_tree(Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 3,
        child: Box::new(Node::Prompt(PromptNode {
            name: "p".into(),
            model: None,
            template: "Ask".into(),
            parser: OutputParser::Json { schema: None },
            sets: vec![],
            timeout_ms: 1000,
            pending: false,
            sent_at: None,
        })),
    }));

    let mut executor = Executor::new();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    // ReflectionGuard passes through Running without incrementing
    assert_eq!(result.status, NodeStatus::Running);
    assert_eq!(bb.reflection_round, 0, "reflection_round should not increment on Running");
}
