use std::time::Duration;

use decision_dsl::ast::document::{Spec, Tree};
use decision_dsl::ast::eval::Evaluator;
use decision_dsl::ast::node::{
    ActionNode, ConditionNode, CooldownNode, Node, NodeStatus, PromptNode, SequenceNode,
    SetVarNode, SubTreeNode,
};
use decision_dsl::ast::parser_out::OutputParser;
use decision_dsl::ast::runtime::{Executor, TickContext, DslRunner};
use decision_dsl::ext::blackboard::{Blackboard, BlackboardValue};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand};
use decision_dsl::ext::traits::{MockClock, MockSession, NullLogger};

fn simple_tree(root: Node) -> Tree {
    Tree {
        api_version: "decision.agile-agent.io/v1".into(),
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
    session: &'a mut MockSession,
    clock: &'a MockClock,
) -> TickContext<'a> {
    TickContext {
        blackboard: bb,
        session,
        clock,
        logger: &NullLogger,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Integration: DecisionRules-style tree (Selector with conditions + actions)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_decision_rules_sequence_guard_action() {
    let mut bb = Blackboard::default();
    bb.provider_output = "error in system".into();

    let mut session = MockSession::new();
    let clock = MockClock::new();

    // Tree: Sequence
    //   [0] Condition: provider_output contains "error" ?
    //   [1] Action: Escalate
    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "root".into(),
        children: vec![
            Node::Condition(ConditionNode {
                name: "check_error".into(),
                evaluator: Evaluator::OutputContains {
                    pattern: "error".into(),
                    case_sensitive: true,
                },
            }),
            Node::Action(ActionNode {
                name: "escalate".into(),
                command: DecisionCommand::Human(HumanCommand::Escalate {
                    reason: "Error detected".into(),
                    context: None,
                }),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    assert!(matches!(
        result.commands[0],
        DecisionCommand::Human(HumanCommand::Escalate { .. })
    ));
}

// ═════════════════════════════════════════════════════════════════════════════
// Integration: Switch on Prompt → 2 ticks → branch
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_prompt_two_ticks_branch() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();

    // Tree: Sequence
    //   [0] Prompt (ask for approval)
    //   [1] Action (WakeUp)
    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![
            Node::Prompt(PromptNode {
                name: "ask".into(),
                model: None,
                template: "Approve?".into(),
                parser: OutputParser::Enum {
                    values: vec!["yes".into(), "no".into()],
                    case_sensitive: false,
                },
                sets: vec![],
                timeout_ms: 5000,
                pending: false,
                sent_at: None,
            }),
            Node::Action(ActionNode {
                name: "wake".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();

    // Tick 1: Prompt sends, returns Running
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);
    assert_eq!(session.sent_messages().len(), 1);

    // Tick 2: Session ready with "yes"
    session.push_reply("yes");
    session.set_ready(true);
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    assert!(matches!(result.commands[0], DecisionCommand::Agent(AgentCommand::WakeUp)));
}

// ═════════════════════════════════════════════════════════════════════════════
// Integration: SubTree scope isolation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_subtree_scope_isolation() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();

    // Inner tree: SetVar(key="inner", value="hidden")
    let inner = Node::Sequence(SequenceNode {
        name: "inner_seq".into(),
        children: vec![
            Node::SetVar(SetVarNode {
                name: "set_inner".into(),
                key: "inner_var".into(),
                value: BlackboardValue::String("hidden".into()),
            }),
            Node::Condition(ConditionNode {
                name: "check".into(),
                evaluator: Evaluator::Script {
                    expression: r#"inner_var == "hidden""#.into(),
                },
            }),
        ],
        active_child: None,
    });

    // Outer tree: SubTree(inner) then Action
    let mut tree = simple_tree(Node::Sequence(SequenceNode {
        name: "outer_seq".into(),
        children: vec![
            Node::SubTree(SubTreeNode {
                name: "sub".into(),
                ref_name: "inner".into(),
                resolved_root: Some(Box::new(inner)),
            }),
            Node::Action(ActionNode {
                name: "act".into(),
                command: DecisionCommand::Agent(AgentCommand::WakeUp),
                when: None,
            }),
        ],
        active_child: None,
    }));

    let mut executor = Executor::new();
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    // SubTree succeeds, Action succeeds -> Sequence succeeds
    assert_eq!(result.status, NodeStatus::Success);
    // Verify inner_var was NOT leaked outside SubTree
    assert_eq!(bb.get("inner_var"), None);
}

// ═════════════════════════════════════════════════════════════════════════════
// Integration: Cooldown with MockClock
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_cooldown_with_mock_clock() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let mut clock = MockClock::new();

    // Tree: Cooldown(100ms) -> Action(WakeUp)
    let mut tree = simple_tree(Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 100,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    }));

    let mut executor = Executor::new();

    // First tick: passes cooldown, action succeeds
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);

    // Reset executor so we tick again from root
    executor.reset();

    // Second tick immediately: cooldown blocks
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Failure);

    // Advance clock past cooldown
    clock.advance(Duration::from_millis(150));
    executor.reset();

    // Third tick: cooldown expired, action succeeds again
    let mut ctx = tick_ctx(&mut bb, &mut session, &clock);
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
}
