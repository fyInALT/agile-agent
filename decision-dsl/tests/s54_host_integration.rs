//! Host Integration Example
//!
//! This test demonstrates the minimal public API surface for integrating
//! decision-dsl into a host application (e.g. agent-decision).

use decision_dsl::ast::eval::EvaluatorRegistry;
use decision_dsl::ast::parser::{DslParser, YamlParser};
use decision_dsl::ast::runtime::{DslRunner, Executor, MetricsCollector, NullMetricsCollector, TickContext};
use decision_dsl::ext::blackboard::Blackboard;
use decision_dsl::ext::command::{DecisionCommand, HumanCommand};
use decision_dsl::ext::traits::{MockClock, MockSession, NullLogger};

#[test]
fn host_integration_full_cycle() {
    // 1. Parse a behavior tree from YAML
    let yaml = r#"
apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "approve-or-escalate"
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
                  reason: "Unsafe output"
"#;

    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    let registry = EvaluatorRegistry::new();
    let mut tree = doc.desugar(&registry).unwrap();

    // 2. Prepare blackboard from host state
    let mut bb = Blackboard::default();
    bb.task_description = "Review output".into();
    bb.provider_output = "unsafe command".into();

    // 3. Create executor and tick
    let mut session = MockSession::new();
    let clock = MockClock::new();
    let mut executor = Executor::new();

    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &NullLogger);

    let result = executor.tick(&mut tree, &mut ctx).unwrap();

    // 4. Consume commands
    assert_eq!(result.commands.len(), 1);
    match &result.commands[0] {
        DecisionCommand::Human(HumanCommand::Escalate { reason, .. }) => {
            assert_eq!(reason, "Unsafe output");
        }
        other => panic!("expected Escalate, got {other:?}"),
    }
}

#[test]
fn host_integration_public_api_is_minimal() {
    // Verify that the key public types are accessible without deep AST knowledge
    let _parser = YamlParser::new();
    let _executor = Executor::new();
    let _bb = Blackboard::default();
}

#[test]
fn tick_context_new_constructor() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::new();
    let clock = MockClock::new();

    let _ctx = TickContext::new(&mut bb, &mut session, &clock, &NullLogger);
}

#[test]
fn null_metrics_collector_is_no_op() {
    let metrics = NullMetricsCollector;
    use std::time::Duration;
    use decision_dsl::ast::node::NodeStatus;

    metrics.record_tick("test_tree", Duration::from_millis(10));
    metrics.record_rule_match("rule_1", 1, Duration::from_millis(5));
    metrics.record_node("node_a", "Condition", NodeStatus::Success, Duration::from_millis(2));
    metrics.record_prompt("ask", "standard", 100, 50, 25);
    metrics.record_subtree("handler", Duration::from_millis(8), NodeStatus::Success);
}
