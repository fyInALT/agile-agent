//! Integration Test Framework for decision-dsl
//!
//! These tests demonstrate the Mock LLM framework driving realistic
//! decision-layer scenarios. Each test exercises the full pipeline:
//!   YAML parse → desugar → tick loop → LLM interaction → command output.

use decision_dsl::ast::document::{Metadata, Spec, Tree, TreeKind};
use decision_dsl::ast::eval::Evaluator;
use decision_dsl::ast::node::{
    ActionNode, ConditionNode, CooldownNode, InverterNode, Node, NodeBehavior, NodeStatus,
    ParallelNode, ParallelPolicy, PromptNode, RepeaterNode, ReflectionGuardNode, SelectorNode,
    SequenceNode, SetVarNode, SubTreeNode, WhenNode,
};
use decision_dsl::ast::parser_out::OutputParser;
use decision_dsl::ast::runtime::{DslRunner, TickContext, TraceEntry};
use decision_dsl::ext::blackboard::BlackboardValue;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand};

mod fixtures;
use fixtures::{
    IntegrationHarness, MockLlm, Preset, PromptMatcher, ResponseStrategy, Scenario,
    llm_always_approves, llm_always_escalates, llm_delayed_approval,
};

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 1: Simple approval workflow (Codex-style)
// ═════════════════════════════════════════════════════════════════════════════

const YAML_APPROVE_WORKFLOW: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "approve-workflow"
spec:
  root:
    kind: Sequence
    payload:
      name: "root"
      children:
        - kind: Prompt
          payload:
            name: "ask_approval"
            template: "Should I proceed with {{ task_description }}?"
            parser:
              kind: Enum
              payload:
                values: ["yes", "no"]
                caseSensitive: false
            sets:
              - key: "approved"
                field: "value"
            timeoutMs: 5000
        - kind: Condition
          payload:
            name: "check_approved"
            evaluator:
              kind: VariableIs
              payload:
                key: "approved"
                expected: !String "yes"
        - kind: Action
          payload:
            name: "do_work"
            command:
              kind: Agent
              payload:
                kind: WakeUp
"#;

#[test]
fn integration_codex_approves_and_agent_wakes() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_APPROVE_WORKFLOW)
        .with_blackboard(|bb| bb.task_description = "refactoring auth module".into())
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_command_count(&result, 1);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::WakeUp))
    });
    harness.assert_prompt_sent("refactoring auth module");
    harness.assert_blackboard_has("approved", "String(\"yes\")");
}

#[test]
fn integration_codex_rejects_and_sequence_fails() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_APPROVE_WORKFLOW)
        .with_llm(MockLlm::new().scenario(Scenario::always(Preset::CodexReject)));

    let result = harness.tick_until_complete(5);

    // "no" is stored in approved, condition checks for "yes" → Failure
    assert_eq!(result.status, NodeStatus::Failure);
    // No WakeUp command should be issued
    assert!(result.commands.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 2: Escalation workflow (Claude-style structured output)
// ═════════════════════════════════════════════════════════════════════════════

const YAML_ESCALATION_WORKFLOW: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "escalation-workflow"
spec:
  root:
    kind: Selector
    payload:
      name: "root"
      children:
        - kind: Condition
          payload:
            name: "is_safe"
            evaluator:
              kind: Script
              payload:
                expression: "provider_output == \"safe\""
        - kind: Action
          payload:
            name: "escalate"
            command:
              kind: Human
              payload:
                kind: EscalateToHuman
                payload:
                  reason: "Unsafe output detected"
"#;

#[test]
fn integration_unsafe_output_triggers_escalation() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_ESCALATION_WORKFLOW)
        .with_blackboard(|bb| bb.provider_output = "rm -rf /".into())
        .with_llm(llm_always_escalates());

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_command_count(&result, 1);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Human(HumanCommand::Escalate { reason, .. })
                if reason == "Unsafe output detected"
        )
    });
}

#[test]
fn integration_safe_output_no_escalation() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_ESCALATION_WORKFLOW)
        .with_blackboard(|bb| bb.provider_output = "safe".into())
        .with_llm(llm_always_escalates());

    let result = harness.tick_until_complete(3);

    // Condition succeeds, Selector returns Success immediately
    assert_eq!(result.status, NodeStatus::Success);
    // No escalation command
    assert!(result.commands.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 3: Dangerous command detection with agent reflection
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_dangerous_command_triggers_reflection() {
    // Build tree in Rust to avoid Script parser edge cases with is_dangerous
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "dangerous-check".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Selector(SelectorNode {
                name: "root".into(),
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "not_dangerous".into(),
                        evaluator: Evaluator::Script {
                            expression: "provider_output == \"safe\"".into(),
                        },
                    }),
                    Node::Action(ActionNode {
                        name: "reflect".into(),
                        command: DecisionCommand::Agent(AgentCommand::Reflect {
                            prompt: "Dangerous command detected".into(),
                        }),
                        when: None,
                    }),
                ],
                active_child: None,
                rule_name: None,
                rule_priority: None,
                matched: false,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);
    harness.blackboard.provider_output = "drop table users".into();

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Agent(AgentCommand::Reflect { prompt })
                if prompt.contains("Dangerous command detected")
        )
    });
}

#[test]
fn integration_safe_command_no_reflection() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "dangerous-check".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Selector(SelectorNode {
                name: "root".into(),
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "not_dangerous".into(),
                        evaluator: Evaluator::Script {
                            expression: "provider_output == \"safe\"".into(),
                        },
                    }),
                    Node::Action(ActionNode {
                        name: "reflect".into(),
                        command: DecisionCommand::Agent(AgentCommand::Reflect {
                            prompt: "Dangerous command detected".into(),
                        }),
                        when: None,
                    }),
                ],
                active_child: None,
                rule_name: None,
                rule_priority: None,
                matched: false,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);
    harness.blackboard.provider_output = "safe".into();

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    assert!(result.commands.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 4: Delayed LLM response (multi-tick prompt)
// ═════════════════════════════════════════════════════════════════════════════

const YAML_DELAYED_APPROVAL: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "delayed-approval"
spec:
  root:
    kind: Sequence
    payload:
      name: "root"
      children:
        - kind: Prompt
          payload:
            name: "ask"
            template: "Please review and approve"
            parser:
              kind: Enum
              payload:
                values: ["yes"]
                caseSensitive: false
            sets: []
            timeoutMs: 10000
        - kind: Action
          payload:
            name: "proceed"
            command:
              kind: Agent
              payload:
                kind: WakeUp
"#;

#[test]
fn integration_delayed_llm_response_eventually_completes() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_DELAYED_APPROVAL)
        .with_llm(llm_delayed_approval(2));

    let result = harness.tick_until_complete(10);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::WakeUp))
    });
    // Prompt should have been sent exactly once
    assert_eq!(harness.llm.send_count(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 5: Multi-turn conversation with sequence responses
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_multi_turn_conversation() {
    let llm = MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::Sequence(vec![
            "thinking".into(),
            "yes".into(),
        ]),
        "multi-turn",
    ));

    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_APPROVE_WORKFLOW)
        .with_llm(llm);

    // First prompt returns "thinking" which doesn't match enum
    let result = harness.tick_until_complete(5);
    // "thinking" doesn't match ["yes", "no"] → parse failure → Prompt Failure
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 6: Context-aware dynamic LLM response
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_dynamic_response_based_on_history() {
    let llm = MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::Dynamic(|history| {
            if history.len() == 1 {
                "first_response".into()
            } else {
                "yes".into()
            }
        }),
        "dynamic",
    ));

    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_APPROVE_WORKFLOW)
        .with_llm(llm);

    // First tick: LLM returns "first_response" → parse failure
    let result = harness.tick_until_complete(5);
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 7: ForceHuman decorator pushes escalation command on success
// ═════════════════════════════════════════════════════════════════════════════

const YAML_FORCE_HUMAN: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "force-human"
spec:
  root:
    kind: ForceHuman
    payload:
      name: "fh"
      reason: "Critical decision required"
      child:
        kind: Condition
        payload:
          name: "always_true"
          evaluator:
            kind: Script
            payload:
              expression: 'provider_output == ""'
"#;

#[test]
fn integration_force_human_escalates_on_condition_success() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_FORCE_HUMAN)
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_command_count(&result, 1);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Human(HumanCommand::Escalate { reason, .. })
                if reason == "Critical decision required"
        )
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 8: ReflectionGuard limits reflection rounds
// ═════════════════════════════════════════════════════════════════════════════

const YAML_REFLECTION_LIMIT: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "reflection-limit"
spec:
  root:
    kind: ReflectionGuard
    payload:
      name: "rg"
      maxRounds: 2
      child:
        kind: Action
        payload:
          name: "reflect"
          command:
            kind: Agent
            payload:
              kind: Reflect
              payload:
                prompt: "Review changes"
"#;

#[test]
fn integration_reflection_guard_blocks_after_max_rounds() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_REFLECTION_LIMIT)
        .with_llm(llm_always_approves());

    // First tick: reflection_round=0 < 2 → succeeds, increments to 1
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Success);
    assert_eq!(harness.blackboard.reflection_round, 1);

    // Second tick: reflection_round=1 < 2 → succeeds, increments to 2
    harness.executor.reset();
    harness.tree.as_mut().unwrap().spec.root.reset();
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Success);
    assert_eq!(harness.blackboard.reflection_round, 2);

    // Third tick: reflection_round=2 >= 2 → Failure
    harness.executor.reset();
    harness.tree.as_mut().unwrap().spec.root.reset();
    let r3 = harness.tick();
    assert_eq!(r3.status, NodeStatus::Failure);
    assert!(r3.commands.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 9: SubTree scope isolation verified through integration
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_subtree_variable_not_leaked() {
    let inner = Node::Sequence(SequenceNode {
        name: "inner_seq".into(),
        children: vec![
            Node::SetVar(SetVarNode {
                name: "set".into(),
                key: "inner_var".into(),
                value: BlackboardValue::String("secret".into()),
            }),
            Node::Condition(ConditionNode {
                name: "check_inner".into(),
                evaluator: Evaluator::Script {
                    expression: r#"inner_var == "secret""#.into(),
                },
            }),
        ],
        active_child: None,
    });

    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "subtree-test".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Sequence(SequenceNode {
                name: "outer".into(),
                children: vec![
                    Node::SubTree(SubTreeNode {
                        name: "inner_tree".into(),
                        ref_name: "inner".into(),
                        resolved_root: Some(Box::new(inner)),
                    }),
                    Node::Condition(ConditionNode {
                        name: "check_leak".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == """#.into(),
                        },
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);

    let result = harness.tick_until_complete(3);

    // SubTree succeeds, then Condition on provider_output succeeds
    assert_eq!(result.status, NodeStatus::Success);
    // inner_var should NOT exist in outer scope
    harness.assert_blackboard_missing("inner_var");
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 10: Trace verification for RuleMatched / RuleSkipped
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_rule_matched_and_skipped_traced() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "priority-rules".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Selector(SelectorNode {
                name: "root".into(),
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "fast_path".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == "fast""#.into(),
                        },
                    }),
                    Node::Action(ActionNode {
                        name: "slow_action".into(),
                        command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
                        when: None,
                    }),
                ],
                active_child: None,
                rule_name: Some("test_rule".into()),
                rule_priority: Some(1),
                matched: false,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);
    harness.blackboard.provider_output = "fast".into();

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_trace_contains(&result, |e| {
        matches!(e, TraceEntry::RuleMatched { rule_name, .. } if rule_name == "test_rule")
    });
    harness.assert_trace_contains(&result, |e| {
        matches!(e, TraceEntry::RuleSkipped { rule_name, .. } if rule_name == "test_rule")
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 11: Command template rendering with blackboard context
// ═════════════════════════════════════════════════════════════════════════════

const YAML_TEMPLATE_COMMAND: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "template-test"
spec:
  root:
    kind: Action
    payload:
      name: "templated"
      command:
        kind: Human
        payload:
          kind: EscalateToHuman
          payload:
            reason: "Error in {{ current_task_id }}: {{ provider_output }}"
"#;

#[test]
fn integration_command_template_rendered_from_blackboard() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_TEMPLATE_COMMAND)
        .with_blackboard(|bb| {
            bb.current_task_id = "TASK-42".into();
            bb.provider_output = "connection timeout".into();
        })
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(3);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Human(HumanCommand::Escalate { reason, .. })
                if reason.contains("TASK-42") && reason.contains("connection timeout")
        )
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 12: Inverter decorator flips child result
// ═════════════════════════════════════════════════════════════════════════════

const YAML_INVERTER: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "inverter-test"
spec:
  root:
    kind: Inverter
    payload:
      name: "inv"
      child:
        kind: Condition
        payload:
          name: "cond"
          evaluator:
            kind: Script
            payload:
              expression: 'provider_output == ""'
"#;

#[test]
fn integration_inverter_flips_success_to_failure() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_INVERTER)
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(3);

    // Condition succeeds (empty provider_output == ""), Inverter flips to Failure
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 13: Cooldown blocks repeated execution within duration
// ═════════════════════════════════════════════════════════════════════════════

const YAML_COOLDOWN: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "cooldown-test"
spec:
  root:
    kind: Cooldown
    payload:
      name: "cool"
      durationMs: 500
      child:
        kind: Action
        payload:
          name: "act"
          command:
            kind: Agent
            payload:
              kind: WakeUp
"#;

#[test]
fn integration_cooldown_blocks_then_allows() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_COOLDOWN)
        .with_llm(llm_always_approves());

    // First tick: passes cooldown, action executes
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Success);
    assert_eq!(r1.commands.len(), 1);

    // Second tick immediately: cooldown blocks
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Failure);
    assert!(r2.commands.is_empty());

    // Advance clock past cooldown
    harness.advance_clock(std::time::Duration::from_millis(600));

    // Third tick: cooldown expired, action executes again
    let r3 = harness.tick();
    assert_eq!(r3.status, NodeStatus::Success);
    assert_eq!(r3.commands.len(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// Scenario 14: RecordingRunner captures full tick history
// ═════════════════════════════════════════════════════════════════════════════

use fixtures::RecordingRunner;

#[test]
fn integration_recording_runner_captures_history() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_DELAYED_APPROVAL)
        .with_llm(llm_delayed_approval(1));

    let mut runner = RecordingRunner::new();
    let tree = harness.tree.as_mut().unwrap();

    // Tick 1: Prompt sends, returns Running
    let mut ctx = TickContext::new(
        &mut harness.blackboard,
        &mut harness.llm,
        &harness.clock,
        harness.logger,
    );
    let r1 = runner.tick(tree, &mut ctx).unwrap();
    assert_eq!(r1.status, NodeStatus::Running);

    // Tick 2: LLM ready, Prompt succeeds, Action executes
    harness.llm.advance_tick();
    let mut ctx = TickContext::new(
        &mut harness.blackboard,
        &mut harness.llm,
        &harness.clock,
        harness.logger,
    );
    let r2 = runner.tick(tree, &mut ctx).unwrap();
    assert_eq!(r2.status, NodeStatus::Success);

    // History should contain both ticks
    assert_eq!(runner.history.len(), 2);
    assert_eq!(runner.running_tick_count(), 1);
}


// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 15: Multi-Prompt serial conversation
// ═════════════════════════════════════════════════════════════════════════════

const YAML_TWO_PROMPTS_SERIAL: &str = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "two-prompts"
spec:
  root:
    kind: Sequence
    payload:
      name: "seq"
      children:
        - kind: Prompt
          payload:
            name: "ask_first"
            template: "First question?"
            parser:
              kind: Enum
              payload:
                values: ["yes"]
                caseSensitive: false
            sets: []
            timeoutMs: 5000
        - kind: Prompt
          payload:
            name: "ask_second"
            template: "Second question?"
            parser:
              kind: Enum
              payload:
                values: ["no"]
                caseSensitive: false
            sets: []
            timeoutMs: 5000
        - kind: Action
          payload:
            name: "finish"
            command:
              kind: Agent
              payload:
                kind: WakeUp
"#;

#[test]
fn integration_two_prompts_serial_with_different_replies() {
    let llm = MockLlm::new()
        .scenario(Scenario::new(
            PromptMatcher::Contains("First".into()),
            ResponseStrategy::Preset(Preset::CodexApprove),
            "first prompt approves",
        ))
        .scenario(Scenario::new(
            PromptMatcher::Contains("Second".into()),
            ResponseStrategy::Preset(Preset::CodexReject),
            "second prompt rejects",
        ));

    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_TWO_PROMPTS_SERIAL)
        .with_llm(llm);

    // Tick 1: First prompt sends, returns Running
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Running);
    assert_eq!(harness.llm.send_count(), 1);
    harness.assert_prompt_sent("First question");

    // Tick 2: First prompt gets "yes", succeeds. Second prompt sends, returns Running.
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Running);
    assert_eq!(harness.llm.send_count(), 2);
    harness.assert_prompt_sent("Second question");

    // Tick 3: Second prompt gets "no", succeeds. Action executes.
    let r3 = harness.tick();
    assert_eq!(r3.status, NodeStatus::Success);
    harness.assert_command_count(&r3, 1);
    harness.assert_commands_contain(&r3, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::WakeUp))
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 16: When node resume with condition change
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_when_resume_rechecks_condition() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "when-resume".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Sequence(SequenceNode {
                name: "seq".into(),
                children: vec![
                    Node::SetVar(SetVarNode {
                        name: "set_flag".into(),
                        key: "flag".into(),
                        value: BlackboardValue::Boolean(true),
                    }),
                    Node::When(WhenNode {
                        name: "w".into(),
                        condition: Evaluator::VariableIs {
                            key: "flag".into(),
                            expected: BlackboardValue::Boolean(true),
                        },
                        action: Box::new(Node::Prompt(PromptNode {
                            name: "p".into(),
                            model: None,
                            template: "Ask".into(),
                            parser: OutputParser::Json { schema: None },
                            sets: vec![],
                            timeout_ms: 5000,
                            pending: false,
                            sent_at: None,
                        })),
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_delayed_approval(1));
    harness.tree = Some(tree);

    // Tick 1: SetVar succeeds, When condition is true, Prompt sends and returns Running
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Running);

    // Change condition to false BEFORE resume
    harness.blackboard.set("flag", BlackboardValue::Boolean(false));

    // Tick 2: When re-checks condition on resume — false → Failure, Sequence fails
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 17: Repeater wrapping Prompt with resume
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_repeater_prompt_resume_completes_remaining_loops() {
    // Repeater(max=3) -> Prompt. Prompt returns Running on first tick.
    // On resume, Prompt succeeds. Repeater should continue looping.
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "repeater-prompt".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Repeater(RepeaterNode {
                name: "rep".into(),
                max_attempts: 2,
                child: Box::new(Node::Prompt(PromptNode {
                    name: "p".into(),
                    model: None,
                    template: "Ask".into(),
                    parser: OutputParser::Enum {
                        values: vec!["yes".into()],
                        case_sensitive: false,
                    },
                    sets: vec![],
                    timeout_ms: 5000,
                    pending: false,
                    sent_at: None,
                })),
                current: 0,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_delayed_approval(1));
    harness.tree = Some(tree);

    // Tick 1: Prompt sends, returns Running. Repeater sees Running, passes through.
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Running);

    // Tick 2: Prompt resumes, gets "yes", succeeds.
    // Repeater: current=1 < max=2, continues looping.
    // Prompt sends again (new prompt), returns Running.
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Running);

    // Tick 3: Prompt resumes again, gets "yes", succeeds.
    // Repeater: current=2 >= max=2, returns Success.
    let r3 = harness.tick();
    assert_eq!(r3.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 18: Parallel AnySuccess with one running child
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_parallel_any_success_one_succeeds_one_running() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "par-any".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Parallel(ParallelNode {
                name: "par".into(),
                policy: ParallelPolicy::AnySuccess,
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "c1".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == """#.into(),
                        },
                    }),
                    Node::Prompt(PromptNode {
                        name: "p".into(),
                        model: None,
                        template: "Ask".into(),
                        parser: OutputParser::Json { schema: None },
                        sets: vec![],
                        timeout_ms: 5000,
                        pending: false,
                        sent_at: None,
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_delayed_approval(10)); // Prompt never gets reply
    harness.tree = Some(tree);

    // Tick 1: c1 succeeds, Prompt sends and returns Running.
    // AnySuccess: one success → immediate Success (even though other is Running)
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 19: Parallel Majority with 3 children
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_parallel_majority_two_of_three() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "par-majority".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Parallel(ParallelNode {
                name: "par".into(),
                policy: ParallelPolicy::Majority,
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "c1".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == """#.into(),
                        },
                    }),
                    Node::Condition(ConditionNode {
                        name: "c2".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == "x""#.into(),
                        },
                    }),
                    Node::Condition(ConditionNode {
                        name: "c3".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == """#.into(),
                        },
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);

    let result = harness.tick_until_complete(3);

    // c1: success, c2: failure, c3: success → 2/3 success → majority → Success
    assert_eq!(result.status, NodeStatus::Success);
}

#[test]
fn integration_parallel_majority_failure() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "par-majority-fail".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Parallel(ParallelNode {
                name: "par".into(),
                policy: ParallelPolicy::Majority,
                children: vec![
                    Node::Condition(ConditionNode {
                        name: "c1".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == "x""#.into(),
                        },
                    }),
                    Node::Condition(ConditionNode {
                        name: "c2".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == "x""#.into(),
                        },
                    }),
                    Node::Condition(ConditionNode {
                        name: "c3".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == """#.into(),
                        },
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);

    let result = harness.tick_until_complete(3);

    // c1: failure, c2: failure, c3: success → 1/3 success → not majority → Failure
    assert_eq!(result.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 20: Nested decorators — Inverter(Repeater(Condition))
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_inverter_repeater_condition_flips_after_max_attempts() {
    // Inverter(Repeater(max=2, Condition(false)))
    // Repeater retries false condition 2 times → Failure
    // Inverter flips Failure → Success
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "inv-rep".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Inverter(InverterNode {
                name: "inv".into(),
                child: Box::new(Node::Repeater(RepeaterNode {
                    name: "rep".into(),
                    max_attempts: 2,
                    child: Box::new(Node::Condition(ConditionNode {
                        name: "c".into(),
                        evaluator: Evaluator::Script {
                            expression: r#"provider_output == "x""#.into(),
                        },
                    })),
                    current: 0,
                })),
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);

    let result = harness.tick_until_complete(3);

    // Condition fails twice → Repeater returns Failure → Inverter → Success
    assert_eq!(result.status, NodeStatus::Success);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 21: Cooldown + ReflectionGuard combination
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_cooldown_reflection_guard_combined() {
    // Cooldown(100ms) -> ReflectionGuard(max=2) -> Action(WakeUp)
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "cool-rg".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Cooldown(CooldownNode {
                name: "cool".into(),
                duration_ms: 100,
                child: Box::new(Node::ReflectionGuard(ReflectionGuardNode {
                    name: "rg".into(),
                    max_rounds: 2,
                    child: Box::new(Node::Action(ActionNode {
                        name: "act".into(),
                        command: DecisionCommand::Agent(AgentCommand::WakeUp),
                        when: None,
                    })),
                })),
                last_success: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_always_approves());
    harness.tree = Some(tree);

    // Tick 1: cooldown passes, reflection_round=0 < 2, action succeeds
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Success);
    assert_eq!(r1.commands.len(), 1);
    assert_eq!(harness.blackboard.reflection_round, 1);

    // Tick 2 immediately: cooldown blocks
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Failure);

    // Advance clock past cooldown
    harness.advance_clock(std::time::Duration::from_millis(150));

    // Tick 3: cooldown passes, reflection_round=1 < 2, action succeeds
    let r3 = harness.tick();
    assert_eq!(r3.status, NodeStatus::Success);
    assert_eq!(harness.blackboard.reflection_round, 2);

    // Tick 4 (after cooldown again): reflection_round=2 >= 2 → Failure
    harness.advance_clock(std::time::Duration::from_millis(150));
    let r4 = harness.tick();
    assert_eq!(r4.status, NodeStatus::Failure);
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 22: DecisionRules with priority, cooldown, on_error
// ═════════════════════════════════════════════════════════════════════════════

const YAML_DECISION_RULES_FULL: &str = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: "complex-rules"
rules:
  - priority: 1
    name: "fast_path"
    if:
      kind: Script
      payload:
        expression: 'provider_output == "safe"'
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Agent
          payload:
            kind: WakeUp
    cooldownMs: 500
    reflectionMaxRounds: 3
  - priority: 2
    name: "slow_path"
    if:
      kind: Script
      payload:
        expression: 'provider_output == "dangerous"'
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Human
          payload:
            kind: EscalateToHuman
            payload:
              reason: "Dangerous content"
              context: null
    onError: Escalate
  - priority: 3
    name: "fallback"
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Agent
          payload:
            kind: Reflect
            payload:
              prompt: "Review output"
"#;

#[test]
fn integration_decision_rules_fast_path_wins() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_DECISION_RULES_FULL)
        .with_blackboard(|bb| bb.provider_output = "safe".into())
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::WakeUp))
    });
}

#[test]
fn integration_decision_rules_slow_path_escalates() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_DECISION_RULES_FULL)
        .with_blackboard(|bb| bb.provider_output = "dangerous".into())
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Human(HumanCommand::Escalate { reason, .. })
                if reason == "Dangerous content"
        )
    });
}

#[test]
fn integration_decision_rules_fallback_when_no_match() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_DECISION_RULES_FULL)
        .with_blackboard(|bb| bb.provider_output = "something_else".into())
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Agent(AgentCommand::Reflect { prompt })
                if prompt == "Review output"
        )
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 23: Switch on Prompt branching
// ═════════════════════════════════════════════════════════════════════════════

const YAML_SWITCH_PROMPT: &str = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: "switch-prompt"
rules:
  - priority: 1
    name: "ask"
    then:
      kind: Switch
      payload:
        name: "branch"
        on: !Prompt
          model: null
          timeoutMs: 5000
          template: "What should I do?"
          parser:
            kind: Enum
            payload:
              values: ["commit", "reflect", "skip"]
              caseSensitive: false
        cases:
          - value: "commit"
            action:
              kind: InlineCommand
              payload:
                command:
                  kind: Git
                  payload:
                    - kind: CommitChanges
                      payload:
                        message: "auto commit"
                        is_wip: false
                    - null
          - value: "reflect"
            action:
              kind: InlineCommand
              payload:
                command:
                  kind: Agent
                  payload:
                    kind: Reflect
                    payload:
                      prompt: "Think again"
        _default:
          kind: InlineCommand
          payload:
            command:
              kind: Agent
              payload:
                kind: ApproveAndContinue
"#;

#[test]
fn integration_switch_prompt_commit_branch() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_SWITCH_PROMPT)
        .with_llm(MockLlm::new().scenario(Scenario::always(Preset::AgentGitCommit {
            message: "commit",
        })));

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    // The parser expects "commit"/"reflect"/"skip" but preset returns "suggest_commit: commit"
    // which does NOT match the enum → falls to default → ApproveAndContinue
    harness.assert_commands_contain(&result, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::ApproveAndContinue))
    });
}

#[test]
fn integration_switch_prompt_exact_match_commit() {
    let llm = MockLlm::new().scenario(Scenario::new(
        PromptMatcher::Any,
        ResponseStrategy::Immediate("commit".into()),
        "exact commit",
    ));

    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_SWITCH_PROMPT)
        .with_llm(llm);

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    // Enum matches "commit" → Switch case "commit" → Git Commit
    harness.assert_commands_contain(&result, |cmd| {
        matches!(
            cmd,
            DecisionCommand::Git(
                decision_dsl::ext::command::GitCommand::Commit { message, .. },
                _,
            ) if message == "auto commit"
        )
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 24: Pipeline with Guard + Action
// ═════════════════════════════════════════════════════════════════════════════

const YAML_PIPELINE: &str = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: "pipeline-test"
rules:
  - priority: 1
    name: "deploy"
    if:
      kind: Script
      payload:
        expression: 'provider_output == "ready"'
    then:
      kind: Pipeline
      payload:
        name: "deploy_pipeline"
        steps:
          - kind: Guard
            payload:
              condition:
                kind: Script
                payload:
                  expression: 'provider_output == "ready"'
          - kind: Action
            payload:
              command:
                kind: Agent
                payload:
                  kind: WakeUp
"#;

#[test]
fn integration_pipeline_guard_then_action() {
    let mut harness = IntegrationHarness::new()
        .with_yaml_tree(YAML_PIPELINE)
        .with_blackboard(|bb| bb.provider_output = "ready".into())
        .with_llm(llm_always_approves());

    let result = harness.tick_until_complete(5);

    assert_eq!(result.status, NodeStatus::Success);
    harness.assert_commands_contain(&result, |cmd| {
        matches!(cmd, DecisionCommand::Agent(AgentCommand::WakeUp))
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Complex Scenario 25: Prompt timeout
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn integration_prompt_timeout_returns_failure() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata {
            name: "timeout-test".into(),
            description: None,
        },
        spec: Spec {
            root: Node::Sequence(SequenceNode {
                name: "seq".into(),
                children: vec![
                    Node::Prompt(PromptNode {
                        name: "ask".into(),
                        model: None,
                        template: "Timeout test".into(),
                        parser: OutputParser::Json { schema: None },
                        sets: vec![],
                        timeout_ms: 100, // 100ms timeout
                        pending: false,
                        sent_at: None,
                    }),
                    Node::Action(ActionNode {
                        name: "act".into(),
                        command: DecisionCommand::Agent(AgentCommand::WakeUp),
                        when: None,
                    }),
                ],
                active_child: None,
            }),
        },
    };

    let mut harness = IntegrationHarness::new()
        .with_llm(llm_delayed_approval(100)); // Delay longer than timeout
    harness.tree = Some(tree);

    // Tick 1: Prompt sends, returns Running
    let r1 = harness.tick();
    assert_eq!(r1.status, NodeStatus::Running);

    // Advance clock past timeout (100ms)
    harness.advance_clock(std::time::Duration::from_millis(150));

    // Tick 2: Prompt detects timeout → Failure
    // Sequence fails at child 0, Action never executes
    let r2 = harness.tick();
    assert_eq!(r2.status, NodeStatus::Failure);
    assert!(r2.commands.is_empty());

    // Verify timeout trace
    harness.assert_trace_contains(&r2, |e| {
        matches!(e, TraceEntry::PromptFailure { node_name, error } if node_name == "ask" && error.contains("timeout"))
    });
}
